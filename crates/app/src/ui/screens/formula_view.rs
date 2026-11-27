//! Calculated fields and derived meters on screen (Phase 5 milestone 7; decision #087; sketch
//! `docs/ui-sketches/p5m7-formulas-options.svg`, rows 1 (A and B combined), 2A, 3B, 4A, 5A).
//!
//! - **The ƒ badge** marks every value that's worked out.
//! - **The formula box** (row 1): typed, with **Functions ▾** and **Values ▾** menus that
//!   insert at the cursor, an **Over** picker for functions over past events, a check as you
//!   type, and **ⓘ How to write one** (row 5): a short popup and the full guide.
//! - **+ Calculated field** and **+ Derived meter** (row 3B) open their own dialogs here.
//! - **ƒ rows** (row 2A): calculated values on the event's page and, as values are typed, on
//!   the log form.

use dioxus::prelude::*;
use domain::api::{AssetView, AssetsOutcome, CalculatedValue, EventsOutcome, FormulaHistory};
use domain::asset_form::{FieldForm, MeterForm};
use domain::calculated::{
    FUEL_VALUES, check_calculated_formula, check_derived_formula, derived_now, main_odometer,
};
use domain::custom_field::{CustomField, CustomFieldId, FieldKind};
use domain::event_check::PastReading;
use domain::formula::{
    Context, Dimension, Formula, FormulaError, GUIDE_URL, NoValue, Quantity, Scope, ShowsAs,
    in_units,
};
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::location::TimeZone;
use domain::meter::{EventKind, Meter, MeterDimension, MeterDirection, MeterId, MeterUnit};
use domain::preferences::Preferences;
use domain::quantity::{currency_decimals, show_duration};

use super::vehicle::{precision_label, use_vehicle_page};
use super::vehicle_setup::{KindChecks, SaveButtons, failure, unit_value, units_for};
use crate::ui::screens::{Field, FormError};
use crate::ui::units::{distance_abbr, economy_abbr, use_units, volume_abbr};
use crate::ui::{tr, tr_with};
use crate::{api, data};

/// The ƒ badge: this value is worked out, not typed.
#[component]
pub(super) fn FxBadge() -> Element {
    rsx! {
        span { class: "fx-badge", title: tr(messages::FORMULA_WORKED_OUT), "aria-label": tr(messages::FORMULA_WORKED_OUT), "ƒ" }
    }
}

fn fixed(value: f64, decimals: u8) -> String {
    format!("{value:.*}", usize::from(decimals))
}

/// A worked-out value as `shows` shows it, in the person's `units`: "$0.14 per mi", "1:12:30",
/// "31.4 mpg", "80 %".
pub(super) fn show_value(value: &Quantity, shows: ShowsAs, units: &Preferences) -> String {
    let Some(amount) = in_units(value, units) else {
        return "—".to_owned();
    };
    let money = |decimals: u8| {
        let currency = value.currency.unwrap_or(units.currency);
        format!(
            "{}{}",
            currency.symbol(units.currency),
            fixed(amount, currency_decimals(currency) + decimals)
        )
    };
    let per = |what: String, unit: String| {
        tr_with(
            messages::FORMULA_PER,
            &[
                ("value", MessageArg::from(what)),
                ("unit", MessageArg::from(unit)),
            ],
        )
    };
    match shows {
        ShowsAs::Number { decimals } => fixed(amount, decimals),
        ShowsAs::Percent { decimals } => format!("{} %", fixed(amount, decimals)),
        ShowsAs::Distance => format!("{} {}", fixed(amount, 1), distance_abbr(units.distance)),
        ShowsAs::Duration => {
            // Shown in hours; a duration reads as H:MM:SS.
            let seconds = (amount * 3_600.0).round();
            format!("{seconds:.0}")
                .parse::<i64>()
                .map_or_else(|_| "—".to_owned(), show_duration)
        }
        ShowsAs::Volume => format!("{} {}", fixed(amount, 2), volume_abbr(units.volume)),
        ShowsAs::FuelEconomy => format!("{} {}", fixed(amount, 1), economy_abbr(units.economy)),
        ShowsAs::Money => money(0),
        ShowsAs::MoneyPerDistance => per(money(0), distance_abbr(units.distance)),
        ShowsAs::MoneyPerVolume => per(money(1), volume_abbr(units.volume)),
        ShowsAs::Speed => {
            let unit = match units.distance {
                domain::preferences::DistanceUnit::Miles => "mph",
                domain::preferences::DistanceUnit::Kilometers => "km/h",
            };
            format!("{} {unit}", fixed(amount, 0))
        }
    }
}

