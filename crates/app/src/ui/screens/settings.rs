//! Settings: display preferences (decisions #029, #048), this device's own settings, and the
//! devices signed in.
//!
//! Option 1 of `docs/ui-sketches/p6m1-settings-options.svg` (phase-6.md 1b): a column of
//! sections down the left, under **You**, **This device**, and **Household** (the household
//! Admin, in Admin mode only, #069). On a narrow window the column is a list that opens each
//! section, with ← Settings to go back. The last section open is remembered on the device.
//! Each preference is a row of buttons (option B of `p2m2-settings-options.svg`); changes save at
//! once, and the server answers with the preferences now in effect. What squawks are called
//! (#064) is the household's alone: buttons in its defaults, the word itself in yours.

use dioxus::prelude::*;
use domain::access::ActiveMode;
use domain::api::{PreferencesOutcome, PreferencesView};
use domain::due::DueStep;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::log_area;
use domain::preferences::{
    Choice, CurrencyCode, DateFormat, DistanceUnit, EconomyUnit, IdleSignOut, Originals,
    PreferenceChoices, Preferences, PreferencesPatch, PressureUnit, SquawkWord, VolumeUnit,
};
use domain::timers::Timers;

use super::due_view::{StepInputs, TypedWarnings, warnings_text};
use crate::api;
use crate::data;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::idle::{IdleContext, use_idle};
use crate::ui::screens::problem_from_error;
use crate::ui::theme::ThemeToggle;
use crate::ui::{set_squawk_word, tr, tr_with};

/// A setting shown as a row of buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonSetting {
    Distance,
    Volume,
    Economy,
    Pressure,
    DateFormat,
}

impl ButtonSetting {
    const ALL: [Self; 5] = [
        Self::Distance,
        Self::Volume,
        Self::Economy,
        Self::Pressure,
        Self::DateFormat,
    ];

    const fn label(self) -> MessageId {
        match self {
            Self::Distance => messages::SETTING_DISTANCE,
            Self::Volume => messages::SETTING_VOLUME,
            Self::Economy => messages::SETTING_ECONOMY,
            Self::Pressure => messages::SETTING_PRESSURE,
            Self::DateFormat => messages::SETTING_DATE_FORMAT,
        }
    }

    /// The choices, by index, with their labels.
    const fn choices(self) -> &'static [MessageId] {
        match self {
            Self::Distance => &[messages::UNIT_MILES, messages::UNIT_KILOMETERS],
            Self::Volume => &[
                messages::UNIT_US_GALLONS,
                messages::UNIT_IMPERIAL_GALLONS,
                messages::UNIT_LITERS,
            ],
            Self::Economy => &[
                messages::UNIT_MPG_US,
                messages::UNIT_MPG_UK,
                messages::UNIT_L_PER_100_KM,
                messages::UNIT_KM_PER_L,
            ],
            Self::Pressure => &[messages::UNIT_PSI, messages::UNIT_KPA, messages::UNIT_BAR],
            Self::DateFormat => &[
                messages::DATE_SAMPLE_DAY_MONTH_NAME_YEAR,
                messages::DATE_SAMPLE_MONTH_DAY_YEAR,
                messages::DATE_SAMPLE_DAY_MONTH_YEAR,
                messages::DATE_SAMPLE_YEAR_MONTH_DAY,
            ],
        }
    }

    /// The index of the chosen value, if chosen.
    fn chosen(self, choices: &PreferenceChoices) -> Option<usize> {
        let effective = Preferences::resolve(choices, &PreferenceChoices::default());
        let chosen = match self {
            Self::Distance => choices.distance.is_some(),
            Self::Volume => choices.volume.is_some(),
            Self::Economy => choices.economy.is_some(),
            Self::Pressure => choices.pressure.is_some(),
            Self::DateFormat => choices.date_format.is_some(),
        };
        chosen.then(|| self.index(effective))
    }

    /// The index of the value in effect.
    const fn index(self, preferences: Preferences) -> usize {
        match self {
            Self::Distance => match preferences.distance {
                DistanceUnit::Miles => 0,
                DistanceUnit::Kilometers => 1,
            },
            Self::Volume => match preferences.volume {
                VolumeUnit::UsGallons => 0,
                VolumeUnit::ImperialGallons => 1,
                VolumeUnit::Liters => 2,
            },
            Self::Economy => match preferences.economy {
                EconomyUnit::MpgUs => 0,
                EconomyUnit::MpgUk => 1,
                EconomyUnit::LitersPer100Km => 2,
                EconomyUnit::KmPerLiter => 3,
            },
            Self::Pressure => match preferences.pressure {
                PressureUnit::Psi => 0,
                PressureUnit::Kpa => 1,
                PressureUnit::Bar => 2,
            },
            Self::DateFormat => match preferences.date_format {
                DateFormat::DayMonthNameYear => 0,
                DateFormat::MonthDayYear => 1,
                DateFormat::DayMonthYear => 2,
                DateFormat::YearMonthDay => 3,
            },
        }
    }

    /// The change that sets choice `index`, or `None` to follow the default again.
    fn patch(self, index: Option<usize>) -> PreferencesPatch {
        /// The value at `index`, or "follow the default" for `None`.
        fn pick<T: Copy>(values: &[T], index: Option<usize>) -> Choice<T> {
            index
                .and_then(|i| values.get(i).copied())
                .map_or(Choice::Default, Choice::Set)
        }
        let mut patch = PreferencesPatch::default();
        match self {
            Self::Distance => {
                patch.distance = Some(pick(
                    &[DistanceUnit::Miles, DistanceUnit::Kilometers],
                    index,
                ));
            }
            Self::Volume => {
                patch.volume = Some(pick(
                    &[
                        VolumeUnit::UsGallons,
                        VolumeUnit::ImperialGallons,
                        VolumeUnit::Liters,
                    ],
                    index,
                ));
            }
            Self::Economy => {
                patch.economy = Some(pick(
                    &[
                        EconomyUnit::MpgUs,
                        EconomyUnit::MpgUk,
                        EconomyUnit::LitersPer100Km,
                        EconomyUnit::KmPerLiter,
                    ],
                    index,
                ));
            }
            Self::Pressure => {
                patch.pressure = Some(pick(
                    &[PressureUnit::Psi, PressureUnit::Kpa, PressureUnit::Bar],
                    index,
                ));
            }
            Self::DateFormat => {
                patch.date_format = Some(pick(
                    &[
                        DateFormat::DayMonthNameYear,
                        DateFormat::MonthDayYear,
                        DateFormat::DayMonthYear,
                        DateFormat::YearMonthDay,
                    ],
                    index,
                ));
            }
        }
        patch
    }
}

