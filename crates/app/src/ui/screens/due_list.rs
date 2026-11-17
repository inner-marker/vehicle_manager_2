//! Due & squawks, and the Garage's fleet strip (Phase 5 milestone 3; decision #083; sketch
//! `docs/ui-sketches/p5m3-messages-options.svg`, 2A and 3A).
//!
//! **Due & squawks** (3A) is one list across the vehicles the Garage shows the person: what
//! comes due (Coming up or more urgent), most urgent first, each with its vehicle, what's left,
//! and its chip; then the open and deferred squawks, the most severe first. Filter chips:
//! All, Overdue, Due soon (Due soon and Due very soon), and the squawks.
//!
//! **The fleet strip** (2A) sits above the groups on All vehicles: four tiles, **⚠ Overdue**,
//! **Due soon**, **Open squawks**, each opening Due & squawks with its filter, and on the
//! desktop app **Synced** (or how many changes wait to be sent), opening Messages.

use dioxus::prelude::*;
use domain::api::{DueSubject, FleetDue, FleetOutcome, FleetView};
use domain::due::{DueStep, FleetCounts};
use domain::i18n::{MessageArg, MessageId, messages};

use super::documents::type_name;
use super::due_view::{DueChip, left_text};
use crate::data;
use crate::ui::outbox::SyncTile;
use crate::ui::routes::Route;
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};

/// Which part of Due & squawks shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DueFilter {
    /// Everything.
    All,
    /// Overdue items.
    Overdue,
    /// Items due soon or very soon.
    DueSoon,
    /// Open and deferred squawks.
    Squawks,
}

impl DueFilter {
    /// Every filter, in order, with its chip's label.
    const ALL: [(Self, MessageId); 4] = [
        (Self::All, messages::DUE_FILTER_ALL),
        (Self::Overdue, messages::DUE_FILTER_OVERDUE),
        (Self::DueSoon, messages::DUE_FILTER_SOON),
        (Self::Squawks, messages::DUE_FILTER_SQUAWKS),
    ];

    /// Its name in the address (`/due/overdue`).
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Overdue => "overdue",
            Self::DueSoon => "due-soon",
            Self::Squawks => "squawks",
        }
    }

    /// The filter named `name`; anything else is All.
    pub(crate) fn from_name(name: &str) -> Self {
        Self::ALL
            .into_iter()
            .map(|(filter, _)| filter)
            .find(|filter| filter.name() == name)
            .unwrap_or(Self::All)
    }

    /// Whether an item at `step` shows.
    const fn shows(self, step: DueStep) -> bool {
        match self {
            Self::All => true,
            Self::Overdue => matches!(step, DueStep::Overdue),
            Self::DueSoon => matches!(step, DueStep::DueSoon | DueStep::DueVerySoon),
            Self::Squawks => false,
        }
    }

    /// Whether the squawks show.
    const fn shows_squawks(self) -> bool {
        matches!(self, Self::All | Self::Squawks)
    }
}

/// The due items `filter` shows, in the list's order.
pub(super) fn shown_due(view: &FleetView, filter: DueFilter) -> Vec<&FleetDue> {
    view.due
        .iter()
        .filter(|item| filter.shows(item.entry.due.step))
        .collect()
}

/// Due & squawks, everything showing (the sidebar's address).
#[component]
pub(crate) fn Due() -> Element {
    rsx! { DueScreen { filter: DueFilter::All } }
}

/// Due & squawks with one filter (the fleet strip's tiles).
#[component]
pub(crate) fn DueList(filter: String) -> Element {
    let filter = DueFilter::from_name(&filter);
    rsx! { DueScreen { key: "{filter.name()}", filter } }
}