/// Why there's no value, in words.
pub(super) fn no_value_text(why: &NoValue) -> String {
    match why {
        NoValue::Missing { key } => {
            tr_with(why.message(), &[("key", MessageArg::from(key.as_str()))])
        }
        NoValue::NoEarlier
        | NoValue::NothingInOver
        | NoValue::DivideByZero
        | NoValue::MixedCurrencies
        | NoValue::NotPositive
        | NoValue::TooFew
        | NoValue::NoOdometer
        | NoValue::NotFinite
        | NoValue::Mismatch => tr(why.message()),
    }
}

/// What `shows` is called, for the dialog's menu.
const fn shows_label(shows: ShowsAs) -> MessageId {
    match shows {
        ShowsAs::Number { .. } => messages::SHOWS_NUMBER,
        ShowsAs::Percent { .. } => messages::SHOWS_PERCENT,
        ShowsAs::Distance => messages::SHOWS_DISTANCE,
        ShowsAs::Duration => messages::SHOWS_DURATION,
        ShowsAs::Volume => messages::SHOWS_VOLUME,
        ShowsAs::FuelEconomy => messages::SHOWS_ECONOMY,
        ShowsAs::Money => messages::SHOWS_MONEY,
        ShowsAs::MoneyPerDistance => messages::SHOWS_MONEY_PER_DISTANCE,
        ShowsAs::MoneyPerVolume => messages::SHOWS_MONEY_PER_VOLUME,
        ShowsAs::Speed => messages::SHOWS_SPEED,
    }
}

/// Every choice of what a calculated field shows as (numbers and percentages with 2 decimals
/// to start).
const SHOWS: [ShowsAs; 10] = [
    ShowsAs::Number { decimals: 2 },
    ShowsAs::Percent { decimals: 0 },
    ShowsAs::Distance,
    ShowsAs::Duration,
    ShowsAs::Volume,
    ShowsAs::FuelEconomy,
    ShowsAs::Money,
    ShowsAs::MoneyPerDistance,
    ShowsAs::MoneyPerVolume,
    ShowsAs::Speed,
];

/// The check as you type: "✓ Gives money per distance", or why not.
fn check_line(result: Result<Dimension, FormulaError>) -> Result<String, String> {
    let english = Translator::english();
    result
        .map(|dimension| {
            tr_with(
                messages::FORMULA_CHECK_OK,
                &[("what", MessageArg::from(dimension.describe(english)))],
            )
        })
        .map_err(|error| error.describe(english))
}

// --- Calculated rows -----------------------------------------------------------------------

/// 2A: an event's calculated values, each with the ƒ badge.
#[component]
pub(super) fn CalculatedRows(
    values: Vec<CalculatedValue>,
    fields: Vec<(CustomFieldId, CustomField)>,
    units: Preferences,
) -> Element {
    let rows: Vec<(String, Result<String, String>)> = values
        .iter()
        .filter_map(|value| {
            let (_, field) = fields.iter().find(|(id, _)| *id == value.field)?;
            let FieldKind::Calculated { shows, .. } = &field.kind else {
                return None;
            };
            let shown = match &value.value {
                Ok(quantity) => Ok(show_value(quantity, *shows, &units)),
                Err(why) => Err(no_value_text(why)),
            };
            Some((field.label.as_str().to_owned(), shown))
        })
        .collect();
    if rows.is_empty() {
        return rsx! {};
    }
    rsx! {
        section { class: "event-section",
            h3 { class: "section-heading", {tr(messages::FORMULA_ROWS).to_uppercase()} }
            div { class: "calculated-rows",
                for (label, shown) in rows {
                    div { key: "{label}", class: "calculated-row",
                        FxBadge {}
                        span { class: "calculated-label", {label} }
                        match shown {
                            Ok(text) => rsx! { span { class: "calculated-value", {text} } },
                            Err(why) => rsx! { span { class: "calculated-none", "—" } span { class: "muted-note", {why} } },
                        }
                    }
                }
            }
        }
    }
}

