//! The location picker (decisions #038, #059, #060): **where** first, then the **place**.
//!
//! Used by the place dialog now, and by milestone 6's event forms. In order:
//! 1. **Where:** the device's location (the browser's, through `document::eval`; permission is
//!    asked once), or coordinates typed or pasted ("35.5951, -82.5515").
//! 2. **Place:** the server says which of the person's saved places the point is in, and what
//!    the place data suggests. **Inside a saved place, its own city, state, and country win**
//!    (the alias is what the person chose). Otherwise the suggestion fills the fields, which
//!    stay editable: any city or state can be typed (e.g. a base the data doesn't list). If the
//!    person has already typed, a new suggestion doesn't overwrite them; **Use the suggestion**
//!    does.

use dioxus::prelude::*;
use domain::api::{Located, PlaceSuggestion, PlacesOutcome};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::location::{Coordinates, CountryCode, Fix, NearbyPlace, Place, SavedPlaceId};
use domain::log_area;
use domain::preferences::DistanceUnit;
use domain::time::{Clock, UnixMillis};
use serde::Deserialize;

use crate::api;
use crate::logging::SystemClock;
use crate::ui::screens::{FormError, problem_from_error};
use crate::ui::{tr, tr_with};

/// Asks the browser for its location (once; it may ask the person first). Resolves with the
/// position, or why there isn't one.
const LOCATE_SCRIPT: &str = r"
if (!navigator.geolocation) { return { error: 'unavailable' }; }
return await new Promise((resolve) => {
  navigator.geolocation.getCurrentPosition(
    (p) => resolve({ lat: p.coords.latitude, lon: p.coords.longitude,
                     accuracy: p.coords.accuracy, at: p.timestamp }),
    (e) => resolve({ error: e.code === 1 ? 'denied' : 'unavailable' }),
    { enableHighAccuracy: true, timeout: 15000, maximumAge: 60000 });
});
";

/// What the browser answered.
#[derive(Debug, Deserialize)]
struct BrowserFix {
    lat: Option<f64>,
    lon: Option<f64>,
    accuracy: Option<f64>,
    at: Option<f64>,
    error: Option<String>,
}

/// The browser's answer as a fix, or the message saying why there's none.
fn fix_from(answer: &BrowserFix) -> Result<Fix, MessageId> {
    if answer.error.as_deref() == Some("denied") {
        return Err(messages::PLACE_LOCATION_DENIED);
    }
    let (Some(lat), Some(lon)) = (answer.lat, answer.lon) else {
        return Err(messages::PLACE_LOCATION_UNAVAILABLE);
    };
    let coordinates =
        Coordinates::from_degrees(lat, lon).map_err(|_| messages::PLACE_LOCATION_UNAVAILABLE)?;
    // Whole meters and milliseconds; anything odd is left out rather than guessed.
    let accuracy_m = answer
        .accuracy
        .filter(|meters| meters.is_finite() && *meters >= 0.0 && *meters < 1e9)
        .map(|meters| format!("{meters:.0}").parse::<u32>().unwrap_or(u32::MAX));
    let taken_at = answer
        .at
        .filter(|ms| ms.is_finite())
        .and_then(|ms| format!("{ms:.0}").parse::<i64>().ok())
        .map_or_else(|| SystemClock.now(), UnixMillis::from_millis);
    Ok(Fix {
        coordinates,
        accuracy_m,
        taken_at,
    })
}

/// "± 12 m · just now", "± 30 m · 4 min ago", or "Typed coordinates".
fn fix_text(fix: Option<&Fix>, now: UnixMillis) -> String {
    let Some(fix) = fix else {
        return tr(messages::PLACE_FIX_TYPED);
    };
    let meters = MessageArg::from(i64::from(fix.accuracy_m.unwrap_or(0)));
    let minutes = (now.as_millis() - fix.taken_at.as_millis()).max(0) / 60_000;
    if minutes == 0 {
        tr_with(messages::PLACE_FIX_NOW, &[("meters", meters)])
    } else {
        tr_with(
            messages::PLACE_FIX_MINUTES,
            &[("meters", meters), ("minutes", MessageArg::from(minutes))],
        )
    }
}

/// "Asheville, North Carolina, US".
pub(crate) fn place_text(place: &Place) -> String {
    format!(
        "{}, {}, {}",
        place.city.as_str(),
        place.region.as_str(),
        place.country.as_str()
    )
}

