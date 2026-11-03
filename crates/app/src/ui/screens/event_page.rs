//! An event's page (step 6h; option A of `docs/ui-sketches/p2m6-event-options.svg`).
//!
//! One column: the kind and date, the status ("Draft · locks in 23 h", or "Finalized") and the
//! flags, the actions, the big numbers, then each part as a short list. A draft can be edited,
//! finalized now (with a confirmation, #034), or deleted; a finalized event can only be
//! corrected. An original and its correction link to each other, each with the reason.

use dioxus::prelude::*;
use domain::api::{AssetView, AssetsOutcome, EventView, EventsOutcome, PreferencesOutcome};
use domain::custom_field::FieldKind;
use domain::dates::{format_date, format_time};
use domain::event::{Event, EventId, EventSource, EventStatus, Finalizer, Performer, Requirement};
use domain::event_form::field_text;
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::location::{LocationSource, TimeZone};
use domain::log_area;
use domain::meter::EventKind;
use domain::preferences::Preferences;
use domain::quantity::{economy_value, show_money, show_reading, show_volume, unit_price};
use domain::time::{Clock, UnixMillis};

use super::{FormError, problem_from_error};
use crate::api;
use crate::logging::SystemClock;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::location::place_text;
use crate::ui::routes::Route;
use crate::ui::units::{economy_abbr, field_unit, volume_abbr};
use crate::ui::vehicles::use_vehicles;
use crate::ui::{tr, tr_with};

/// What the page shows.
#[derive(Debug, Clone, PartialEq)]
struct Loaded {
    view: EventView,
    vehicle: AssetView,
    units: Preferences,
    /// The other side of a correction: the correction, or the original.
    linked: Option<EventView>,
}

async fn load(id: EventId) -> Result<Loaded, MessageId> {
    let failed = |error: &ServerFnError| problem_from_error(error).message();
    let outcome = api::event_view(id).await.map_err(|e| failed(&e))?;
    if outcome == EventsOutcome::NotFound {
        return Err(messages::EVENT_NOT_FOUND);
    }
    let EventsOutcome::Event { view } = outcome else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    let view = *view;
    let AssetsOutcome::Asset { view: vehicle } = api::vehicle_page(view.event.asset)
        .await
        .map_err(|e| failed(&e))?
    else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    let units = match api::get_preferences().await {
        Ok(PreferencesOutcome::Preferences { view }) => view.effective,
        _ => Preferences::default(),
    };
    let linked_id = view.corrected_by.or(view.event.corrects);
    let linked = match linked_id {
        Some(other) => match api::event_view(other).await {
            Ok(EventsOutcome::Event { view }) => Some(*view),
            _ => None,
        },
        None => None,
    };
    Ok(Loaded {
        view,
        vehicle: *vehicle,
        units,
        linked,
    })
}

/// `/events/:event_id`.
#[component]
pub(crate) fn EventDetail(event_id: String) -> Element {
    match event_id.parse::<EventId>() {
        Ok(id) => rsx! { EventScreen { id } },
        Err(_) => rsx! { p { class: "placeholder", {tr(messages::EVENT_NOT_FOUND)} } },
    }
}

#[component]
fn EventScreen(id: EventId) -> Element {
    let mut loaded = use_signal(|| None::<Result<Loaded, MessageId>>);
    let reloads = use_signal(|| 0_u32);
    use_effect(use_reactive!(|id| {
        let _ = reloads();
        spawn(async move {
            loaded.set(Some(load(id).await));
        });
    }));
    match loaded() {
        None => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
        Some(Ok(data)) => rsx! { EventBody { data, reloads } },
    }
}

/// "2 Nov 2026" and "3:30 pm" in `zone`.
fn when_text(at: UnixMillis, zone: &TimeZone, units: &Preferences) -> (String, String) {
    let english = Translator::english();
    (
        format_date(at, zone, units.date_format, english).unwrap_or_default(),
        format_time(at, zone, english).unwrap_or_default(),
    )
}

/// "2 Nov 2026, 3:30 pm".
fn at_text(at: UnixMillis, zone: &TimeZone, units: &Preferences) -> String {
    let (date, time) = when_text(at, zone, units);
    format!("{date}, {time}")
}