/// The calculated fields shown on `kind`, worked out on `current` after `earlier`.
pub(super) fn live_values(
    kind: EventKind,
    current: &Scope,
    earlier: &[Scope],
    fields: &[(CustomFieldId, CustomField)],
    meters: &[(MeterId, Meter)],
    zone: &TimeZone,
    units: &Preferences,
) -> Vec<CalculatedValue> {
    fields
        .iter()
        .filter(|(_, field)| field.calculated() && field.shown(kind))
        .filter_map(|(id, field)| {
            let FieldKind::Calculated { formula, .. } = &field.kind else {
                return None;
            };
            let value = formula.evaluate(&Context {
                current,
                earlier,
                main_distance: main_odometer(meters),
                zone,
                units,
            });
            Some(CalculatedValue { field: *id, value })
        })
        .collect()
}

// --- The formula box ---------------------------------------------------------------------

/// The script that puts `text` in place of the selection in the box with id `id`, leaving the
/// cursor `back` characters before its end, and gives the box's new text.
fn insert_script(id: &str, text: &str, back: usize) -> String {
    // JSON string literals, so nothing typed can break out of the script.
    let id = serde_json::to_string(id).unwrap_or_default();
    let text = serde_json::to_string(text).unwrap_or_default();
    format!(
        "const el = document.getElementById({id});
         const start = el.selectionStart ?? el.value.length;
         const end = el.selectionEnd ?? start;
         const text = {text};
         el.value = el.value.slice(0, start) + text + el.value.slice(end);
         const at = start + text.length - {back};
         el.focus();
         el.setSelectionRange(at, at);
         return el.value;"
    )
}

/// A group of things the Values ▾ menu offers: its heading, and each item as inserted.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ValueGroup {
    heading: MessageId,
    items: Vec<String>,
}

