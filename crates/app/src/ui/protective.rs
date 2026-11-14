//! What the desktop shows while protecting its copy (phase-3.md 7d, decision #078;
//! `docs/ui-sketches/p3m7-loss-detection-options.svg` options 1A, 2A, 3B).
//!
//! - [`ProtectedTag`] and [`ProtectedStrip`] (1A): a red **PROTECTED** tag in the top bar and a
//!   strip under it saying why, with **Details** for the choices; a pull that would delete a
//!   lot opens its own dialog (2A) when it happens.
//! - The rebuild, step by step (3B): from the choices, the setup card on a fresh server
//!   ([`RebuildOffer`]), and Diagnostics ([`ProtectionDiagnostics`]).
//! - [`ForgottenNotice`] on the sign-in card when the server no longer knows this computer.
//! - Diagnostics' snapshots of the copy, with Restore.
//!
//! Nothing on the web: it has no copy to protect.

use dioxus::prelude::*;

/// The red **PROTECTED** tag (1A); nothing otherwise, and nothing on the web.
#[component]
pub(crate) fn ProtectedTag() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        return rsx! { desktop::ProtectedTag {} };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

/// The strip under the top bar (1A), its choices, and the held pull's dialog (2A).
#[component]
pub(crate) fn ProtectedStrip() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        return rsx! { desktop::ProtectedStrip {} };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

/// On the sign-in card, when the server no longer knows this computer.
#[component]
pub(crate) fn ForgottenNotice() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        return rsx! { desktop::ForgottenNotice {} };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

/// On the setup card of a fresh server, when this computer has a copy: rebuild from it.
#[component]
pub(crate) fn RebuildOffer() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        return rsx! { desktop::RebuildOffer {} };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

/// Why the copy is protected, in a sentence, if it is (Messages' sync alert, 7d); `None`
/// otherwise. Reactive.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn protected_reason() -> Option<String> {
    crate::protect::PROTECTED().map(|why| desktop::reason(&why))
}

/// Diagnostics: the copy's snapshots (Restore, Take one now), and the rebuild.
#[component]
pub(crate) fn ProtectionDiagnostics() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        return rsx! { desktop::ProtectionDiagnostics {} };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
mod desktop {
    use dioxus::prelude::*;
    use domain::api::AssetsOutcome;
    use domain::i18n::{MessageArg, messages};
    use domain::secret::Secret;

    use crate::protect::{PROTECTED, Protection, RebuildProblem, Rebuilt};
    use crate::ui::auth::{AuthState, use_auth};
    use crate::ui::{tr, tr_with};

    /// Why the copy is protected, in a sentence.
    pub(super) fn reason(why: &Protection) -> String {
        match why {
            Protection::ServerChanged { .. } => tr(messages::PROTECTED_SERVER_CHANGED),
            Protection::WentBack { .. } => tr(messages::PROTECTED_WENT_BACK),
            Protection::Forgotten => tr(messages::PROTECTED_FORGOTTEN),
            Protection::PullHeld { deletions, .. } => tr_with(
                messages::PROTECTED_PULL_HELD,
                &[(
                    "count",
                    MessageArg::from(i64::try_from(deletions.total).unwrap_or(i64::MAX)),
                )],
            ),
        }
    }

    #[component]
    pub(super) fn ProtectedTag() -> Element {
        let Some(why) = PROTECTED() else {
            return rsx! {};
        };
        rsx! {
            span { class: "protected-tag", role: "status", title: reason(&why), {tr(messages::PROTECTED_TAG)} }
        }
    }

    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn ProtectedStrip() -> Element {
        let mut choices = use_signal(|| false);
        let mut rebuilding = use_signal(|| false);
        // A held pull's dialog opens once for that pull (2A); Keep closes it.
        let mut dismissed = use_signal(|| None::<u64>);
        let Some(why) = PROTECTED() else {
            return rsx! {};
        };
        let held = match &why {
            Protection::PullHeld { next_cursor, .. } => Some(*next_cursor),
            Protection::ServerChanged { .. }
            | Protection::WentBack { .. }
            | Protection::Forgotten => None,
        };
        let show_held = held.is_some() && dismissed() != held;
        rsx! {
            div { class: "protected-strip", role: "alert",
                span { class: "protected-strip-text", {reason(&why)} }
                button { class: "secondary-button", r#type: "button", onclick: move |_| choices.set(true),
                    {tr(messages::PROTECTED_DETAILS)}
                }
            }
            if show_held || (choices() && held.is_some()) {
                HeldDialog {
                    why,
                    on_close: move |()| {
                        dismissed.set(held);
                        choices.set(false);
                    },
                }
            } else if choices() {
                ChoicesDialog {
                    why,
                    on_close: move |()| choices.set(false),
                    on_rebuild: move |()| {
                        choices.set(false);
                        rebuilding.set(true);
                    },
                }
            }
            if rebuilding() {
                RebuildDialog { on_close: move |()| rebuilding.set(false) }
            }
        }
    }

