//! What the desktop shows about changes not yet on the server (phase-3.md 5e, decision #076;
//! `docs/ui-sketches/p3m5-outbox-options.svg` options 1A, 2A, 3B).
//!
//! - [`SendChip`]: **↑ 3 to send** in the top bar's sync spot (1A), while changes wait; its
//!   tooltip gives the oldest (and why sending stopped, if it did), and it opens Diagnostics.
//! - Refused changes are messages on this computer (2A, `crate::ui::screens::NotSent`), with
//!   **Open** (back in its form, as typed: [`REOPEN`]) and **Discard**.
//! - [`SignOutCheck`]: signing out with changes not sent lists them first (3B).
//!
//! Nothing on the web: it has no outbox.

use dioxus::prelude::*;
use domain::asset::AssetId;
use domain::event::{EventId, EventInput, EventPatch};
use domain::meter::EventKind;
use domain::operation::{OperationChange, OperationId};
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
use {
    crate::ui::{tr, tr_with},
    domain::api::{
        AssetsOutcome, AttachmentsOutcome, EventsOutcome, PlacesOutcome, PreferencesOutcome,
        SquawksOutcome,
    },
    domain::i18n::{MessageArg, Translator, messages},
    domain::location::TimeZone,
    domain::operation::Refusal,
    domain::time::UnixMillis,
};

/// A refused change opened again in its form (2A's **Open**): the form starts from it, and
/// saving it anew takes the refused one out of Messages ([`settle_reopened`]).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reopen {
    /// The refused operation.
    pub(crate) id: OperationId,
    /// What it did.
    pub(crate) change: OperationChange,
}

/// The change being opened again, if one is.
pub(crate) static REOPEN: GlobalSignal<Option<Reopen>> = Signal::global(|| None);

/// Whether `saved` is the same kind of change, to the same record, as `reopened`.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
fn same_change(reopened: &OperationChange, saved: &OperationChange) -> bool {
    use OperationChange as C;
    match (reopened, saved) {
        (C::AddEvent { input: a }, C::AddEvent { input: b }) => {
            a.asset == b.asset && a.kind == b.kind
        }
        (C::EditEvent { id: a, .. }, C::EditEvent { id: b, .. })
        | (C::CorrectEvent { original: a, .. }, C::CorrectEvent { original: b, .. }) => a == b,
        (C::OpenSquawk { input: a }, C::OpenSquawk { input: b }) => a.asset == b.asset,
        (C::EditSquawk { id: a, .. }, C::EditSquawk { id: b, .. }) => a == b,
        _ => false,
    }
}

/// After `saved` was saved: the refused change it replaces, if it was one opened again (and
/// that's no longer being opened).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn settle_reopened(saved: &OperationChange) -> Option<OperationId> {
    let reopened = REOPEN.peek().clone()?;
    if !same_change(&reopened.change, saved) {
        return None;
    }
    *REOPEN.write() = None;
    Some(reopened.id)
}

/// A refused new event for `asset` of `kind`, being opened again: the log form starts from it.
pub(crate) fn reopened_new_event(asset: AssetId, kind: EventKind) -> Option<EventInput> {
    let reopened = REOPEN.peek().as_ref()?.change.clone();
    if let OperationChange::AddEvent { input } = reopened
        && input.asset == asset
        && input.kind == kind
    {
        return Some(input);
    }
    None
}

/// A refused edit of event `id`, being opened again: its changes, put on the event as it is.
pub(crate) fn reopened_edit(id: EventId) -> Option<EventPatch> {
    let reopened = REOPEN.peek().as_ref()?.change.clone();
    if let OperationChange::EditEvent {
        id: edited,
        changes,
    } = reopened
        && edited == id
    {
        return Some(changes);
    }
    None
}

/// A refused correction of event `original`, being opened again.
pub(crate) fn reopened_correction(original: EventId) -> Option<EventInput> {
    let reopened = REOPEN.peek().as_ref()?.change.clone();
    if let OperationChange::CorrectEvent {
        original: corrected,
        input,
        ..
    } = reopened
        && corrected == original
    {
        return Some(input);
    }
    None
}

