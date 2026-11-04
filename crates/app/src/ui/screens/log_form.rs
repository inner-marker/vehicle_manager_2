//! The fuel-up and reading forms (step 6g; option B of
//! `docs/ui-sketches/p2m6-fuel-up-options.svg`).
//!
//! Each is its own page. **Open:** the required items (the meters an event of this kind needs,
//! and required custom fields; a reading shows every meter), the fuel, and when and where.
//! **Folded,** each with a one-line summary: driving (highway, and towing and hauling where the
//! profile shows them, #033), trip and readings (the other meters and fields, and the
//! vehicle's own fuel economy), and who and notes. **Save** stays at the bottom.
//!
//! A required item can be skipped with a reason (the event is flagged). A reading that doesn't
//! fit its neighbours is refused; the Admin in Admin mode may keep it with a reason. The server
//! checks everything again.
//!
//! **A maintenance or repair job** (step 7i) is the same form with every meter open and the
//! work log's sections (`work_log`), and at the bottom its total, **Save draft**, and
//! **Finish…**: Finish's checks, then "Finish and lock" saves the job and finishes it (#034).
//! **Repair this** on a squawk's page opens a repair with its complaint and sign-off filled
//! in.

use dioxus::prelude::*;
use domain::api::{
    AssetSummary, AssetView, AssetsOutcome, EventsOutcome, LatestReading, PlacesOutcome,
    PreferencesOutcome, RequirementView, ScheduleOutcome, SquawkView, SquawksOutcome,
};
use domain::asset::AssetId;
use domain::custom_field::{CustomField, CustomFieldId, FieldKind, RangeCheck};
use domain::dates::{format_date, utc_to_local};
use domain::event::{
    ClashSide, Event, EventId, EventInput, EventLocation, EventProblem, ReadingClash, Requirement,
};
use domain::event_check::Warning;
use domain::event_form::{NO, YES};
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::location::{CountryCode, SavedPlace, SavedPlaceId, TimeZone};
use domain::log_area;
use domain::meter::{EventKind, Meter, MeterId, MeterUnit};
use domain::preferences::Preferences;
use domain::profile::{Section, profile};
use domain::quantity::{
    duration_as_typed, parse_money, parse_volume, show_money, show_reading, unit_price,
};
use domain::requirement::{Bundle, BundleId};
use domain::squawk::SquawkId;
use domain::time::Clock;

use super::log_chooser::use_log_chooser;
use super::log_input::{Context, Target, Typed, build, required_items, typed_from, zone};
use super::squawks::use_vehicle_squawks;
use super::vehicle_setup::event_from_value;
use super::work_input::{Action, FinishCheck, TypedDisposition, TypedWork, total_minor};
use super::work_log::{WorkLog, job_squawk_ids, job_squawks};
use super::{FormError, problem_from_error};
use crate::api;
use crate::logging::SystemClock;
use crate::ui::location::{Picker, PlacePart, WherePart, place_text, use_picker};
use crate::ui::routes::Route;
use crate::ui::units::{economy_abbr, field_unit, volume_abbr};
use crate::ui::vehicles::use_vehicles;
use crate::ui::{tr, tr_with};

/// The browser's own time zone, for an event with no place data.
pub(super) const ZONE_SCRIPT: &str =
    "return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC';";

/// What the form is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// A new event.
    New,
    /// Changing a draft.
    Edit(EventId),
    /// Correcting a finalized event (a new event, linked, with a reason).
    Correct(EventId),
}

/// `/vehicles/:asset_id/log/:kind`.
#[component]
pub(crate) fn LogForm(asset_id: String, kind: String) -> Element {
    match (asset_id.parse::<AssetId>(), event_from_value(&kind)) {
        (Ok(asset), Some(kind)) => {
            rsx! { LogScreen { asset, kind, mode: Mode::New, original: None, prefill: None } }
        }
        _ => rsx! { p { class: "placeholder", {tr(messages::VEHICLE_NOT_FOUND)} } },
    }
}

/// `/squawks/:squawk_id/repair` (**Repair this**): a new repair of the squawk's vehicle, its
/// complaint the squawk's description and the squawk ready to sign off (choice 16).
#[component]
pub(crate) fn RepairSquawk(squawk_id: String) -> Element {
    let mut loaded = use_signal(|| None::<Result<SquawkView, MessageId>>);
    use_effect(use_reactive!(|squawk_id| {
        spawn(async move {
            let answer = match squawk_id.parse::<SquawkId>() {
                Err(_) => Err(messages::SQUAWK_NOT_FOUND),
                Ok(id) => match api::squawk_view(id).await {
                    Ok(SquawksOutcome::Squawk { view }) => Ok(*view),
                    Ok(_) => Err(messages::SQUAWK_NOT_FOUND),
                    Err(error) => Err(problem_from_error(&error).message()),
                },
            };
            loaded.set(Some(answer));
        });
    }));
    match loaded() {
        None => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
        Some(Ok(view)) => rsx! {
            LogScreen { asset: view.squawk.asset, kind: EventKind::Repair, mode: Mode::New, original: None, prefill: Some(view) }
        },
    }
}

/// `/events/:event_id/edit`: the form, filled in with a draft.
#[component]
pub(crate) fn EditEvent(event_id: String) -> Element {
    rsx! { ExistingForm { event_id, correct: false } }
}

/// `/events/:event_id/correct`: the form, filled in with a finalized event, and a reason.
#[component]
pub(crate) fn CorrectEvent(event_id: String) -> Element {
    rsx! { ExistingForm { event_id, correct: true } }
}

/// Loads an event, then shows its form to edit or correct it.
#[component]
fn ExistingForm(event_id: String, correct: bool) -> Element {
    let mut loaded = use_signal(|| None::<Result<(EventId, Event), MessageId>>);
    use_effect(use_reactive!(|event_id| {
        spawn(async move {
            let answer = match event_id.parse::<EventId>() {
                Err(_) => Err(messages::EVENT_NOT_FOUND),
                Ok(id) => match api::event_view(id).await {
                    Ok(EventsOutcome::Event { view }) => Ok((id, view.event)),
                    Ok(EventsOutcome::NotFound) => Err(messages::EVENT_NOT_FOUND),
                    Ok(_) => Err(messages::SOMETHING_WENT_WRONG),
                    Err(error) => Err(problem_from_error(&error).message()),
                },
            };
            loaded.set(Some(answer));
        });
    }));
    match loaded() {
        None => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
        Some(Ok((id, event))) => {
            let mode = if correct {
                Mode::Correct(id)
            } else {
                Mode::Edit(id)
            };
            rsx! { LogScreen { asset: event.asset, kind: event.kind, mode, original: Some(event), prefill: None } }
        }
    }
}