#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DueScreen(filter: DueFilter) -> Element {
    let units = use_units();
    let mut chosen = use_signal(|| filter);
    let loaded = use_resource(move || async move { data::fleet_due().await });
    let view = match &*loaded.read() {
        None => return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Ok(FleetOutcome::Fleet { view })) => view.clone(),
        Some(Ok(FleetOutcome::Unavailable)) => {
            return rsx! { p { class: "form-error", role: "alert", {tr(messages::SOMETHING_WENT_WRONG)} } };
        }
        Some(Err(error)) => {
            return rsx! { p { class: "form-error", role: "alert", {tr(super::problem_from_error(error).message())} } };
        }
    };
    let due: Vec<FleetDue> = shown_due(&view, chosen()).into_iter().cloned().collect();
    let squawks = if chosen().shows_squawks() {
        view.squawks
    } else {
        Vec::new()
    };
    let nothing = due.is_empty() && squawks.is_empty();
    rsx! {
        div { class: "due-list",
            div { class: "filter-chips", role: "group",
                for (filter, label) in DueFilter::ALL {
                    button {
                        key: "{filter.name()}",
                        class: if chosen() == filter { "chip-filter on" } else { "chip-filter" },
                        "aria-pressed": chosen() == filter,
                        onclick: move |_| chosen.set(filter),
                        {tr(label)}
                    }
                }
            }
            if nothing {
                p { class: "placeholder", {tr(messages::DUE_LIST_NOTHING)} }
            }
            ul { class: "due-rows",
                for (index, item) in due.into_iter().enumerate() {
                    li { key: "due-{index}",
                        Link {
                            class: "due-row",
                            to: Route::VehicleTab {
                                asset_id: item.asset.to_string(),
                                tab: match item.entry.subject {
                                    DueSubject::Requirement { .. } => "maintenance".to_owned(),
                                    DueSubject::Document { .. } => "documents".to_owned(),
                                },
                            },
                            span { class: "due-row-text",
                                span { class: "due-row-name", {subject_name(&item.entry.subject)} }
                                span { class: "due-row-meta",
                                    {format!("{} · {}", item.vehicle, left_text(&item.entry.due, &item.meters, &units()))}
                                }
                            }
                            DueChip { step: item.entry.due.step }
                        }
                    }
                }
            }
            if !squawks.is_empty() {
                h3 { class: "section-heading", {tr(messages::DUE_LIST_SQUAWKS)} }
                ul { class: "due-rows",
                    for item in squawks {
                        li { key: "{item.view.id}",
                            Link {
                                class: "due-row",
                                to: Route::SquawkDetail { squawk_id: item.view.id.to_string() },
                                span { class: "due-row-text",
                                    span { class: "due-row-name", {item.view.squawk.description.as_str().to_owned()} }
                                    span { class: "due-row-meta",
                                        {format!("{} · {} · {}", item.vehicle, tr(item.view.severity.label()), tr(item.view.status.label()))}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A due item's name: a requirement's, or a document's type.
fn subject_name(subject: &DueSubject) -> String {
    match subject {
        DueSubject::Requirement { name, .. } => name.as_str().to_owned(),
        DueSubject::Document { doc_type, .. } => type_name(doc_type),
    }
}

/// A count as a Fluent number.
fn count(value: usize) -> MessageArg {
    MessageArg::from(i64::try_from(value).unwrap_or(i64::MAX))
}

/// The fleet strip (2A): four tiles over the vehicles the Garage shows.
#[component]
pub(crate) fn FleetStrip(counts: FleetCounts) -> Element {
    let tiles = [
        (
            DueFilter::Overdue,
            counts.overdue,
            messages::FLEET_OVERDUE,
            "fleet-overdue",
        ),
        (
            DueFilter::DueSoon,
            counts.due_soon,
            messages::FLEET_DUE_SOON,
            "fleet-due-soon",
        ),
        (
            DueFilter::Squawks,
            counts.open_squawks,
            messages::FLEET_SQUAWKS,
            "fleet-squawks",
        ),
    ];
    let sync = crate::ui::outbox::sync_tile();
    rsx! {
        nav { class: "fleet-strip", "aria-label": tr(messages::FLEET_LABEL),
            for (filter, value, label, class) in tiles {
                Link {
                    key: "{filter.name()}",
                    class: "fleet-tile {class}",
                    to: Route::DueList { filter: filter.name().to_owned() },
                    span { class: "fleet-count", {value.to_string()} }
                    span { class: "fleet-label", {tr_with(label, &[("count", count(value))])} }
                }
            }
            if let Some(tile) = sync {
                Link {
                    class: if tile == SyncTile::Waiting(0) { "fleet-tile fleet-synced" } else { "fleet-tile fleet-waiting" },
                    to: Route::Messages {},
                    span { class: "fleet-count", {sync_count(tile)} }
                    span { class: "fleet-label", {sync_label(tile)} }
                }
            }
        }
    }
}

/// The sync tile's number: ✓ when synced, how many wait, or – offline.
fn sync_count(tile: SyncTile) -> String {
    match tile {
        SyncTile::Offline => "–".to_owned(),
        SyncTile::Waiting(0) => "✓".to_owned(),
        SyncTile::Waiting(waiting) => waiting.to_string(),
    }
}

/// The sync tile's words.
fn sync_label(tile: SyncTile) -> String {
    match tile {
        SyncTile::Offline => tr(messages::FLEET_OFFLINE),
        SyncTile::Waiting(0) => tr(messages::FLEET_SYNCED),
        SyncTile::Waiting(waiting) => tr_with(
            messages::FLEET_WAITING,
            &[(
                "count",
                MessageArg::from(i64::try_from(waiting).unwrap_or(i64::MAX)),
            )],
        ),
    }
}

#[cfg(test)]
mod tests {
    use domain::api::DueEntry;
    use domain::due::Due;
    use domain::label::Label;
    use domain::test_support::test_id;

    use super::*;

    fn item(step: DueStep) -> FleetDue {
        FleetDue {
            asset: test_id(1),
            vehicle: "Outback".into(),
            entry: DueEntry {
                subject: DueSubject::Requirement {
                    id: test_id(20),
                    name: Label::parse("Tire rotation").unwrap(),
                },
                due: Due {
                    step,
                    rules: vec![],
                    date: None,
                    not_done_yet: false,
                },
            },
            meters: vec![],
        }
    }

    #[test]
    fn each_filter_shows_its_items() {
        let view = FleetView {
            due: [
                DueStep::Overdue,
                DueStep::DueVerySoon,
                DueStep::DueSoon,
                DueStep::ComingUp,
            ]
            .map(item)
            .to_vec(),
            ..FleetView::default()
        };
        let steps = |filter| -> Vec<DueStep> {
            shown_due(&view, filter)
                .iter()
                .map(|item| item.entry.due.step)
                .collect()
        };
        assert_eq!(steps(DueFilter::All).len(), 4);
        assert_eq!(steps(DueFilter::Overdue), [DueStep::Overdue]);
        assert_eq!(
            steps(DueFilter::DueSoon),
            [DueStep::DueVerySoon, DueStep::DueSoon]
        );
        assert!(steps(DueFilter::Squawks).is_empty());
        assert!(DueFilter::Squawks.shows_squawks() && !DueFilter::Overdue.shows_squawks());
    }

    #[test]
    fn a_filter_round_trips_through_its_name() {
        for (filter, _) in DueFilter::ALL {
            assert_eq!(DueFilter::from_name(filter.name()), filter);
        }
        assert_eq!(DueFilter::from_name("nonsense"), DueFilter::All);
    }
}