/// A section of Settings: option 1 of `docs/ui-sketches/p6m1-settings-options.svg`, a column of
/// sections down the left (a list that opens each one on a narrow window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Distance, volume, economy, pressure, date format, currency, language.
    Units,
    /// What squawks are called, the due warnings, when due messages start.
    Vehicles,
    /// Keeping originals (a Maintainer's, #036).
    Photos,
    /// When the app signs out by itself (#049).
    SignIn,
    /// Theme, and keep the window on top (a desktop's).
    Appearance,
    /// Lock after and Change PIN (a desktop with a PIN).
    Lock,
    /// Where you're signed in (phase-3.md 2e).
    Devices,
    /// The household's labor rate and shops (#089), for Maintainers and Admins in that mode.
    LaborRates,
    /// The household's defaults, for the household Admin in Admin mode (#069).
    Household,
}

/// The headings in the column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Group {
    You,
    ThisDevice,
    Household,
}

impl Group {
    const fn label(self) -> MessageId {
        match self {
            Self::You => messages::SETTINGS_TAB_YOU,
            Self::ThisDevice => messages::SETTINGS_THIS_DEVICE,
            Self::Household => messages::SETTINGS_TAB_HOUSEHOLD,
        }
    }
}

impl Section {
    const ALL: [Self; 9] = [
        Self::Units,
        Self::Vehicles,
        Self::Photos,
        Self::SignIn,
        Self::Appearance,
        Self::Lock,
        Self::Devices,
        Self::LaborRates,
        Self::Household,
    ];

    const fn label(self) -> MessageId {
        match self {
            Self::Units => messages::SETTINGS_SECTION_UNITS,
            Self::Vehicles => messages::SETTINGS_SECTION_VEHICLES,
            Self::Photos => messages::SETTINGS_SECTION_PHOTOS,
            Self::SignIn => messages::SETTINGS_SIGN_IN,
            Self::Appearance => messages::SETTINGS_SECTION_APPEARANCE,
            Self::Lock => messages::SETTINGS_SECTION_LOCK,
            Self::Devices => messages::SETTINGS_TAB_DEVICES,
            Self::LaborRates => messages::SETTINGS_SECTION_LABOR_RATES,
            Self::Household => messages::SETTINGS_SECTION_HOUSEHOLD,
        }
    }

    const fn group(self) -> Group {
        match self {
            Self::Units | Self::Vehicles | Self::Photos | Self::SignIn => Group::You,
            Self::Appearance | Self::Lock | Self::Devices => Group::ThisDevice,
            Self::LaborRates | Self::Household => Group::Household,
        }
    }