/// What the form needs from the server.
#[derive(Debug, Clone, PartialEq)]
struct Loaded {
    view: AssetView,
    latest: Vec<LatestReading>,
    may_override: bool,
    places: Vec<(SavedPlaceId, SavedPlace)>,
    countries: Vec<(CountryCode, String)>,
    zone: TimeZone,
    units: Preferences,
    /// A job's schedule: the vehicle's requirements and bundles (for its tasks).
    requirements: Vec<RequirementView>,
    bundles: Vec<(BundleId, Bundle)>,
}

async fn load(asset: AssetId, kind: EventKind) -> Result<Loaded, MessageId> {
    let failed = |error: &ServerFnError| problem_from_error(error).message();
    let outcome = api::vehicle_page(asset).await.map_err(|e| failed(&e))?;
    if outcome == AssetsOutcome::NotFound {
        return Err(messages::VEHICLE_NOT_FOUND);
    }
    let AssetsOutcome::Asset { view } = outcome else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    let view = *view;
    let EventsOutcome::LogContext {
        latest,
        may_override_readings: may_override,
    } = api::event_log_context(asset)
        .await
        .map_err(|e| failed(&e))?
    else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    let places = match api::get_places().await {
        Ok(PlacesOutcome::Places { places }) => places,
        _ => Vec::new(),
    };
    let countries = match api::get_countries().await {
        Ok(PlacesOutcome::Countries { countries }) => countries,
        _ => Vec::new(),
    };
    let units = match api::get_preferences().await {
        Ok(PreferencesOutcome::Preferences { view }) => view.effective,
        _ => Preferences::default(),
    };
    let zone = document::eval(ZONE_SCRIPT)
        .join::<String>()
        .await
        .ok()
        .and_then(|zone| TimeZone::parse(&zone).ok())
        .filter(domain::dates::is_known_zone)
        .unwrap_or_else(TimeZone::utc);
    let (requirements, bundles) = if kind.finalizes_by_itself() {
        (Vec::new(), Vec::new())
    } else {
        match api::vehicle_schedule(asset).await {
            Ok(ScheduleOutcome::Schedule {
                requirements,
                bundles,
                ..
            }) => (requirements, bundles),
            _ => (Vec::new(), Vec::new()),
        }
    };
    Ok(Loaded {
        view,
        latest,
        may_override,
        places,
        countries,
        zone,
        units,
        requirements,
        bundles,
    })
}

/// A repair from **Repair this**: its complaint the squawk's description (and notes), the
/// squawk ready to sign off.
fn repair_start(squawk: &SquawkView) -> Typed {
    let notes = squawk.squawk.notes.as_str();
    let complaint = if notes.is_empty() {
        squawk.squawk.description.as_str().to_owned()
    } else {
        format!("{}\n{notes}", squawk.squawk.description.as_str())
    };
    Typed {
        work: TypedWork {
            complaint,
            dispositions: vec![TypedDisposition {
                action: Action::SignOff,
                ..TypedDisposition::undecided(squawk.id)
            }],
            ..TypedWork::default()
        },
        ..Typed::default()
    }
}

#[component]
fn LogScreen(
    asset: AssetId,
    kind: EventKind,
    mode: Mode,
    original: Option<Event>,
    prefill: Option<SquawkView>,
) -> Element {
    let mut loaded = use_signal(|| None::<Result<Loaded, MessageId>>);
    use_effect(use_reactive!(|asset, kind| {
        spawn(async move {
            loaded.set(Some(load(asset, kind).await));
        });
    }));
    match loaded() {
        None => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
        Some(Ok(data)) => {
            let start = match (&original, &prefill) {
                (Some(event), _) => {
                    typed_from(event, &data.view.meters, &data.view.fields, &data.units)
                }
                (None, Some(squawk)) => repair_start(squawk),
                (None, None) => Typed::default(),
            };
            // An edit or correction keeps the event's own zone when the place data has none.
            let data = Loaded {
                zone: original
                    .as_ref()
                    .map_or(data.zone.clone(), |event| event.dates.zone.clone()),
                ..data
            };
            rsx! { FormBody { kind, data, mode, start } }
        }
    }
}

// --- The form's state --------------------------------------------------------------------

/// Everything typed, as signals (copied into a [`Typed`] to save).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Inputs {
    readings: Signal<Vec<(MeterId, String)>>,
    fields: Signal<Vec<(CustomFieldId, String)>>,
    skipped: Signal<Vec<(Requirement, String)>>,
    volume: Signal<String>,
    price: Signal<String>,
    filled_full: Signal<bool>,
    missed_previous: Signal<bool>,
    highway: Signal<u8>,
    towing: Signal<u8>,
    hauling: Signal<u8>,
    towed: Signal<Vec<AssetId>>,
    economy: Signal<String>,
    when: Signal<Option<(String, String)>>,
    no_place_reason: Signal<Option<String>>,
    someone: Signal<Option<(String, String)>>,
    notes: Signal<String>,
    override_reason: Signal<Option<String>>,
    original_location: Signal<Option<EventLocation>>,
    original_start: Signal<Option<domain::time::UnixMillis>>,
    picker: Picker,
    /// A job's work log.
    work: Signal<TypedWork>,
    /// The problem shown, and where.
    problem: Signal<Option<(Target, String)>>,
}

impl Inputs {
    fn typed(self) -> Typed {
        Typed {
            readings: (self.readings)(),
            fields: (self.fields)(),
            skipped: (self.skipped)(),
            volume: (self.volume)(),
            price: (self.price)(),
            filled_full: (self.filled_full)(),
            missed_previous: (self.missed_previous)(),
            highway: (self.highway)(),
            towing: (self.towing)(),
            hauling: (self.hauling)(),
            towed: (self.towed)(),
            economy: (self.economy)(),
            when: (self.when)(),
            city: (self.picker.city)(),
            region: (self.picker.region)(),
            country: (self.picker.country)(),
            coordinates: (self.picker.coordinates)(),
            fix: (self.picker.fix)(),
            located: (self.picker.located)(),
            no_place_reason: (self.no_place_reason)(),
            someone: (self.someone)(),
            notes: (self.notes)(),
            override_reason: (self.override_reason)(),
            original_location: (self.original_location)(),
            original_start: (self.original_start)(),
            work: (self.work)(),
        }
    }

    fn problem_at(self, target: Target) -> Option<String> {
        (self.problem)()
            .filter(|(at, _)| *at == target)
            .map(|(_, message)| message)
    }