/// The picker's state, shared with the form that uses it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Picker {
    /// Where, once chosen.
    pub coordinates: Signal<Option<Coordinates>>,
    /// The device's fix, if the coordinates came from it (`None`: typed).
    pub fix: Signal<Option<Fix>>,
    /// What the server knows about the point.
    pub located: Signal<Option<Located>>,
    /// The place fields, as shown (and typed).
    pub city: Signal<String>,
    /// See `city`.
    pub region: Signal<String>,
    /// The country code.
    pub country: Signal<String>,
    /// The person has typed in the place fields, so a new suggestion doesn't overwrite them.
    pub typed: Signal<bool>,
    /// A saved place being edited (it doesn't count as "you're at" itself).
    pub editing: Option<SavedPlaceId>,
    /// The person's units, for the nearby places (the defaults until they're loaded).
    pub units: Signal<domain::preferences::Preferences>,
}

impl Picker {
    /// Fills the place fields with `place`.
    fn fill(mut self, place: &Place) {
        self.city.set(place.city.as_str().to_owned());
        self.region.set(place.region.as_str().to_owned());
        self.country.set(place.country.as_str().to_owned());
        self.typed.set(false);
    }

    /// The saved place the point is in, other than the one being edited.
    fn at_saved(self) -> Option<(SavedPlaceId, domain::location::SavedPlace)> {
        (self.located)()
            .and_then(|located| located.saved)
            .filter(|(id, _)| Some(*id) != self.editing)
    }

    /// Sets new coordinates, then asks the server what it knows about them.
    pub(crate) fn locate(
        mut self,
        coordinates: Coordinates,
        fix: Option<Fix>,
        mut error: Signal<Option<String>>,
    ) {
        self.coordinates.set(Some(coordinates));
        self.fix.set(fix);
        self.located.set(None);
        spawn(async move {
            match api::locate(coordinates).await {
                Ok(PlacesOutcome::Located { located }) => {
                    let located = *located;
                    self.located.set(Some(located));
                    // A saved place's own place wins; otherwise the suggestion fills the
                    // fields unless the person has typed in them.
                    if let Some((_, saved)) = self.at_saved() {
                        self.fill(&saved.place);
                    } else if let Some(suggestion) = (self.located)().and_then(|l| l.suggestion)
                        && !(self.typed)()
                    {
                        self.fill(&suggestion.place);
                    }
                    error.set(None);
                }
                Ok(other) => {
                    tracing::debug!(target: log_area::UI, ?other, "locate: unexpected answer");
                    error.set(Some(tr(messages::SOMETHING_WENT_WRONG)));
                }
                Err(failure) => error.set(Some(tr(problem_from_error(&failure).message()))),
            }
        });
    }
}

/// A new picker: empty, or showing `start` (an existing place's coordinates and place).
pub(crate) fn use_picker(
    start: Option<(Coordinates, &Place)>,
    editing: Option<SavedPlaceId>,
) -> Picker {
    let coordinates = start.map(|(at, _)| at);
    let text = |part: fn(&Place) -> String| start.map(|(_, place)| part(place)).unwrap_or_default();
    let (city, region, country) = (
        text(|p| p.city.as_str().to_owned()),
        text(|p| p.region.as_str().to_owned()),
        text(|p| p.country.as_str().to_owned()),
    );
    Picker {
        coordinates: use_signal(|| coordinates),
        fix: use_signal(|| None),
        located: use_signal(|| None),
        city: use_signal(|| city),
        region: use_signal(|| region),
        country: use_signal(|| country),
        // An existing place's own values are kept until the person chooses the suggestion.
        typed: use_signal(|| coordinates.is_some()),
        editing,
        units: crate::ui::units::use_units(),
    }
}

/// "1.9 km SW · Capital", "0.7 mi N · Town · county seat".
pub(crate) fn nearby_detail(place: &NearbyPlace, unit: DistanceUnit) -> String {
    let meters = f64::from(place.distance_m);
    let distance = match unit {
        DistanceUnit::Kilometers => tr_with(
            messages::PLACE_DISTANCE_KM,
            &[("km", MessageArg::from(format!("{:.1}", meters / 1000.0)))],
        ),
        DistanceUnit::Miles => tr_with(
            messages::PLACE_DISTANCE_MI,
            &[(
                "miles",
                MessageArg::from(format!("{:.1}", meters / 1609.344)),
            )],
        ),
    };
    let mut parts = vec![
        format!("{distance} {}", tr(place.direction.label())),
        tr(place.kind.label()),
    ];
    parts.extend(place.seat.map(|seat| tr(seat.label())));
    parts.join(" · ")
}