    /// Its name on this device's memory.
    const fn key(self) -> &'static str {
        match self {
            Self::Units => "units",
            Self::Vehicles => "vehicles",
            Self::Photos => "photos",
            Self::SignIn => "sign-in",
            Self::Appearance => "appearance",
            Self::Lock => "lock",
            Self::Devices => "devices",
            Self::LaborRates => "labor-rates",
            Self::Household => "household",
        }
    }

    fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|section| section.key() == key)
    }
}

/// Which sections this person sees, here and now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Offered {
    /// In Maintainer mode or above: keeping originals (#036, #069) and the labor rates (#089).
    maintainer: bool,
    /// On a desktop with a PIN.
    lock: bool,
    /// The household Admin, in Admin mode (#069).
    household: bool,
}

impl Offered {
    const fn includes(self, section: Section) -> bool {
        match section {
            Section::Photos | Section::LaborRates => self.maintainer,
            Section::Lock => self.lock,
            Section::Household => self.household,
            Section::Units
            | Section::Vehicles
            | Section::SignIn
            | Section::Appearance
            | Section::Devices => true,
        }
    }

    /// The sections shown, in the column's order.
    fn sections(self) -> impl Iterator<Item = Section> {
        Section::ALL
            .into_iter()
            .filter(move |section| self.includes(*section))
    }

    /// The sections shown, each with whether it starts its group (its heading goes above it).
    fn with_headings(self) -> Vec<(Section, bool)> {
        let mut previous = None;
        self.sections()
            .map(|section| {
                let first = previous != Some(section.group());
                previous = Some(section.group());
                (section, first)
            })
            .collect()
    }

    /// The section to show: the chosen one if it's offered, otherwise the first.
    fn showing(self, chosen: Option<Section>) -> Section {
        chosen
            .filter(|section| self.includes(*section))
            .unwrap_or(Section::Units)
    }
}

/// The device memory's key for the section last open.
const SECTION_KEY: &str = "settingsSection";

/// The Settings screen.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Settings() -> Element {
    let auth = use_auth();
    let offered = Offered {
        maintainer: matches!(auth(), AuthState::SignedIn(session) if session.mode >= ActiveMode::Maintainer),
        lock: crate::ui::lock::has_lock(),
        household: matches!(auth(), AuthState::SignedIn(session) if session.mode == ActiveMode::Admin),
    };
    // Test timers (#054) show each sign-out choice's real length.
    let timers = match auth() {
        AuthState::SignedIn(session) => session.timers,
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::Unreachable(_) => Timers::NORMAL,
    };
    let mut chosen = use_signal(|| None::<Section>);
    // On a narrow window, whether a section is open (otherwise the list is).
    let mut opened = use_signal(|| false);
    let view = use_signal(|| None::<PreferencesView>);
    let error = use_signal(|| None::<String>);
    let idle = use_idle();

    // Loads after mounting, never during server rendering.
    use_effect(move || {
        spawn(async move {
            apply(data::get_preferences().await, view, error, idle);
        });
    });
    use_effect(move || {
        spawn(async move {
            if let Some(section) = crate::ui::device_memory::recall(SECTION_KEY)
                .await
                .as_deref()
                .and_then(Section::from_key)
            {
                chosen.set(Some(section));
            }
        });
    });

    let shown = offered.showing(chosen());
    let save = move |patch: PreferencesPatch| {
        let household = offered.household && shown == Section::Household;
        spawn(async move {
            let result = if household {
                api::set_household_preferences(patch).await
            } else {
                crate::data::set_my_preferences(patch).await
            };
            apply(result, view, error, idle);
        });
    };
    let mut open = move |section: Section| {
        chosen.set(Some(section));
        opened.set(true);
        crate::ui::device_memory::remember(SECTION_KEY, section.key());
    };

    rsx! {
        div { class: if opened() { "settings-split section-open" } else { "settings-split" },
            nav { class: "settings-nav", "aria-label": tr(messages::NAV_SETTINGS),
                for (section, first_of_group) in offered.with_headings() {
                    div { key: "{section.key()}", class: "settings-nav-entry",
                    if first_of_group {
                        p { class: "settings-group", {tr(section.group().label())} }
                    }
                    button {
                        class: if section == shown { "settings-nav-item on" } else { "settings-nav-item" },
                        "aria-current": if section == shown { "page" } else { "false" },
                        onclick: move |_| open(section),
                        span { {tr(section.label())} }
                        span { class: "settings-nav-chevron", "aria-hidden": "true", "›" }
                    }
                    }
                }
            }
            div { class: "settings",
                button {
                    class: "link-button settings-back",
                    onclick: move |_| opened.set(false),
                    {tr(messages::SETTINGS_BACK)}
                }
                h2 { class: "settings-title", {tr(shown.label())} }
                SectionBody { section: shown, view: view(), error: error(), timers, on_save: save }
            }
        }
    }
}