    fn skipping(self, item: Requirement) -> bool {
        (self.skipped)().iter().any(|(have, _)| *have == item)
    }
}

/// The form's signals, starting from `start` (empty for a new event).
fn use_inputs(start: &Typed) -> Inputs {
    let place = super::log_input::place(start).ok().flatten();
    let mut picker = use_picker(start.coordinates.zip(place.as_ref()), None);
    // A place without coordinates (typed by hand) still fills the place fields.
    let (city, region, country) = (
        start.city.clone(),
        start.region.clone(),
        start.country.clone(),
    );
    use_hook(move || {
        if (picker.coordinates)().is_none() && !city.is_empty() {
            picker.city.set(city);
            picker.region.set(region);
            picker.country.set(country);
            picker.typed.set(true);
        }
    });
    let start = start.clone();
    Inputs {
        readings: use_signal(|| start.readings.clone()),
        fields: use_signal(|| start.fields.clone()),
        skipped: use_signal(|| start.skipped.clone()),
        volume: use_signal(|| start.volume.clone()),
        price: use_signal(|| start.price.clone()),
        filled_full: use_signal(|| start.original_location.is_none() || start.filled_full),
        missed_previous: use_signal(|| start.missed_previous),
        highway: use_signal(|| {
            if start.original_location.is_none() {
                50
            } else {
                start.highway
            }
        }),
        towing: use_signal(|| start.towing),
        hauling: use_signal(|| start.hauling),
        towed: use_signal(|| start.towed.clone()),
        economy: use_signal(|| start.economy.clone()),
        when: use_signal(|| start.when.clone()),
        no_place_reason: use_signal(|| start.no_place_reason.clone()),
        someone: use_signal(|| start.someone.clone()),
        notes: use_signal(|| start.notes.clone()),
        override_reason: use_signal(|| start.override_reason.clone()),
        original_location: use_signal(|| start.original_location.clone()),
        original_start: use_signal(|| start.original_start),
        picker,
        work: use_signal(|| start.work.clone()),
        problem: use_signal(|| None),
    }
}

/// The text typed for `key` in a list of typed values.
fn text_of<K: PartialEq>(list: &[(K, String)], key: &K) -> String {
    list.iter()
        .find(|(have, _)| have == key)
        .map(|(_, text)| text.clone())
        .unwrap_or_default()
}

/// Sets the text typed for `key`.
fn set_text<K: PartialEq + Clone + 'static>(
    mut list: Signal<Vec<(K, String)>>,
    key: &K,
    text: String,
) {
    let mut values = list();
    match values.iter_mut().find(|(have, _)| have == key) {
        Some(entry) => entry.1 = text,
        None => values.push((key.clone(), text)),
    }
    list.set(values);
}

// --- The page ----------------------------------------------------------------------------

/// What the person sees after a save with warnings (otherwise the event's page opens).
#[derive(Debug, Clone, PartialEq)]
struct SavedWith(EventId, Vec<String>);

/// Sends the event as `mode` says.
async fn send(
    mode: Mode,
    input: EventInput,
    reason: String,
) -> Result<EventsOutcome, ServerFnError> {
    match mode {
        Mode::New => api::add_event(input).await,
        Mode::Edit(id) => api::edit_event(id, input).await,
        Mode::Correct(original) => api::correct_event(original, input, reason).await,
    }
}

/// "Lower than 52,310 mi on 1 Mar 2026. Check it, or correct that event."
fn clash_text(
    clash: ReadingClash,
    meters: &[(MeterId, Meter)],
    units: &Preferences,
    zone: &TimeZone,
) -> String {
    let Some((_, meter)) = meters.iter().find(|(id, _)| *id == clash.meter) else {
        return tr(messages::EVENT_READING_OUT_OF_ORDER);
    };
    let date =
        format_date(clash.at, zone, units.date_format, Translator::english()).unwrap_or_default();
    let id = match clash.side {
        ClashSide::BelowEarlier => messages::LOG_CLASH_BELOW,
        ClashSide::AboveLater => messages::LOG_CLASH_ABOVE,
    };
    tr_with(
        id,
        &[
            (
                "value",
                MessageArg::from(show_reading(clash.value, meter.unit, meter.precision)),
            ),
            ("unit", MessageArg::from(meter_abbr(meter.unit))),
            ("date", MessageArg::from(date)),
        ],
    )
}

/// Where the server's refusal shows, and what it says.
fn refusal(
    refused: EventProblem,
    mode: Mode,
    meters: &[(MeterId, Meter)],
    units: &Preferences,
    zone: &TimeZone,
) -> (Target, String) {
    if let EventProblem::ReadingOutOfOrder(clash) = refused {
        return (
            Target::Meter(clash.meter),
            clash_text(clash, meters, units, zone),
        );
    }
    // A correction's own reason (the others are checked as they're typed).
    let reason = matches!(
        refused,
        EventProblem::ReasonEmpty | EventProblem::ReasonTooLong
    );
    if reason && matches!(mode, Mode::Correct(_)) {
        return (Target::Correction, tr(refused.message()));
    }
    (Target::of_problem(refused), tr(refused.message()))
}

