//! Squawks on screen (milestone 7 step 7f; plan §3 Squawks; decisions #051, #063, #064).
//!
//! The human's pick from `docs/ui-sketches/p2m7-squawks-options.svg` (choice 14):
//! - **The Overview's card** lists the outstanding ones, High first (option A's panel).
//! - **The Squawks tab** has Outstanding / Resolved / All filters, with the list and the chosen
//!   squawk side by side on wide screens; on phones the list, then one squawk (option B).
//! - **Opening one is a quick sheet** (option B): what's wrong and the severity, with the notes
//!   and when it was noticed folded away. **+ Log**, the vehicle's page, and an event's page
//!   ("found on" it) open it; a squawk's page and the tab's detail change it.
//! - **A squawk's page** (`/squawks/:id`): its severity and status, who opened it and when, the
//!   event it was found on, its notes, and its history from jobs.
//!
//! Every word for squawks comes from the household's choice (#064, through `tr`).

use dioxus::prelude::*;
use domain::access::{ActiveMode, AssetAction, Role};
use domain::api::{
    AssetView, AssetsOutcome, EventsOutcome, SquawkInput, SquawkView, SquawksOutcome,
};
use domain::asset::{Asset, AssetId};
use domain::dates::{format_date, local_to_utc, utc_to_local};
use domain::event::EventId;
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::location::TimeZone;
use domain::log_area;
use domain::notes::Notes;
use domain::preferences::Preferences;
use domain::profile::high_severity_name;
use domain::squawk::{Disposition, Severity, Squawk, SquawkId, SquawkStatus, SquawkText};
use domain::time::{Clock, UnixMillis};

use super::log_form::ZONE_SCRIPT;
use super::problem_from_error;
use super::vehicle::Tab;
use crate::api;
use crate::logging::SystemClock;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::routes::Route;
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};

// --- The sheet ---------------------------------------------------------------------------

/// What the sheet is open for.
#[derive(Debug, Clone, PartialEq)]
struct SheetTarget {
    /// Bumped on every opening, so the form starts afresh.
    serial: u32,
    asset: AssetId,
    vehicle: String,
    /// What High means for this vehicle's kind ("Not drivable").
    high_word: String,
    /// The event it's reported from, if any.
    found_on: Option<EventId>,
    /// The squawk being changed, if it isn't a new one.
    editing: Option<(SquawkId, Squawk)>,
}

/// The squawk sheet's state, in context (provided by the shell).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SquawkSheet {
    target: Signal<Option<SheetTarget>>,
    /// Bumped whenever a squawk is opened, changed, or removed, so lists load again.
    changes: Signal<u32>,
}

impl SquawkSheet {
    /// Opens the sheet for a new squawk on `asset` (`id`), found on `found_on` if given.
    pub(crate) fn open_new(mut self, id: AssetId, asset: &Asset, found_on: Option<EventId>) {
        let serial = self.next_serial();
        self.target.set(Some(SheetTarget {
            serial,
            asset: id,
            vehicle: asset.name.as_str().to_owned(),
            high_word: high_severity_name(&asset.kind, Translator::english()),
            found_on,
            editing: None,
        }));
    }

    /// Opens the sheet to change `squawk` (`id`) on `asset`.
    fn edit(mut self, id: SquawkId, squawk: Squawk, asset: &Asset) {
        let serial = self.next_serial();
        self.target.set(Some(SheetTarget {
            serial,
            asset: squawk.asset,
            vehicle: asset.name.as_str().to_owned(),
            high_word: high_severity_name(&asset.kind, Translator::english()),
            found_on: squawk.found_on,
            editing: Some((id, squawk)),
        }));
    }

    fn next_serial(self) -> u32 {
        self.target
            .peek()
            .as_ref()
            .map_or(0, |target| target.serial.wrapping_add(1))
    }

    fn close(mut self) {
        self.target.set(None);
    }

    /// Tells the lists something changed.
    fn changed(mut self) {
        self.changes += 1;
    }
}

/// Puts a [`SquawkSheet`] in context; call once, in the shell.
pub(crate) fn provide_squawk_sheet() -> SquawkSheet {
    use_context_provider(|| SquawkSheet {
        target: Signal::new(None),
        changes: Signal::new(0),
    })
}

/// The [`SquawkSheet`], from context.
pub(crate) fn use_squawk_sheet() -> SquawkSheet {
    use_context::<SquawkSheet>()
}

/// Whether someone with `role` on a vehicle may open squawks on it in `mode` (a Driver or
/// higher).
pub(crate) fn may_open_squawks(role: Role, mode: ActiveMode) -> bool {
    role.min(mode.cap()) >= AssetAction::OpenSquawk.required_role()
}