/// One section's rows. The preference rows wait for the server's answer; the device's own
/// rows (theme, window, lock) and Devices don't.
#[component]
fn SectionBody(
    section: Section,
    view: Option<PreferencesView>,
    error: Option<String>,
    timers: Timers,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    // Admin mode is above Maintainer, so the household's rows include keeping originals.
    let household_tab = section == Section::Household;
    match section {
        Section::Appearance => rsx! {
            div { class: "setting-row",
                span { class: "setting-label", {tr(messages::SETTINGS_THEME)} }
                ThemeToggle {}
            }
            KeepOnTop {}
        },
        Section::Lock => rsx! { crate::ui::lock::LockSettings {} },
        Section::Devices => rsx! { super::devices::DevicesTab {} },
        Section::LaborRates => rsx! { super::labor_rates::LaborRates {} },
        Section::Units
        | Section::Vehicles
        | Section::Photos
        | Section::SignIn
        | Section::Household => rsx! {
            if let Some(message) = error {
                p { class: "form-error", role: "alert", {message} }
            }
            if let Some(current) = view {
                p { class: "muted-note",
                    if household_tab { {tr(messages::SETTINGS_HOUSEHOLD_HELP)} } else { {tr(messages::SETTINGS_YOU_HELP)} }
                }
                if household_tab {
                    h3 { class: "section-heading", {tr(messages::SETTINGS_SECTION_UNITS)} }
                }
                if household_tab || section == Section::Units {
                    for setting in ButtonSetting::ALL {
                        SettingButtons { key: "{setting:?}", setting, view: current, household_tab, on_save }
                    }
                    CurrencyRow { view: current, household_tab, on_save }
                    LanguageRow {}
                }
                if household_tab {
                    h3 { class: "section-heading", {tr(messages::SETTINGS_SECTION_VEHICLES)} }
                }
                if household_tab || section == Section::Vehicles {
                    SquawkWordRow { view: current, household_tab, on_save }
                    DueWarningsRow { view: current, household_tab, on_save }
                    DueMessagesRow { view: current, household_tab, on_save }
                }
                if household_tab {
                    h3 { class: "section-heading", {tr(messages::SETTINGS_SECTION_PHOTOS)} }
                }
                if household_tab || section == Section::Photos {
                    OriginalsRow { view: current, household_tab, on_save }
                }
                if household_tab {
                    h3 { class: "section-heading", {tr(messages::SETTINGS_SIGN_IN)} }
                }
                if household_tab || section == Section::SignIn {
                    IdleSignOutRow { view: current, household_tab, timers, on_save }
                }
                if household_tab {
                    OwnNamesRow { view: current, on_save }
                }
            }
        },
    }
}

/// Shows the server's answer, or what went wrong; tells the idle watcher the sign-out setting.
fn apply(
    result: Result<PreferencesOutcome, ServerFnError>,
    mut view: Signal<Option<PreferencesView>>,
    mut error: Signal<Option<String>>,
    mut idle: IdleContext,
) {
    match result {
        Ok(PreferencesOutcome::Preferences { view: next }) => {
            tracing::debug!(target: log_area::UI, effective = ?next.effective, "preferences loaded");
            error.set(None);
            idle.sign_out.set(next.effective.idle_sign_out);
            set_squawk_word(next.effective.squawk_word);
            crate::ui::set_home_currency(next.effective.currency);
            view.set(Some(next));
        }
        Ok(PreferencesOutcome::NotAllowed) => {
            error.set(Some(tr(messages::SETTINGS_HOUSEHOLD_ADMIN_ONLY)));
        }
        Ok(PreferencesOutcome::BadDueWarnings) => error.set(Some(tr(messages::DUE_BAD_WARNINGS))),
        Ok(PreferencesOutcome::Unavailable) => error.set(Some(tr(messages::SOMETHING_WENT_WRONG))),
        Err(failure) => error.set(Some(tr(problem_from_error(&failure).message()))),
    }
}

/// One setting: its buttons, and on the You tab the "Use household default" switch.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SettingButtons(
    setting: ButtonSetting,
    view: PreferencesView,
    household_tab: bool,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let household_effective = Preferences::resolve(&view.household, &PreferenceChoices::default());
    // On the You tab, "following" means no choice of mine; the buttons then show the
    // household's value, and tapping one makes it my own choice.
    let (selected, following) = if household_tab {
        (
            setting
                .chosen(&view.household)
                .unwrap_or_else(|| setting.index(household_effective)),
            false,
        )
    } else {
        match setting.chosen(&view.mine) {
            Some(index) => (index, false),
            None => (setting.index(household_effective), true),
        }
    };
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(setting.label())} }
            div { class: "choice-buttons",
                for (index, label) in setting.choices().iter().enumerate() {
                    button {
                        key: "{index}",
                        class: match (index == selected, following) {
                            (true, false) => "choice on",
                            (true, true) => "choice following",
                            (false, _) => "choice",
                        },
                        "aria-pressed": index == selected,
                        onclick: move |_| on_save.call(setting.patch(Some(index))),
                        {tr(*label)}
                    }
                }
            }
            if !household_tab {
                label { class: "switch",
                    input {
                        r#type: "checkbox",
                        checked: following,
                        // Turning it on clears my choice; turning it off keeps the shown value
                        // as my own.
                        onchange: move |_| on_save.call(setting.patch(if following { Some(selected) } else { None })),
                    }
                    span { {tr(messages::SETTINGS_USE_HOUSEHOLD)} }
                }
            }
        }
    }
}

