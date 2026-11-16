//! What's due, on screen (Phase 5 milestone 2; decision #082; sketch
//! `docs/ui-sketches/p5m2-due-options.svg`, 1A, 2A, 3A, 4A): the step's chip and colour, and
//! what's left in words ("in 310 mi or Oil life 9 % · ~19 Nov", "by 12 Dec", "was due 2 Nov").
//!
//! Colour is never the only sign: every chip says its step ("Coming up", "Due soon",
//! "Due very soon", "⚠ Overdue").

use dioxus::prelude::*;
use domain::api::{AssetView, DueEntry, DueOutcome, DueSubject};
use domain::dates::CalendarDate;
use domain::due::{
    DaySteps, Due, DueDefaults, DueStep, IntervalSteps, LastDoneBefore, Left, RuleDue,
};
use domain::event::Reading;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::meter::{Meter, MeterId};
use domain::preferences::Preferences;
use domain::quantity::{parse_reading, reading_value, show_reading};
use domain::requirement::Requirement;

use super::documents::{date_text, type_name};
use crate::data;
use crate::ui::routes::Route;
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};

/// How many items the Overview's "Coming up" shows (#082).
const COMING_UP: usize = 5;

/// The class that colours a step (its chip, a card's edge).
pub(super) const fn step_class(step: DueStep) -> &'static str {
    match step {
        DueStep::Ok => "due-ok",
        DueStep::ComingUp => "due-coming-up",
        DueStep::DueSoon => "due-soon",
        DueStep::DueVerySoon => "due-very-soon",
        DueStep::Overdue => "due-overdue",
    }
}

/// A step's chip: its word, in its colour.
#[component]
pub(super) fn DueChip(step: DueStep) -> Element {
    rsx! {
        span { class: "status-chip due-chip {step_class(step)}", {tr(step.label())} }
    }
}

/// A date (`2026-12-10`) as the person writes dates.
fn civil(date: &str, units: &Preferences) -> String {
    CalendarDate::parse(date).map_or_else(|_| date.to_owned(), |date| date_text(&date, units))
}

/// One rule's "what's left": "in 310 mi", "Oil life 9 %" (a meter counting down), "310 mi over",
/// "by 12 Dec", "was due 2 Nov".
fn rule_text(rule: &RuleDue, meters: &[(MeterId, Meter)], units: &Preferences) -> Option<String> {
    match &rule.left {
        Left::Meter { meter, left, .. } => {
            let (_, meter) = meters.iter().find(|(id, _)| id == meter)?;
            let amount = format!(
                "{} {}",
                show_reading(left.abs(), meter.unit, meter.precision),
                tr(meter.unit.abbr())
            );
            // A distance reads "in 310 mi"; another meter names itself ("Oil life: 80 % to go").
            let named = meter.dimension != domain::meter::MeterDimension::Distance;
            let args = [
                ("amount", MessageArg::from(amount)),
                ("meter", MessageArg::from(meter.name.as_str().to_owned())),
            ];
            let id = match (*left < 0, named) {
                (false, false) => messages::DUE_IN,
                (true, false) => messages::DUE_OVER_BY,
                (false, true) => messages::DUE_TO_GO,
                (true, true) => messages::DUE_METER_OVER,
            };
            Some(tr_with(id, &args))
        }
        Left::Date { due } => {
            let date = MessageArg::from(civil(due, units));
            Some(if rule.step == DueStep::Overdue {
                tr_with(messages::DUE_WAS, &[("date", date)])
            } else {
                tr_with(messages::DUE_BY, &[("date", date)])
            })
        }
    }
}