#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FormBody(kind: EventKind, data: Loaded, mode: Mode, start: Typed) -> Element {
    let inputs = use_inputs(&start);
    let units = data.units;
    let mut busy = use_signal(|| false);
    let mut saved = use_signal(|| None::<SavedWith>);
    let mut show_override = use_signal(|| None::<MeterId>);
    let mut reason = use_signal(String::new);
    let chooser = use_log_chooser();
    let navigator = use_navigator();
    let view = data.view.clone();
    let asset = view.id;
    let job = !kind.finalizes_by_itself();
    let all_squawks = use_vehicle_squawks(asset);
    let mut finishing = use_signal(|| false);
    let required = required_items(kind, &view.meters, &view.fields);
    // A reading, or a job's readings at the start, shows every meter.
    let open_meters: Vec<(MeterId, Meter)> = view
        .meters
        .iter()
        .filter(|(id, _)| {
            kind == EventKind::Reading || job || required.contains(&Requirement::Meter(*id))
        })
        .cloned()
        .collect();
    let required_fields: Vec<(CustomFieldId, CustomField)> = view
        .fields
        .iter()
        .filter(|(id, _)| required.contains(&Requirement::Field(*id)))
        .cloned()
        .collect();
    let (meters, fields, fallback) = (view.meters.clone(), view.fields.clone(), data.zone.clone());
    let may_override = data.may_override;
    // Save draft (`false`), or save and Finish (`true`, a job).
    let submit = Callback::new(move |finish: bool| {
        let context = Context {
            asset,
            kind,
            meters: &meters,
            fields: &fields,
            units: &units,
            fallback_zone: &fallback,
            now: SystemClock.now(),
        };
        let mut problem = inputs.problem;
        let input = match build(&inputs.typed(), &context) {
            Ok(input) => input,
            Err((target, message)) => {
                problem.set(Some((target, tr(message))));
                return;
            }
        };
        let zone = input.dates.zone.clone();
        let names = (meters.clone(), fields.clone());
        busy.set(true);
        spawn(async move {
            let result = send(mode, input, reason()).await;
            busy.set(false);
            match result {
                Ok(EventsOutcome::Saved { id, .. }) if finish => {
                    chooser.remember(asset);
                    match api::finish_event(id).await {
                        Ok(EventsOutcome::Done) => {
                            tracing::debug!(target: log_area::UI, event = %id, "job finished");
                            navigator.replace(Route::EventDetail {
                                event_id: id.to_string(),
                            });
                        }
                        Ok(EventsOutcome::Invalid { problem: refused }) => {
                            // Saved, but not finished: the draft's form, with the reason.
                            finishing.set(false);
                            problem.set(Some(refusal(refused, mode, &names.0, &units, &zone)));
                            navigator.replace(Route::EditEvent {
                                event_id: id.to_string(),
                            });
                        }
                        other => {
                            tracing::debug!(target: log_area::UI, ?other, "finish: unexpected answer");
                            finishing.set(false);
                            problem
                                .set(Some((Target::General, tr(messages::SOMETHING_WENT_WRONG))));
                        }
                    }
                }
                Ok(EventsOutcome::Saved { id, warnings }) => {
                    chooser.remember(asset);
                    tracing::debug!(target: log_area::UI, event = %id, warnings = warnings.len(), "event saved");
                    if warnings.is_empty() {
                        navigator.replace(Route::EventDetail {
                            event_id: id.to_string(),
                        });
                    } else {
                        let texts = warnings
                            .iter()
                            .map(|w| warning_text(*w, &names.0, &names.1))
                            .collect();
                        saved.set(Some(SavedWith(id, texts)));
                    }
                }
                Ok(EventsOutcome::Invalid { problem: refused }) => {
                    if let EventProblem::ReadingOutOfOrder(clash) = refused
                        && may_override
                    {
                        show_override.set(Some(clash.meter));
                    }
                    problem.set(Some(refusal(refused, mode, &names.0, &units, &zone)));
                }
                Ok(EventsOutcome::NotAllowed) => {
                    problem.set(Some((Target::General, tr(messages::VEHICLES_NOT_ALLOWED))));
                }
                Ok(other) => {
                    tracing::debug!(target: log_area::UI, ?other, "save event: unexpected answer");
                    problem.set(Some((Target::General, tr(messages::SOMETHING_WENT_WRONG))));
                }
                Err(error) => problem.set(Some((
                    Target::General,
                    tr(problem_from_error(&error).message()),
                ))),
            }
            finishing.set(false);
        });
    });
    if let Some(SavedWith(id, warnings)) = saved() {
        return rsx! { SavedNotice { event: id, warnings } };
    }
    let latest = data.latest.clone();
    let zone_now = zone(&inputs.typed(), &data.zone);
    let shown_squawks = job_squawks(
        &(all_squawks)()
            .map(|loaded| loaded.squawks)
            .unwrap_or_default(),
        &(inputs.work)(),
    );
    let checks = finish_checks(&inputs, &required, &shown_squawks);
    let currency = units.currency;
    let work_problem = |target: Target| inputs.problem_at(target);
    let end_problem = (inputs.problem)().and_then(|(at, text)| match at {
        Target::EndReading(meter) => Some((meter, text)),
        Target::Meter(_)
        | Target::Field(_)
        | Target::Fuel
        | Target::When
        | Target::Where
        | Target::Correction
        | Target::Work
        | Target::Squawks
        | Target::General => None,
    });
    rsx! {
        div { class: if job { "log-form job" } else { "log-form" },
            Link { class: "vehicle-back", to: Route::VehicleDetail { asset_id: asset.to_string() }, {format!("‹ {}", view.asset.name.as_str())} }
            if let Mode::Correct(_) = mode {
                div { class: "log-item",
                    p { class: "muted-note", {tr(messages::LOG_CORRECT_NOTE)} }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LOG_CORRECT_REASON)} }
                        input { class: "field-input", autocomplete: "off", value: reason(), oninput: move |event| reason.set(event.value()) }
                    }
                    FormError { message: inputs.problem_at(Target::Correction) }
                }
            }
            for (id, meter) in open_meters {
                MeterInput {
                    key: "{id}",
                    inputs, id, meter,
                    latest: latest.iter().find(|reading| reading.meter == id).copied(),
                    required: required.contains(&Requirement::Meter(id)),
                    show_override: show_override() == Some(id),
                    zone: zone_now.clone(),
                    units,
                }
            }
            for (id, field) in required_fields {
                FieldInput { key: "{id}", inputs, id, field, required: true, units }
            }
            if kind == EventKind::Fuel {
                FuelPart { inputs, units }
            }
            if job {
                WorkLog {
                    kind,
                    work: inputs.work,
                    vehicle: view.clone(),
                    requirements: data.requirements.clone(),
                    bundles: data.bundles.clone(),
                    squawks: shown_squawks,
                    currency,
                    work_problem: work_problem(Target::Work),
                    squawk_problem: work_problem(Target::Squawks),
                    end_problem,
                }
            }
            WhenWherePart { inputs, places: data.places.clone(), countries: data.countries, zone: zone_now.clone() }
            if kind == EventKind::Fuel {
                DrivingFold { inputs, view: view.clone() }
            }
            MoreFold { inputs, kind, view, required, units, zone: zone_now }
            WhoFold { inputs }
            FormError { message: inputs.problem_at(Target::General) }
            if job {
                div { class: "save-bar job-bar",
                    div { class: "job-total",
                        span { class: "section-heading", {tr(messages::WORK_TOTAL).to_uppercase()} }
                        strong { {show_money(total_minor(&(inputs.work)(), currency), currency)} }
                    }
                    div { class: "form-buttons",
                        button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: move |_| submit.call(false),
                            {tr(if busy() { messages::LOG_SAVING } else { messages::WORK_SAVE_DRAFT })}
                        }
                        button { class: "primary-button", r#type: "button", disabled: busy(), onclick: move |_| finishing.set(true),
                            {tr(messages::WORK_FINISH)}
                        }
                    }
                }
            } else {
                div { class: "save-bar",
                    p { class: "muted-note", {tr(messages::LOG_DRAFT_NOTE)} }
                    button { class: "primary-button", r#type: "button", disabled: busy(), onclick: move |_| submit.call(false),
                        {tr(if busy() { messages::LOG_SAVING } else { messages::LOG_SAVE })}
                    }
                }
            }
        }
        if finishing() {
            FinishDialog { checks, busy: busy(), on_finish: move |()| submit.call(true), on_close: move |()| finishing.set(false) }
        }
    }
}