/// The currency: a list (too many for buttons), with "Use household default" on the You tab.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn CurrencyRow(
    view: PreferencesView,
    household_tab: bool,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let household = view.household.currency.unwrap_or_default();
    let (current, following) = if household_tab {
        (household, false)
    } else {
        view.mine
            .currency
            .map_or((household, true), |mine| (mine, false))
    };
    // The common ones, plus the current one if it's not among them.
    let mut codes: Vec<CurrencyCode> = CurrencyCode::COMMON.to_vec();
    if !codes.contains(&current) {
        codes.push(current);
    }
    let set = move |code: Option<CurrencyCode>| {
        on_save.call(PreferencesPatch {
            currency: Some(code.map_or(Choice::Default, Choice::Set)),
            ..PreferencesPatch::default()
        });
    };
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_CURRENCY)} }
            select {
                class: "role-select",
                onchange: move |event| set(CurrencyCode::parse(&event.value()).ok()),
                for code in codes {
                    option { key: "{code}", value: "{code}", selected: code == current, "{code}" }
                }
            }
            if !household_tab {
                label { class: "switch",
                    input {
                        r#type: "checkbox",
                        checked: following,
                        onchange: move |_| set(if following { Some(current) } else { None }),
                    }
                    span { {tr(messages::SETTINGS_USE_HOUSEHOLD)} }
                }
            }
        }
    }
}

/// The sign-out choices, in list order, with their labels.
const IDLE_SIGN_OUT_CHOICES: [(IdleSignOut, MessageId); 5] = [
    (IdleSignOut::OneHour, messages::IDLE_SIGN_OUT_ONE_HOUR),
    (IdleSignOut::FourHours, messages::IDLE_SIGN_OUT_FOUR_HOURS),
    (IdleSignOut::EightHours, messages::IDLE_SIGN_OUT_EIGHT_HOURS),
    (
        IdleSignOut::TwentyFourHours,
        messages::IDLE_SIGN_OUT_TWENTY_FOUR_HOURS,
    ),
    (IdleSignOut::Never, messages::IDLE_SIGN_OUT_NEVER),
];

/// "Sign out when idle": a list, with "Use household default" on the You tab (#049).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn IdleSignOutRow(
    view: PreferencesView,
    household_tab: bool,
    timers: Timers,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let household = view.household.idle_sign_out.unwrap_or_default();
    let (current, following) = if household_tab {
        (household, false)
    } else {
        view.mine
            .idle_sign_out
            .map_or((household, true), |mine| (mine, false))
    };
    let set = move |choice: Option<IdleSignOut>| {
        on_save.call(PreferencesPatch {
            idle_sign_out: Some(choice.map_or(Choice::Default, Choice::Set)),
            ..PreferencesPatch::default()
        });
    };
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_IDLE_SIGN_OUT)} }
            select {
                class: "role-select",
                // The value is the choice's index in IDLE_SIGN_OUT_CHOICES.
                onchange: move |event| {
                    let picked = event
                        .value()
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| IDLE_SIGN_OUT_CHOICES.get(index))
                        .map(|(choice, _)| *choice);
                    if picked.is_some() {
                        set(picked);
                    }
                },
                for (index, (choice, label)) in IDLE_SIGN_OUT_CHOICES.iter().enumerate() {
                    option { key: "{index}", value: "{index}", selected: *choice == current, {sign_out_label(*choice, *label, timers)} }
                }
            }
            if !household_tab {
                label { class: "switch",
                    input {
                        r#type: "checkbox",
                        checked: following,
                        onchange: move |_| set(if following { Some(current) } else { None }),
                    }
                    span { {tr(messages::SETTINGS_USE_HOUSEHOLD)} }
                }
            }
        }
    }
}

