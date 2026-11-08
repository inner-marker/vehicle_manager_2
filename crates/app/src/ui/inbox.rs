//! The signed-in person's messages, shared by the shell and the Messages screen (#050, #053).
//!
//! The shell asks the server again on every navigation and whenever the mode changes (Admin
//! messages only show in Admin mode). From the answer come the unread **badge** on Messages
//! and the **urgent banner** under the top bar; the Messages screen shows the list itself.

use dioxus::prelude::*;
use domain::api::{InboxOutcome, MarkMessage};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::inbox::{Inbox, InboxItem, MessageKey, Severity};
use domain::log_area;
use domain::time_utils::format_utc_minute;

use super::routes::Route;
use super::{tr, tr_with};
use crate::api;

/// The messages state, in context (provided by the shell).
#[derive(Debug, Clone, Copy)]
pub(crate) struct InboxContext {
    /// The person's inbox, once loaded.
    pub inbox: Signal<Option<Inbox>>,
    /// The message open on the Messages screen.
    pub selected: Signal<Option<MessageKey>>,
}

/// Puts a fresh [`InboxContext`] in context; call once, in the shell.
pub(crate) fn provide_inbox_context() -> InboxContext {
    use_context_provider(|| InboxContext {
        inbox: Signal::new(None),
        selected: Signal::new(None),
    })
}

/// The [`InboxContext`], from context.
pub(crate) fn use_inbox() -> InboxContext {
    use_context::<InboxContext>()
}

/// Keeps the server's answer, if it's an inbox. Returns what went wrong otherwise.
pub(crate) fn apply(
    mut context: InboxContext,
    result: Result<InboxOutcome, ServerFnError>,
) -> Option<MessageId> {
    match result {
        Ok(InboxOutcome::Inbox { inbox }) => {
            tracing::debug!(target: log_area::UI, items = inbox.items.len(), unread = inbox.unread, "inbox loaded");
            context.inbox.set(Some(inbox));
            None
        }
        // The message is gone (e.g. its cause was fixed): the next refresh shows the rest.
        Ok(InboxOutcome::NotFound) => None,
        Ok(InboxOutcome::Unavailable) => Some(messages::SOMETHING_WENT_WRONG),
        Err(error) => {
            tracing::debug!(target: log_area::UI, %error, "inbox request failed");
            Some(super::screens::problem_from_error(&error).message())
        }
    }
}

/// Asks the server for the inbox again.
pub(crate) async fn refresh_inbox(context: InboxContext) {
    let _ = apply(context, api::get_messages().await);
}

/// Marks a message read or dismisses it, keeping the server's new answer.
pub(crate) async fn mark(
    context: InboxContext,
    key: MessageKey,
    action: MarkMessage,
) -> Option<MessageId> {
    apply(
        context,
        api::mark_message(key.as_str().to_owned(), action).await,
    )
}

/// The unread count, for the badge (0 until loaded).
pub(crate) fn unread_count(context: InboxContext) -> usize {
    context
        .inbox
        .read()
        .as_ref()
        .map_or(0, |inbox| inbox.unread)
}

/// A severity's word (always shown next to its color).
pub(crate) const fn severity_label(severity: Severity) -> MessageId {
    match severity {
        Severity::Urgent => messages::MESSAGES_SEVERITY_URGENT,
        Severity::Warning => messages::MESSAGES_SEVERITY_WARNING,
        Severity::Info => messages::MESSAGES_SEVERITY_INFO,
    }
}

/// A severity's CSS class (its color).
pub(crate) const fn severity_class(severity: Severity) -> &'static str {
    match severity {
        Severity::Urgent => "severity-urgent",
        Severity::Warning => "severity-warning",
        Severity::Info => "severity-info",
    }
}

/// "WARNING · 2026-10-31 07:00 UTC".
pub(crate) fn severity_and_time(item: &InboxItem) -> String {
    let time = tr_with(
        messages::MESSAGES_TIME_UTC,
        &[(
            "time",
            MessageArg::from(format_utc_minute(item.message.raised_at)),
        )],
    );
    format!("{} · {time}", tr(severity_label(item.message.severity)))
}

/// The newest urgent message, if any (the inbox lists unread first, so look at them all).
fn newest_urgent(inbox: &Inbox) -> Option<&InboxItem> {
    inbox
        .items
        .iter()
        .filter(|item| item.message.severity == Severity::Urgent)
        .max_by_key(|item| item.message.raised_at)
}

/// The red strip under the top bar while an urgent message is showing, with **View**.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn UrgentBanner() -> Element {
    let mut context = use_inbox();
    let navigator = use_navigator();
    let urgent = context
        .inbox
        .read()
        .as_ref()
        .and_then(newest_urgent)
        .cloned();
    let Some(item) = urgent else {
        return rsx! {};
    };
    let key = item.message.key.clone();
    rsx! {
        div { class: "urgent-banner", role: "alert",
            strong { class: "urgent-banner-label", {tr(messages::URGENT_BANNER_LABEL)} }
            span { class: "urgent-banner-text", {tr(item.message.kind.title())} }
            button {
                class: "urgent-banner-view",
                onclick: move |_| {
                    context.selected.set(Some(key.clone()));
                    navigator.push(Route::Messages {});
                },
                {tr(messages::URGENT_BANNER_VIEW)}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::access::ActiveMode;
    use domain::inbox::{ServerProblem, ServerState, inbox, server_messages};
    use domain::time::UnixMillis;

    use super::*;

    #[test]
    fn the_banner_shows_the_newest_urgent_message() {
        let messages = server_messages(&ServerState {
            problem: Some((
                ServerProblem::IntegrityCheckFailed,
                UnixMillis::from_millis(9),
            )),
            jobs: vec![],
            place_lookup_problem: None,
        });
        let seen = inbox(&messages, &[], ActiveMode::Driver);
        let newest = newest_urgent(&seen).map(|item| item.message.raised_at);
        assert_eq!(newest, Some(UnixMillis::from_millis(9)));
        assert!(newest_urgent(&Inbox::default()).is_none());
    }
}