/// Finish's checks, as far as the form can tell (the server checks everything again): the
/// required readings, the place, and every squawk the job deals with decided.
fn finish_checks(
    inputs: &Inputs,
    required: &[Requirement],
    squawks: &[SquawkView],
) -> Vec<FinishCheck> {
    let typed = inputs.typed();
    let readings = required.iter().all(|item| match item {
        Requirement::Meter(id) => {
            inputs.skipping(*item) || !text_of(&typed.readings, id).trim().is_empty()
        }
        Requirement::Field(id) => {
            inputs.skipping(*item) || !text_of(&typed.fields, id).trim().is_empty()
        }
    });
    let place = matches!(super::log_input::place(&typed), Ok(Some(_)))
        || typed
            .no_place_reason
            .as_ref()
            .is_some_and(|reason| !reason.trim().is_empty());
    let undecided = typed.work.undecided(&job_squawk_ids(squawks));
    let mut checks = vec![FinishCheck::Readings(readings), FinishCheck::Place(place)];
    checks.extend(
        squawks
            .iter()
            .filter(|view| undecided.contains(&view.id))
            .map(|view| FinishCheck::Undecided(view.squawk.description.as_str().to_owned())),
    );
    checks
}

/// "Finish this job?": its checks, then **Finish and lock** once they all pass (#034).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FinishDialog(
    checks: Vec<FinishCheck>,
    busy: bool,
    on_finish: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let ready = checks.iter().all(FinishCheck::passes);
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div { class: "dialog finish-dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::WORK_FINISH_TITLE)} }
                ul { class: "finish-checks",
                    for (index, check) in checks.into_iter().enumerate() {
                        li { key: "{index}", class: if check.passes() { "finish-check ok" } else { "finish-check missing" },
                            span { class: "finish-mark", if check.passes() { "✓" } else { "✕" } }
                            span {
                                match &check {
                                    FinishCheck::Undecided(description) => tr_with(check.message(), &[("description", MessageArg::from(description.as_str()))]),
                                    FinishCheck::Readings(_) | FinishCheck::Place(_) => tr(check.message()),
                                }
                            }
                        }
                    }
                }
                p { class: "dialog-text", {tr(messages::WORK_FINISH_LOCKS)} }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::WORK_FINISH_BACK)} }
                    button { class: "primary-button", r#type: "button", disabled: !ready || busy, onclick: move |_| on_finish.call(()),
                        {tr(if busy { messages::LOG_SAVING } else { messages::WORK_FINISH_LOCK })}
                    }
                }
            }
        }
    }
}

/// A warning in words.
fn warning_text(
    warning: Warning,
    meters: &[(MeterId, Meter)],
    fields: &[(CustomFieldId, CustomField)],
) -> String {
    match warning {
        Warning::OutOfRange { field, check } => {
            let name = fields
                .iter()
                .find(|(id, _)| *id == field)
                .map(|(_, field)| field.label.as_str().to_owned())
                .unwrap_or_default();
            let id = match check {
                RangeCheck::Above => messages::LOG_WARNING_ABOVE,
                RangeCheck::Below | RangeCheck::Within => messages::LOG_WARNING_BELOW,
            };
            tr_with(id, &[("name", MessageArg::from(name))])
        }
        Warning::ReadingKept { meter } => {
            let name = meters
                .iter()
                .find(|(id, _)| *id == meter)
                .map(|(_, meter)| meter.name.as_str().to_owned())
                .unwrap_or_default();
            tr_with(
                messages::LOG_WARNING_KEPT,
                &[("name", MessageArg::from(name))],
            )
        }
    }
}

/// "Saved as a draft", with the warnings, and Done.
#[component]
fn SavedNotice(event: EventId, warnings: Vec<String>) -> Element {
    rsx! {
        div { class: "log-form",
            p { class: "picker-note good", {tr(messages::LOG_SAVED)} }
            for warning in warnings {
                p { key: "{warning}", class: "picker-note warn", {warning} }
            }
            Link { class: "primary-button", to: Route::EventDetail { event_id: event.to_string() }, {tr(messages::LOG_DONE)} }
        }
    }
}

// --- Readings and fields -----------------------------------------------------------------

/// A meter's unit, short: "mi", "km", "h".
fn meter_abbr(unit: MeterUnit) -> String {
    tr(match unit {
        MeterUnit::Miles => messages::UNIT_ABBR_MI,
        MeterUnit::Kilometers => messages::UNIT_ABBR_KM,
        MeterUnit::Hours => messages::UNIT_ABBR_H,
    })
}

/// "last 52,310 · 1 Mar 2026".
fn latest_text(
    meter: &Meter,
    latest: LatestReading,
    zone: &TimeZone,
    units: &Preferences,
) -> String {
    let date =
        format_date(latest.at, zone, units.date_format, Translator::english()).unwrap_or_default();
    tr_with(
        messages::LOG_LAST_READING,
        &[
            (
                "value",
                MessageArg::from(show_reading(latest.value, meter.unit, meter.precision)),
            ),
            ("date", MessageArg::from(date)),
        ],
    )
}

/// A section heading, with REQUIRED when it is.
#[component]
fn Heading(text: String, required: bool) -> Element {
    rsx! {
        h3 { class: "section-heading",
            {text.to_uppercase()}
            if required {
                span { class: "badge required-badge", {tr(messages::LOG_REQUIRED).to_uppercase()} }
            }
        }
    }
}