/// A sign-out choice's label; with test timers, also its test length, e.g.
/// "After 1 hour (test: 2 min)".
fn sign_out_label(choice: IdleSignOut, label: MessageId, timers: Timers) -> String {
    let test_minutes = timers
        .test_minutes()
        .and_then(|_| timers.sign_out_after_millis(choice))
        .map(|millis| millis / 60_000);
    match test_minutes {
        Some(minutes) => tr_with(
            messages::IDLE_SIGN_OUT_TEST,
            &[
                ("label", MessageArg::from(tr(label))),
                ("minutes", MessageArg::from(minutes)),
            ],
        ),
        None => tr(label),
    }
}

/// What squawks are called (#064): the household's word, as buttons for its Admin, and as
/// plain text (with who chooses it) on the You tab.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SquawkWordRow(
    view: PreferencesView,
    household_tab: bool,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let current = view.effective.squawk_word;
    let name = |word: SquawkWord| {
        domain::i18n::Translator::english().tr_for(word, messages::SQUAWK_WORD_CHOICE)
    };
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_SQUAWK_WORD)} }
            if household_tab {
                div { class: "choice-buttons",
                    for word in SquawkWord::ALL {
                        button {
                            key: "{word:?}",
                            class: if word == current { "choice on" } else { "choice" },
                            "aria-pressed": word == current,
                            onclick: move |_| on_save.call(squawk_word_patch(word)),
                            {name(word)}
                        }
                    }
                }
            } else {
                span { class: "setting-value", {name(current)} }
                p { class: "muted-note", {tr(messages::SETTING_SQUAWK_WORD_HOUSEHOLD)} }
            }
        }
    }
}

/// When things come due (#082): the household's Admin sets the steps on the Household tab;
/// everyone sees them in words.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DueWarningsRow(
    view: PreferencesView,
    household_tab: bool,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let current = view.effective.due_warnings;
    let typed = TypedWarnings::of(&current);
    let interval = use_signal(|| typed.interval.clone());
    let days = use_signal(|| typed.days.clone());
    let mut problem = use_signal(|| None::<MessageId>);
    let patch = |choice| PreferencesPatch {
        due_warnings: Some(choice),
        ..PreferencesPatch::default()
    };
    rsx! {
        div { class: "setting-row due-warnings-row",
            span { class: "setting-label", {tr(messages::SETTING_DUE_WARNINGS)} }
            if household_tab {
                StepInputs { label: messages::DUE_PERCENT_LEFT, values: interval, unit: messages::DUE_PERCENT }
                StepInputs { label: messages::DUE_DAYS_BEFORE, values: days, unit: messages::DUE_DAYS }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "check-row",
                    button {
                        class: "secondary-button",
                        r#type: "button",
                        onclick: move |_| {
                            let typed = TypedWarnings { interval: interval(), days: days() };
                            match typed.parse() {
                                Ok(warnings) => {
                                    problem.set(None);
                                    on_save.call(patch(Choice::Set(warnings)));
                                }
                                Err(message) => problem.set(Some(message)),
                            }
                        },
                        {tr(messages::ACTION_SAVE)}
                    }
                    button {
                        class: "link-button",
                        r#type: "button",
                        onclick: move |_| on_save.call(patch(Choice::Default)),
                        {tr(messages::DUE_BUILT_IN)}
                    }
                }
            } else {
                span { class: "setting-value", {warnings_text(&current)} }
                p { class: "muted-note", {tr(messages::SETTING_DUE_WARNINGS_HOUSEHOLD)} }
            }
        }
    }
}

/// The steps due messages may start at (#083), in order.
const DUE_MESSAGE_STARTS: [DueStep; 3] =
    [DueStep::ComingUp, DueStep::DueSoon, DueStep::DueVerySoon];

/// When what's due starts sending messages (#083; sketch `p5m3-messages-options.svg`, 4A): the
/// household's Admin chooses on the Household tab; everyone sees it in words.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DueMessagesRow(
    view: PreferencesView,
    household_tab: bool,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let current = view.effective.due_messages_from;
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_DUE_MESSAGES)} }
            if household_tab {
                select {
                    class: "role-select",
                    // The value is the step's index in DUE_MESSAGE_STARTS.
                    onchange: move |event| {
                        let picked = event
                            .value()
                            .parse::<usize>()
                            .ok()
                            .and_then(|index| DUE_MESSAGE_STARTS.get(index).copied());
                        if let Some(step) = picked {
                            on_save.call(PreferencesPatch {
                                due_messages_from: Some(Choice::Set(step)),
                                ..PreferencesPatch::default()
                            });
                        }
                    },
                    for (index, step) in DUE_MESSAGE_STARTS.into_iter().enumerate() {
                        option { key: "{index}", value: "{index}", selected: step == current, {tr(step.label())} }
                    }
                }
                p { class: "muted-note", {tr(messages::SETTING_DUE_MESSAGES_NOTE)} }
            } else {
                span { class: "setting-value", {tr(current.label())} }
                p { class: "muted-note", {tr(messages::SETTING_SQUAWK_WORD_HOUSEHOLD)} }
            }
        }
    }
}

