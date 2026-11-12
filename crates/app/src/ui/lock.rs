//! The desktop's app lock (phase-3.md 3c, decision #074; `docs/ui-sketches/p3m3-lock-options.svg`
//! options 1B and 2B).
//!
//! [`LockGate`] stands in front of the whole app on the desktop:
//! - **Choose a PIN** right after a desktop pairs (required): the sign-in and the password
//!   typed at sign-in wait in memory only until the PIN seals them (`crate::sealed`), then
//!   they're dropped.
//! - **Locked** when the app opens with a sealed sign-in, and when it's been idle for the
//!   person's "Lock after" (the idle watcher calls [`lock_now`]). While locked, the key and token
//!   aren't in memory and nothing behind the gate runs. The PIN or the password opens the
//!   sealed sign-in; after 5 wrong PINs only the password works, after 10 wrong tries the
//!   desktop signs out and forgets it.
//!
//! [`LockSettings`] adds **Lock after** and **Change PIN** to Settings → This device (3e).
//!
//! Unlocking runs Argon2 (about 0.3 s) on a blocking thread, so the window stays responsive.
//! The web app has no lock: [`LockGate`] just shows the app there.

use dioxus::prelude::*;

/// Where the desktop's lock stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LockState {
    /// Open: the app shows.
    Open,
    /// Just paired: a PIN must be chosen before the app shows (the desktop only).
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    ChoosePin,
    /// Locked: the PIN or password opens it.
    Locked,
}

/// The lock, for the whole app.
pub(crate) static LOCK: GlobalSignal<LockState> = Signal::global(initial_state);

/// At launch: locked if this desktop has a sealed sign-in for its server.
fn initial_state() -> LockState {
    if crate::pairing::has_sealed_sign_in() {
        LockState::Locked
    } else {
        LockState::Open
    }
}

/// Shows the app, or (on the desktop) the lock or "Choose a PIN" in front of it.
#[component]
pub(crate) fn LockGate(children: Element) -> Element {
    match LOCK() {
        LockState::Open => children,
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
        LockState::ChoosePin => rsx! { desktop::ChoosePin {} },
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
        LockState::Locked => rsx! { desktop::Locked {} },
        #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
        LockState::Locked => children,
    }
}

/// Locks the desktop now, if it has a sealed sign-in to unlock: forgets the key and token from
/// memory and shows the lock. Nothing on the web.
pub(crate) fn lock_now() {
    if crate::client::has_device_key() && crate::pairing::has_sealed_sign_in() {
        tracing::info!(target: domain::log_area::AUTH, "locking this desktop");
        crate::client::set_device_key(None);
        crate::client::set_session_token(None);
        // The copy's key goes with them (#075).
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
        dioxus::core::spawn_forever(crate::local_copy::close());
        *LOCK.write() = LockState::Locked;
    }
}

/// How long this desktop may sit idle before it locks (Settings → This device; 15 minutes by
/// default), or `None`: no lock while open (the web, a desktop that isn't paired, or "Only when
/// the app opens").
#[must_use]
pub(crate) fn lock_after_millis() -> Option<i64> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        if !crate::client::has_device_key() {
            return None;
        }
        let setting = crate::data_folder::platform_data_folder()
            .map(|folder| crate::settings::load(&folder).lock_after)
            .unwrap_or_default();
        setting.after_millis()
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    None
}

/// After a desktop pairs: holds the password typed (in memory only, with the sign-in already in
/// memory) and asks for a PIN.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) fn choose_pin_after_pairing(password: domain::secret::Secret) {
    desktop::hold_for_pin(password);
    *LOCK.write() = LockState::ChoosePin;
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
mod desktop {
    use std::sync::Mutex;

    use dioxus::prelude::*;
    use domain::i18n::{MessageArg, MessageId, messages};
    use domain::log_area;
    use domain::pin::{LockAfter, UnlockStep, check_new_pin};
    use domain::secret::Secret;

    use super::{LOCK, LockState};
    use crate::device_credential::{self, Credential, SealedCredential};
    use crate::ui::auth::{self, use_auth};
    use crate::ui::{tr, tr_with};