/// What a change was, in a few words: "Fuel-up on Truck". `vehicle` names a vehicle by ID.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn describe(change: &OperationChange, vehicle: &dyn Fn(AssetId) -> String) -> String {
    use OperationChange as C;
    match change {
        C::AddEvent { input } => tr_with(
            messages::CHANGE_EVENT,
            &[
                ("kind", MessageArg::from(tr(input.kind.label()).as_str())),
                ("vehicle", MessageArg::from(vehicle(input.asset).as_str())),
            ],
        ),
        C::CorrectEvent { input, .. } => tr_with(
            messages::CHANGE_EVENT_CORRECT,
            &[
                ("kind", MessageArg::from(tr(input.kind.label()).as_str())),
                ("vehicle", MessageArg::from(vehicle(input.asset).as_str())),
            ],
        ),
        C::EditEvent { .. } => tr(messages::CHANGE_EVENT_EDIT),
        C::FinalizeEvent { .. } => tr(messages::CHANGE_EVENT_FINALIZE),
        C::FinishEvent { .. } => tr(messages::CHANGE_EVENT_FINISH),
        C::DeleteEvent { .. } => tr(messages::CHANGE_EVENT_DELETE),
        C::OpenSquawk { input } => tr_with(
            messages::CHANGE_SQUAWK,
            &[
                ("text", MessageArg::from(input.description.as_str())),
                ("vehicle", MessageArg::from(vehicle(input.asset).as_str())),
            ],
        ),
        C::EditSquawk { .. } => tr(messages::CHANGE_SQUAWK_EDIT),
        C::DeleteSquawk { .. } => tr(messages::CHANGE_SQUAWK_DELETE),
        C::SetMyPreferences { .. } => tr(messages::CHANGE_PREFERENCES),
        C::AddPlace { form } => tr_with(
            messages::CHANGE_PLACE,
            &[("name", MessageArg::from(form.name.as_str()))],
        ),
        C::EditPlace { .. } => tr(messages::CHANGE_PLACE_EDIT),
        C::DeletePlace { .. } => tr(messages::CHANGE_PLACE_DELETE),
        C::SetGarageOrder { .. } => tr(messages::CHANGE_GARAGE_ORDER),
        C::AddAttachment { input } => tr_with(
            messages::CHANGE_ATTACHMENT_ADD,
            &[("vehicle", MessageArg::from(vehicle(input.asset).as_str()))],
        ),
        C::MoveAttachment { .. } => tr(messages::CHANGE_ATTACHMENT_MOVE),
        C::RemoveAttachment { .. } => tr(messages::CHANGE_ATTACHMENT_REMOVE),
    }
}

/// The vehicles a change names (to look up their names).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn vehicle_of(change: &OperationChange) -> Option<AssetId> {
    match change {
        OperationChange::AddEvent { input } | OperationChange::CorrectEvent { input, .. } => {
            Some(input.asset)
        }
        OperationChange::OpenSquawk { input } => Some(input.asset),
        OperationChange::AddAttachment { input } => Some(input.asset),
        OperationChange::EditEvent { .. }
        | OperationChange::FinalizeEvent { .. }
        | OperationChange::FinishEvent { .. }
        | OperationChange::DeleteEvent { .. }
        | OperationChange::EditSquawk { .. }
        | OperationChange::DeleteSquawk { .. }
        | OperationChange::SetMyPreferences { .. }
        | OperationChange::AddPlace { .. }
        | OperationChange::EditPlace { .. }
        | OperationChange::DeletePlace { .. }
        | OperationChange::SetGarageOrder { .. }
        | OperationChange::MoveAttachment { .. }
        | OperationChange::RemoveAttachment { .. } => None,
    }
}

/// Why the server refused a change, as the form would have said it.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn refusal_text(refusal: &Refusal) -> String {
    let id = match refusal {
        Refusal::Events(EventsOutcome::Invalid { problem }) => problem.message(),
        Refusal::Events(EventsOutcome::NotFound) => messages::EVENT_NOT_FOUND,
        Refusal::Squawks(SquawksOutcome::Invalid { problem }) => problem.message(),
        Refusal::Squawks(SquawksOutcome::NotFound) => messages::SQUAWK_NOT_FOUND,
        Refusal::Places(PlacesOutcome::Invalid { problem }) => problem.message(),
        Refusal::Attachments(AttachmentsOutcome::Invalid { problem }) => problem.message(),
        Refusal::Events(EventsOutcome::NotAllowed)
        | Refusal::Squawks(SquawksOutcome::NotAllowed)
        | Refusal::Preferences(PreferencesOutcome::NotAllowed)
        | Refusal::Assets(AssetsOutcome::NotAllowed)
        | Refusal::Attachments(AttachmentsOutcome::NotAllowed) => messages::VEHICLES_NOT_ALLOWED,
        Refusal::Events(_)
        | Refusal::Squawks(_)
        | Refusal::Places(_)
        | Refusal::Preferences(_)
        | Refusal::Assets(_)
        | Refusal::Attachments(_) => messages::SOMETHING_WENT_WRONG,
    };
    tr(id)
}

/// `at` on this computer's clock, as "12 November 2026 3:42 pm".
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn when_text(at: UnixMillis, zone: &TimeZone) -> String {
    let translator = Translator::english();
    let date = domain::dates::format_date(
        at,
        zone,
        domain::preferences::DateFormat::DayMonthNameYear,
        translator,
    );
    let time = domain::dates::format_time(at, zone, translator);
    match (date, time) {
        (Ok(date), Ok(time)) => format!("{date} {time}"),
        _ => domain::time_utils::format_utc_minute(at),
    }
}