/// The change that makes `word` the household's.
fn squawk_word_patch(word: SquawkWord) -> PreferencesPatch {
    PreferencesPatch {
        squawk_word: Some(Choice::Set(word)),
        ..PreferencesPatch::default()
    }
}

/// Whether Maintainers and Admins may keep a photo's original (#036): the household's Admin
/// chooses, for everyone.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn OriginalsRow(
    view: PreferencesView,
    household_tab: bool,
    on_save: EventHandler<PreferencesPatch>,
) -> Element {
    let current = view.effective.originals;
    let name = |choice: Originals| {
        tr(match choice {
            Originals::Allowed => messages::SETTING_ORIGINALS_ALLOWED,
            Originals::Off => messages::SETTING_ORIGINALS_OFF,
        })
    };
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_ORIGINALS)} }
            if household_tab {
                div { class: "choice-buttons",
                    for choice in [Originals::Allowed, Originals::Off] {
                        button {
                            key: "{choice:?}",
                            class: if choice == current { "choice on" } else { "choice" },
                            "aria-pressed": choice == current,
                            onclick: move |_| on_save.call(originals_patch(choice)),
                            {name(choice)}
                        }
                    }
                }
                p { class: "muted-note", {tr(messages::SETTING_ORIGINALS_NOTE)} }
            } else {
                span { class: "setting-value", {name(current)} }
                p { class: "muted-note", {tr(messages::SETTING_SQUAWK_WORD_HOUSEHOLD)} }
            }
        }
    }
}

/// Household: whether each person may change their own name and username (#091).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn OwnNamesRow(view: PreferencesView, on_save: EventHandler<PreferencesPatch>) -> Element {
    let current = view.effective.own_names;
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_OWN_NAMES)} }
            div { class: "choice-buttons",
                for (choice, label) in [(false, messages::SETTING_OWN_NAMES_OFF), (true, messages::SETTING_OWN_NAMES_ON)] {
                    button {
                        key: "{choice}",
                        class: if choice == current { "choice on" } else { "choice" },
                        "aria-pressed": choice == current,
                        onclick: move |_| on_save.call(PreferencesPatch {
                            own_names: Some(Choice::Set(choice)),
                            ..PreferencesPatch::default()
                        }),
                        {tr(label)}
                    }
                }
            }
            p { class: "muted-note", {tr(messages::SETTING_OWN_NAMES_NOTE)} }
        }
    }
}

/// The change that sets whether originals may be kept.
fn originals_patch(choice: Originals) -> PreferencesPatch {
    PreferencesPatch {
        originals: Some(Choice::Set(choice)),
        ..PreferencesPatch::default()
    }
}