/// How the coordinates are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Where {
    /// The device's location.
    Device,
    /// Typed or pasted.
    Typed,
}

/// 1 · Where: the device's location, or typed coordinates.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn WherePart(picker: Picker) -> Element {
    let mut typing = use_signal(|| false);
    let mut typed_text = use_signal(String::new);
    // Which way was chosen; only that button is highlighted (none until one is chosen).
    let mut chosen = use_signal(|| None::<Where>);
    let mut finding = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let use_location = move |_| {
        chosen.set(Some(Where::Device));
        typing.set(false);
        finding.set(true);
        error.set(None);
        spawn(async move {
            let answer = document::eval(LOCATE_SCRIPT).join::<BrowserFix>().await;
            finding.set(false);
            match answer
                .map_err(|_| messages::PLACE_LOCATION_UNAVAILABLE)
                .and_then(|a| fix_from(&a))
            {
                Ok(fix) => picker.locate(fix.coordinates, Some(fix), error),
                Err(message) => {
                    tracing::debug!(target: log_area::UI, "no device location");
                    error.set(Some(tr(message)));
                }
            }
        });
    };
    let apply_typed = move |_| match Coordinates::parse_pair(&typed_text()) {
        Ok(coordinates) => {
            typing.set(false);
            picker.locate(coordinates, None, error);
        }
        Err(problem) => error.set(Some(tr(problem.message()))),
    };
    let now = SystemClock.now();
    rsx! {
        h3 { class: "section-heading", {tr(messages::PLACE_STEP_WHERE).to_uppercase()} }
        div { class: "picker-buttons",
            button {
                class: if chosen() == Some(Where::Device) { "secondary-button chosen" } else { "secondary-button" },
                "aria-pressed": if chosen() == Some(Where::Device) { "true" } else { "false" },
                r#type: "button",
                disabled: finding(),
                onclick: use_location,
                if finding() { {tr(messages::PLACE_LOCATING)} } else { {format!("◎ {}", tr(messages::PLACE_USE_LOCATION))} }
            }
            button {
                class: if chosen() == Some(Where::Typed) { "secondary-button chosen" } else { "secondary-button" },
                "aria-pressed": if chosen() == Some(Where::Typed) { "true" } else { "false" },
                r#type: "button",
                onclick: move |_| {
                    chosen.set(Some(Where::Typed));
                    typing.set(true);
                },
                {tr(messages::PLACE_TYPE_COORDINATES)}
            }
        }
        if typing() {
            label { class: "field",
                span { class: "field-label", {tr(messages::PLACE_COORDINATES)} }
                div { class: "picker-typed",
                    input {
                        class: "field-input mono",
                        autocomplete: "off",
                        spellcheck: "false",
                        placeholder: "35.5951, -82.5515",
                        value: typed_text(),
                        oninput: move |event| typed_text.set(event.value()),
                    }
                    button { class: "secondary-button", r#type: "button", onclick: apply_typed, {tr(messages::PLACE_COORDINATES_APPLY)} }
                }
                span { class: "muted-note", {tr(messages::PLACE_COORDINATES_HINT)} }
            }
        }
        match (picker.coordinates)() {
            Some(at) => rsx! {
                div { class: "picker-fix",
                    span { class: "mono", {format!("{:.5}, {:.5}", at.latitude(), at.longitude())} }
                    span { class: "muted-note", {fix_text((picker.fix)().as_ref(), now)} }
                    if (picker.fix)().is_some_and(|fix| fix.is_stale(now)) {
                        span { class: "form-error", {tr(messages::PLACE_FIX_STALE)} }
                    }
                }
            },
            None => rsx! { p { class: "muted-note", {tr(messages::PLACE_NO_LOCATION)} } },
        }
        FormError { message: error() }
    }
}