/// "23 h" or "4 min" until a draft locks (or "soon", once it's due).
fn locks_in(event: &Event, delay_millis: i64, now: UnixMillis) -> String {
    let left = event.last_edited_at.as_millis() + delay_millis - now.as_millis();
    let minutes = left / 60_000;
    if left <= 0 {
        tr(messages::EVENT_LOCKS_SOON)
    } else if minutes >= 60 {
        tr_with(
            messages::EVENT_LOCKS_HOURS,
            &[("hours", MessageArg::from(minutes / 60))],
        )
    } else {
        tr_with(
            messages::EVENT_LOCKS_MINUTES,
            &[("minutes", MessageArg::from(minutes.max(1)))],
        )
    }
}

/// What's open over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Finalize,
    Delete,
}

#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn EventBody(data: Loaded, reloads: Signal<u32>) -> Element {
    let auth = use_auth();
    let mut confirm = use_signal(|| None::<Confirm>);
    let Loaded {
        view,
        vehicle,
        units,
        linked,
    } = data;
    let event = view.event.clone();
    let zone = event.dates.zone.clone();
    let delay = match auth() {
        AuthState::SignedIn(session) => session.timers.finalize_after_millis(),
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::Unreachable(_) => domain::event::FINALIZE_AFTER_MILLIS,
    };
    let status = match (event.status, event.finalized) {
        (EventStatus::Finalized, Some(done)) => tr_with(
            messages::EVENT_STATUS_FINALIZED,
            &[(
                "date",
                MessageArg::from(when_text(done.at, &zone, &units).0),
            )],
        ),
        _ => tr_with(
            messages::EVENT_STATUS_DRAFT,
            &[(
                "when",
                MessageArg::from(locks_in(&event, delay, SystemClock.now())),
            )],
        ),
    };
    let finalized = event.status == EventStatus::Finalized;
    rsx! {
        div { class: "event-page",
            Link { class: "vehicle-back", to: Route::VehicleDetail { asset_id: vehicle.id.to_string() }, {format!("‹ {}", vehicle.asset.name.as_str())} }
            h2 { class: "event-title", {format!("{} · {}", tr(event.kind.label()), at_text(event.dates.start, &zone, &units))} }
            div { class: "chip-row",
                span { class: if finalized { "status-chip finalized" } else { "status-chip draft" }, {status} }
                for flag in event.flags() {
                    span { key: "{flag:?}", class: "status-chip flag", {format!("⚑ {}", tr(flag.label()))} }
                }
            }
            LinkBanner { view: view.clone(), linked, units }
            div { class: "check-row",
                if view.may_change {
                    Link { class: "secondary-button", to: Route::EditEvent { event_id: view.id.to_string() }, {tr(messages::ACTION_EDIT)} }
                    button { class: "primary-button", r#type: "button", onclick: move |_| confirm.set(Some(Confirm::Finalize)), {tr(messages::EVENT_FINALIZE_NOW)} }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| confirm.set(Some(Confirm::Delete)), {tr(messages::EVENT_DELETE)} }
                }
                if view.may_correct {
                    Link { class: "secondary-button", to: Route::CorrectEvent { event_id: view.id.to_string() }, {tr(messages::EVENT_CORRECT)} }
                }
            }
            Tiles { event: event.clone(), vehicle: vehicle.clone(), units }
            Details { view: view.clone(), vehicle, units }
        }
        if let Some(what) = confirm() {
            ConfirmDialog { what, id: view.id, asset: event.asset, reloads, on_close: move |()| confirm.set(None) }
        }
    }
}