/// One meter's reading: its last value as a hint, skipping (if required), and the Admin's
/// override when the server refused it.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MeterInput(
    inputs: Inputs,
    id: MeterId,
    meter: Meter,
    latest: Option<LatestReading>,
    required: bool,
    show_override: bool,
    zone: TimeZone,
    units: Preferences,
) -> Element {
    let item = Requirement::Meter(id);
    let label = format!("{} ({})", meter.name.as_str(), meter_abbr(meter.unit));
    rsx! {
        div { class: "log-item",
            Heading { text: meter.name.as_str().to_owned(), required }
            if inputs.skipping(item) {
                SkipReason { inputs, item }
            } else {
                label { class: "field",
                    span { class: "field-label", {label} }
                    div { class: "reading-input",
                        input {
                            class: if inputs.problem_at(Target::Meter(id)).is_some() { "field-input big invalid" } else { "field-input big" },
                            inputmode: "decimal",
                            autocomplete: "off",
                            value: text_of(&(inputs.readings)(), &id),
                            oninput: move |event| set_text(inputs.readings, &id, event.value()),
                        }
                        if let Some(latest) = latest {
                            span { class: "reading-hint", {latest_text(&meter, latest, &zone, &units)} }
                        }
                    }
                }
                if required {
                    SkipLink { inputs, item, label: messages::LOG_SKIP }
                }
            }
            FormError { message: inputs.problem_at(Target::Meter(id)) }
            if show_override {
                OverrideBox { inputs }
            }
        }
    }
}

/// The link that starts skipping a required item.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SkipLink(inputs: Inputs, item: Requirement, label: MessageId) -> Element {
    rsx! {
        button {
            class: "link-button",
            r#type: "button",
            onclick: move |_| set_text(inputs.skipped, &item, String::new()),
            {tr(label)}
        }
    }
}

/// Skipping: "Skipped", why, and "Enter it after all".
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SkipReason(inputs: Inputs, item: Requirement) -> Element {
    let mut skipped = inputs.skipped;
    rsx! {
        p { class: "skipped-mark", {tr(messages::LOG_SKIPPED)} }
        label { class: "field",
            span { class: "field-label", {tr(messages::LOG_SKIP_REASON)} }
            input {
                class: "field-input",
                autocomplete: "off",
                value: text_of(&skipped(), &item),
                oninput: move |event| set_text(skipped, &item, event.value()),
            }
        }
        button {
            class: "link-button",
            r#type: "button",
            onclick: move |_| skipped.with_mut(|list| list.retain(|(have, _)| *have != item)),
            {tr(messages::LOG_UNSKIP)}
        }
    }
}

/// The Admin's "keep it anyway", with a reason.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn OverrideBox(inputs: Inputs) -> Element {
    let mut reason = inputs.override_reason;
    rsx! {
        div { class: "override-box",
            span { class: "override-title", {tr(messages::LOG_OVERRIDE_TITLE).to_uppercase()} }
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_OVERRIDE_REASON)} }
                input {
                    class: "field-input",
                    autocomplete: "off",
                    value: reason().unwrap_or_default(),
                    oninput: move |event| reason.set(Some(event.value())),
                }
            }
            span { class: "muted-note", {tr(messages::LOG_OVERRIDE_NOTE)} }
        }
    }
}

/// One custom field, typed as its kind is.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FieldInput(
    inputs: Inputs,
    id: CustomFieldId,
    field: CustomField,
    required: bool,
    units: Preferences,
) -> Element {
    let item = Requirement::Field(id);
    let label = field_unit(&field.kind, units).map_or_else(
        || field.label.as_str().to_owned(),
        |unit| format!("{} ({unit})", field.label.as_str()),
    );
    let value = text_of(&(inputs.fields)(), &id);
    let set = move |event: FormEvent| set_text(inputs.fields, &id, event.value());
    rsx! {
        div { class: "log-item",
            if required {
                Heading { text: field.label.as_str().to_owned(), required }
            }
            if inputs.skipping(item) {
                SkipReason { inputs, item }
            } else {
                label { class: "field",
                    span { class: "field-label", {label} }
                    match &field.kind {
                        FieldKind::YesNo => rsx! {
                            select { class: "field-input", onchange: set,
                                option { value: "", selected: value.is_empty(), "—" }
                                option { value: YES, selected: value == YES, {tr(messages::LOG_YES)} }
                                option { value: NO, selected: value == NO, {tr(messages::LOG_NO)} }
                            }
                        },
                        FieldKind::Choice { options } => rsx! {
                            select { class: "field-input", onchange: set,
                                option { value: "", selected: value.is_empty(), "—" }
                                for choice in options.clone() {
                                    option { key: "{choice.as_str()}", value: choice.as_str(), selected: value == choice.as_str(), {choice.as_str().to_owned()} }
                                }
                            }
                        },
                        FieldKind::Date => rsx! { input { class: "field-input", r#type: "date", value, oninput: set } },
                        FieldKind::Text => rsx! { input { class: "field-input", autocomplete: "off", value, oninput: set } },
                        // Digits only, with the `:` put in as they're typed (number pads have none).
                        FieldKind::Duration => rsx! {
                            input {
                                class: "field-input",
                                inputmode: "numeric",
                                autocomplete: "off",
                                placeholder: "HH:MM",
                                value,
                                oninput: move |event: FormEvent| set_text(inputs.fields, &id, duration_as_typed(&event.value())),
                            }
                        },
                        FieldKind::Number { .. } | FieldKind::Distance | FieldKind::Volume | FieldKind::FuelEconomy => rsx! {
                            input { class: "field-input", inputmode: "decimal", autocomplete: "off", value, oninput: set }
                        },
                    }
                }
                if required {
                    SkipLink { inputs, item, label: messages::LOG_SKIP_FIELD }
                }
            }
            FormError { message: inputs.problem_at(Target::Field(id)) }
        }
    }
}

// --- Fuel --------------------------------------------------------------------------------

/// "3.459 USD per gal", once both the volume and price are typed.
fn per_unit(inputs: &Inputs, units: &Preferences) -> Option<String> {
    let volume = parse_volume(&(inputs.volume)(), units.volume).ok()?;
    let cost = parse_money(&(inputs.price)(), units.currency).ok()?;
    let price = unit_price(cost, volume, units.currency, units.volume)?;
    Some(tr_with(
        messages::LOG_PER_UNIT,
        &[
            ("price", MessageArg::from(price)),
            ("currency", MessageArg::from(units.currency.as_str())),
            ("unit", MessageArg::from(volume_abbr(units.volume))),
        ],
    ))
}

/// Volume, price, full and missed fill.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FuelPart(inputs: Inputs, units: Preferences) -> Element {
    let (mut volume, mut price) = (inputs.volume, inputs.price);
    let (mut full, mut missed) = (inputs.filled_full, inputs.missed_previous);
    let volume_label = tr_with(
        messages::LOG_VOLUME,
        &[("unit", MessageArg::from(volume_abbr(units.volume)))],
    );
    let price_label = tr_with(
        messages::LOG_PRICE,
        &[("currency", MessageArg::from(units.currency.as_str()))],
    );
    rsx! {
        div { class: "log-item",
            Heading { text: tr(messages::LOG_SECTION_FUEL), required: true }
            div { class: "field-row",
                label { class: "field",
                    span { class: "field-label", {volume_label} }
                    input { class: "field-input big", inputmode: "decimal", autocomplete: "off", value: volume(), oninput: move |event| volume.set(event.value()) }
                }
                label { class: "field",
                    span { class: "field-label", {price_label} }
                    input { class: "field-input big", inputmode: "decimal", autocomplete: "off", value: price(), oninput: move |event| price.set(event.value()) }
                }
            }
            if let Some(text) = per_unit(&inputs, &units) {
                p { class: "muted-note", {text} }
            }
            div { class: "check-row",
                label { class: "switch",
                    input { r#type: "checkbox", checked: full(), onchange: move |event| full.set(event.checked()) }
                    span { {tr(messages::LOG_FILLED_FULL)} }
                }
                label { class: "switch",
                    input { r#type: "checkbox", checked: missed(), onchange: move |event| missed.set(event.checked()) }
                    span { {tr(messages::LOG_MISSED)} }
                }
            }
            FormError { message: inputs.problem_at(Target::Fuel) }
        }
    }
}

// --- When and where ----------------------------------------------------------------------

/// When (now, or a date and time typed) and where (the picker, or a saved place).
#[component]
fn WhenWherePart(
    inputs: Inputs,
    places: Vec<(SavedPlaceId, SavedPlace)>,
    countries: Vec<(CountryCode, String)>,
    zone: TimeZone,
) -> Element {
    rsx! {
        div { class: "log-item",
            Heading { text: tr(messages::LOG_SECTION_WHEN_WHERE), required: false }
            WhenRow { inputs, zone }
            WhereRows { inputs, places, countries }
        }
    }
}

#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn WhenRow(inputs: Inputs, zone: TimeZone) -> Element {
    let mut when = inputs.when;
    let Some((date, time)) = when() else {
        return rsx! {
            div { class: "summary-row",
                span { class: "summary-text", {tr(messages::LOG_NOW)} }
                button {
                    class: "link-button",
                    r#type: "button",
                    onclick: move |_| {
                        let local = utc_to_local(SystemClock.now(), &zone).unwrap_or_default();
                        when.set(Some(local));
                    },
                    {tr(messages::LOG_CHANGE)}
                }
            }
        };
    };
    rsx! {
        div { class: "field-row",
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_DATE)} }
                input { class: "field-input", r#type: "date", value: date, oninput: move |event| when.set(Some((event.value(), time.clone()))) }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_TIME)} }
                input {
                    class: "field-input",
                    r#type: "time",
                    value: (when)().map(|(_, time)| time).unwrap_or_default(),
                    oninput: move |event| when.set(Some(((when)().map(|(date, _)| date).unwrap_or_default(), event.value()))),
                }
            }
        }
        span { class: "muted-note", {tr_with(messages::LOG_ZONE, &[("zone", MessageArg::from(zone.as_str()))])} }
        FormError { message: inputs.problem_at(Target::When) }
    }
}