/// What the picker says about the point: at a saved place, offshore, the suggestion, or that
/// lookup is unavailable.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PlaceNotes(picker: Picker) -> Element {
    let Some(located) = (picker.located)() else {
        return rsx! {};
    };
    if let Some((_, saved)) = picker.at_saved() {
        let meters = (picker.coordinates)()
            .map_or(0.0, |at| saved.coordinates.distance_m(at))
            .round();
        let meters = format!("{meters:.0}").parse::<i64>().unwrap_or(0);
        return rsx! {
            p { class: "picker-note good",
                {tr_with(messages::PLACE_AT_SAVED, &[("name", MessageArg::from(saved.name.as_str())), ("meters", MessageArg::from(meters))])}
            }
        };
    }
    let Some(suggestion) = located.suggestion else {
        return rsx! { p { class: "picker-note", {tr(messages::PLACE_LOOKUP_UNAVAILABLE)} } };
    };
    let differs = (picker.typed)() && !matches_fields(&picker, &suggestion);
    rsx! {
        SuggestionNote { suggestion: suggestion.clone() }
        if differs {
            button {
                class: "secondary-button",
                r#type: "button",
                onclick: move |_| picker.fill(&suggestion.place),
                {tr(messages::PLACE_USE_SUGGESTION)}
            }
        }
    }
}

/// The nearby places to choose the city from (#061): tap one to fill City or town. Not shown
/// inside a saved place (its own place is used) or without a suggestion.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn NearbyChoices(picker: Picker) -> Element {
    let nearby = (picker.located)()
        .filter(|_| picker.at_saved().is_none())
        .and_then(|located| located.suggestion)
        .map(|suggestion| suggestion.nearby)
        .unwrap_or_default();
    if nearby.is_empty() {
        return rsx! {};
    }
    let mut city = picker.city;
    let mut typed = picker.typed;
    let unit = (picker.units)().distance;
    rsx! {
        div { class: "nearby", role: "group", "aria-label": tr(messages::PLACE_NEARBY),
            span { class: "field-label", {tr(messages::PLACE_NEARBY).to_uppercase()} }
            for place in nearby {
                {
                    let name = place.name.as_str().to_owned();
                    let on = city() == name;
                    rsx! {
                        button {
                            key: "{name}-{place.distance_m}",
                            r#type: "button",
                            class: if on { "nearby-choice chosen" } else { "nearby-choice" },
                            "aria-pressed": if on { "true" } else { "false" },
                            onclick: move |_| {
                                city.set(name.clone());
                                typed.set(false);
                            },
                            span { class: "nearby-mark", if on { "◉" } else { "○" } }
                            span { class: "nearby-name", {place.name.as_str().to_owned()} }
                            span { class: "nearby-detail", {nearby_detail(&place, unit)} }
                        }
                    }
                }
            }
            span { class: "muted-note", {tr(messages::PLACE_NEARBY_NOTE)} }
        }
    }
}

/// Whether the fields already say what `suggestion` does.
fn matches_fields(picker: &Picker, suggestion: &PlaceSuggestion) -> bool {
    (picker.city)() == suggestion.place.city.as_str()
        && (picker.region)() == suggestion.place.region.as_str()
        && (picker.country)().eq_ignore_ascii_case(suggestion.place.country.as_str())
}

/// The suggestion, or the offshore warning.
#[component]
fn SuggestionNote(suggestion: PlaceSuggestion) -> Element {
    let place = MessageArg::from(place_text(&suggestion.place));
    if !suggestion.inside_country {
        return rsx! { p { class: "picker-note warn", {tr_with(messages::PLACE_OFFSHORE, &[("place", place)])} } };
    }
    let zone = suggestion
        .time_zone
        .as_ref()
        .map_or_else(String::new, |zone| zone.as_str().to_owned());
    rsx! {
        div { class: "picker-note good",
            strong { {tr_with(messages::PLACE_SUGGESTED, &[("place", place)])} }
            span { class: "muted-note",
                {tr_with(messages::PLACE_SUGGESTED_DETAIL, &[("zone", MessageArg::from(zone)), ("version", MessageArg::from(suggestion.data_version.as_str()))])}
            }
        }
    }
}