/// Whether someone with `role` on a vehicle may log maintenance and repairs on it in `mode` (a
/// Maintainer or higher, in that mode): **Repair this**.
fn may_log_jobs(role: Role, mode: ActiveMode) -> bool {
    role.min(mode.cap()) >= AssetAction::LogMaintenance.required_role()
}

/// The person's mode (Driver until signed in).
fn current_mode(auth: AuthState) -> ActiveMode {
    match auth {
        AuthState::SignedIn(session) => session.mode,
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::Unreachable(_) => ActiveMode::Driver,
    }
}

/// The sheet, while it's open (rendered by the shell).
#[component]
pub(crate) fn SquawkSheetDialog() -> Element {
    let sheet = use_squawk_sheet();
    let Some(target) = (sheet.target)() else {
        return rsx! {};
    };
    rsx! { SheetForm { key: "{target.serial}", target } }
}

/// The browser's time zone (UTC until it's known).
pub(super) fn use_device_zone() -> Signal<TimeZone> {
    let mut zone = use_signal(TimeZone::utc);
    use_effect(move || {
        spawn(async move {
            if let Ok(found) = document::eval(ZONE_SCRIPT).join::<String>().await
                && let Ok(found) = TimeZone::parse(&found)
                && domain::dates::is_known_zone(&found)
            {
                zone.set(found);
            }
        });
    });
    zone
}

/// What the sheet's fields hold.
#[derive(Debug, Clone, PartialEq)]
struct Typed {
    description: String,
    severity: Severity,
    notes: String,
    /// When it was noticed, as typed (date, time); `None`: now (or, when changing one, as it
    /// was).
    when: Option<(String, String)>,
}

/// The input to send, or the problem with what's typed.
fn squawk_input(
    typed: &Typed,
    target: &SheetTarget,
    zone: &TimeZone,
    now: UnixMillis,
) -> Result<SquawkInput, MessageId> {
    let description =
        SquawkText::parse(&typed.description).map_err(domain::squawk::SquawkProblem::message)?;
    let notes = Notes::parse(&typed.notes).map_err(domain::notes::NotesError::message)?;
    let opened_at = match (&typed.when, &target.editing) {
        (Some((date, time)), _) => {
            local_to_utc(date, time, zone).map_err(domain::dates::DateError::message)?
        }
        (None, Some((_, squawk))) => squawk.opened_at,
        (None, None) => now,
    };
    Ok(SquawkInput {
        asset: target.asset,
        description,
        notes,
        severity: typed.severity,
        opened_at,
        found_on: target.found_on,
    })
}