/// The language: English (US) only so far, so nothing to change.
#[component]
fn LanguageRow() -> Element {
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTING_LANGUAGE)} }
            span { class: "setting-value", {tr(messages::LANGUAGE_ENGLISH_US)} }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::command::Patch;

    use super::*;

    const EVERYTHING: Offered = Offered {
        maintainer: true,
        lock: true,
        household: true,
    };
    const DRIVER_ON_THE_WEB: Offered = Offered {
        maintainer: false,
        lock: false,
        household: false,
    };

    #[test]
    fn each_section_has_its_own_key_and_comes_back_from_it() {
        for section in Section::ALL {
            assert_eq!(Section::from_key(section.key()), Some(section));
        }
        assert_eq!(Section::from_key("nonsense"), None);
    }

    #[test]
    fn each_group_heading_shows_once_since_its_sections_are_together() {
        let groups: Vec<Group> = Section::ALL.iter().map(|section| section.group()).collect();
        let mut changes = groups.windows(2).filter(|pair| pair[0] != pair[1]).count();
        changes += 1;
        assert_eq!(changes, 3);
    }

    #[test]
    fn a_driver_on_the_web_sees_no_photos_lock_or_household() {
        let shown: Vec<Section> = DRIVER_ON_THE_WEB.sections().collect();
        assert_eq!(
            shown,
            [
                Section::Units,
                Section::Vehicles,
                Section::SignIn,
                Section::Appearance,
                Section::Devices
            ]
        );
        assert_eq!(EVERYTHING.sections().count(), Section::ALL.len());
        let headings: Vec<Section> = DRIVER_ON_THE_WEB
            .with_headings()
            .into_iter()
            .filter_map(|(section, first)| first.then_some(section))
            .collect();
        assert_eq!(headings, [Section::Units, Section::Appearance]);
    }

    #[test]
    fn the_remembered_section_opens_only_while_it_is_offered() {
        assert_eq!(EVERYTHING.showing(None), Section::Units);
        assert_eq!(
            EVERYTHING.showing(Some(Section::Household)),
            Section::Household
        );
        assert_eq!(
            DRIVER_ON_THE_WEB.showing(Some(Section::Household)),
            Section::Units
        );
        assert_eq!(
            DRIVER_ON_THE_WEB.showing(Some(Section::Lock)),
            Section::Units
        );
        assert_eq!(
            DRIVER_ON_THE_WEB.showing(Some(Section::Devices)),
            Section::Devices
        );
    }

    #[test]
    fn a_maintainer_sees_the_labor_rates_under_household() {
        let maintainer = Offered {
            maintainer: true,
            ..DRIVER_ON_THE_WEB
        };
        let headings: Vec<Section> = maintainer
            .with_headings()
            .into_iter()
            .filter_map(|(section, first)| first.then_some(section))
            .collect();
        assert_eq!(
            headings,
            [Section::Units, Section::Appearance, Section::LaborRates]
        );
        assert!(!maintainer.includes(Section::Household));
        assert_eq!(
            DRIVER_ON_THE_WEB.showing(Some(Section::LaborRates)),
            Section::Units
        );
    }

    #[test]
    fn every_button_sets_the_value_it_shows() {
        for setting in ButtonSetting::ALL {
            for index in 0..setting.choices().len() {
                // Apply the button's change to empty choices, then read it back.
                let patch = setting.patch(Some(index));
                let choices = PreferenceChoices {
                    distance: patch.distance.and_then(Choice::value),
                    volume: patch.volume.and_then(Choice::value),
                    economy: patch.economy.and_then(Choice::value),
                    pressure: patch.pressure.and_then(Choice::value),
                    date_format: patch.date_format.and_then(Choice::value),
                    ..PreferenceChoices::default()
                };
                assert_eq!(setting.chosen(&choices), Some(index), "{setting:?} {index}");
            }
        }
    }

    #[test]
    fn choosing_originals_changes_only_that() {
        for choice in [Originals::Allowed, Originals::Off] {
            let patch = originals_patch(choice);
            assert_eq!(patch.changed_fields(), vec!["originals"]);
            assert_eq!(patch.originals, Some(Choice::Set(choice)));
        }
    }

    #[test]
    fn choosing_a_squawk_word_changes_only_that() {
        for word in SquawkWord::ALL {
            let patch = squawk_word_patch(word);
            assert_eq!(patch.changed_fields(), vec!["squawk_word"]);
            assert_eq!(patch.squawk_word, Some(Choice::Set(word)));
        }
    }

    #[test]
    fn following_the_default_clears_the_choice() {
        for setting in ButtonSetting::ALL {
            let patch = setting.patch(None);
            assert_eq!(
                patch.changed_fields().len(),
                1,
                "{setting:?} changes one setting"
            );
            assert!(setting.chosen(&PreferenceChoices::default()).is_none());
        }
    }

    #[test]
    fn the_sign_out_list_has_every_choice_once() {
        let listed: Vec<IdleSignOut> = IDLE_SIGN_OUT_CHOICES
            .iter()
            .map(|(choice, _)| *choice)
            .collect();
        assert_eq!(listed.len(), 5);
        assert!(listed.contains(&IdleSignOut::default()));
        assert!(listed.contains(&IdleSignOut::Never));
        for (index, choice) in listed.iter().enumerate() {
            assert!(!listed[index + 1..].contains(choice), "{choice:?} twice");
        }
    }

    #[test]
    fn test_timers_show_each_sign_out_choice_in_minutes() {
        let test = Timers::test(2).unwrap();
        assert_eq!(
            sign_out_label(IdleSignOut::OneHour, messages::IDLE_SIGN_OUT_ONE_HOUR, test),
            "After 1 hour (test: 2 min)"
        );
        assert_eq!(
            sign_out_label(IdleSignOut::Never, messages::IDLE_SIGN_OUT_NEVER, test),
            "Never"
        );
        assert_eq!(
            sign_out_label(
                IdleSignOut::OneHour,
                messages::IDLE_SIGN_OUT_ONE_HOUR,
                Timers::NORMAL
            ),
            "After 1 hour"
        );
    }

    #[test]
    fn the_defaults_are_the_first_choice_of_each_setting() {
        for setting in ButtonSetting::ALL {
            assert_eq!(setting.index(Preferences::default()), 0, "{setting:?}");
        }
    }
}

/// The desktop's "Keep window on top" row (phase-3.md 1b).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[component]
fn KeepOnTop() -> Element {
    rsx! { crate::ui::desktop_window::KeepOnTopRow {} }
}

/// Nothing on the web: a browser's window isn't the app's.
#[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
#[component]
fn KeepOnTop() -> Element {
    rsx! {}
}