/// 2 · Place: the notes, then city, state or province, and country (all editable).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn PlacePart(picker: Picker, countries: Vec<(CountryCode, String)>) -> Element {
    let Picker {
        mut city,
        mut region,
        mut country,
        mut typed,
        ..
    } = picker;
    rsx! {
        h3 { class: "section-heading", {tr(messages::PLACE_STEP_PLACE).to_uppercase()} }
        PlaceNotes { picker }
        NearbyChoices { picker }
        div { class: "field-row",
            label { class: "field",
                span { class: "field-label", {tr(messages::PLACE_CITY)} }
                input { class: "field-input", autocomplete: "off", value: city(), oninput: move |event| { city.set(event.value()); typed.set(true); } }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::PLACE_REGION)} }
                input { class: "field-input", autocomplete: "off", value: region(), oninput: move |event| { region.set(event.value()); typed.set(true); } }
            }
        }
        if countries.is_empty() {
            // No place data: a two-letter code instead of the list.
            label { class: "field",
                span { class: "field-label", {tr(messages::PLACE_COUNTRY_CODE)} }
                input { class: "field-input mono", autocomplete: "off", maxlength: "2", value: country(), oninput: move |event| { country.set(event.value()); typed.set(true); } }
            }
        } else {
            label { class: "field",
                span { class: "field-label", {tr(messages::PLACE_COUNTRY)} }
                select {
                    class: "field-input",
                    onchange: move |event| { country.set(event.value()); typed.set(true); },
                    option { value: "", selected: country().is_empty(), "—" }
                    for (code, name) in countries {
                        option { key: "{code.as_str()}", value: code.as_str(), selected: country().eq_ignore_ascii_case(code.as_str()), {name} }
                    }
                }
            }
        }
        p { class: "muted-note", {tr(messages::PLACE_TYPED_NOTE)} }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(lat: Option<f64>, error: Option<&str>) -> BrowserFix {
        BrowserFix {
            lat,
            lon: Some(-82.5515),
            accuracy: Some(12.4),
            at: Some(1_700_000_000_000.0),
            error: error.map(str::to_owned),
        }
    }

    #[test]
    fn the_browsers_answer_becomes_a_fix_or_a_reason() {
        let fix = fix_from(&answer(Some(35.5951), None)).unwrap();
        assert_eq!(fix.coordinates.lat_e7(), 355_951_000);
        assert_eq!(fix.accuracy_m, Some(12));
        assert_eq!(fix.taken_at, UnixMillis::from_millis(1_700_000_000_000));
        assert_eq!(
            fix_from(&answer(None, Some("denied"))),
            Err(messages::PLACE_LOCATION_DENIED)
        );
        assert_eq!(
            fix_from(&answer(None, Some("unavailable"))),
            Err(messages::PLACE_LOCATION_UNAVAILABLE)
        );
        assert_eq!(
            fix_from(&answer(Some(95.0), None)),
            Err(messages::PLACE_LOCATION_UNAVAILABLE),
            "out of range"
        );
    }

    #[test]
    fn a_nearby_place_says_how_far_which_way_and_what_it_is() {
        use domain::location::{Compass, PlaceKind, PlaceName, Seat};
        let place = NearbyPlace {
            name: PlaceName::parse("Asheville").unwrap(),
            distance_m: 1_900,
            direction: Compass::SW,
            kind: PlaceKind::Town,
            seat: Some(Seat::County),
        };
        assert_eq!(
            nearby_detail(&place, DistanceUnit::Kilometers),
            "1.9 km SW · Town · county seat"
        );
        assert_eq!(
            nearby_detail(&place, DistanceUnit::Miles),
            "1.2 mi SW · Town · county seat"
        );
        let plain = NearbyPlace {
            seat: None,
            kind: PlaceKind::Capital,
            distance_m: 400,
            ..place
        };
        assert_eq!(
            nearby_detail(&plain, DistanceUnit::Kilometers),
            "0.4 km SW · Capital"
        );
    }

    #[test]
    fn a_fix_says_how_sure_and_how_old() {
        let fix = fix_from(&answer(Some(35.5951), None)).unwrap();
        let at = |minutes: i64| UnixMillis::from_millis(1_700_000_000_000 + minutes * 60_000);
        assert_eq!(fix_text(Some(&fix), at(0)), "± 12 m · just now");
        assert_eq!(fix_text(Some(&fix), at(4)), "± 12 m · 4 min ago");
        assert_eq!(fix_text(None, at(0)), "Typed coordinates");
    }
}