/// A refusal's message.
fn refusal(outcome: &SquawksOutcome) -> MessageId {
    match outcome {
        SquawksOutcome::Invalid { problem } => problem.message(),
        SquawksOutcome::NotFound => messages::SQUAWK_NOT_FOUND,
        SquawksOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        SquawksOutcome::Squawks { .. }
        | SquawksOutcome::Squawk { .. }
        | SquawksOutcome::Saved { .. }
        | SquawksOutcome::Done
        | SquawksOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// The sheet's form: what's wrong, the severity, and (folded away) notes and when.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SheetForm(target: SheetTarget) -> Element {
    let sheet = use_squawk_sheet();
    let zone = use_device_zone();
    let start = target.editing.as_ref().map(|(_, squawk)| squawk.clone());
    let mut typed = use_signal(|| Typed {
        description: start
            .as_ref()
            .map(|squawk| squawk.description.as_str().to_owned())
            .unwrap_or_default(),
        severity: start
            .as_ref()
            .map(|squawk| squawk.severity)
            .unwrap_or_default(),
        notes: start
            .as_ref()
            .map(|squawk| squawk.notes.as_str().to_owned())
            .unwrap_or_default(),
        when: None,
    });
    let mut more = use_signal(|| {
        start
            .as_ref()
            .is_some_and(|squawk| !squawk.notes.as_str().is_empty())
    });
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let title = if target.editing.is_some() {
        tr(messages::SQUAWK_SHEET_EDIT_TITLE)
    } else {
        tr_with(
            messages::SQUAWK_SHEET_TITLE,
            &[("vehicle", MessageArg::from(target.vehicle.as_str()))],
        )
    };
    let save_target = target.clone();
    let save = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        let input = match squawk_input(&typed(), &save_target, &zone(), SystemClock.now()) {
            Ok(input) => input,
            Err(message) => {
                problem.set(Some(message));
                return;
            }
        };
        let editing = save_target.editing.as_ref().map(|(id, _)| *id);
        busy.set(true);
        spawn(async move {
            let result = match editing {
                Some(id) => api::edit_squawk(id, input).await,
                None => api::open_squawk(input).await,
            };
            busy.set(false);
            match result {
                Ok(SquawksOutcome::Saved { id }) => {
                    tracing::debug!(target: log_area::UI, squawk = %id, "squawk saved");
                    sheet.changed();
                    sheet.close();
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let severity = typed().severity;
    let when_shown = typed().when.or_else(|| {
        let at = start
            .as_ref()
            .map_or_else(|| SystemClock.now(), |squawk| squawk.opened_at);
        utc_to_local(at, &zone()).ok()
    });
    let (shown_date, shown_time) = when_shown.unwrap_or_default();
    let (date_now, time_now) = (shown_date.clone(), shown_time.clone());
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| sheet.close(),
            form {
                class: "dialog squawk-sheet",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                onsubmit: save,
                h2 { class: "dialog-title", {title} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SQUAWK_DESCRIPTION_LABEL)} }
                    input {
                        class: "field-input",
                        value: typed().description,
                        maxlength: "500",
                        oninput: move |event| typed.write().description = event.value(),
                    }
                }
                div { class: "field",
                    span { class: "field-label", {tr(messages::SQUAWK_SEVERITY_LABEL)} }
                    div { class: "choice-buttons severity-buttons", role: "group",
                        for each in Severity::ALL {
                            button {
                                key: "{each:?}",
                                r#type: "button",
                                class: if each == severity { format!("choice on {}", severity_class(each)) } else { "choice".to_owned() },
                                "aria-pressed": each == severity,
                                onclick: move |_| typed.write().severity = each,
                                {tr(each.label())}
                            }
                        }
                    }
                }
                if severity == Severity::High {
                    p { class: "squawk-high-note",
                        {tr_with(messages::SQUAWK_HIGH_NOTE, &[("word", MessageArg::from(target.high_word.as_str()))])}
                    }
                }
                if more() {
                    label { class: "field",
                        span { class: "field-label", {tr(messages::SQUAWK_NOTES_LABEL)} }
                        textarea {
                            class: "field-input",
                            rows: "3",
                            value: typed().notes,
                            oninput: move |event| typed.write().notes = event.value(),
                        }
                    }
                    div { class: "field-row",
                        label { class: "field",
                            span { class: "field-label", {tr(messages::SQUAWK_NOTICED_DATE)} }
                            input {
                                class: "field-input",
                                r#type: "date",
                                value: shown_date,
                                oninput: move |event: FormEvent| typed.write().when = Some((event.value(), time_now.clone())),
                            }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::LOG_TIME)} }
                            input {
                                class: "field-input",
                                r#type: "time",
                                value: shown_time,
                                oninput: move |event: FormEvent| typed.write().when = Some((date_now.clone(), event.value())),
                            }
                        }
                    }
                } else {
                    button { class: "link-button squawk-more", r#type: "button", onclick: move |_| more.set(true),
                        {format!("▸ {}", tr(messages::SQUAWK_MORE))}
                    }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| sheet.close(), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::ACTION_SAVE)} }
                }
            }
        }
    }
}

// --- Chips -------------------------------------------------------------------------------

/// The CSS class for a severity's colours.
const fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Low => "severity-low",
        Severity::Medium => "severity-medium",
        Severity::High => "severity-high",
    }
}

/// "HIGH · Not drivable", "MEDIUM", "LOW".
fn severity_text(severity: Severity, high_word: &str) -> String {
    let name = tr(severity.label()).to_uppercase();
    if severity == Severity::High {
        format!("{name} · {high_word}")
    } else {
        name
    }
}

#[component]
pub(super) fn SeverityChip(severity: Severity, high_word: String) -> Element {
    rsx! {
        span { class: "severity-chip {severity_class(severity)}", {severity_text(severity, &high_word)} }
    }
}

const fn status_class(status: SquawkStatus) -> &'static str {
    match status {
        SquawkStatus::Open => "squawk-status open",
        SquawkStatus::Deferred => "squawk-status deferred",
        SquawkStatus::Resolved => "squawk-status resolved",
    }
}

#[component]
pub(super) fn StatusChip(status: SquawkStatus) -> Element {
    rsx! { span { class: status_class(status), {tr(status.label())} } }
}

/// A date in the person's format, in `zone`.
fn date_text(at: UnixMillis, zone: &TimeZone, units: &Preferences) -> String {
    format_date(at, zone, units.date_format, Translator::english()).unwrap_or_default()
}

/// The outstanding ones, High first, then the newest.
fn outstanding_first(squawks: &[SquawkView]) -> Vec<SquawkView> {
    let mut outstanding: Vec<SquawkView> = squawks
        .iter()
        .filter(|view| view.status.is_outstanding())
        .cloned()
        .collect();
    outstanding.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(b.squawk.opened_at.cmp(&a.squawk.opened_at))
    });
    outstanding
}