/// Changes waiting longer than this are worth a message (#076): a day.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) const LONG_WAIT_MILLIS: i64 = 24 * 60 * 60 * 1000;

/// So many changes waiting are worth a message (#076).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) const LONG_WAIT_COUNT: u64 = 100;

/// Whether changes have waited long enough (or piled up enough) to say so in Messages.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn waiting_too_long(waiting: u64, oldest: Option<UnixMillis>, now: UnixMillis) -> bool {
    waiting >= LONG_WAIT_COUNT
        || oldest
            .is_some_and(|at| now.as_millis().saturating_sub(at.as_millis()) >= LONG_WAIT_MILLIS)
}

/// This computer's own messages (refused changes, changes waiting too long, sending stopped),
/// for the Messages badge; reactive. Always 0 on the web.
pub(crate) fn alert_count() -> usize {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        use domain::time::Clock;
        let summary = crate::local_copy::OUTBOX();
        let long = waiting_too_long(
            summary.waiting,
            summary.oldest_waiting,
            crate::logging::SystemClock.now(),
        );
        let stopped = crate::local_copy::PUSH_STOPPED().is_some() && summary.waiting > 0;
        usize::try_from(summary.refused).unwrap_or(usize::MAX)
            + usize::from(long)
            + usize::from(stopped)
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    0
}

/// How many changes are on this computer and not on the server (waiting, or refused): signing
/// out would lose them. Always 0 on the web.
pub(crate) fn unsent_count() -> u64 {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        let summary = crate::local_copy::OUTBOX.peek();
        summary.waiting + summary.refused
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    0
}

/// **↑ 3 to send** (1A), while changes wait; nothing otherwise, and nothing on the web.
#[cfg_attr(
    all(feature = "desktop", not(target_arch = "wasm32")),
    expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )
)]
#[component]
pub(crate) fn SendChip() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        let zone = crate::ui::screens::use_device_zone();
        let navigator = use_navigator();
        let summary = crate::local_copy::OUTBOX();
        if summary.waiting == 0 {
            return rsx! {};
        }
        let count = i64::try_from(summary.waiting).unwrap_or(i64::MAX);
        let when = summary
            .oldest_waiting
            .map(|at| when_text(at, &zone()))
            .unwrap_or_default();
        let mut note = tr_with(
            messages::OUTBOX_CHIP_NOTE,
            &[
                ("count", MessageArg::from(count)),
                ("when", MessageArg::from(when.as_str())),
            ],
        );
        if let Some(stop) = crate::local_copy::PUSH_STOPPED() {
            note = format!("{note}\n{}", stop_text(stop));
        }
        return rsx! {
            button {
                class: "send-chip",
                r#type: "button",
                title: note,
                onclick: move |_| {
                    navigator.push(crate::ui::routes::Route::Diagnostics {});
                },
                {tr_with(messages::OUTBOX_CHIP, &[("count", MessageArg::from(count))])}
            }
        };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

/// Why sending stopped, in words.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn stop_text(stop: domain::sync::PushStop) -> String {
    use domain::sync::PushStop;
    match stop {
        PushStop::AppTooOld { .. } => tr(messages::OUTBOX_STOPPED_APP),
        PushStop::ServerTooOld { .. } => tr(messages::OUTBOX_STOPPED_SERVER),
        PushStop::ClockAhead { ahead_millis } => tr_with(
            messages::OUTBOX_STOPPED_CLOCK,
            &[("minutes", MessageArg::from((ahead_millis / 60_000).max(1)))],
        ),
        PushStop::Unavailable => tr(messages::OUTBOX_STOPPED_BUSY),
    }
}

/// How many changes the sign-out warning lists by name before "and 5 more" (3B).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
const LISTED: usize = 4;