/// Rows 1 and 5: the formula box with its menus, check, and help.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn FormulaBox(
    /// The box's element id (for inserting at the cursor).
    id: &'static str,
    text: Signal<String>,
    values: Vec<ValueGroup>,
    /// Offer functions over past events, and the Over picker.
    history: bool,
    /// The check as you type (none while it's empty).
    check: Option<Result<String, String>>,
) -> Element {
    let mut text = text;
    let mut help = use_signal(|| false);
    let insert = move |what: &str, back: usize| {
        let what = what.to_owned();
        spawn(async move {
            if let Ok(value) = document::eval(&insert_script(id, &what, back))
                .join::<String>()
                .await
            {
                text.set(value);
            }
        });
    };
    let functions: Vec<&'static str> = domain::formula::Function::ALL
        .iter()
        .map(|function| function.name())
        .collect();
    rsx! {
        div { class: "formula-field",
            div { class: "formula-head",
                span { class: "field-label", {tr(messages::FORMULA_LABEL)} }
                button { class: "link-button", r#type: "button", "aria-expanded": help(), onclick: move |_| help.set(!help()),
                    {tr(messages::FORMULA_HOW_TO)}
                }
                div { class: "formula-menus",
                    details { class: "formula-menu",
                        summary { class: "secondary-button", {tr(messages::FORMULA_FUNCTIONS)} }
                        div { class: "formula-menu-list",
                            p { class: "formula-menu-heading", {tr(if history { messages::FORMULA_FUNCTIONS_OVER } else { messages::FORMULA_FUNCTIONS_LISTED })} }
                            for name in functions {
                                button { key: "{name}", class: "formula-menu-item mono", r#type: "button",
                                    onclick: move |_| insert(&format!("{name}()"), 1),
                                    {name}
                                }
                            }
                            if history {
                                p { class: "formula-menu-heading", {tr(messages::FORMULA_FUNCTIONS_ONE)} }
                                button { class: "formula-menu-item mono", r#type: "button", onclick: move |_| insert("round(, 2)", 4), "round( , places)" }
                            }
                            button { class: "formula-menu-item mono", r#type: "button", onclick: move |_| insert("abs()", 1), "abs" }
                        }
                    }
                    details { class: "formula-menu",
                        summary { class: "secondary-button", {tr(messages::FORMULA_VALUES)} }
                        div { class: "formula-menu-list",
                            for group in values {
                                p { key: "{group.heading.as_str()}", class: "formula-menu-heading", {tr(group.heading)} }
                                for item in group.items {
                                    button { key: "{item}", class: "formula-menu-item mono", r#type: "button",
                                        onclick: move |_| insert(&format!("{item} "), 0),
                                        {item.clone()}
                                    }
                                }
                            }
                            p { class: "muted-note", {tr(messages::FORMULA_VALUES_NOTE)} }
                        }
                    }
                }
            }
            if help() {
                FormulaHelp { on_close: move |()| help.set(false) }
            }
            textarea {
                id,
                class: "field-input formula-input mono",
                rows: "3",
                spellcheck: "false",
                autocomplete: "off",
                value: text(),
                oninput: move |event| text.set(event.value()),
            }
            match check {
                Some(Ok(line)) => rsx! { p { class: "formula-check ok", {line} } },
                Some(Err(line)) => rsx! { p { class: "formula-check bad", role: "alert", {line} } },
                None => rsx! {},
            }
            if history {
                div { class: "formula-over",
                    span { class: "field-label", {tr(messages::FORMULA_OVER)} }
                    div { class: "filter-chips",
                        for (label, over) in [(messages::FORMULA_OVER_CUMULATIVE, ", cumulative"), (messages::FORMULA_OVER_EVENTS, ", last 5"), (messages::FORMULA_OVER_MONTHS, ", last 3 months"), (messages::FORMULA_OVER_DISTANCE, ", last 1000 mi")] {
                            button { key: "{over}", class: "chip-filter", r#type: "button", onclick: move |_| insert(over, 0), {tr(label)} }
                        }
                    }
                    p { class: "muted-note", {tr(messages::FORMULA_OVER_NOTE)} }
                }
            }
        }
    }
}

/// Row 5: the short help, and the full guide.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FormulaHelp(on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "formula-help", role: "note",
            div { class: "formula-help-head",
                strong { {tr(messages::FORMULA_HELP_TITLE)} }
                button { class: "icon-button", r#type: "button", "aria-label": tr(messages::ACTION_CLOSE), onclick: move |_| on_close.call(()), "✕" }
            }
            for line in [messages::FORMULA_HELP_VALUES, messages::FORMULA_HELP_OPERATORS, messages::FORMULA_HELP_OVER, messages::FORMULA_HELP_MINUS] {
                p { key: "{line.as_str()}", {tr(line)} }
            }
            p { class: "field-label", {tr(messages::FORMULA_HELP_EXAMPLES)} }
            for example in ["cost ÷ distance-since-last(odometer)", "mean(trip-time, last 3 months)", "odometer + idle-hours × 33 mi/h"] {
                p { key: "{example}", class: "mono", {example} }
            }
            a { class: "link-button", href: GUIDE_URL, target: "_blank", rel: "noopener", {tr(messages::FORMULA_HELP_GUIDE)} }
        }
    }
}

// --- Values a formula can use --------------------------------------------------------------

/// The Values ▾ menu for a calculated field shown on `kinds`.
fn field_values(
    view: &AssetView,
    kinds: &[EventKind],
    except: Option<CustomFieldId>,
) -> Vec<ValueGroup> {
    let mut groups = Vec::new();
    let fields: Vec<String> = view
        .fields
        .iter()
        .filter(|(id, field)| {
            Some(*id) != except
                && !field.calculated()
                && field.enabled
                && kinds.iter().any(|kind| field.shown(*kind))
        })
        .filter(|(_, field)| {
            matches!(
                field.kind,
                FieldKind::Number { .. }
                    | FieldKind::YesNo
                    | FieldKind::Distance
                    | FieldKind::Duration
                    | FieldKind::Volume
                    | FieldKind::FuelEconomy
                    | FieldKind::TreadDepth
            )
        })
        .map(|(_, field)| field.key.as_str().to_owned())
        .collect();
    let mut this_event = fields;
    if kinds.contains(&EventKind::Fuel) {
        this_event.extend(FUEL_VALUES.iter().map(|(key, ..)| (*key).to_owned()));
    }
    if !this_event.is_empty() {
        groups.push(ValueGroup {
            heading: messages::FORMULA_VALUES_EVENT,
            items: this_event,
        });
    }
    let meters: Vec<String> = view
        .meters
        .iter()
        .map(|(_, meter)| meter.key.as_str().to_owned())
        .collect();
    if !meters.is_empty() {
        groups.push(ValueGroup {
            heading: messages::FORMULA_VALUES_METERS,
            items: meters.clone(),
        });
    }
    let mut since: Vec<String> = meters
        .iter()
        .map(|key| format!("distance-since-last({key})"))
        .collect();
    since.push("time-since-last".to_owned());
    groups.push(ValueGroup {
        heading: messages::FORMULA_VALUES_SINCE,
        items: since,
    });
    groups.push(numbers());
    groups
}

fn numbers() -> ValueGroup {
    ValueGroup {
        heading: messages::FORMULA_VALUES_NUMBERS,
        items: vec![
            "33 mi/h".to_owned(),
            "1000 mi".to_owned(),
            "5 gal".to_owned(),
            "50%".to_owned(),
        ],
    }
}

/// The Values ▾ menu for a derived meter: the typed meters that aren't reset.
fn meter_values(view: &AssetView, except: Option<MeterId>) -> Vec<ValueGroup> {
    vec![
        ValueGroup {
            heading: messages::FORMULA_VALUES_METERS,
            items: view
                .meters
                .iter()
                .filter(|(id, meter)| {
                    Some(*id) != except && !meter.derived() && !meter.resettable()
                })
                .map(|(_, meter)| meter.key.as_str().to_owned())
                .collect(),
        },
        numbers(),
    ]
}

/// The vehicle's earlier values on the log form's context, by kind.
fn use_history(asset: domain::asset::AssetId) -> Resource<(Vec<FormulaHistory>, Vec<PastReading>)> {
    use_resource(move || async move {
        match data::event_log_context(asset).await {
            Ok(EventsOutcome::LogContext {
                history, latest, ..
            }) => (
                history,
                latest
                    .into_iter()
                    .map(|reading| PastReading {
                        meter: reading.meter,
                        at: reading.at,
                        value: reading.value,
                        reset: false,
                    })
                    .collect(),
            ),
            _ => (Vec::new(), Vec::new()),
        }
    })
}

// --- + Calculated field --------------------------------------------------------------------

/// Row 1: add or change a calculated field.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn CalculatedFieldDialog(
    view: AssetView,
    kinds: Vec<EventKind>,
    current: Option<(CustomFieldId, CustomField)>,
    on_close: EventHandler<()>,
) -> Element {
    let page = use_vehicle_page();
    let units = use_units();
    let existing = current.clone();
    let (start_formula, start_shows) = match existing.as_ref().map(|(_, field)| &field.kind) {
        Some(FieldKind::Calculated { formula, shows }) => (formula.as_str().to_owned(), *shows),
        _ => (String::new(), ShowsAs::Number { decimals: 2 }),
    };
    let label = use_signal(|| {
        existing
            .as_ref()
            .map(|(_, field)| field.label.as_str().to_owned())
            .unwrap_or_default()
    });
    let shown_on = use_signal(|| {
        existing.as_ref().map_or_else(
            || kinds.first().copied().into_iter().collect(),
            |(_, field)| field.shown_on.clone(),
        )
    });
    let formula = use_signal(|| start_formula);
    let mut shows = use_signal(|| start_shows);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let history = use_history(view.id);
    let editing_id = current.as_ref().map(|(id, _)| *id);
    let others: Vec<(CustomFieldId, CustomField)> = view
        .fields
        .iter()
        .filter(|(id, _)| Some(*id) != editing_id)
        .cloned()
        .collect();
    let checked = (!formula().trim().is_empty()).then(|| {
        Formula::parse(&formula()).and_then(|parsed| {
            check_calculated_formula(&parsed, shows(), &shown_on(), &others, &view.meters)
                .map(|()| parsed)
        })
    });
    let check = checked.as_ref().map(|result| {
        check_line(
            result
                .as_ref()
                .map(|_| shows().dimension())
                .map_err(Clone::clone),
        )
    });
    // The preview, from the latest event of the first kind it's shown on.
    let preview = match (&checked, history.read().as_ref()) {
        (Some(Ok(parsed)), Some((histories, _))) => shown_on().first().and_then(|kind| {
            let earlier = &histories.iter().find(|each| each.kind == *kind)?.earlier;
            let (current, before) = earlier.split_last()?;
            let value = parsed.evaluate(&Context {
                current,
                earlier: before,
                main_distance: main_odometer(&view.meters),
                zone: &TimeZone::utc(),
                units: &units(),
            });
            let shown = value.map_or_else(
                |why| no_value_text(&why),
                |value| show_value(&value, shows(), &units()),
            );
            Some(tr_with(
                messages::FORMULA_PREVIEW,
                &[
                    ("kind", MessageArg::from(tr(kind.label()).to_lowercase())),
                    ("value", MessageArg::from(shown)),
                ],
            ))
        }),
        _ => None,
    };
    let values = field_values(&view, &shown_on(), editing_id);
    let enabled = existing.as_ref().is_none_or(|(_, field)| field.enabled);
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let parsed = match Formula::parse(&formula()) {
            Ok(parsed) => parsed,
            Err(problem) => {
                error.set(Some(problem.describe(Translator::english())));
                return;
            }
        };
        let form = FieldForm {
            shown_on: shown_on(),
            label: label(),
            kind: FieldKind::Calculated {
                formula: parsed,
                shows: shows(),
            },
            enabled,
            required_for: vec![],
            range: None,
        };
        busy.set(true);
        let asset = view.id;
        spawn(async move {
            let result = match editing_id {
                Some(id) => api::edit_vehicle_field(id, form).await,
                None => api::add_vehicle_field(asset, form).await,
            };
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                page.reload();
                on_close.call(());
            }
        });
    };
    let decimals = if let ShowsAs::Number { decimals } | ShowsAs::Percent { decimals } = shows() {
        Some(decimals)
    } else {
        None
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog wide-dialog formula-dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(if editing_id.is_none() { messages::FORMULA_FIELD_ADD_TITLE } else { messages::FORMULA_FIELD_EDIT_TITLE })} }
                div { class: "field-row",
                    Field { label: messages::FIELD_LABEL, value: label, autofocus: true }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::FORMULA_SHOWS_AS)} }
                        select {
                            class: "field-input",
                            onchange: move |event| {
                                if let Some(chosen) = SHOWS.iter().find(|each| shows_value(**each) == event.value()) {
                                    shows.set(*chosen);
                                }
                            },
                            for choice in SHOWS {
                                option { key: "{shows_value(choice)}", value: shows_value(choice), selected: shows_value(shows()) == shows_value(choice), {tr(shows_label(choice))} }
                            }
                        }
                    }
                    if let Some(places) = decimals {
                        label { class: "field",
                            span { class: "field-label", {tr(messages::FIELD_PRECISION)} }
                            select {
                                class: "field-input",
                                onchange: move |event| {
                                    let places = event.value().parse().unwrap_or(0);
                                    shows.set(if matches!(shows(), ShowsAs::Percent { .. }) {
                                        ShowsAs::Percent { decimals: places }
                                    } else {
                                        ShowsAs::Number { decimals: places }
                                    });
                                },
                                for each in 0..=ShowsAs::MAX_DECIMALS {
                                    option { key: "{each}", value: "{each}", selected: each == places, "{each}" }
                                }
                            }
                        }
                    }
                }
                FormulaBox { id: "calculated-formula", text: formula, values, history: true, check }
                if let Some(line) = preview {
                    p { class: "formula-preview", FxBadge {} {line} }
                }
                KindChecks { label: messages::FIELD_SHOWN_ON, kinds, chosen: shown_on, note: messages::FORMULA_SHOWN_ON_NOTE }
                FormError { message: error() }
                SaveButtons { busy: busy(), on_close }
            }
        }
    }
}