// --- A vehicle's squawks -----------------------------------------------------------------

/// One vehicle's squawks, and whether the person may open one.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct VehicleSquawks {
    pub squawks: Vec<SquawkView>,
    pub may_open: bool,
}

impl VehicleSquawks {
    /// How many are outstanding.
    pub(super) fn outstanding(&self) -> usize {
        self.squawks
            .iter()
            .filter(|view| view.status.is_outstanding())
            .count()
    }
}

/// Loads `asset`'s squawks, again after any change from the sheet.
pub(super) fn use_vehicle_squawks(asset: AssetId) -> Signal<Option<VehicleSquawks>> {
    let sheet = use_squawk_sheet();
    let mut loaded = use_signal(|| None::<VehicleSquawks>);
    use_effect(use_reactive!(|asset| {
        let _ = (sheet.changes)();
        spawn(async move {
            match api::vehicle_squawks(asset).await {
                Ok(SquawksOutcome::Squawks { squawks, may_open }) => {
                    tracing::debug!(target: log_area::UI, %asset, count = squawks.len(), "squawks loaded");
                    loaded.set(Some(VehicleSquawks { squawks, may_open }));
                }
                Ok(other) => {
                    tracing::debug!(target: log_area::UI, ?other, "squawks: unexpected answer");
                    loaded.set(Some(VehicleSquawks {
                        squawks: Vec::new(),
                        may_open: false,
                    }));
                }
                Err(error) => {
                    tracing::debug!(target: log_area::UI, %error, "squawks failed");
                }
            }
        });
    }));
    loaded
}

/// One squawk's row: its severity and status, what's wrong, and when.
#[component]
fn SquawkRow(view: SquawkView, high_word: String, zone: TimeZone, units: Preferences) -> Element {
    rsx! {
        span { class: "squawk-row-top",
            SeverityChip { severity: view.severity, high_word }
            StatusChip { status: view.status }
        }
        span { class: "squawk-row-text", {view.squawk.description.as_str().to_owned()} }
        span { class: "squawk-row-meta",
            {format!("{} · {}", view.opened_by_name, date_text(view.squawk.opened_at, &zone, &units))}
        }
    }
}

/// The Overview's card: the outstanding squawks, High first (option A's panel).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn SquawksCard(
    vehicle: AssetView,
    squawks: Option<VehicleSquawks>,
    on_all: EventHandler<()>,
) -> Element {
    let sheet = use_squawk_sheet();
    let units = use_units();
    let zone = use_device_zone();
    let high_word = high_severity_name(&vehicle.asset.kind, Translator::english());
    let outstanding = squawks
        .as_ref()
        .map(|loaded| outstanding_first(&loaded.squawks))
        .unwrap_or_default();
    let may_open = squawks.as_ref().is_some_and(|loaded| loaded.may_open);
    let asset = vehicle.asset.clone();
    rsx! {
        section { class: "vehicle-panel wide squawks-card",
            div { class: "panel-heading-row",
                h3 { class: "section-heading", {tr(messages::PANEL_SQUAWKS).to_uppercase()} }
                if may_open {
                    button {
                        class: "link-button",
                        r#type: "button",
                        onclick: move |_| sheet.open_new(vehicle.id, &asset, None),
                        {tr(messages::SQUAWK_NEW)}
                    }
                }
            }
            if squawks.is_none() {
                p { class: "muted-note", {tr(messages::LOADING)} }
            } else if outstanding.is_empty() {
                p { class: "muted-note", {tr(messages::SQUAWKS_NONE_OUTSTANDING)} }
            }
            for view in outstanding {
                Link {
                    key: "{view.id}",
                    class: "squawk-row",
                    to: Route::SquawkDetail { squawk_id: view.id.to_string() },
                    SquawkRow { view, high_word: high_word.clone(), zone: zone(), units: units() }
                }
            }
            button { class: "link-button", r#type: "button", onclick: move |_| on_all.call(()),
                {tr(messages::SQUAWKS_ALL)}
            }
        }
    }
}

/// The Squawks tab's filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filter {
    Outstanding,
    Resolved,
    All,
}

impl Filter {
    const ALL: [Self; 3] = [Self::Outstanding, Self::Resolved, Self::All];

    fn shows(self, status: SquawkStatus) -> bool {
        match self {
            Self::Outstanding => status.is_outstanding(),
            Self::Resolved => !status.is_outstanding(),
            Self::All => true,
        }
    }