/// The link between an original and its correction, with the reason.
#[component]
fn LinkBanner(view: EventView, linked: Option<EventView>, units: Preferences) -> Element {
    let Some(other) = linked else {
        return rsx! {};
    };
    if view.corrected_by == Some(other.id) {
        let reason = other
            .event
            .correction_reason
            .as_ref()
            .map(|reason| reason.as_str().to_owned())
            .unwrap_or_default();
        let date = when_text(other.event.received_at, &other.event.dates.zone, &units).0;
        return rsx! {
            div { class: "link-banner corrected",
                strong { {tr_with(messages::EVENT_CORRECTED_BY, &[("date", MessageArg::from(date)), ("name", MessageArg::from(other.logged_by_name.as_str())), ("reason", MessageArg::from(reason))])} }
                Link { to: Route::EventDetail { event_id: other.id.to_string() }, {tr(messages::EVENT_SEE_CORRECTION)} }
            }
        };
    }
    let reason = view
        .event
        .correction_reason
        .as_ref()
        .map(|reason| reason.as_str().to_owned())
        .unwrap_or_default();
    let date = when_text(other.event.dates.start, &other.event.dates.zone, &units).0;
    rsx! {
        div { class: "link-banner correction",
            strong { {tr_with(messages::EVENT_CORRECTS, &[("kind", MessageArg::from(tr(other.event.kind.label()).to_lowercase())), ("date", MessageArg::from(date)), ("reason", MessageArg::from(reason))])} }
            Link { to: Route::EventDetail { event_id: other.id.to_string() }, {tr(messages::EVENT_SEE_ORIGINAL)} }
        }
    }
}

/// The big numbers: each reading, and a fuel-up's volume and price.
#[component]
fn Tiles(event: Event, vehicle: AssetView, units: Preferences) -> Element {
    let mut tiles: Vec<(String, String)> = event
        .readings
        .iter()
        .filter_map(|reading| {
            let (_, meter) = vehicle.meters.iter().find(|(id, _)| *id == reading.meter)?;
            Some((
                format!(
                    "{} {}",
                    show_reading(reading.value, meter.unit, meter.precision),
                    tr(meter_abbr(meter.unit))
                ),
                meter.name.as_str().to_owned(),
            ))
        })
        .collect();
    if let Some(fuel) = &event.fuel {
        tiles.push((
            show_volume(fuel.volume_ml, units.volume),
            tr(if fuel.filled_full {
                messages::EVENT_FILLED_FULL
            } else {
                messages::EVENT_PARTIAL_FILL
            }),
        ));
        if let Some(cost) = fuel.cost_minor {
            let per = unit_price(cost, fuel.volume_ml, fuel.currency, units.volume).map(|price| {
                tr_with(
                    messages::LOG_PER_UNIT,
                    &[
                        ("price", MessageArg::from(price)),
                        ("currency", MessageArg::from(fuel.currency.as_str())),
                        ("unit", MessageArg::from(volume_abbr(units.volume))),
                    ],
                )
            });
            tiles.push((show_money(cost, fuel.currency), per.unwrap_or_default()));
        }
    }
    rsx! {
        div { class: "event-tiles",
            for (big, small) in tiles {
                div { key: "{big}", class: "event-tile",
                    strong { {big} }
                    span { {small} }
                }
            }
        }
    }
}

/// A meter unit's short name.
const fn meter_abbr(unit: domain::meter::MeterUnit) -> MessageId {
    match unit {
        domain::meter::MeterUnit::Miles => messages::UNIT_ABBR_MI,
        domain::meter::MeterUnit::Kilometers => messages::UNIT_ABBR_KM,
        domain::meter::MeterUnit::Hours => messages::UNIT_ABBR_H,
    }
}

/// One short list: a heading and its rows.
#[component]
fn Rows(heading: MessageId, rows: Vec<(String, String)>) -> Element {
    if rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        section { class: "event-section",
            h3 { class: "section-heading", {tr(heading).to_uppercase()} }
            dl { class: "event-rows",
                for (key, value) in rows {
                    div { key: "{key}", class: "event-row",
                        dt { {key} }
                        dd { {value} }
                    }
                }
            }
        }
    }
}

/// Where: the place (or why there's none), where it came from, and the zone.
fn where_rows(event: &Event) -> Vec<(String, String)> {
    let location = &event.location;
    let mut rows = Vec::new();
    if let Some(name) = &location.place_name {
        rows.push((tr(messages::EVENT_PLACE_NAME), name.as_str().to_owned()));
    }
    match (&location.place, &location.missing_reason) {
        (Some(place), _) => rows.push((tr(messages::EVENT_PLACE), place_text(place))),
        (None, reason) => rows.push((
            tr(messages::EVENT_NO_PLACE),
            reason
                .as_ref()
                .map(|r| r.as_str().to_owned())
                .unwrap_or_default(),
        )),
    }
    let source = tr(match location.source {
        LocationSource::Gps => messages::EVENT_SOURCE_GPS,
        LocationSource::SavedPlace => messages::EVENT_SOURCE_SAVED,
        LocationSource::Map => messages::EVENT_SOURCE_MAP,
        LocationSource::Manual => messages::EVENT_SOURCE_MANUAL,
    });
    let source = location
        .accuracy_m
        .map_or(source.clone(), |meters| format!("{source} · ± {meters} m"));
    rows.push((tr(messages::EVENT_FROM), source));
    rows.push((
        tr(messages::EVENT_ZONE),
        event.dates.zone.as_str().to_owned(),
    ));
    rows
}