/// What's left on `due`, in words: each rule's, joined by "or", then the projected date if
/// it's an estimate ("in 310 mi or by 12 Dec · ~19 Nov"); "Not done yet" first when it is.
pub(super) fn left_text(due: &Due, meters: &[(MeterId, Meter)], units: &Preferences) -> String {
    let parts: Vec<String> = due
        .rules
        .iter()
        .filter_map(|rule| rule_text(rule, meters, units))
        .collect();
    let mut text = parts.join(&format!(" {} ", tr(messages::DUE_OR)));
    if let Some(date) = due.date.as_ref().filter(|date| date.estimate) {
        text = format!(
            "{text} · {}",
            tr_with(
                messages::DUE_ABOUT,
                &[("date", MessageArg::from(civil(&date.on, units)))]
            )
        );
    }
    if due.not_done_yet {
        text = format!("{} · {text}", tr(messages::DUE_NOT_DONE_YET));
    }
    text
}

// --- Warnings as typed ----------------------------------------------------------------------

/// The three steps as typed: yellow, orange, red.
pub(super) type TypedSteps = [String; 3];

/// A requirement's (or the household's) warnings as typed: percentages of an interval left,
/// and days before a date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TypedWarnings {
    /// Coming up, due soon, due very soon: % of the interval left.
    pub(super) interval: TypedSteps,
    /// The same, in days before a date.
    pub(super) days: TypedSteps,
}

impl TypedWarnings {
    /// `warnings` as typed.
    pub(super) fn of(warnings: &DueDefaults) -> Self {
        Self {
            interval: [
                warnings.interval.coming_up.to_string(),
                warnings.interval.due_soon.to_string(),
                warnings.interval.very_soon.to_string(),
            ],
            days: days_typed(warnings.days),
        }
    }

    /// The warnings, or why not.
    ///
    /// # Errors
    ///
    /// [`messages::DUE_BAD_WARNINGS`] if a number isn't one, or they don't make sense.
    pub(super) fn parse(&self) -> Result<DueDefaults, MessageId> {
        let percent = |text: &str| {
            text.trim()
                .parse::<u8>()
                .map_err(|_| messages::DUE_BAD_WARNINGS)
        };
        let warnings = DueDefaults {
            interval: IntervalSteps {
                coming_up: percent(&self.interval[0])?,
                due_soon: percent(&self.interval[1])?,
                very_soon: percent(&self.interval[2])?,
            },
            days: parse_days(&self.days)?,
        };
        if warnings.valid() {
            Ok(warnings)
        } else {
            Err(messages::DUE_BAD_WARNINGS)
        }
    }
}

/// Day steps as typed.
pub(super) fn days_typed(days: DaySteps) -> TypedSteps {
    [
        days.coming_up.to_string(),
        days.due_soon.to_string(),
        days.very_soon.to_string(),
    ]
}

/// Day steps from what's typed.
///
/// # Errors
///
/// [`messages::DUE_BAD_WARNINGS`] if a number isn't one, or they don't make sense.
pub(super) fn parse_days(typed: &TypedSteps) -> Result<DaySteps, MessageId> {
    let day = |text: &str| {
        text.trim()
            .parse::<u16>()
            .map_err(|_| messages::DUE_BAD_WARNINGS)
    };
    let days = DaySteps {
        coming_up: day(&typed[0])?,
        due_soon: day(&typed[1])?,
        very_soon: day(&typed[2])?,
    };
    if days.valid() {
        Ok(days)
    } else {
        Err(messages::DUE_BAD_WARNINGS)
    }
}

/// "20 / 10 / 5% left · 60 / 30 / 14 days before a date".
pub(super) fn warnings_text(warnings: &DueDefaults) -> String {
    let interval = &warnings.interval;
    tr_with(
        messages::DUE_WARNINGS_TEXT,
        &[
            (
                "interval",
                MessageArg::from(format!(
                    "{} / {} / {}",
                    interval.coming_up, interval.due_soon, interval.very_soon
                )),
            ),
            ("days", MessageArg::from(days_text(warnings.days))),
        ],
    )
}

/// "60 / 30 / 14".
pub(super) fn days_text(days: DaySteps) -> String {
    format!(
        "{} / {} / {}",
        days.coming_up, days.due_soon, days.very_soon
    )
}