    fn label(self, squawks: &[SquawkView]) -> String {
        let count = |filter: Self| {
            let shown = squawks
                .iter()
                .filter(|view| filter.shows(view.status))
                .count();
            MessageArg::from(i64::try_from(shown).unwrap_or(i64::MAX))
        };
        match self {
            Self::Outstanding => tr_with(
                messages::SQUAWKS_FILTER_OUTSTANDING,
                &[("count", count(self))],
            ),
            Self::Resolved => tr_with(messages::SQUAWKS_FILTER_RESOLVED, &[("count", count(self))]),
            Self::All => tr(messages::SQUAWKS_FILTER_ALL),
        }
    }
}

/// The Squawks tab: filters, the list, and the chosen squawk beside it (option B).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn SquawksTab(vehicle: AssetView, squawks: Option<VehicleSquawks>) -> Element {
    let sheet = use_squawk_sheet();
    let units = use_units();
    let zone = use_device_zone();
    let mut filter = use_signal(|| Filter::Outstanding);
    let mut selected = use_signal(|| None::<SquawkId>);
    let Some(loaded) = squawks else {
        return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } };
    };
    let high_word = high_severity_name(&vehicle.asset.kind, Translator::english());
    let mut shown: Vec<SquawkView> = if filter() == Filter::Outstanding {
        outstanding_first(&loaded.squawks)
    } else {
        loaded
            .squawks
            .iter()
            .filter(|view| filter().shows(view.status))
            .cloned()
            .collect()
    };
    if filter() == Filter::All {
        // Outstanding first (High first), then the rest, newest first.
        let mut rest: Vec<SquawkView> = shown
            .iter()
            .filter(|view| !view.status.is_outstanding())
            .cloned()
            .collect();
        shown = outstanding_first(&loaded.squawks);
        shown.append(&mut rest);
    }
    let chosen =
        selected().and_then(|id| loaded.squawks.iter().find(|view| view.id == id).cloned());
    let asset = vehicle.asset.clone();
    let layout = if chosen.is_some() {
        "squawks has-selection"
    } else {
        "squawks"
    };
    rsx! {
        div { class: layout,
            div { class: "squawks-list",
                div { class: "squawks-toolbar",
                    div { class: "filter-chips", role: "group",
                        for each in Filter::ALL {
                            button {
                                key: "{each:?}",
                                class: if filter() == each { "chip-filter on" } else { "chip-filter" },
                                "aria-pressed": filter() == each,
                                onclick: move |_| filter.set(each),
                                {each.label(&loaded.squawks)}
                            }
                        }
                    }
                    if loaded.may_open {
                        button {
                            class: "link-button",
                            r#type: "button",
                            onclick: move |_| sheet.open_new(vehicle.id, &asset, None),
                            {tr(messages::SQUAWK_NEW)}
                        }
                    }
                }
                if shown.is_empty() {
                    p { class: "muted-note", {tr(messages::SQUAWKS_NONE_HERE)} }
                }
                for view in shown {
                    button {
                        key: "{view.id}",
                        class: if chosen.as_ref().is_some_and(|open| open.id == view.id) { "squawk-row selected" } else { "squawk-row" },
                        r#type: "button",
                        onclick: move |_| selected.set(Some(view.id)),
                        SquawkRow { view: view.clone(), high_word: high_word.clone(), zone: zone(), units: units() }
                    }
                }
            }
            div { class: "squawks-detail",
                if let Some(view) = chosen {
                    div { key: "{view.id}", class: "squawk-detail-inner",
                        button { class: "link-button squawks-back", r#type: "button", onclick: move |_| selected.set(None),
                            {format!("‹ {}", tr(messages::TAB_SQUAWKS))}
                        }
                        SquawkBody { view, vehicle, on_removed: move |()| selected.set(None) }
                    }
                } else {
                    p { class: "placeholder", {tr(messages::SQUAWKS_SELECT_HINT)} }
                }
            }
        }
    }
}

// --- One squawk --------------------------------------------------------------------------

/// "Fuel-up · 3 Nov ›": the event a squawk was found on, linked.
#[component]
fn FoundOn(event: EventId, zone: TimeZone, units: Preferences) -> Element {
    let mut label = use_signal(|| None::<String>);
    use_effect(use_reactive!(|event, zone, units| {
        spawn(async move {
            if let Ok(EventsOutcome::Event { view }) = api::event_view(event).await {
                let event = &view.event;
                label.set(Some(format!(
                    "{} · {}",
                    tr(event.kind.label()),
                    date_text(event.dates.start, &zone, &units)
                )));
            }
        });
    }));
    let text = label().unwrap_or_else(|| tr(messages::SQUAWK_FOUND_ON_EVENT));
    rsx! {
        Link { class: "squawk-found-on", to: Route::EventDetail { event_id: event.to_string() },
            {tr_with(messages::SQUAWK_FOUND_ON, &[("event", MessageArg::from(text))])}
        }
    }
}

