//! Places: each person's saved places (decisions #059, #060; option A of
//! `docs/ui-sketches/p2m5-places-options.svg`).
//!
//! One list (name, city, state, and country, radius; Edit and Remove) with **+ Add place**.
//! Adding and editing open the place dialog, which is the location picker
//! (`crate::ui::location`) plus a name and radius. Only the person sees their places; no role
//! is needed.

use dioxus::prelude::*;
use domain::api::PlacesOutcome;
use domain::i18n::{MessageArg, messages};
use domain::location::{CountryCode, DEFAULT_RADIUS_M, SavedPlace, SavedPlaceForm, SavedPlaceId};
use domain::log_area;

use crate::api;
use crate::ui::location::{PlacePart, WherePart, place_text, use_picker};
use crate::ui::screens::{Field, FormError, problem_from_error};
use crate::ui::{tr, tr_with};

/// The radius choices, in meters.
const RADII: [u32; 8] = [25, 50, 100, 150, 250, 500, 1_000, 2_000];

/// "150 m", or "2 km" from a kilometer up.
pub(crate) fn distance_text(meters: u32) -> String {
    if meters >= 1_000 && meters.is_multiple_of(1_000) {
        tr_with(
            messages::PLACES_KILOMETERS,
            &[("km", MessageArg::from(i64::from(meters / 1_000)))],
        )
    } else {
        tr_with(
            messages::PLACES_METERS,
            &[("meters", MessageArg::from(i64::from(meters)))],
        )
    }
}

/// What went wrong, if the answer isn't a success.
fn failure(result: &Result<PlacesOutcome, ServerFnError>) -> Option<String> {
    match result {
        Ok(
            PlacesOutcome::Done
            | PlacesOutcome::Added { .. }
            | PlacesOutcome::Places { .. }
            | PlacesOutcome::Located { .. }
            | PlacesOutcome::Countries { .. }
            | PlacesOutcome::DataInfo { .. },
        ) => None,
        Ok(PlacesOutcome::Invalid { problem }) => Some(tr(problem.message())),
        Ok(PlacesOutcome::NotFound | PlacesOutcome::Unavailable) => {
            Some(tr(messages::SOMETHING_WENT_WRONG))
        }
        Err(error) => Some(tr(problem_from_error(error).message())),
    }
}

/// What's open over the list.
#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    Add,
    Edit(SavedPlaceId, SavedPlace),
    Remove(SavedPlaceId, String),
}

/// The Places screen.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Places() -> Element {
    let mut places = use_signal(|| None::<Vec<(SavedPlaceId, SavedPlace)>>);
    let mut error = use_signal(|| None::<String>);
    let mut dialog = use_signal(|| None::<Dialog>);
    let mut reloads = use_signal(|| 0_u32);
    // Loads after mounting (never during server rendering), and again after each change.
    use_effect(move || {
        let _ = reloads();
        spawn(async move {
            match api::get_places().await {
                Ok(PlacesOutcome::Places { places: list }) => {
                    tracing::debug!(target: log_area::UI, count = list.len(), "places loaded");
                    places.set(Some(list));
                }
                other => {
                    error.set(failure(&other).or_else(|| Some(tr(messages::SOMETHING_WENT_WRONG))));
                }
            }
        });
    });
    let Some(list) = places() else {
        return rsx! {
            FormError { message: error() }
            p { class: "auth-loading", {tr(messages::LOADING)} }
        };
    };
    let changed = move |()| {
        dialog.set(None);
        reloads += 1;
    };
    rsx! {
        div { class: "places",
            div { class: "places-toolbar",
                button { class: "primary-button", onclick: move |_| dialog.set(Some(Dialog::Add)), {tr(messages::PLACES_ADD)} }
                p { class: "muted-note", {tr(messages::PLACES_INTRO)} }
            }
            FormError { message: error() }
            if list.is_empty() {
                p { class: "placeholder", {tr(messages::PLACES_EMPTY)} }
            }
            for (id, place) in list {
                PlaceRow {
                    key: "{id}",
                    place: place.clone(),
                    on_remove: {
                        let name = place.name.as_str().to_owned();
                        move |()| dialog.set(Some(Dialog::Remove(id, name.clone())))
                    },
                    on_edit: move |()| dialog.set(Some(Dialog::Edit(id, place.clone()))),
                }
            }
            p { class: "muted-note", {tr(messages::PLACES_REMOVE_NOTE)} }
        }
        match dialog() {
            None => rsx! {},
            Some(Dialog::Add) => rsx! { PlaceDialog { current: None, on_close: move |()| dialog.set(None), on_saved: changed } },
            Some(Dialog::Edit(id, place)) => rsx! { PlaceDialog { current: Some((id, place)), on_close: move |()| dialog.set(None), on_saved: changed } },
            Some(Dialog::Remove(id, name)) => rsx! { ConfirmRemove { id, name, on_close: move |()| dialog.set(None), on_removed: changed } },
        }
    }
}