/// Three step fields (coming up, due soon, due very soon) after `label`, each followed by
/// `unit` ("%" or "days").
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn StepInputs(label: MessageId, values: Signal<TypedSteps>, unit: MessageId) -> Element {
    let mut values = values;
    let steps = [DueStep::ComingUp, DueStep::DueSoon, DueStep::DueVerySoon];
    rsx! {
        div { class: "field",
            span { class: "field-label", {tr(label)} }
            div { class: "step-inputs",
                for (index, step) in steps.into_iter().enumerate() {
                    label { key: "{index}", class: "step-input",
                        span { class: "status-chip due-chip {step_class(step)}", {tr(step.label())} }
                        input {
                            class: "field-input step-number",
                            inputmode: "numeric",
                            value: "{values()[index]}",
                            oninput: move |event| values.write()[index] = event.value(),
                        }
                        span { {tr(unit)} }
                    }
                }
            }
        }
    }
}

// --- A requirement's due settings (4A) -----------------------------------------------------

/// "Last done before these records" and the warnings, as typed in a requirement's dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TypedDueSettings {
    /// The date (`2026-08-02`), empty for none.
    pub(super) before_on: String,
    /// Each meter's reading then, as typed (empty: none).
    pub(super) before: Vec<(MeterId, String)>,
    /// Its own warnings; `None` follows the household's.
    pub(super) warnings: Option<TypedWarnings>,
}