/// One entry of a squawk's history, newest first: a job's disposition, or its opening.
#[derive(Debug, Clone, PartialEq)]
struct HistoryEntry {
    at: UnixMillis,
    /// The job, or `None` for the opening.
    event: Option<EventId>,
    what: String,
    /// "Low → Medium", if the severity changed.
    change: Option<String>,
}

/// A squawk's history as shown: its opening, then each counted disposition with the severity
/// it changed (from the one before), newest first.
fn history(view: &SquawkView) -> Vec<HistoryEntry> {
    let mut entries = vec![HistoryEntry {
        at: view.squawk.opened_at,
        event: None,
        what: tr_with(
            messages::SQUAWK_HISTORY_OPENED,
            &[(
                "severity",
                MessageArg::from(tr(view.squawk.severity.label())),
            )],
        ),
        change: None,
    }];
    let mut counted = view.history.clone();
    counted.sort_by_key(|entry| (entry.at, entry.event));
    let mut severity = view.squawk.severity;
    for entry in counted {
        let what = match &entry.disposition.action {
            Disposition::SignedOff { notes, .. } if notes.as_str().is_empty() => {
                tr(messages::SQUAWK_HISTORY_SIGNED_OFF)
            }
            Disposition::SignedOff { notes, .. } => format!(
                "{}: {}",
                tr(messages::SQUAWK_HISTORY_SIGNED_OFF),
                notes.as_str()
            ),
            Disposition::Deferred { reason } => tr_with(
                messages::SQUAWK_HISTORY_DEFERRED,
                &[("reason", MessageArg::from(reason.as_str()))],
            ),
        };
        let change = entry
            .disposition
            .severity
            .filter(|next| *next != severity)
            .map(|next| {
                let text = format!("{} → {}", tr(severity.label()), tr(next.label()));
                severity = next;
                text
            });
        entries.push(HistoryEntry {
            at: entry.at,
            event: Some(entry.event),
            what,
            change,
        });
    }
    entries.reverse();
    entries
}

/// A squawk: its chips, what's wrong, who opened it, notes, history, and its actions.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SquawkBody(view: SquawkView, vehicle: AssetView, on_removed: EventHandler<()>) -> Element {
    let sheet = use_squawk_sheet();
    let auth = use_auth();
    let units = use_units();
    let zone = use_device_zone();
    let mut confirm = use_signal(|| false);
    let high_word = high_severity_name(&vehicle.asset.kind, Translator::english());
    let squawk = view.squawk.clone();
    let entries = history(&view);
    let asset = vehicle.asset;
    rsx! {
        div { class: "squawk-body",
            div { class: "chip-row",
                SeverityChip { severity: view.severity, high_word }
                StatusChip { status: view.status }
            }
            h2 { class: "squawk-title", {squawk.description.as_str().to_owned()} }
            p { class: "muted-note",
                {tr_with(messages::SQUAWK_OPENED_BY, &[
                    ("name", MessageArg::from(view.opened_by_name.as_str())),
                    ("date", MessageArg::from(date_text(squawk.opened_at, &zone(), &units()))),
                ])}
            }
            if let Some(event) = squawk.found_on {
                FoundOn { event, zone: zone(), units: units() }
            }
            if !squawk.notes.as_str().is_empty() {
                h3 { class: "section-heading", {tr(messages::SQUAWK_NOTES_HEADING).to_uppercase()} }
                p { class: "squawk-notes", {squawk.notes.as_str().to_owned()} }
            }
            h3 { class: "section-heading", {tr(messages::SQUAWK_HISTORY_HEADING).to_uppercase()} }
            ol { class: "squawk-history",
                for (index, entry) in entries.into_iter().enumerate() {
                    li { key: "{index}", class: "squawk-history-entry",
                        match entry.event {
                            Some(event) => rsx! {
                                Link { class: "squawk-history-job", to: Route::EventDetail { event_id: event.to_string() },
                                    {tr_with(messages::SQUAWK_HISTORY_JOB, &[("date", MessageArg::from(date_text(entry.at, &zone(), &units())))])}
                                }
                            },
                            None => rsx! {
                                span { class: "squawk-history-job", {date_text(entry.at, &zone(), &units())} }
                            },
                        }
                        span { {entry.what} }
                        if let Some(change) = entry.change {
                            span { class: "squawk-history-change", {change} }
                        }
                    }
                }
            }
            if view.status.is_outstanding() && may_log_jobs(vehicle.role, current_mode(auth())) {
                Link { class: "primary-button squawk-repair", to: Route::RepairSquawk { squawk_id: view.id.to_string() },
                    {tr(messages::SQUAWK_REPAIR_THIS)}
                }
            }
            if view.may_change {
                div { class: "check-row",
                    button {
                        class: "secondary-button",
                        r#type: "button",
                        onclick: move |_| sheet.edit(view.id, squawk.clone(), &asset),
                        {tr(messages::ACTION_EDIT)}
                    }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| confirm.set(true), {tr(messages::SQUAWK_REMOVE)} }
                }
            } else if !view.history.is_empty() {
                p { class: "muted-note squawk-locked", {tr(messages::SQUAWK_LOCKED_NOTE)} }
            }
        }
        if confirm() {
            RemoveDialog { id: view.id, on_close: move |removed: bool| {
                confirm.set(false);
                if removed {
                    on_removed.call(());
                }
            } }
        }
    }
}