    /// The password typed at sign-in, until the PIN is chosen (then dropped).
    static PENDING_PASSWORD: Mutex<Option<Secret>> = Mutex::new(None);

    /// A notice for the sign-in card after the lock signed this desktop out.
    pub(crate) static SIGNED_OUT_BY_LOCK: GlobalSignal<bool> = Signal::global(|| false);

    pub(super) fn hold_for_pin(password: Secret) {
        *PENDING_PASSWORD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(password);
    }

    fn take_password() -> Option<Secret> {
        PENDING_PASSWORD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    fn server() -> &'static str {
        dioxus_fullstack::get_server_url()
    }

    /// The server's host, for "Stuart · nas.example.ts.net".
    fn server_label() -> String {
        let address = server();
        address
            .split_once("://")
            .map_or(address, |(_, rest)| rest)
            .trim_end_matches('/')
            .to_owned()
    }

    /// Runs `work` on a blocking thread (Argon2 takes ~0.3 s).
    async fn off_the_ui<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        tokio::task::spawn_blocking(work).await.ok()
    }

    /// Dots for the digits typed (at least 6, more as more are typed), and a keypad. The
    /// keyboard works too: digits, Backspace, and Enter.
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    fn Keypad(value: Signal<String>, busy: bool, on_submit: EventHandler<()>) -> Element {
        let mut value = value;
        let typed = value().chars().count();
        let shown = typed.max(domain::pin::PIN_MIN_DIGITS);
        let mut press = move |key: char| {
            if busy {
                return;
            }
            match key {
                '⌫' => {
                    value.write().pop();
                }
                '✓' => on_submit.call(()),
                digit if value.peek().chars().count() < domain::pin::PIN_MAX_DIGITS => {
                    value.write().push(digit);
                }
                _ => {}
            }
        };
        rsx! {
            div {
                class: "keypad",
                tabindex: "0",
                onmounted: move |event| async move {
                    let _ = event.data().set_focus(true).await;
                },
                onkeydown: move |event| {
                    let key = event.key();
                    if key == Key::Backspace {
                        press('⌫');
                    } else if key == Key::Enter {
                        press('✓');
                    } else if let Key::Character(text) = key
                        && let Some(digit) = text.chars().next().filter(char::is_ascii_digit)
                    {
                        press(digit);
                    }
                },
                div { class: "pin-dots", "aria-label": "{typed} digits",
                    for i in 0..shown {
                        span { key: "{i}", class: if i < typed { "pin-dot on" } else { "pin-dot" } }
                    }
                }
                div { class: "keypad-keys",
                    for key in ['1', '2', '3', '4', '5', '6', '7', '8', '9', '✓', '0', '⌫'] {
                        button {
                            key: "{key}",
                            class: if key == '✓' { "keypad-key ok" } else { "keypad-key" },
                            r#type: "button",
                            tabindex: "-1",
                            disabled: busy,
                            onclick: move |_| press(key),
                            {key.to_string()}
                        }
                    }
                }
            }
        }
    }

    /// "Choose a PIN" (option 1B): typed, then typed again; then the sign-in is sealed with it
    /// and the password typed at sign-in.
    #[component]
    pub(super) fn ChoosePin() -> Element {
        let auth = use_auth();
        let digits = use_signal(String::new);
        let mut first = use_signal(|| None::<Secret>);
        let mut problem = use_signal(|| None::<MessageId>);
        let mut busy = use_signal(|| false);
        let submit = move |()| {
            let mut digits = digits;
            let entered = digits();
            match first() {
                None => match check_new_pin(&entered) {
                    Ok(pin) => {
                        first.set(Some(pin));
                        problem.set(None);
                    }
                    Err(error) => problem.set(Some(error.message())),
                },
                Some(pin) if pin.expose() != entered.trim() => {
                    first.set(None);
                    problem.set(Some(messages::PIN_MISMATCH));
                }
                Some(pin) => {
                    let person = if let auth::AuthState::SignedIn(session) = auth() {
                        Some((session.display_name, session.username.as_str().to_owned()))
                    } else {
                        None
                    };
                    busy.set(true);
                    spawn(async move {
                        let saved = seal_and_save(pin, person).await;
                        busy.set(false);
                        match saved {
                            Ok(()) => *LOCK.write() = LockState::Open,
                            Err(message) => problem.set(Some(message)),
                        }
                    });
                }
            }
            digits.set(String::new());
        };
        let again = first().is_some();
        rsx! {
            div { class: "lock-screen",
                div { class: "lock-panel",
                    h1 { class: "lock-title", {tr(messages::PIN_CHOOSE_TITLE)} }
                    p { class: "lock-subtitle",
                        if again { {tr(messages::PIN_CHOOSE_AGAIN)} } else { {tr(messages::PIN_CHOOSE_HELP)} }
                    }
                    Keypad { value: digits, busy: busy(), on_submit: submit }
                    if let Some(message) = problem() {
                        p { class: "lock-problem", role: "alert", {tr(message)} }
                    }
                    if busy() {
                        p { class: "lock-subtitle", {tr(messages::PIN_SAVING)} }
                    }
                }
            }
        }
    }

    /// Seals the sign-in in memory with `pin` and the password typed at sign-in, and keeps it.
    async fn seal_and_save(pin: Secret, person: Option<(String, String)>) -> Result<(), MessageId> {
        let (Some(key), Some(token)) =
            (crate::client::device_key(), crate::client::session_token())
        else {
            return Err(messages::PIN_SAVE_FAILED);
        };
        let Some(password) = take_password() else {
            return Err(messages::PIN_SAVE_FAILED);
        };
        // The key of this computer's copy of the server's records (#075): new with each pairing.
        let database_key = crate::local_copy::new_key().map_err(|_| messages::PIN_SAVE_FAILED)?;
        let credential = Credential {
            secret_key: key,
            token,
            database_key,
        };
        let address = server().to_owned();
        let sealed =
            off_the_ui(move || SealedCredential::seal(&credential, &address, &pin, &password))
                .await
                .and_then(Result::ok);
        let Some(mut sealed) = sealed else {
            return Err(messages::PIN_SAVE_FAILED);
        };
        (sealed.person, sealed.username) = person.unzip();
        device_credential::save(server(), &sealed).map_err(|error| {
            tracing::warn!(target: log_area::AUTH, %error, "couldn't keep the sealed sign-in");
            messages::PIN_SAVE_FAILED
        })?;
        tracing::info!(target: log_area::AUTH, "PIN chosen; this desktop's sign-in is sealed");
        crate::local_copy::open(database_key).await;
        Ok(())
    }

    /// The words under the keypad for the tries left.
    fn tries_note(step: UnlockStep) -> Option<String> {
        match step {
            UnlockStep::PinOrPassword { pin_left, .. } if pin_left < domain::pin::PIN_TRIES => {
                Some(tr_with(
                    messages::LOCK_PIN_TRIES_LEFT,
                    &[("count", MessageArg::from(i64::from(pin_left)))],
                ))
            }
            UnlockStep::PasswordOnly { left } => Some(tr_with(
                messages::LOCK_PASSWORD_ONLY,
                &[("count", MessageArg::from(i64::from(left)))],
            )),
            UnlockStep::PinOrPassword { .. } | UnlockStep::SignOut => None,
        }
    }

    /// What one unlock attempt came to.
    enum Attempt {
        /// Opened; with the wrong tries before it (how many, and when the last was), to report.
        Opened(Credential, Option<(u32, i64)>),
        Wrong(UnlockStep),
        Gone,
    }

    /// Tries `secret` as the PIN (or the password) against the stored sign-in, counting a wrong
    /// try; at the limit, forgets the sign-in.
    async fn attempt(secret: Secret, as_password: bool) -> Attempt {
        let Some(mut sealed) = device_credential::load(server()) else {
            return Attempt::Gone;
        };
        let address = server().to_owned();
        let copy = sealed.clone();
        let opened = off_the_ui(move || {
            if as_password {
                copy.open_with_password(&address, &secret)
            } else {
                copy.open_with_pin(&address, &secret)
            }
        })
        .await
        .flatten();
        if let Some(credential) = opened {
            let before = (sealed.tries.total() > 0).then(|| {
                (
                    sealed.tries.total(),
                    sealed.last_wrong_ms.unwrap_or_default(),
                )
            });
            if before.is_some() {
                sealed.tries = domain::pin::WrongTries::default();
                sealed.last_wrong_ms = None;
                let _ = device_credential::save(server(), &sealed);
            }
            return Attempt::Opened(credential, before);
        }
        sealed.last_wrong_ms = Some(
            <crate::logging::SystemClock as domain::time::Clock>::now(&crate::logging::SystemClock)
                .as_millis(),
        );
        sealed.tries = if as_password {
            sealed.tries.wrong_password()
        } else {
            sealed.tries.wrong_pin()
        };
        let step = sealed.tries.step();
        tracing::info!(target: log_area::AUTH, wrong = sealed.tries.total(), "wrong unlock");
        if step == UnlockStep::SignOut {
            tracing::warn!(target: log_area::AUTH, "too many wrong unlocks; forgetting this desktop's sign-in");
            device_credential::forget(server());
        } else if let Err(error) = device_credential::save(server(), &sealed) {
            tracing::warn!(target: log_area::AUTH, %error, "couldn't record the wrong try");
        }
        Attempt::Wrong(step)
    }

    /// Tells the household Admin of the wrong tries before an unlock (phase-3.md 3d, #074).
    async fn report(before: Option<(u32, i64)>) {
        if let Some((count, last)) = before {
            let last = domain::time::UnixMillis::from_millis(last);
            match crate::api::report_failed_unlocks(count, last).await {
                Ok(domain::api::DevicesOutcome::Done) => {
                    tracing::info!(target: log_area::AUTH, count, "reported the wrong unlock tries");
                }
                Ok(outcome) => {
                    tracing::warn!(target: log_area::AUTH, ?outcome, "the server refused the wrong unlock tries' report");
                }
                Err(error) => {
                    tracing::warn!(target: log_area::AUTH, %error, "couldn't report the wrong unlock tries");
                }
            }
        }
    }

    /// Too many wrong tries (or the sign-in is gone): signs this desktop out, with a notice on
    /// the sign-in card.
    fn signed_out_by_lock(auth: Signal<auth::AuthState>) {
        crate::pairing::forget();
        *SIGNED_OUT_BY_LOCK.write() = true;
        *LOCK.write() = LockState::Open;
        // Not `spawn`: opening the gate unmounts the screen that called this, which would
        // cancel the task with it.
        dioxus::core::spawn_forever(async move { auth::refresh(auth).await });
    }

    /// Settings → This device: "Lock after" (kept in `settings.json`, per computer).
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn LockAfterRow() -> Element {
        let mut current = use_signal(|| {
            crate::data_folder::platform_data_folder()
                .map(|folder| crate::settings::load(&folder).lock_after)
                .unwrap_or_default()
        });
        rsx! {
            div { class: "setting-row",
                span { class: "setting-label", {tr(messages::SETTINGS_LOCK_AFTER)} }
                select {
                    class: "role-select",
                    // The value is the choice's index in LockAfter::ALL.
                    onchange: move |event| {
                        let Some(picked) = event
                            .value()
                            .parse::<usize>()
                            .ok()
                            .and_then(|index| LockAfter::ALL.get(index).copied())
                        else {
                            return;
                        };
                        current.set(picked);
                        let saved = crate::data_folder::platform_data_folder()
                            .map(|folder| crate::settings::update(&folder, |saved| saved.lock_after = picked));
                        if let Some(Err(error)) = saved {
                            tracing::warn!(target: log_area::APP, %error, "couldn't save 'lock after'");
                        }
                    },
                    for (index, choice) in LockAfter::ALL.iter().enumerate() {
                        option { key: "{index}", value: "{index}", selected: *choice == current(), {tr(choice.label())} }
                    }
                }
            }
        }
    }

    /// What changing the PIN came to.
    enum Changed {
        Done,
        /// The new PIN broke a rule, or the two didn't match: nothing was tried.
        Refused(MessageId),
        /// The current PIN or password was wrong (counted like a wrong unlock).
        Wrong(UnlockStep),
        Failed,
        SignedOut,
    }

    /// Changes the PIN: checks the new one, then opens the sign-in with `current` (the PIN, or
    /// the password), counting a wrong one as a wrong unlock, and seals it with the new PIN.
    async fn change_pin(current: Secret, as_password: bool, new: &str, again: &str) -> Changed {
        let pin = match check_new_pin(new) {
            Ok(pin) => pin,
            Err(error) => return Changed::Refused(error.message()),
        };
        if pin.expose() != again.trim() {
            return Changed::Refused(messages::PIN_MISMATCH);
        }
        let (credential, before) = match attempt(current, as_password).await {
            Attempt::Opened(credential, before) => (credential, before),
            Attempt::Wrong(UnlockStep::SignOut) | Attempt::Gone => return Changed::SignedOut,
            Attempt::Wrong(step) => return Changed::Wrong(step),
        };
        report(before).await;
        // Loaded again: the attempt cleared the wrong tries.
        let Some(sealed) = device_credential::load(server()) else {
            return Changed::Failed;
        };
        let address = server().to_owned();
        let renewed = off_the_ui(move || sealed.with_new_pin(&credential, &address, &pin))
            .await
            .and_then(Result::ok);
        let Some(renewed) = renewed else {
            return Changed::Failed;
        };
        match device_credential::save(server(), &renewed) {
            Ok(()) => {
                tracing::info!(target: log_area::AUTH, "PIN changed");
                Changed::Done
            }
            Err(error) => {
                tracing::warn!(target: log_area::AUTH, %error, "couldn't keep the sign-in sealed with the new PIN");
                Changed::Failed
            }
        }
    }

    /// Settings → This device: "Change PIN", which opens a small form: the current PIN (or the
    /// password), then the new one twice.
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn ChangePinRow() -> Element {
        let auth = use_auth();
        let mut open = use_signal(|| false);
        let mut use_password = use_signal(|| false);
        let mut current = use_signal(String::new);
        let mut new = use_signal(String::new);
        let mut again = use_signal(String::new);
        let mut busy = use_signal(|| false);
        let mut problem = use_signal(|| None::<String>);
        let mut done = use_signal(|| false);
        let mut close = move || {
            open.set(false);
            use_password.set(false);
            current.set(String::new());
            new.set(String::new());
            again.set(String::new());
            problem.set(None);
        };
        let submit = move |event: FormEvent| {
            event.prevent_default();
            let typed = Secret::new(current());
            let as_password = use_password();
            let (new_pin, new_again) = (new(), again());
            busy.set(true);
            spawn(async move {
                let result = change_pin(typed, as_password, &new_pin, &new_again).await;
                busy.set(false);
                match result {
                    Changed::Done => {
                        close();
                        done.set(true);
                    }
                    Changed::Refused(message) => {
                        new.set(String::new());
                        again.set(String::new());
                        problem.set(Some(tr(message)));
                    }
                    Changed::Wrong(step) => {
                        current.set(String::new());
                        if matches!(step, UnlockStep::PasswordOnly { .. }) {
                            use_password.set(true);
                        }
                        let wrong = tr(if as_password {
                            messages::LOCK_WRONG_PASSWORD
                        } else {
                            messages::LOCK_WRONG_PIN
                        });
                        problem.set(Some(match tries_note(step) {
                            Some(note) => format!("{wrong} {note}"),
                            None => wrong,
                        }));
                    }
                    Changed::Failed => problem.set(Some(tr(messages::PIN_SAVE_FAILED))),
                    Changed::SignedOut => signed_out_by_lock(auth),
                }
            });
        };
        // After 5 wrong PINs only the password opens the sign-in, here as on the lock screen.
        let only_password = open()
            && device_credential::load(server()).is_some_and(|sealed| {
                matches!(sealed.tries.step(), UnlockStep::PasswordOnly { .. })
            });
        let by_password = use_password() || only_password;
        rsx! {
            div { class: "setting-row",
                span { class: "setting-label", {tr(messages::SETTINGS_PIN)} }
                if !open() {
                    button {
                        class: "secondary-button",
                        r#type: "button",
                        onclick: move |_| {
                            done.set(false);
                            open.set(true);
                        },
                        {tr(messages::PIN_CHANGE)}
                    }
                }
                if done() {
                    span { class: "muted-note", role: "status", {tr(messages::PIN_CHANGED)} }
                }
            }
            if open() {
                form { class: "device-rename", onsubmit: submit,
                    label { class: "field",
                        span { class: "field-label",
                            if by_password { {tr(messages::PIN_CHANGE_PASSWORD)} } else { {tr(messages::PIN_CHANGE_CURRENT)} }
                        }
                        input {
                            class: "field-input",
                            r#type: "password",
                            inputmode: if by_password { "text" } else { "numeric" },
                            autocomplete: if by_password { "current-password" } else { "off" },
                            value: current(),
                            oninput: move |event| current.set(event.value()),
                        }
                    }
                    if !only_password {
                        button {
                            class: "link-button",
                            r#type: "button",
                            onclick: move |_| {
                                current.set(String::new());
                                use_password.set(!use_password());
                            },
                            if use_password() { {tr(messages::LOCK_USE_PIN)} } else { {tr(messages::LOCK_USE_PASSWORD)} }
                        }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::PIN_CHANGE_NEW)} }
                        input {
                            class: "field-input",
                            r#type: "password",
                            inputmode: "numeric",
                            autocomplete: "off",
                            maxlength: "{domain::pin::PIN_MAX_DIGITS}",
                            value: new(),
                            oninput: move |event| new.set(event.value()),
                        }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::PIN_CHANGE_NEW_AGAIN)} }
                        input {
                            class: "field-input",
                            r#type: "password",
                            inputmode: "numeric",
                            autocomplete: "off",
                            maxlength: "{domain::pin::PIN_MAX_DIGITS}",
                            value: again(),
                            oninput: move |event| again.set(event.value()),
                        }
                    }
                    if let Some(message) = problem() {
                        p { class: "form-error", role: "alert", {message} }
                    }
                    div { class: "form-buttons",
                        button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: move |_| close(), {tr(messages::ACTION_CANCEL)} }
                        button { class: "primary-button", r#type: "submit", disabled: busy(),
                            if busy() { {tr(messages::PIN_SAVING)} } else { {tr(messages::PIN_CHANGE)} }
                        }
                    }
                }
            }
        }
    }

    /// Locked (option 2B): the PIN on a keypad, or the password; and signing out.
    #[expect(
        unused_qualifications,
        reason = "false positive in rsx! event-handler expansion"
    )]
    #[component]
    pub(super) fn Locked() -> Element {
        let auth = use_auth();
        let stored = use_hook(|| device_credential::load(server()));
        let person = stored.as_ref().and_then(|sealed| sealed.person.clone());
        let initial = stored
            .as_ref()
            .map_or(UnlockStep::SignOut, |sealed| sealed.tries.step());
        let mut step = use_signal(|| initial);
        let mut use_password = use_signal(|| matches!(initial, UnlockStep::PasswordOnly { .. }));
        let digits = use_signal(String::new);
        let mut password = use_signal(String::new);
        let mut busy = use_signal(|| false);
        let mut wrong = use_signal(|| false);

        let mut finish = move |result: Attempt| match result {
            Attempt::Opened(credential, before) => {
                let database_key = credential.database_key;
                crate::client::set_session_token(Some(credential.token));
                crate::client::set_device_key(Some(credential.secret_key));
                tracing::info!(target: log_area::AUTH, "unlocked");
                *LOCK.write() = LockState::Open;
                // Not `spawn`: opening the gate unmounts this screen, and its tasks with it, so
                // the report was cancelled before it was sent.
                dioxus::core::spawn_forever(async move {
                    // The copy first: it needs no connection (#075).
                    crate::local_copy::open(database_key).await;
                    auth::refresh(auth).await;
                    report(before).await;
                });
            }
            Attempt::Wrong(UnlockStep::SignOut) | Attempt::Gone => signed_out_by_lock(auth),
            Attempt::Wrong(next) => {
                wrong.set(true);
                if matches!(next, UnlockStep::PasswordOnly { .. }) {
                    use_password.set(true);
                }
                step.set(next);
            }
        };
        let submit_pin = move |()| {
            let mut digits = digits;
            let pin = Secret::new(digits());
            digits.set(String::new());
            busy.set(true);
            spawn(async move {
                let result = attempt(pin, false).await;
                busy.set(false);
                finish(result);
            });
        };
        let submit_password = move |event: FormEvent| {
            event.prevent_default();
            let typed = Secret::new(password());
            password.set(String::new());
            busy.set(true);
            spawn(async move {
                let result = attempt(typed, true).await;
                busy.set(false);
                finish(result);
            });
        };
        let sign_out = move |_| {
            crate::pairing::forget();
            *LOCK.write() = LockState::Open;
            dioxus::core::spawn_forever(async move { auth::refresh(auth).await });
        };
        let who = match person {
            Some(name) => format!("{name} · {}", server_label()),
            None => server_label(),
        };
        let only_password = matches!(step(), UnlockStep::PasswordOnly { .. });
        rsx! {
            div { class: "lock-screen",
                div { class: "lock-panel",
                    h1 { class: "lock-title", {tr(messages::LOCK_TITLE)} }
                    p { class: "lock-subtitle", {who} }
                    if use_password() {
                        form { class: "lock-password", onsubmit: submit_password,
                            label { class: "field",
                                span { class: "field-label", {tr(messages::FIELD_PASSWORD)} }
                                input {
                                    class: "field-input",
                                    r#type: "password",
                                    autocomplete: "current-password",
                                    value: password(),
                                    oninput: move |event| password.set(event.value()),
                                    onmounted: move |event| async move {
                                        let _ = event.data().set_focus(true).await;
                                    },
                                }
                            }
                            button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::LOCK_UNLOCK)} }
                        }
                    } else {
                        Keypad { value: digits, busy: busy(), on_submit: submit_pin }
                    }
                    if wrong() && !use_password() {
                        p { class: "lock-problem", role: "alert", {tr(messages::LOCK_WRONG_PIN)} }
                    } else if wrong() {
                        p { class: "lock-problem", role: "alert", {tr(messages::LOCK_WRONG_PASSWORD)} }
                    }
                    if let Some(note) = tries_note(step()) {
                        p { class: "lock-subtitle", {note} }
                    }
                    if busy() {
                        p { class: "lock-subtitle", {tr(messages::LOCK_UNLOCKING)} }
                    }
                    div { class: "lock-links",
                        if !only_password {
                            button {
                                class: "link-button",
                                r#type: "button",
                                onclick: move |_| { wrong.set(false); use_password.set(!use_password()); },
                                if use_password() { {tr(messages::LOCK_USE_PIN)} } else { {tr(messages::LOCK_USE_PASSWORD)} }
                            }
                        }
                        button { class: "link-button muted", r#type: "button", onclick: sign_out, {tr(messages::LOCK_SIGN_OUT)} }
                    }
                }
            }
        }
    }
}

/// Settings → This device (phase-3.md 3e, option 3A): **Lock after** and **Change PIN**, on a
/// desktop with a sealed sign-in. Nothing on the web.
#[component]
pub(crate) fn LockSettings() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if crate::pairing::has_sealed_sign_in() {
        return rsx! {
            desktop::LockAfterRow {}
            desktop::ChangePinRow {}
        };
    }
    rsx! {}
}

/// On the sign-in card: "Signed out after too many wrong tries", once. Nothing on the web.
#[component]
pub(crate) fn LockSignedOutNotice() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if desktop::SIGNED_OUT_BY_LOCK() {
        return rsx! {
            p { class: "check-problem", role: "status", {crate::ui::tr(domain::i18n::messages::LOCK_SIGNED_OUT)} }
        };
    }
    rsx! {}
}