#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn WhereRows(
    inputs: Inputs,
    places: Vec<(SavedPlaceId, SavedPlace)>,
    countries: Vec<(CountryCode, String)>,
) -> Element {
    let picker = inputs.picker;
    // Folded once there's a place (an edit or correction starts with one).
    let mut changing = use_signal(|| {
        super::log_input::place(&inputs.typed())
            .ok()
            .flatten()
            .is_none()
    });
    let mut no_place = inputs.no_place_reason;
    let error = use_signal(|| None::<String>);
    let place = super::log_input::place(&inputs.typed()).ok().flatten();
    let saved = (picker.located)().and_then(|located| located.saved);
    if let (Some(place), false) = (&place, changing()) {
        let title = saved.map_or_else(
            || place.city.as_str().to_owned(),
            |(_, saved)| saved.name.as_str().to_owned(),
        );
        return rsx! {
            div { class: "summary-row",
                span { class: "summary-text",
                    strong { {format!("📍 {title}")} }
                    span { class: "muted-note", {place_text(place)} }
                }
                button { class: "link-button", r#type: "button", onclick: move |_| changing.set(true), {tr(messages::LOG_CHANGE)} }
            }
            FormError { message: inputs.problem_at(Target::Where) }
        };
    }
    if let Some(reason) = no_place() {
        return rsx! {
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_NO_PLACE_REASON)} }
                input { class: "field-input", autocomplete: "off", value: reason, oninput: move |event| no_place.set(Some(event.value())) }
            }
            button { class: "link-button", r#type: "button", onclick: move |_| no_place.set(None), {tr(messages::LOG_PLACE_AFTER_ALL)} }
            FormError { message: inputs.problem_at(Target::Where) }
        };
    }
    rsx! {
        if !places.is_empty() {
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_SAVED_PLACE)} }
                select {
                    class: "field-input",
                    onchange: move |event| {
                        if let Some((_, place)) = places.iter().find(|(id, _)| id.to_string() == event.value()) {
                            picker.locate(place.coordinates, None, error);
                        }
                    },
                    option { value: "", "—" }
                    for (id, place) in places.clone() {
                        option { key: "{id}", value: "{id}", {place.name.as_str().to_owned()} }
                    }
                }
            }
        }
        WherePart { picker }
        PlacePart { picker, countries }
        FormError { message: error() }
        div { class: "check-row",
            if place.is_some() {
                button { class: "secondary-button", r#type: "button", onclick: move |_| changing.set(false), {tr(messages::LOG_PLACE_DONE)} }
            }
            button { class: "link-button", r#type: "button", onclick: move |_| no_place.set(Some(String::new())), {tr(messages::LOG_NO_PLACE)} }
        }
        FormError { message: inputs.problem_at(Target::Where) }
    }
}

// --- The folded sections -----------------------------------------------------------------

/// A folded section: its title and one-line summary, then what's in it.
#[component]
fn Fold(title: String, summary: String, children: Element) -> Element {
    rsx! {
        details { class: "fold",
            summary { class: "fold-head",
                span { class: "fold-title", {title} }
                span { class: "fold-summary", {summary} }
            }
            div { class: "fold-body", {children} }
        }
    }
}

/// A percentage slider in 5% steps, with marks at 0, 25, 50, 75, and 100.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn Slider(label: MessageId, value: Signal<u8>) -> Element {
    let mut value = value;
    rsx! {
        label { class: "field slider",
            span { class: "slider-head",
                span { class: "field-label", {tr(label)} }
                strong { {format!("{}%", value())} }
            }
            input {
                r#type: "range",
                min: "0",
                max: "100",
                step: "5",
                value: "{value()}",
                oninput: move |event| value.set(event.value().parse().unwrap_or(0)),
            }
            span { class: "slider-marks",
                for mark in ["0", "25", "50", "75", "100"] {
                    span { key: "{mark}", {mark} }
                }
            }
        }
    }
}