/// "Remove this squawk?": only for one opened by mistake.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RemoveDialog(id: SquawkId, on_close: EventHandler<bool>) -> Element {
    let sheet = use_squawk_sheet();
    let mut problem = use_signal(|| None::<MessageId>);
    let remove = move |_| {
        spawn(async move {
            match api::delete_squawk(id).await {
                Ok(SquawksOutcome::Done) => {
                    tracing::debug!(target: log_area::UI, squawk = %id, "squawk removed");
                    sheet.changed();
                    on_close.call(true);
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(false),
            div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::SQUAWK_REMOVE_TITLE)} }
                p { class: "dialog-text", {tr(messages::SQUAWK_REMOVE_TEXT)} }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(false), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button", r#type: "button", onclick: remove, {tr(messages::SQUAWK_REMOVE)} }
                }
            }
        }
    }
}

/// What a squawk's page shows.
#[derive(Debug, Clone, PartialEq)]
struct Loaded {
    view: SquawkView,
    vehicle: AssetView,
}

async fn load(id: SquawkId) -> Result<Loaded, MessageId> {
    let failed = |error: &ServerFnError| problem_from_error(error).message();
    let view = match api::squawk_view(id).await.map_err(|e| failed(&e))? {
        SquawksOutcome::Squawk { view } => *view,
        other @ (SquawksOutcome::Squawks { .. }
        | SquawksOutcome::Saved { .. }
        | SquawksOutcome::Done
        | SquawksOutcome::NotFound
        | SquawksOutcome::NotAllowed
        | SquawksOutcome::Invalid { .. }
        | SquawksOutcome::Unavailable) => return Err(refusal(&other)),
    };
    let AssetsOutcome::Asset { view: vehicle } = api::vehicle_page(view.squawk.asset)
        .await
        .map_err(|e| failed(&e))?
    else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    Ok(Loaded {
        view,
        vehicle: *vehicle,
    })
}

/// `/squawks/:squawk_id`: one squawk's page, with links back to its vehicle and the tab.
#[component]
pub(crate) fn SquawkDetail(squawk_id: String) -> Element {
    match squawk_id.parse::<SquawkId>() {
        Ok(id) => rsx! { SquawkScreen { id } },
        Err(_) => rsx! { p { class: "placeholder", {tr(messages::SQUAWK_NOT_FOUND)} } },
    }
}

#[component]
fn SquawkScreen(id: SquawkId) -> Element {
    let sheet = use_squawk_sheet();
    let navigator = use_navigator();
    let mut loaded = use_signal(|| None::<Result<Loaded, MessageId>>);
    use_effect(use_reactive!(|id| {
        let _ = (sheet.changes)();
        spawn(async move {
            loaded.set(Some(load(id).await));
        });
    }));
    match loaded() {
        None => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
        Some(Ok(Loaded { view, vehicle })) => {
            let asset_id = vehicle.id.to_string();
            let back = Route::VehicleTab {
                asset_id: asset_id.clone(),
                tab: Tab::Squawks.slug().to_owned(),
            };
            rsx! {
                div { class: "squawk-page",
                    div { class: "back-links",
                        Link { class: "vehicle-back", to: Route::VehicleDetail { asset_id }, {format!("‹ {}", vehicle.asset.name.as_str())} }
                        Link { class: "vehicle-back", to: back.clone(), {format!("‹ {}", tr(Tab::Squawks.label()))} }
                    }
                    SquawkBody { view, vehicle, on_removed: move |()| { navigator.push(back.clone()); } }
                }
            }
        }
    }
}

