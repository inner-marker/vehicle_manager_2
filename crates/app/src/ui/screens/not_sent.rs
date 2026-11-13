//! This computer's own messages, at the top of Messages (phase-3.md 5e, decision #076; sketch
//! 2A): each change the server refused (amber, with what, why, **Open**, and **Discard**),
//! changes waiting too long (a day, or 100), and why sending stopped. They're kept on the
//! desktop, so they show offline too. Nothing on the web.

use dioxus::prelude::*;

/// The desktop's own messages; nothing on the web.
#[component]
pub(crate) fn NotSent() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        return rsx! { desktop::NotSent {} };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
mod desktop {
    use std::collections::HashMap;

    use dioxus::prelude::*;
    use domain::api::{AssetsOutcome, SquawksOutcome};
    use domain::asset::AssetId;
    use domain::i18n::{MessageArg, messages};
    use domain::operation::OperationChange;
    use domain::squawk::Squawk;
    use domain::time::Clock;
    use storage::outbox::{OutboxEntry, OutboxState};

    use super::super::squawks::{SquawkSheet, use_squawk_sheet};
    use super::super::vehicle_setup::event_value;
    use crate::logging::SystemClock;
    use crate::ui::outbox::{
        REOPEN, Reopen, describe, refusal_text, stop_text, vehicle_names, waiting_too_long,
        when_text,
    };
    use crate::ui::routes::Route;
    use crate::ui::screens::use_device_zone;
    use crate::ui::{tr, tr_with};

    #[component]
    pub(super) fn NotSent() -> Element {
        let zone = use_device_zone();
        let summary = crate::local_copy::OUTBOX();
        let stopped = crate::local_copy::PUSH_STOPPED().filter(|_| summary.waiting > 0);
        let mut refused = use_signal(Vec::<OutboxEntry>::new);
        let mut names = use_signal(HashMap::<AssetId, String>::new);
        // Again whenever the outbox changes.
        use_effect(move || {
            let _changed = crate::local_copy::OUTBOX();
            spawn(async move {
                let entries: Vec<OutboxEntry> = crate::local_copy::outbox_entries()
                    .await
                    .into_iter()
                    .filter(|entry| matches!(entry.state, OutboxState::Refused { .. }))
                    .collect();
                names.set(vehicle_names(entries.iter().map(|entry| &entry.operation.change)).await);
                refused.set(entries);
            });
        });
        let long = waiting_too_long(summary.waiting, summary.oldest_waiting, SystemClock.now());
        let oldest = summary
            .oldest_waiting
            .map(|at| when_text(at, &zone()))
            .unwrap_or_default();
        rsx! {
            if let Some(stop) = stopped {
                div { class: "not-sent-card", role: "status",
                    p { class: "not-sent-title", {format!("⚠ {}", tr(messages::OUTBOX_STOPPED_TITLE))} }
                    p { class: "not-sent-reason", {stop_text(stop)} }
                }
            }
            if long {
                div { class: "not-sent-card", role: "status",
                    p { class: "not-sent-title", {format!("⚠ {}", tr(messages::OUTBOX_LONG_TITLE))} }
                    p { class: "not-sent-reason",
                        {tr_with(messages::OUTBOX_LONG_BODY, &[
                            ("count", MessageArg::from(i64::try_from(summary.waiting).unwrap_or(i64::MAX))),
                            ("when", MessageArg::from(oldest.as_str())),
                        ])}
                    }
                }
            }
            for entry in refused() {
                RefusedCard { key: "{entry.operation.id}", entry, names: names() }
            }
        }
    }