/// Driving: the percentages and the vehicles towed.
fn driving_rows(
    event: &Event,
    names: &dyn Fn(domain::asset::AssetId) -> String,
) -> Vec<(String, String)> {
    let Some(fuel) = &event.fuel else {
        return Vec::new();
    };
    let mut towing = format!("{}%", fuel.towing_pct);
    if !fuel.towed.is_empty() {
        let towed: Vec<String> = fuel.towed.iter().map(|id| names(*id)).collect();
        towing = format!("{towing} · {}", towed.join(", "));
    }
    vec![
        (tr(messages::LOG_HIGHWAY), format!("{}%", fuel.highway_pct)),
        (tr(messages::LOG_TOWING), towing),
        (tr(messages::LOG_HAULING), format!("{}%", fuel.hauling_pct)),
    ]
}

/// Trip and readings: field values, skipped items, and the vehicle's own fuel economy.
fn trip_rows(event: &Event, vehicle: &AssetView, units: &Preferences) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = event
        .fields
        .iter()
        .filter_map(|value| {
            let (_, field) = vehicle.fields.iter().find(|(id, _)| *id == value.field)?;
            let text = field_text(&field.kind, &value.data, units);
            let text = match (&field.kind, field_unit(&field.kind, *units)) {
                (FieldKind::Duration, _) | (_, None) => text,
                (_, Some(unit)) => format!("{text} {unit}"),
            };
            Some((field.label.as_str().to_owned(), text))
        })
        .collect();
    for skip in &event.skipped {
        let name = match skip.item {
            Requirement::Meter(id) => vehicle
                .meters
                .iter()
                .find(|(have, _)| *have == id)
                .map(|(_, m)| m.name.as_str().to_owned()),
            Requirement::Field(id) => vehicle
                .fields
                .iter()
                .find(|(have, _)| *have == id)
                .map(|(_, f)| f.label.as_str().to_owned()),
        };
        rows.push((
            name.unwrap_or_default(),
            tr_with(
                messages::EVENT_SKIPPED_BECAUSE,
                &[("reason", MessageArg::from(skip.reason.as_str()))],
            ),
        ));
    }
    if let Some(economy) = event.fuel.as_ref().and_then(|fuel| fuel.reported_economy) {
        rows.push((
            tr(messages::EVENT_REPORTED_ECONOMY),
            format!(
                "{} {}",
                economy_value(economy, units.economy),
                economy_abbr(units.economy)
            ),
        ));
    }
    rows
}

/// A short list's rows: each label and its value.
type RowList = Vec<(String, String)>;