/// Driving since the last fill-up: highway, and towing and hauling where the profile shows
/// them, with the vehicles towed.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DrivingFold(inputs: Inputs, view: AssetView) -> Element {
    let vehicles = use_vehicles();
    let towing_shown = profile(&view.asset.kind)
        .is_some_and(|profile| profile.sections.contains(&Section::TowingHauling));
    let others: Vec<AssetSummary> = (vehicles.garage)()
        .map(|garage| garage.assets)
        .unwrap_or_default()
        .into_iter()
        .filter(|other| other.id != view.id)
        .collect();
    let mut towed = inputs.towed;
    let mut parts = vec![tr_with(
        messages::LOG_PCT_HIGHWAY,
        &[("pct", MessageArg::from(i64::from((inputs.highway)())))],
    )];
    if (inputs.towing)() > 0 {
        parts.push(tr_with(
            messages::LOG_PCT_TOWING,
            &[("pct", MessageArg::from(i64::from((inputs.towing)())))],
        ));
    }
    if (inputs.hauling)() > 0 {
        parts.push(tr_with(
            messages::LOG_PCT_HAULING,
            &[("pct", MessageArg::from(i64::from((inputs.hauling)())))],
        ));
    }
    rsx! {
        Fold { title: tr(messages::LOG_FOLD_DRIVING), summary: parts.join(" · "),
            Slider { label: messages::LOG_HIGHWAY, value: inputs.highway }
            if towing_shown {
                Slider { label: messages::LOG_TOWING, value: inputs.towing }
                Slider { label: messages::LOG_HAULING, value: inputs.hauling }
                if (inputs.towing)() > 0 && !others.is_empty() {
                    span { class: "field-label", {tr(messages::LOG_TOWED)} }
                    div { class: "chip-row",
                        for other in others {
                            {
                                let on = towed().contains(&other.id);
                                let id = other.id;
                                rsx! {
                                    button {
                                        key: "{id}",
                                        r#type: "button",
                                        class: if on { "choice-chip chosen" } else { "choice-chip" },
                                        "aria-pressed": if on { "true" } else { "false" },
                                        onclick: move |_| towed.with_mut(|list| {
                                            if list.contains(&id) { list.retain(|have| *have != id); } else { list.push(id); }
                                        }),
                                        {other.asset.name.as_str().to_owned()}
                                    }
                                }
                            }
                        }
                    }
                }
            }
            FormError { message: inputs.problem_at(Target::Fuel) }
        }
    }
}

/// Trip and readings: the meters and fields that aren't required, and (for a fuel-up) the
/// vehicle's own fuel economy.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MoreFold(
    inputs: Inputs,
    kind: EventKind,
    view: AssetView,
    required: Vec<Requirement>,
    units: Preferences,
    zone: TimeZone,
) -> Element {
    let meters: Vec<(MeterId, Meter)> = view
        .meters
        .iter()
        .filter(|(id, _)| kind == EventKind::Fuel && !required.contains(&Requirement::Meter(*id)))
        .cloned()
        .collect();
    let fields: Vec<(CustomFieldId, CustomField)> = view
        .fields
        .iter()
        .filter(|(id, field)| field.shown(kind) && !required.contains(&Requirement::Field(*id)))
        .cloned()
        .collect();
    if meters.is_empty() && fields.is_empty() && kind != EventKind::Fuel {
        return rsx! {};
    }
    let mut economy = inputs.economy;
    let entered: Vec<String> = fields
        .iter()
        .filter_map(|(id, field)| {
            let text = text_of(&(inputs.fields)(), id);
            (!text.trim().is_empty()).then(|| format!("{} {text}", field.label.as_str()))
        })
        .chain(
            (!economy().trim().is_empty())
                .then(|| format!("{} {}", economy_abbr(units.economy), economy())),
        )
        .collect();
    let summary = if entered.is_empty() {
        tr(messages::LOG_FOLD_MORE_EMPTY)
    } else {
        entered.join(" · ")
    };
    rsx! {
        Fold { title: tr(messages::LOG_FOLD_MORE), summary,
            for (id, meter) in meters {
                MeterInput { key: "{id}", inputs, id, meter, latest: None, required: false, show_override: false, zone: zone.clone(), units }
            }
            for (id, field) in fields {
                FieldInput { key: "{id}", inputs, id, field, required: false, units }
            }
            if kind == EventKind::Fuel {
                label { class: "field",
                    span { class: "field-label", {tr_with(messages::LOG_REPORTED_ECONOMY, &[("unit", MessageArg::from(economy_abbr(units.economy)))])} }
                    input { class: "field-input", inputmode: "decimal", autocomplete: "off", value: economy(), oninput: move |event| economy.set(event.value()) }
                }
            }
        }
    }
}

/// Who did it, and notes.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn WhoFold(inputs: Inputs) -> Element {
    let mut someone = inputs.someone;
    let mut notes = inputs.notes;
    let who = someone().map_or_else(|| tr(messages::LOG_WHO_ME), |(name, _)| name);
    let summary = format!(
        "{who} · {}",
        tr(if notes().trim().is_empty() {
            messages::LOG_NOTES_NONE
        } else {
            messages::LOG_NOTES_SOME
        })
    );
    rsx! {
        Fold { title: tr(messages::LOG_FOLD_WHO), summary,
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_DONE_BY)} }
                select {
                    class: "field-input",
                    onchange: move |event| someone.set((event.value() == "someone").then(|| (String::new(), String::new()))),
                    option { value: "me", selected: someone().is_none(), {tr(messages::LOG_DONE_BY_ME)} }
                    option { value: "someone", selected: someone().is_some(), {tr(messages::LOG_DONE_BY_SOMEONE)} }
                }
            }
            if let Some((name, contact)) = someone() {
                div { class: "field-row",
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LOG_THEIR_NAME)} }
                        input { class: "field-input", autocomplete: "off", value: name, oninput: move |event| someone.set(Some((event.value(), contact.clone()))) }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LOG_THEIR_CONTACT)} }
                        input {
                            class: "field-input",
                            autocomplete: "off",
                            value: (someone)().map(|(_, contact)| contact).unwrap_or_default(),
                            oninput: move |event| someone.set(Some(((someone)().map(|(name, _)| name).unwrap_or_default(), event.value()))),
                        }
                    }
                }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_NOTES)} }
                textarea { class: "field-input", rows: "3", value: notes(), oninput: move |event| notes.set(event.value()) }
            }
        }
    }
}