impl TypedDueSettings {
    /// A requirement's settings as typed (empty for a new one).
    pub(super) fn of(requirement: Option<&Requirement>, meters: &[(MeterId, Meter)]) -> Self {
        let before = requirement.and_then(|requirement| requirement.before_records.as_ref());
        Self {
            before_on: before.map(|before| before.on.clone()).unwrap_or_default(),
            before: before
                .map(|before| {
                    before
                        .readings
                        .iter()
                        .filter_map(|reading| {
                            let (_, meter) = meters.iter().find(|(id, _)| *id == reading.meter)?;
                            Some((
                                reading.meter,
                                reading_value(reading.value, meter.unit, meter.precision),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            warnings: requirement
                .and_then(|requirement| requirement.warnings.as_ref())
                .map(TypedWarnings::of),
        }
    }

    /// The settings to save: "last done before" (none when nothing's typed) and the
    /// warnings.
    ///
    /// # Errors
    ///
    /// The message for the first problem: a reading that isn't one, or warnings that don't
    /// make sense. The server checks the date and the meters (#082).
    pub(super) fn parse(
        &self,
        meters: &[(MeterId, Meter)],
    ) -> Result<(Option<LastDoneBefore>, Option<DueDefaults>), MessageId> {
        let mut readings = Vec::new();
        for (id, text) in self
            .before
            .iter()
            .filter(|(_, text)| !text.trim().is_empty())
        {
            let (_, meter) = meters
                .iter()
                .find(|(have, _)| have == id)
                .ok_or(messages::SOMETHING_WENT_WRONG)?;
            let value = parse_reading(text, meter.unit, meter.precision)
                .map_err(domain::quantity::QuantityError::message)?;
            readings.push(Reading {
                meter: *id,
                value,
                reset: false,
            });
        }
        let on = self.before_on.trim();
        let before = if on.is_empty() && readings.is_empty() {
            None
        } else if on.is_empty() {
            return Err(messages::REQUIREMENT_BAD_BEFORE_RECORDS);
        } else {
            Some(LastDoneBefore {
                on: on.to_owned(),
                readings,
            })
        };
        let warnings = self
            .warnings
            .as_ref()
            .map(TypedWarnings::parse)
            .transpose()?;
        Ok((before, warnings))
    }
}

/// A requirement dialog's "Last done before these records" and "Warn me" (4A): the warnings
/// folded to the household's until **Change**. `meters` are the ones its rules count on.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn DueSettings(
    settings: Signal<TypedDueSettings>,
    meters: Vec<(MeterId, Meter)>,
    household: DueDefaults,
) -> Element {
    let mut settings = settings;
    let current = settings();
    let mut interval = use_signal(|| {
        current.warnings.as_ref().map_or_else(
            || TypedWarnings::of(&household).interval,
            |warnings| warnings.interval.clone(),
        )
    });
    let mut days = use_signal(|| {
        current.warnings.as_ref().map_or_else(
            || TypedWarnings::of(&household).days,
            |warnings| warnings.days.clone(),
        )
    });
    // The step fields write back into the settings as they change.
    use_effect(move || {
        let (interval, days) = (interval(), days());
        if let Some(warnings) = settings.write().warnings.as_mut() {
            warnings.interval = interval;
            warnings.days = days;
        }
    });
    rsx! {
        fieldset { class: "field due-settings",
            legend { class: "field-label", {tr(messages::DUE_BEFORE_RECORDS)} }
            div { class: "field-row",
                label { class: "field",
                    span { class: "field-label", {tr(messages::DUE_BEFORE_DATE)} }
                    input {
                        class: "field-input",
                        r#type: "date",
                        value: "{current.before_on}",
                        oninput: move |event| settings.write().before_on = event.value(),
                    }
                }
                for (id, meter) in meters {
                    label { key: "{id}", class: "field",
                        span { class: "field-label", {format!("{} ({})", meter.name.as_str(), tr(meter.unit.abbr()))} }
                        input {
                            class: "field-input",
                            inputmode: "decimal",
                            value: current.before.iter().find(|(have, _)| *have == id).map(|(_, text)| text.clone()).unwrap_or_default(),
                            oninput: move |event| {
                                let mut typed = settings.write();
                                typed.before.retain(|(have, _)| *have != id);
                                typed.before.push((id, event.value()));
                            },
                        }
                    }
                }
            }
        }
        div { class: "field due-warn",
            span { class: "field-label", {tr(messages::DUE_WARN_ME)} }
            if current.warnings.is_some() {
                StepInputs { label: messages::DUE_PERCENT_LEFT, values: interval, unit: messages::DUE_PERCENT }
                StepInputs { label: messages::DUE_DAYS_BEFORE, values: days, unit: messages::DUE_DAYS }
                button {
                    class: "link-button",
                    r#type: "button",
                    onclick: move |_| settings.write().warnings = None,
                    {tr(messages::DUE_USE_HOUSEHOLD)}
                }
            } else {
                div { class: "check-row",
                    span { class: "muted-note",
                        {tr_with(messages::DUE_HOUSEHOLD_DEFAULT, &[("warnings", MessageArg::from(warnings_text(&household)))])}
                    }
                    button {
                        class: "link-button",
                        r#type: "button",
                        onclick: move |_| {
                            let start = TypedWarnings::of(&household);
                            interval.set(start.interval.clone());
                            days.set(start.days.clone());
                            settings.write().warnings = Some(start);
                        },
                        {tr(messages::DUE_CHANGE)}
                    }
                }
            }
        }
    }
}

/// What a due item is called: a requirement's name, or a document's type.
fn subject_name(entry: &DueEntry) -> String {
    match &entry.subject {
        DueSubject::Requirement { name, .. } => name.as_str().to_owned(),
        DueSubject::Document { doc_type, .. } => type_name(doc_type),
    }
}

/// The Overview's "Coming up" (2A): the 5 most urgent items, each with what's left and its
/// chip, then "See all on Maintenance".
#[component]
pub(crate) fn ComingUp(view: AssetView) -> Element {
    let units = use_units();
    let asset = view.id;
    let loaded = use_resource(move || async move { data::vehicle_due(asset).await });
    let entries = match &*loaded.read() {
        Some(Ok(DueOutcome::Due { entries })) => Some(entries.clone()),
        _ => None,
    };
    rsx! {
        section { class: "vehicle-panel",
            h3 { class: "section-heading", {tr(messages::PANEL_DUE).to_uppercase()} }
            match entries {
                None => rsx! {},
                Some(entries) if entries.is_empty() => rsx! {
                    p { class: "muted-note", {tr(messages::PANEL_DUE_NONE)} }
                },
                Some(entries) => rsx! {
                    ul { class: "coming-up",
                        for (index, entry) in entries.into_iter().take(COMING_UP).enumerate() {
                            li { key: "{index}",
                                span { class: "coming-up-name", {subject_name(&entry)} }
                                span { class: "due-left", {left_text(&entry.due, &view.meters, &units())} }
                                DueChip { step: entry.due.step }
                            }
                        }
                    }
                },
            }
            Link {
                class: "panel-link",
                to: Route::VehicleTab { asset_id: asset.to_string(), tab: "maintenance".to_owned() },
                {tr(messages::PANEL_DUE_SEE_ALL)}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::due::DueDate;
    use domain::i18n::Translator;
    use domain::label::Label;
    use domain::meter::{MeterDimension, MeterDirection, MeterUnit};
    use domain::test_support::test_id;

    fn meters() -> Vec<(MeterId, Meter)> {
        vec![(
            test_id(10),
            Meter {
                asset: test_id(1),
                name: Label::parse("Odometer").unwrap(),
                dimension: MeterDimension::Distance,
                unit: MeterUnit::Miles,
                precision: 0,
                description: None,
                required_for: vec![],
                direction: MeterDirection::Up,
                reset_to: None,
            },
        )]
    }

    #[test]
    fn every_step_has_its_word_and_colour() {
        for step in [
            DueStep::Ok,
            DueStep::ComingUp,
            DueStep::DueSoon,
            DueStep::DueVerySoon,
            DueStep::Overdue,
        ] {
            assert!(Translator::english().has(step.label()), "{step:?}");
            assert!(step_class(step).starts_with("due-"));
        }
    }

    #[test]
    fn whats_left_reads_as_one_line() {
        let due = Due {
            step: DueStep::DueVerySoon,
            rules: vec![
                RuleDue {
                    step: DueStep::DueVerySoon,
                    left: Left::Meter {
                        meter: test_id(10),
                        left: 310 * 1_609_344 / 1_000,
                        projected: Some("2099-11-19".into()),
                    },
                },
                RuleDue {
                    step: DueStep::Ok,
                    left: Left::Date {
                        due: "2099-12-10".into(),
                    },
                },
            ],
            date: Some(DueDate {
                on: "2099-11-19".into(),
                estimate: true,
            }),
            not_done_yet: false,
        };
        let units = Preferences::default();
        let text = left_text(&due, &meters(), &units);
        assert!(text.starts_with("in 310 mi or by "), "{text}");
        assert!(text.contains(" · ~"), "{text}");
        let over = Due {
            rules: vec![RuleDue {
                step: DueStep::Overdue,
                left: Left::Meter {
                    meter: test_id(10),
                    left: -1_609,
                    projected: None,
                },
            }],
            date: None,
            not_done_yet: true,
            ..due
        };
        assert_eq!(
            left_text(&over, &meters(), &units),
            "Not done yet · 1 mi over"
        );
        // Another meter names itself.
        let mut oil = meters();
        oil[0].1.name = Label::parse("Oil life").unwrap();
        oil[0].1.dimension = MeterDimension::Percentage;
        oil[0].1.unit = MeterUnit::Percent;
        let falling = Due {
            rules: vec![RuleDue {
                step: DueStep::ComingUp,
                left: Left::Meter {
                    meter: test_id(10),
                    left: 1_500,
                    projected: None,
                },
            }],
            not_done_yet: false,
            ..over
        };
        assert_eq!(left_text(&falling, &oil, &units), "Oil life: 15 % to go");
    }
}