/// Who and notes, and the record: logged, edited, finalized.
fn record_rows(view: &EventView, units: &Preferences) -> (RowList, RowList) {
    let event = &view.event;
    let zone = &event.dates.zone;
    let mut who = Vec::new();
    match &event.performed_by {
        Some(Performer::Named { name, contact }) => {
            let text = contact
                .as_ref()
                .map_or_else(|| name.clone(), |contact| format!("{name} · {contact}"));
            who.push((tr(messages::LOG_DONE_BY), text));
        }
        Some(Performer::User { .. }) | None => who.push((
            tr(messages::LOG_DONE_BY),
            tr_with(
                messages::EVENT_DONE_BY_LOGGER,
                &[("name", MessageArg::from(view.logged_by_name.as_str()))],
            ),
        )),
    }
    if !event.notes.as_str().is_empty() {
        who.push((tr(messages::LOG_NOTES), event.notes.as_str().to_owned()));
    }
    if let Some(reason) = &event.reading_override {
        who.push((tr(messages::EVENT_OVERRIDE), reason.as_str().to_owned()));
    }
    let source = tr(match event.source {
        EventSource::Typed => messages::EVENT_TYPED,
        EventSource::Voice => messages::EVENT_VOICE,
        EventSource::Import => messages::EVENT_IMPORTED,
    });
    let mut record = vec![
        (
            tr(messages::EVENT_LOGGED),
            format!(
                "{} · {} · {source}",
                view.logged_by_name,
                at_text(event.received_at, zone, units)
            ),
        ),
        (
            tr(messages::EVENT_LAST_EDITED),
            at_text(event.last_edited_at, zone, units),
        ),
    ];
    if let Some(done) = event.finalized {
        let by = match done.by {
            Finalizer::User(_) => view.finalized_by_name.clone().unwrap_or_default(),
            Finalizer::Server => tr(messages::EVENT_BY_SERVER),
        };
        record.push((
            tr(messages::EVENT_FINALIZED_ROW),
            format!("{by} · {}", at_text(done.at, zone, units)),
        ));
    }
    (who, record)
}

#[component]
fn Details(view: EventView, vehicle: AssetView, units: Preferences) -> Element {
    let vehicles = use_vehicles();
    let names = move |id: domain::asset::AssetId| {
        (vehicles.garage)()
            .and_then(|garage| garage.assets.into_iter().find(|asset| asset.id == id))
            .map(|asset| asset.asset.name.as_str().to_owned())
            .unwrap_or_default()
    };
    let event = &view.event;
    let (who, record) = record_rows(&view, &units);
    rsx! {
        Rows { heading: messages::LOG_SECTION_WHEN_WHERE, rows: where_rows(event) }
        if event.kind == EventKind::Fuel {
            Rows { heading: messages::LOG_FOLD_DRIVING, rows: driving_rows(event, &names) }
        }
        Rows { heading: messages::LOG_FOLD_MORE, rows: trip_rows(event, &vehicle, &units) }
        Rows { heading: messages::LOG_FOLD_WHO, rows: who }
        Rows { heading: messages::EVENT_RECORD, rows: record }
    }
}

/// Finalize now's or Delete's confirmation.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ConfirmDialog(
    what: Confirm,
    id: EventId,
    asset: domain::asset::AssetId,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let navigator = use_navigator();
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| None::<String>);
    let (title, text, action) = match what {
        Confirm::Finalize => (
            messages::EVENT_FINALIZE_TITLE,
            messages::EVENT_FINALIZE_TEXT,
            messages::EVENT_FINALIZE,
        ),
        Confirm::Delete => (
            messages::EVENT_DELETE_TITLE,
            messages::EVENT_DELETE_TEXT,
            messages::EVENT_DELETE,
        ),
    };
    let go = move |_| {
        busy.set(true);
        spawn(async move {
            let result = match what {
                Confirm::Finalize => api::finalize_event(id).await,
                Confirm::Delete => api::delete_event(id).await,
            };
            busy.set(false);
            match result {
                Ok(EventsOutcome::Done) => {
                    tracing::debug!(target: log_area::UI, event = %id, ?what, "event changed");
                    on_close.call(());
                    match what {
                        Confirm::Finalize => reloads.with_mut(|count| *count += 1),
                        Confirm::Delete => {
                            navigator.replace(Route::VehicleDetail {
                                asset_id: asset.to_string(),
                            });
                        }
                    }
                }
                Ok(EventsOutcome::Invalid { problem }) => error.set(Some(tr(problem.message()))),
                Ok(EventsOutcome::NotAllowed) => {
                    error.set(Some(tr(messages::VEHICLES_NOT_ALLOWED)));
                }
                Ok(_) => error.set(Some(tr(messages::SOMETHING_WENT_WRONG))),
                Err(failure) => error.set(Some(tr(problem_from_error(&failure).message()))),
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "dialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr(title)} }
                p { class: "dialog-text", {tr(text)} }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button {
                        class: if what == Confirm::Delete { "primary-button danger-button" } else { "primary-button" },
                        r#type: "button",
                        disabled: busy(),
                        onclick: go,
                        {tr(action)}
                    }
                }
            }
        }
    }
}