    /// One refused change (2A).
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    fn RefusedCard(entry: OutboxEntry, names: HashMap<AssetId, String>) -> Element {
        let zone = use_device_zone();
        let navigator = use_navigator();
        let sheet = use_squawk_sheet();
        let mut busy = use_signal(|| false);
        let OutboxState::Refused { refusal, .. } = &entry.state else {
            return rsx! {};
        };
        let vehicle = |id: AssetId| names.get(&id).cloned().unwrap_or_default();
        let what = describe(&entry.operation.change, &vehicle);
        let why = refusal_text(refusal);
        let made = tr_with(
            messages::REFUSED_MADE,
            &[(
                "when",
                MessageArg::from(when_text(entry.made_at, &zone()).as_str()),
            )],
        );
        let id = entry.operation.id;
        let change = entry.operation.change.clone();
        let open = move |_| {
            let change = change.clone();
            busy.set(true);
            spawn(async move {
                open_again(id, change, navigator, sheet).await;
                busy.set(false);
            });
        };
        let discard = move |_| {
            busy.set(true);
            spawn(async move {
                if REOPEN.peek().as_ref().is_some_and(|reopen| reopen.id == id) {
                    *REOPEN.write() = None;
                }
                crate::local_copy::discard(id).await;
                busy.set(false);
            });
        };
        rsx! {
            div { class: "not-sent-card", role: "status",
                p { class: "not-sent-title",
                    {format!("⚠ {}", tr_with(messages::REFUSED_TITLE, &[("what", MessageArg::from(what.as_str()))]))}
                }
                p { class: "not-sent-reason", {why} }
                p { class: "muted-note", {made} }
                div { class: "not-sent-buttons",
                    button { class: "primary-button", r#type: "button", disabled: busy(), onclick: open,
                        {tr(messages::REFUSED_OPEN)}
                    }
                    button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: discard,
                        {tr(messages::REFUSED_DISCARD)}
                    }
                }
            }
        }
    }

    /// **Open**: an event or squawk goes back in its form, as typed, and stays in Messages
    /// until it's saved anew (or discarded); anything else opens where it's made.
    async fn open_again(
        id: domain::operation::OperationId,
        change: OperationChange,
        navigator: dioxus::router::Navigator,
        sheet: SquawkSheet,
    ) {
        let reopen = |change: &OperationChange| {
            *REOPEN.write() = Some(Reopen {
                id,
                change: change.clone(),
            });
        };
        match &change {
            OperationChange::AddEvent { input } => {
                reopen(&change);
                navigator.push(Route::LogForm {
                    asset_id: input.asset.to_string(),
                    kind: event_value(input.kind).to_owned(),
                });
            }
            OperationChange::EditEvent { id: event, .. } => {
                reopen(&change);
                navigator.push(Route::EditEvent {
                    event_id: event.to_string(),
                });
            }
            OperationChange::CorrectEvent { original, .. } => {
                reopen(&change);
                navigator.push(Route::CorrectEvent {
                    event_id: original.to_string(),
                });
            }
            OperationChange::FinalizeEvent { id: event }
            | OperationChange::FinishEvent { id: event }
            | OperationChange::DeleteEvent { id: event } => {
                navigator.push(Route::EventDetail {
                    event_id: event.to_string(),
                });
            }
            OperationChange::OpenSquawk { .. } | OperationChange::EditSquawk { .. } => {
                if !open_squawk_again(&change, navigator, sheet).await {
                    return;
                }
                reopen(&change);
            }
            OperationChange::DeleteSquawk { id: squawk_id } => {
                navigator.push(Route::SquawkDetail {
                    squawk_id: squawk_id.to_string(),
                });
            }
            OperationChange::SetMyPreferences { .. } => {
                navigator.push(Route::Settings {});
            }
            OperationChange::AddPlace { .. }
            | OperationChange::EditPlace { .. }
            | OperationChange::DeletePlace { .. } => {
                navigator.push(Route::Places {});
            }
            OperationChange::SetGarageOrder { .. } => {
                navigator.push(Route::AllVehicles {});
            }
        }
    }

    /// A squawk opened again in its sheet, as typed (on the squawk as it is now, for an edit);
    /// whether the sheet opened (otherwise the squawk's page is shown).
    async fn open_squawk_again(
        change: &OperationChange,
        navigator: dioxus::router::Navigator,
        sheet: SquawkSheet,
    ) -> bool {
        let (editing, squawk) = match change {
            OperationChange::OpenSquawk { input } => (
                None,
                Squawk {
                    asset: input.asset,
                    description: input.description.clone(),
                    notes: input.notes.clone(),
                    severity: input.severity,
                    opened_at: input.opened_at,
                    // Not shown: the server sets them.
                    opened_by: input.asset.cast(),
                    found_on: input.found_on,
                    received_at: input.opened_at,
                },
            ),
            OperationChange::EditSquawk { id, changes } => {
                let Ok(SquawksOutcome::Squawk { view }) = crate::data::squawk_view(*id).await
                else {
                    navigator.push(Route::SquawkDetail {
                        squawk_id: id.to_string(),
                    });
                    return false;
                };
                let mut squawk = view.squawk;
                let mut changes = changes.clone();
                domain::command::Patch::keep_only(
                    &mut changes,
                    &domain::squawk::SquawkPatch::EDITABLE,
                );
                squawk.apply(changes);
                (Some(*id), squawk)
            }
            OperationChange::AddEvent { .. }
            | OperationChange::EditEvent { .. }
            | OperationChange::FinalizeEvent { .. }
            | OperationChange::FinishEvent { .. }
            | OperationChange::DeleteEvent { .. }
            | OperationChange::CorrectEvent { .. }
            | OperationChange::DeleteSquawk { .. }
            | OperationChange::SetMyPreferences { .. }
            | OperationChange::AddPlace { .. }
            | OperationChange::EditPlace { .. }
            | OperationChange::DeletePlace { .. }
            | OperationChange::SetGarageOrder { .. } => return false,
        };
        let Some(asset) = vehicle(squawk.asset).await else {
            return false;
        };
        sheet.reopen(editing, squawk, &asset);
        true
    }

    /// The vehicle, for the squawk sheet.
    async fn vehicle(id: AssetId) -> Option<domain::asset::Asset> {
        match crate::data::vehicle_page(id).await {
            Ok(AssetsOutcome::Asset { view }) => Some(view.asset),
            _ => None,
        }
    }
}