/// "Report an issue" on an event's page: opens the sheet, found on that event.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn ReportSquawk(vehicle: AssetView, event: EventId) -> Element {
    let sheet = use_squawk_sheet();
    let auth = use_auth();
    if !may_open_squawks(vehicle.role, current_mode(auth())) {
        return rsx! {};
    }
    rsx! {
        button {
            class: "secondary-button",
            r#type: "button",
            onclick: move |_| sheet.open_new(vehicle.id, &vehicle.asset, Some(event)),
            {tr(messages::SQUAWK_REPORT)}
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::squawk::{CountedDisposition, SquawkDisposition};
    use domain::test_support::test_id;

    use super::*;

    fn view(severity: Severity, status: SquawkStatus, opened: i64) -> SquawkView {
        SquawkView {
            id: test_id(u32::try_from(opened).unwrap()),
            squawk: Squawk {
                asset: test_id(10),
                description: SquawkText::parse("Rattle").unwrap(),
                notes: Notes::default(),
                severity: Severity::Low,
                opened_at: UnixMillis::from_millis(opened),
                opened_by: test_id(1),
                found_on: None,
                received_at: UnixMillis::from_millis(opened),
            },
            status,
            severity,
            history: vec![],
            opened_by_name: "Sam".into(),
            may_change: true,
        }
    }

    #[test]
    fn outstanding_ones_come_high_first_then_newest() {
        let squawks = [
            view(Severity::Low, SquawkStatus::Open, 3),
            view(Severity::High, SquawkStatus::Deferred, 1),
            view(Severity::Low, SquawkStatus::Open, 5),
            view(Severity::High, SquawkStatus::Resolved, 9),
        ];
        let order: Vec<i64> = outstanding_first(&squawks)
            .iter()
            .map(|view| view.squawk.opened_at.as_millis())
            .collect();
        assert_eq!(order, vec![1, 5, 3]);
    }

    #[test]
    fn the_history_is_newest_first_with_each_severity_change() {
        let mut squawk = view(Severity::Medium, SquawkStatus::Resolved, 1);
        let job = |at: i64, action: Disposition, severity: Option<Severity>| CountedDisposition {
            at: UnixMillis::from_millis(at),
            event: test_id(u32::try_from(at).unwrap()),
            disposition: SquawkDisposition {
                squawk: squawk.id,
                action,
                severity,
            },
        };
        squawk.history = vec![
            job(
                30,
                Disposition::SignedOff {
                    notes: Notes::default(),
                    task: None,
                },
                None,
            ),
            job(
                20,
                Disposition::Deferred {
                    reason: domain::event::Reason::parse("Parts on order").unwrap(),
                },
                Some(Severity::Medium),
            ),
        ];
        let entries = history(&squawk);
        let shown: Vec<(Option<EventId>, &str, Option<&str>)> = entries
            .iter()
            .map(|entry| (entry.event, entry.what.as_str(), entry.change.as_deref()))
            .collect();
        assert_eq!(
            shown,
            vec![
                (Some(test_id(30)), "Signed off", None),
                (
                    Some(test_id(20)),
                    "Deferred: Parts on order",
                    Some("Low → Medium")
                ),
                (None, "Opened · Low", None),
            ]
        );
    }

    #[test]
    fn drivers_and_up_may_open_squawks() {
        assert!(may_open_squawks(Role::Driver, ActiveMode::Driver));
        assert!(may_open_squawks(Role::Admin, ActiveMode::Driver));
        assert!(!may_open_squawks(Role::Viewer, ActiveMode::Admin));
    }

    #[test]
    fn a_squawk_typed_into_the_sheet_becomes_its_input() {
        let target = SheetTarget {
            serial: 0,
            asset: test_id(10),
            vehicle: "Truck".into(),
            high_word: "Not drivable".into(),
            found_on: Some(test_id(100)),
            editing: None,
        };
        let utc = TimeZone::parse("UTC").unwrap();
        let now = UnixMillis::from_millis(1_700_000_000_000);
        let typed = Typed {
            description: "  Brake light out ".into(),
            severity: Severity::High,
            notes: String::new(),
            when: None,
        };
        let input = squawk_input(&typed, &target, &utc, now).unwrap();
        assert_eq!(input.description.as_str(), "Brake light out");
        assert_eq!((input.opened_at, input.found_on), (now, Some(test_id(100))));
        let noticed = Typed {
            when: Some(("2026-11-01".into(), "09:30".into())),
            ..typed.clone()
        };
        assert_eq!(
            squawk_input(&noticed, &target, &utc, now)
                .unwrap()
                .opened_at,
            local_to_utc("2026-11-01", "09:30", &utc).unwrap()
        );
        let empty = Typed {
            description: " ".into(),
            ..typed
        };
        assert_eq!(
            squawk_input(&empty, &target, &utc, now),
            Err(messages::SQUAWK_DESCRIPTION_EMPTY)
        );
    }
}