/// What a `ShowsAs` is, as an `<option>` value.
const fn shows_value(shows: ShowsAs) -> &'static str {
    match shows {
        ShowsAs::Number { .. } => "number",
        ShowsAs::Percent { .. } => "percent",
        ShowsAs::Distance => "distance",
        ShowsAs::Duration => "duration",
        ShowsAs::Volume => "volume",
        ShowsAs::FuelEconomy => "fuel-economy",
        ShowsAs::Money => "money",
        ShowsAs::MoneyPerDistance => "money-per-distance",
        ShowsAs::MoneyPerVolume => "money-per-volume",
        ShowsAs::Speed => "speed",
    }
}

// --- + Derived meter -------------------------------------------------------------------------

/// Row 3B: add or change a derived meter.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn DerivedMeterDialog(
    view: AssetView,
    current: Option<(MeterId, Meter)>,
    on_close: EventHandler<()>,
) -> Element {
    let page = use_vehicle_page();
    let existing = current.as_ref().map(|(_, meter)| meter.clone());
    let name = use_signal(|| {
        existing
            .as_ref()
            .map(|meter| meter.name.as_str().to_owned())
            .unwrap_or_default()
    });
    let mut dimension = use_signal(|| {
        existing
            .as_ref()
            .map_or(MeterDimension::Distance, |meter| meter.dimension)
    });
    let mut unit = use_signal(|| {
        existing
            .as_ref()
            .map_or(MeterUnit::Miles, |meter| meter.unit)
    });
    let mut precision = use_signal(|| existing.as_ref().map_or(0, |meter| meter.precision));
    let formula = use_signal(|| {
        existing
            .as_ref()
            .and_then(|meter| meter.formula.as_ref())
            .map(|formula| formula.as_str().to_owned())
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let history = use_history(view.id);
    let editing_id = current.as_ref().map(|(id, _)| *id);
    let adding = editing_id.is_none();
    let checked = (!formula().trim().is_empty()).then(|| {
        Formula::parse(&formula()).and_then(|parsed| {
            check_derived_formula(editing_id, &parsed, dimension(), &view.meters).map(|()| parsed)
        })
    });
    let wanted = match dimension() {
        MeterDimension::Distance => Dimension::DISTANCE,
        MeterDimension::Duration => Dimension::TIME,
        MeterDimension::Percentage => Dimension::NONE,
    };
    let check = checked
        .as_ref()
        .map(|result| check_line(result.as_ref().map(|_| wanted).map_err(Clone::clone)));
    // Its value now, from each input's latest reading.
    let now = match (&checked, history.read().as_ref()) {
        (Some(Ok(parsed)), Some((_, latest))) => domain::label::Label::parse(&name())
            .ok()
            .and_then(|label| {
                let meter = Meter {
                    asset: view.id,
                    key: domain::custom_field::FieldKey::from_label(&label, &[]),
                    name: label,
                    dimension: dimension(),
                    unit: unit(),
                    precision: precision(),
                    description: None,
                    required_for: vec![],
                    direction: MeterDirection::Up,
                    reset_to: None,
                    formula: Some(parsed.clone()),
                };
                derived_now(&meter, &view.meters, latest)
            })
            .map(|value| {
                tr_with(
                    messages::FORMULA_NOW,
                    &[(
                        "value",
                        MessageArg::from(format!(
                            "{} {}",
                            domain::quantity::show_reading(value, unit(), precision()),
                            tr(unit().abbr())
                        )),
                    )],
                )
            }),
        _ => None,
    };
    let values = meter_values(&view, editing_id);
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let parsed = match Formula::parse(&formula()) {
            Ok(parsed) => parsed,
            Err(problem) => {
                error.set(Some(problem.describe(Translator::english())));
                return;
            }
        };
        let form = MeterForm {
            name: name(),
            dimension: dimension(),
            unit: unit(),
            precision: precision(),
            description: String::new(),
            required_for: vec![],
            direction: MeterDirection::Up,
            reset_to: None,
            formula: Some(parsed),
        };
        busy.set(true);
        let asset = view.id;
        spawn(async move {
            let result: Result<AssetsOutcome, ServerFnError> = match editing_id {
                Some(id) => api::edit_vehicle_meter(id, form).await,
                None => api::add_vehicle_meter(asset, form).await,
            };
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                page.reload();
                on_close.call(());
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog wide-dialog formula-dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(if adding { messages::FORMULA_METER_ADD_TITLE } else { messages::FORMULA_METER_EDIT_TITLE })} }
                Field { label: messages::FIELD_NAME, value: name, autofocus: true }
                div { class: "field-row",
                    label { class: "field",
                        span { class: "field-label", {tr(messages::FIELD_MEASURES)} }
                        select {
                            class: "field-input",
                            disabled: !adding,
                            onchange: move |event| {
                                let chosen = match event.value().as_str() {
                                    "duration" => MeterDimension::Duration,
                                    "percentage" => MeterDimension::Percentage,
                                    _ => MeterDimension::Distance,
                                };
                                dimension.set(chosen);
                                if !unit().fits(chosen) {
                                    unit.set(units_for(chosen)[0]);
                                }
                            },
                            option { value: "distance", selected: dimension() == MeterDimension::Distance, {tr(messages::DIMENSION_DISTANCE)} }
                            option { value: "duration", selected: dimension() == MeterDimension::Duration, {tr(messages::DIMENSION_DURATION)} }
                            option { value: "percentage", selected: dimension() == MeterDimension::Percentage, {tr(messages::DIMENSION_PERCENTAGE)} }
                        }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::FIELD_UNIT)} }
                        select {
                            class: "field-input",
                            onchange: move |event| {
                                if let Some(chosen) = MeterUnit::ALL.into_iter().find(|unit| unit_value(*unit) == event.value()) {
                                    unit.set(chosen);
                                }
                            },
                            for choice in units_for(dimension()) {
                                option { key: "{unit_value(choice)}", value: unit_value(choice), selected: unit() == choice, {tr(choice.label())} }
                            }
                        }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::FIELD_PRECISION)} }
                        select {
                            class: "field-input",
                            onchange: move |event| precision.set(event.value().parse().unwrap_or(0)),
                            for places in 0..=domain::meter::MAX_PRECISION {
                                option { key: "{places}", value: "{places}", selected: precision() == places, {tr(precision_label(places))} }
                            }
                        }
                    }
                }
                FormulaBox { id: "derived-formula", text: formula, values, history: false, check }
                if let Some(line) = now {
                    p { class: "formula-preview", FxBadge {} {line} }
                }
                p { class: "muted-note", {tr(messages::FORMULA_METER_NOTE)} }
                FormError { message: error() }
                SaveButtons { busy: busy(), on_close }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::preferences::{CurrencyCode, EconomyUnit};

    use super::*;

    const MILE: f64 = 1_609.344;

    #[test]
    fn values_show_in_the_persons_units() {
        let units = Preferences::default();
        let per_mile = Quantity {
            value: 0.14 / MILE,
            dimension: Dimension::MONEY_PER_DISTANCE,
            percent: false,
            currency: Some(CurrencyCode::USD),
        };
        assert_eq!(
            show_value(&per_mile, ShowsAs::MoneyPerDistance, &units),
            format!("{}0.14 per mi", CurrencyCode::USD.symbol(units.currency))
        );
        let trip = Quantity::of(4_350.0, Dimension::TIME);
        assert_eq!(
            show_value(&trip, ShowsAs::Duration, &units),
            show_duration(4_350)
        );
        let oil = Quantity::percentage(0.8);
        assert_eq!(
            show_value(&oil, ShowsAs::Percent { decimals: 0 }, &units),
            "80 %"
        );
        assert_eq!(
            show_value(
                &Quantity::number(2.5),
                ShowsAs::Number { decimals: 2 },
                &units
            ),
            "2.50"
        );
        let economy = Quantity::of(30.0 * MILE / 3.785_411_784, Dimension::ECONOMY);
        assert_eq!(
            show_value(&economy, ShowsAs::FuelEconomy, &units),
            format!("30.0 {}", economy_abbr(units.economy))
        );
        let metric = Preferences {
            economy: EconomyUnit::LitersPer100Km,
            ..units
        };
        assert_eq!(
            show_value(&economy, ShowsAs::FuelEconomy, &metric),
            format!("7.8 {}", economy_abbr(metric.economy))
        );
        let speed = Quantity::of(30.0 * MILE / 3_600.0, Dimension::SPEED);
        assert_eq!(show_value(&speed, ShowsAs::Speed, &units), "30 mph");
    }

    #[test]
    fn the_check_says_what_it_gives_or_why_not() {
        assert_eq!(
            check_line(Ok(Dimension::MONEY_PER_DISTANCE)),
            Ok("✓ Gives money per distance".to_owned())
        );
        let refused = check_line(Err(FormulaError::NestedOver)).unwrap_err();
        assert!(refused.contains("can't go inside"), "{refused}");
        assert_eq!(
            no_value_text(&NoValue::NoEarlier),
            tr(messages::FORMULA_NO_VALUE_NO_EARLIER)
        );
        assert!(no_value_text(&NoValue::Missing { key: "cost".into() }).contains("cost"));
    }

    #[test]
    fn inserting_quotes_what_it_inserts() {
        let script = insert_script("calculated-formula", "a\"); alert(1); (\"", 1);
        assert!(script.contains(r#"document.getElementById("calculated-formula")"#));
        assert!(script.contains(r#"const text = "a\"); alert(1); (\"";"#));
        assert!(script.contains("- 1;"));
    }

    #[test]
    fn every_shows_as_has_its_own_option() {
        let mut values: Vec<&str> = SHOWS.iter().map(|shows| shows_value(*shows)).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(values.len(), SHOWS.len());
    }
}