/// Signing out with changes not sent (3B): lists them, with **Send now** (sends, then signs
/// out if nothing is left), **Cancel**, and **Sign out anyway**.
#[cfg_attr(
    all(feature = "desktop", not(target_arch = "wasm32")),
    expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )
)]
#[component]
pub(crate) fn SignOutCheck(on_cancel: EventHandler<()>, on_sign_out: EventHandler<()>) -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        let mut lines = use_signal(Vec::<String>::new);
        let mut busy = use_signal(|| false);
        let summary = crate::local_copy::OUTBOX();
        use_effect(move || {
            let _changed = crate::local_copy::OUTBOX();
            spawn(async move {
                let entries = crate::local_copy::outbox_entries().await;
                let names =
                    vehicle_names(entries.iter().map(|entry| &entry.operation.change)).await;
                let vehicle = |id: AssetId| names.get(&id).cloned().unwrap_or_default();
                lines.set(
                    entries
                        .iter()
                        .map(|entry| {
                            let what = describe(&entry.operation.change, &vehicle);
                            match entry.state {
                                storage::outbox::OutboxState::Waiting => what,
                                storage::outbox::OutboxState::Refused { .. } => tr_with(
                                    messages::SIGN_OUT_NOT_SAVED,
                                    &[("what", MessageArg::from(what.as_str()))],
                                ),
                            }
                        })
                        .collect(),
                );
            });
        });
        let all = lines();
        let more = all.len().saturating_sub(LISTED);
        let count = i64::try_from(summary.waiting + summary.refused).unwrap_or(i64::MAX);
        let offline = crate::ui::offline::is_offline_now();
        let send_now = move |_| {
            busy.set(true);
            spawn(async move {
                let all_sent = crate::local_copy::send_all().await;
                busy.set(false);
                if all_sent && crate::local_copy::OUTBOX.peek().refused == 0 {
                    on_sign_out.call(());
                }
            });
        };
        return rsx! {
            div { class: "dialog-scrim", onclick: move |_| on_cancel.call(()),
                div {
                    class: "dialog",
                    role: "dialog",
                    "aria-modal": "true",
                    onclick: move |event| event.stop_propagation(),
                    h2 { class: "dialog-title",
                        {tr_with(messages::SIGN_OUT_WAITING_TITLE, &[("count", MessageArg::from(count))])}
                    }
                    ul { class: "unsent-list",
                        for (n, line) in all.iter().take(LISTED).enumerate() {
                            li { key: "{n}", {line.clone()} }
                        }
                        if more > 0 {
                            li { class: "muted-note",
                                {tr_with(messages::SIGN_OUT_AND_MORE, &[("count", MessageArg::from(i64::try_from(more).unwrap_or(i64::MAX)))])}
                            }
                        }
                    }
                    p { class: "dialog-text", {tr(messages::SIGN_OUT_WAITING_BODY)} }
                    div { class: "form-buttons",
                        button {
                            class: "primary-button",
                            r#type: "button",
                            disabled: busy() || offline || summary.waiting == 0,
                            title: crate::ui::offline::needs_server_note(),
                            onclick: send_now,
                            {tr(messages::SIGN_OUT_SEND_NOW)}
                        }
                        button { class: "secondary-button", r#type: "button", onclick: move |_| on_cancel.call(()),
                            {tr(messages::ACTION_CANCEL)}
                        }
                        button { class: "danger-button", r#type: "button", disabled: busy(), onclick: move |_| on_sign_out.call(()),
                            {tr(messages::SIGN_OUT_ANYWAY)}
                        }
                    }
                }
            }
        };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    {
        let _ = (on_cancel, on_sign_out);
        rsx! {}
    }
}

/// The names of the vehicles `changes` name, from the data layer (the copy on a desktop).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) async fn vehicle_names<'a>(
    changes: impl Iterator<Item = &'a OperationChange>,
) -> std::collections::HashMap<AssetId, String> {
    let mut names = std::collections::HashMap::new();
    for asset in changes.filter_map(vehicle_of) {
        if names.contains_key(&asset) {
            continue;
        }
        if let Ok(AssetsOutcome::Asset { view }) = crate::data::vehicle_page(asset).await {
            names.insert(asset, view.asset.name.as_str().to_owned());
        }
    }
    names
}

#[cfg(all(test, feature = "desktop", not(target_arch = "wasm32")))]
mod tests {
    use domain::test_support::test_id;

    use super::*;

    #[test]
    fn changes_wait_too_long_after_a_day_or_a_hundred() {
        let now = UnixMillis::from_millis(1_800_000_000_000);
        let ago = |millis: i64| Some(UnixMillis::from_millis(now.as_millis() - millis));
        assert!(!waiting_too_long(0, None, now));
        assert!(!waiting_too_long(3, ago(LONG_WAIT_MILLIS - 1), now));
        assert!(waiting_too_long(3, ago(LONG_WAIT_MILLIS), now));
        assert!(waiting_too_long(LONG_WAIT_COUNT, ago(1), now));
        assert!(!waiting_too_long(LONG_WAIT_COUNT - 1, ago(1), now));
    }

    #[test]
    fn a_change_saved_again_matches_only_the_one_it_reopened() {
        let reopened = OperationChange::EditEvent {
            id: test_id(1),
            changes: EventPatch::default(),
        };
        assert!(same_change(
            &reopened,
            &OperationChange::EditEvent {
                id: test_id(1),
                changes: EventPatch::default()
            }
        ));
        assert!(!same_change(
            &reopened,
            &OperationChange::EditEvent {
                id: test_id(2),
                changes: EventPatch::default()
            }
        ));
        assert!(!same_change(
            &reopened,
            &OperationChange::DeleteEvent { id: test_id(1) }
        ));
    }
}