    /// The choices (Details).
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    fn ChoicesDialog(
        why: Protection,
        on_close: EventHandler<()>,
        on_rebuild: EventHandler<()>,
    ) -> Element {
        let mut busy = use_signal(|| false);
        let may_use = matches!(
            why,
            Protection::ServerChanged { .. } | Protection::WentBack { .. }
        );
        let may_rebuild = matches!(why, Protection::ServerChanged { .. });
        let use_server = move |_| {
            busy.set(true);
            spawn(async move {
                crate::protect::use_this_server().await;
                busy.set(false);
                on_close.call(());
            });
        };
        rsx! {
            div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
                div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                    h2 { class: "dialog-title", {tr(messages::PROTECTED_TITLE)} }
                    p { class: "dialog-text", {reason(&why)} }
                    if why == Protection::Forgotten {
                        p { class: "muted-note",
                            {tr_with(messages::PROTECTED_SIGN_IN_NOTE, &[("count", MessageArg::from(i64::try_from(crate::ui::outbox::unsent_count()).unwrap_or(i64::MAX)))])}
                        }
                    }
                    div { class: "protected-choices",
                        button { class: "primary-button", r#type: "button", onclick: move |_| on_close.call(()),
                            {tr(messages::PROTECTED_KEEP)}
                        }
                        if may_use {
                            button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: use_server,
                                {tr(messages::PROTECTED_USE_SERVER)}
                            }
                            p { class: "muted-note", {tr(messages::PROTECTED_USE_SERVER_NOTE)} }
                        }
                        if may_rebuild {
                            button { class: "danger-button", r#type: "button", disabled: busy(), onclick: move |_| on_rebuild.call(()),
                                {tr(messages::PROTECTED_REBUILD)}
                            }
                        }
                    }
                }
            }
        }
    }

    /// A pull that would delete a lot (2A).
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    fn HeldDialog(why: Protection, on_close: EventHandler<()>) -> Element {
        let mut names = use_signal(Vec::<(String, u64)>::new);
        let mut busy = use_signal(|| false);
        let Protection::PullHeld { deletions, .. } = why else {
            return rsx! {};
        };
        let by_vehicle = deletions.by_vehicle.clone();
        use_effect(move || {
            let by_vehicle = by_vehicle.clone();
            spawn(async move {
                let mut found = Vec::new();
                for (asset, count) in by_vehicle.into_iter().take(5) {
                    let name = match crate::data::vehicle_page(asset).await {
                        Ok(AssetsOutcome::Asset { view }) => view.asset.name.as_str().to_owned(),
                        _ => asset.to_string(),
                    };
                    found.push((name, count));
                }
                names.set(found);
            });
        });
        let apply = move |_| {
            busy.set(true);
            spawn(async move {
                crate::protect::apply_held().await;
                busy.set(false);
                on_close.call(());
            });
        };
        let count = i64::try_from(deletions.total).unwrap_or(i64::MAX);
        rsx! {
            div { class: "dialog-scrim",
                div { class: "dialog", role: "dialog", "aria-modal": "true",
                    h2 { class: "dialog-title", {tr_with(messages::HELD_TITLE, &[("count", MessageArg::from(count))])} }
                    ul { class: "unsent-list",
                        for (n, (name, count)) in names().into_iter().enumerate() {
                            li { key: "{n}",
                                {tr_with(messages::HELD_VEHICLE, &[
                                    ("vehicle", MessageArg::from(name.as_str())),
                                    ("count", MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX))),
                                ])}
                            }
                        }
                    }
                    p { class: "dialog-text", {tr(messages::HELD_NOTE)} }
                    div { class: "form-buttons",
                        button { class: "primary-button", r#type: "button", onclick: move |_| on_close.call(()),
                            {tr(messages::HELD_KEEP)}
                        }
                        button { class: "danger-button", r#type: "button", disabled: busy(), onclick: apply,
                            {tr(messages::HELD_APPLY)}
                        }
                    }
                    p { class: "muted-note", {tr(messages::HELD_APPLY_NOTE)} }
                }
            }
        }
    }

    /// The rebuild, step by step (3B): the checks, what happens, then the code, the password,
    /// and REBUILD typed; then the temporary passwords to give out.
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn RebuildDialog(on_close: EventHandler<()>) -> Element {
        let auth = use_auth();
        let mut step = use_signal(|| 1_u8);
        let mut complete = use_signal(|| false);
        let mut nothing_waiting = use_signal(|| false);
        let mut code = use_signal(String::new);
        let mut password = use_signal(String::new);
        let mut typed = use_signal(String::new);
        let mut busy = use_signal(|| false);
        let mut problem = use_signal(|| None::<String>);
        let mut done = use_signal(|| None::<Rebuilt>);
        use_effect(move || {
            spawn(async move {
                if let Some(copy) = crate::local_copy::current() {
                    complete.set(
                        copy.pull_state()
                            .await
                            .ok()
                            .flatten()
                            .is_some_and(|state| state.complete()),
                    );
                    let summary = copy.outbox_summary().await.unwrap_or_default();
                    nothing_waiting.set(summary.waiting == 0 && summary.refused == 0);
                }
            });
        });
        let fresh = matches!(auth(), AuthState::NeedsSetup);
        let ready = complete() && nothing_waiting() && fresh;
        let rebuild = move |_| {
            busy.set(true);
            problem.set(None);
            spawn(async move {
                let result =
                    crate::protect::rebuild(Secret::new(code()), Secret::new(password())).await;
                busy.set(false);
                match result {
                    Ok(rebuilt) => done.set(Some(rebuilt)),
                    Err(why) => problem.set(Some(problem_text(&why))),
                }
            });
        };
        let mark = |ok: bool| if ok { "✓" } else { "✗" };
        rsx! {
            div { class: "dialog-scrim",
                div { class: "dialog wide-dialog", role: "dialog", "aria-modal": "true",
                    if let Some(rebuilt) = done() {
                        h2 { class: "dialog-title", {tr(messages::REBUILD_DONE)} }
                        if !rebuilt.passwords.is_empty() {
                            p { class: "dialog-text", {tr(messages::REBUILD_PASSWORDS)} }
                            ul { class: "unsent-list",
                                for person in rebuilt.passwords.iter() {
                                    li { key: "{person.username}", class: "mono",
                                        {format!("{} ({}): {}", person.display_name, person.username, person.password.expose())}
                                    }
                                }
                            }
                        }
                        if !rebuilt.left_out.is_empty() {
                            p { class: "muted-note",
                                {tr_with(messages::REBUILD_LEFT_OUT, &[("names", MessageArg::from(rebuilt.left_out.join(", ").as_str()))])}
                            }
                        }
                        div { class: "form-buttons",
                            button { class: "primary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CLOSE)} }
                        }
                    } else {
                        h2 { class: "dialog-title", {tr_with(messages::REBUILD_TITLE, &[("step", MessageArg::from(i64::from(step())))])} }
                        match step() {
                            1 => rsx! {
                                ul { class: "rebuild-checks",
                                    li { {format!("{} {}", mark(complete()), tr(messages::REBUILD_CHECK_COMPLETE))} }
                                    li { {format!("{} {}", mark(nothing_waiting()), tr(messages::REBUILD_CHECK_NOTHING_WAITING))} }
                                    li { {format!("{} {}", mark(fresh), tr(messages::REBUILD_CHECK_FRESH))} }
                                }
                                if !ready {
                                    p { class: "muted-note", {tr(messages::REBUILD_NOT_READY)} }
                                }
                            },
                            2 => rsx! {
                                ul { class: "unsent-list",
                                    li { {tr(messages::REBUILD_WHAT_RECORDS)} }
                                    li { {tr(messages::REBUILD_WHAT_PASSWORDS)} }
                                    li { {tr(messages::REBUILD_WHAT_DEVICES)} }
                                    li { {tr(messages::REBUILD_WHAT_PHOTOS)} }
                                }
                            },
                            _ => rsx! {
                                label { class: "field",
                                    span { class: "field-label", {tr(messages::REBUILD_CODE)} }
                                    input { r#type: "text", autocomplete: "off", value: "{code}", oninput: move |event| code.set(event.value()) }
                                }
                                label { class: "field",
                                    span { class: "field-label", {tr(messages::REBUILD_PASSWORD)} }
                                    input { r#type: "password", autocomplete: "current-password", value: "{password}", oninput: move |event| password.set(event.value()) }
                                }
                                label { class: "field",
                                    span { class: "field-label", {tr(messages::REBUILD_TYPE)} }
                                    input { r#type: "text", autocomplete: "off", value: "{typed}", oninput: move |event| typed.set(event.value()) }
                                }
                                if let Some(problem) = problem() {
                                    p { class: "form-error", role: "alert", {problem} }
                                }
                            },
                        }
                        div { class: "form-buttons",
                            if step() > 1 {
                                button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: move |_| step -= 1, {tr(messages::REBUILD_BACK)} }
                            } else {
                                button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                            }
                            if step() < 3 {
                                button { class: "primary-button", r#type: "button", disabled: !ready, onclick: move |_| step += 1, {tr(messages::REBUILD_NEXT)} }
                            } else {
                                button {
                                    class: "danger-button",
                                    r#type: "button",
                                    disabled: busy() || typed() != "REBUILD" || code().is_empty() || password().is_empty(),
                                    onclick: rebuild,
                                    if busy() { {tr(messages::REBUILD_BUSY)} } else { {tr(messages::REBUILD_BUTTON)} }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Why a rebuild didn't happen, in words.
    fn problem_text(why: &RebuildProblem) -> String {
        match why {
            RebuildProblem::NotReady => tr(messages::REBUILD_NOT_READY),
            RebuildProblem::WrongCode => tr(messages::REBUILD_WRONG_CODE),
            RebuildProblem::TooManyAttempts(seconds) => {
                crate::ui::screens::too_many_attempts(*seconds)
            }
            RebuildProblem::NotFresh => tr(messages::REBUILD_NOT_FRESH),
            RebuildProblem::NotAllowed => tr(messages::REBUILD_NOT_ALLOWED),
            RebuildProblem::Failed => tr(messages::REBUILD_FAILED),
        }
    }

    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn ForgottenNotice() -> Element {
        let mut confirming = use_signal(|| false);
        if PROTECTED() != Some(Protection::Forgotten) {
            return rsx! {};
        }
        let unsent = crate::ui::outbox::unsent_count();
        let delete = move |_| {
            confirming.set(false);
            crate::pairing::forget();
        };
        rsx! {
            div { class: "protected-notice", role: "status",
                p { {tr_with(messages::PROTECTED_SIGN_IN_NOTE, &[("count", MessageArg::from(i64::try_from(unsent).unwrap_or(i64::MAX)))])} }
                if confirming() {
                    p { class: "muted-note", {tr(messages::PROTECTED_DELETE_CONFIRM)} }
                    div { class: "form-buttons",
                        button { class: "secondary-button", r#type: "button", onclick: move |_| confirming.set(false), {tr(messages::ACTION_CANCEL)} }
                        button { class: "danger-button", r#type: "button", onclick: delete, {tr(messages::PROTECTED_DELETE_COPY)} }
                    }
                } else {
                    button { class: "link-button", r#type: "button", onclick: move |_| confirming.set(true), {tr(messages::PROTECTED_DELETE_COPY)} }
                }
            }
        }
    }

    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn RebuildOffer() -> Element {
        let mut open = use_signal(|| false);
        if crate::local_copy::current().is_none() {
            return rsx! {};
        }
        rsx! {
            div { class: "protected-notice", role: "status",
                p { {tr(messages::REBUILD_OFFER)} }
                button { class: "secondary-button", r#type: "button", onclick: move |_| open.set(true), {tr(messages::PROTECTED_REBUILD)} }
            }
            if open() {
                RebuildDialog { on_close: move |()| open.set(false) }
            }
        }
    }

    /// A snapshot's file name as a time: `snapshot-20261113T080000Z.sqlite` → "2026-11-13
    /// 08:00:00 UTC" (with seconds: two taken in the same minute differ).
    pub(super) fn snapshot_when(path: &std::path::Path) -> String {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let stamp = name
            .strip_prefix("snapshot-")
            .and_then(|rest| rest.strip_suffix(".sqlite"))
            .unwrap_or(&name);
        match (
            stamp.get(0..4),
            stamp.get(4..6),
            stamp.get(6..8),
            stamp.get(9..11),
            stamp.get(11..13),
            stamp.get(13..15),
        ) {
            (Some(y), Some(m), Some(d), Some(h), Some(min), Some(sec)) => {
                format!("{y}-{m}-{d} {h}:{min}:{sec} UTC")
            }
            _ => name,
        }
    }

    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn ProtectionDiagnostics() -> Element {
        let mut snapshots = use_signal(crate::protect::snapshots);
        let mut confirming = use_signal(|| None::<usize>);
        let mut busy = use_signal(|| false);
        let mut rebuilding = use_signal(|| false);
        if crate::local_copy::current().is_none() {
            return rsx! {};
        }
        let take = move |_| {
            busy.set(true);
            spawn(async move {
                crate::protect::snapshot("taken from Diagnostics").await;
                snapshots.set(crate::protect::snapshots());
                busy.set(false);
            });
        };
        let mut restore = move |index: usize| {
            let Some(path) = snapshots.peek().get(index).cloned() else {
                return;
            };
            busy.set(true);
            confirming.set(None);
            spawn(async move {
                crate::protect::restore(path).await;
                snapshots.set(crate::protect::snapshots());
                busy.set(false);
            });
        };
        let may_rebuild = matches!(PROTECTED(), Some(Protection::ServerChanged { .. }));
        rsx! {
            section { class: "about",
                h3 { class: "section-heading", {tr(messages::SNAPSHOTS_HEADING).to_uppercase()} }
                if snapshots().is_empty() {
                    p { class: "muted-note", {tr(messages::SNAPSHOT_NONE)} }
                }
                ul { class: "snapshot-list",
                    for (index, path) in snapshots().into_iter().enumerate() {
                        li { key: "{path.display()}", class: "snapshot-row",
                            span { {snapshot_when(&path)} }
                            if confirming() == Some(index) {
                                span { class: "muted-note", {tr(messages::SNAPSHOT_RESTORE_CONFIRM)} }
                                button { class: "secondary-button", r#type: "button", onclick: move |_| confirming.set(None), {tr(messages::ACTION_CANCEL)} }
                                button { class: "danger-button", r#type: "button", disabled: busy(), onclick: move |_| restore(index),
                                    {tr(messages::SNAPSHOT_RESTORE)}
                                }
                            } else {
                                button { class: "link-button", r#type: "button", disabled: busy(), onclick: move |_| confirming.set(Some(index)),
                                    {tr(messages::SNAPSHOT_RESTORE)}
                                }
                            }
                        }
                    }
                }
                button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: take, {tr(messages::SNAPSHOT_TAKE)} }
                if may_rebuild {
                    button { class: "danger-button", r#type: "button", onclick: move |_| rebuilding.set(true), {tr(messages::PROTECTED_REBUILD)} }
                }
            }
            if rebuilding() {
                RebuildDialog { on_close: move |()| rebuilding.set(false) }
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn a_snapshots_name_reads_as_its_time() {
            assert_eq!(
                snapshot_when(std::path::Path::new("/x/snapshot-20261113T080512Z.sqlite")),
                "2026-11-13 08:05:12 UTC"
            );
            assert_eq!(snapshot_when(std::path::Path::new("odd")), "odd");
        }
    }
}