/// One place: name, its place, radius; Edit and Remove. On phones, tapping it edits it.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PlaceRow(place: SavedPlace, on_edit: EventHandler<()>, on_remove: EventHandler<()>) -> Element {
    rsx! {
        div { class: "place-row",
            button { class: "place-row-text", onclick: move |_| on_edit.call(()),
                strong { {place.name.as_str().to_owned()} }
                span { class: "muted-note", {place_text(&place.place)} }
            }
            span { class: "place-radius", {distance_text(place.radius_m)} }
            div { class: "place-row-buttons",
                button { class: "secondary-button", onclick: move |_| on_edit.call(()), {tr(messages::ACTION_EDIT)} }
                button { class: "secondary-button", onclick: move |_| on_remove.call(()), {tr(messages::ACTION_REMOVE)} }
            }
        }
    }
}

/// Every country, for the picker (empty while the server has no place data).
fn use_countries() -> Signal<Vec<(CountryCode, String)>> {
    let mut countries = use_signal(Vec::new);
    use_effect(move || {
        spawn(async move {
            if let Ok(PlacesOutcome::Countries { countries: list }) = api::get_countries().await {
                countries.set(list);
            }
        });
    });
    countries
}

/// Add a place, or edit one: the location picker, then the name and radius.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PlaceDialog(
    current: Option<(SavedPlaceId, SavedPlace)>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing = current.as_ref().map(|(id, _)| *id);
    let existing = current.as_ref().map(|(_, place)| place.clone());
    let picker = use_picker(
        existing
            .as_ref()
            .map(|place| (place.coordinates, &place.place)),
        editing,
    );
    let countries = use_countries();
    let name = use_signal(|| {
        existing
            .as_ref()
            .map(|place| place.name.as_str().to_owned())
            .unwrap_or_default()
    });
    let mut radius = use_signal(|| existing.as_ref().map_or(DEFAULT_RADIUS_M, |p| p.radius_m));
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    // New coordinates answer "set where it is first" (and any older complaint).
    use_effect(move || {
        if (picker.coordinates)().is_some() {
            error.set(None);
        }
    });
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let Some(at) = (picker.coordinates)() else {
            error.set(Some(tr(messages::PLACE_NEEDS_LOCATION)));
            return;
        };
        let form = SavedPlaceForm {
            name: name(),
            latitude: format!("{:.7}", at.latitude()),
            longitude: format!("{:.7}", at.longitude()),
            radius_m: radius(),
            city: (picker.city)(),
            region: (picker.region)(),
            country: (picker.country)(),
        };
        busy.set(true);
        spawn(async move {
            let result = match editing {
                Some(id) => api::edit_place(id, form).await,
                None => api::add_place(form).await,
            };
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                on_saved.call(());
            }
        });
    };
    let title = if editing.is_some() {
        messages::PLACE_EDIT_TITLE
    } else {
        messages::PLACE_ADD_TITLE
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog wide-dialog place-dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(title)} }
                WherePart { picker }
                PlacePart { picker, countries: countries() }
                h3 { class: "section-heading", {tr(messages::PLACE_STEP_NAME).to_uppercase()} }
                div { class: "field-row",
                    Field { label: messages::PLACE_NAME, value: name }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::PLACE_RADIUS)} }
                        select {
                            class: "field-input",
                            onchange: move |event| radius.set(event.value().parse().unwrap_or(DEFAULT_RADIUS_M)),
                            for meters in RADII {
                                option { key: "{meters}", value: "{meters}", selected: radius() == meters, {distance_text(meters)} }
                            }
                        }
                    }
                }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", disabled: busy(),
                        if busy() { {tr(messages::ACTION_SAVING)} } else { {tr(messages::ACTION_SAVE)} }
                    }
                }
            }
        }
    }
}

/// "Remove Home garage?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ConfirmRemove(
    id: SavedPlaceId,
    name: String,
    on_close: EventHandler<()>,
    on_removed: EventHandler<()>,
) -> Element {
    let mut error = use_signal(|| None::<String>);
    let remove = move |_| {
        spawn(async move {
            let result = api::delete_place(id).await;
            error.set(failure(&result));
            if error().is_none() {
                on_removed.call(());
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "alertdialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr_with(messages::PLACE_REMOVE_TITLE, &[("name", MessageArg::from(name))])} }
                p { class: "dialog-text", {tr(messages::PLACE_REMOVE_TEXT)} }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button filled", onclick: remove, {tr(messages::ACTION_REMOVE)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_read_in_meters_or_kilometers() {
        assert_eq!(distance_text(150), "150 m");
        assert_eq!(distance_text(1_000), "1 km");
        assert_eq!(distance_text(2_000), "2 km");
        assert_eq!(distance_text(1_500), "1500 m");
        for meters in RADII {
            assert!(domain::location::check_radius(meters).is_ok(), "{meters}");
        }
        assert!(RADII.contains(&DEFAULT_RADIUS_M));
    }
}
