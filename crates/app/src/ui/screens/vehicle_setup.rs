//! A vehicle's Setup tab (option A of `docs/ui-sketches/p2m4-vehicle-options.svg`, #057).
//!
//! Stacked sections: **Details** (edited in a dialog), **Meters** and **Custom fields** (add,
//! edit, remove; a field shows its stable key), **Required readings** (which meters each
//! kind of event must record), and **Group and status** (move with a preview of who gains or
//! loses access; the status menu; delete with the name typed). Each control shows only when
//! the person may use it now; the server decides again.

use dioxus::prelude::*;
use domain::api::{AssetView, AssetsOutcome, GroupSummary, MoveChange};
use domain::asset::{Asset, AssetId, LifecycleStatus};
use domain::asset_form::{AssetForm, FieldForm, MeterForm, confirms_name};
use domain::custom_field::{CustomField, CustomFieldId, FieldKind, FieldRange, MAX_DECIMALS};
use domain::group::GroupId;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::label::Label;
use domain::meter::{EventKind, MAX_PRECISION, Meter, MeterDimension, MeterId, MeterUnit};
use domain::preferences::Preferences;
use domain::profile::profile;
use domain::quantity::{
    QuantityError, distance_value, economy_is_inverse, economy_value, parse_distance,
    parse_duration, parse_economy, parse_volume, show_duration, volume_value,
};

use super::users::role_label;
use super::vehicle::{
    VehiclePage, identifier_name, meter_summary, precision_label, required_text, use_vehicle_page,
};
use super::vehicles::status_label;
use crate::api;
use crate::ui::icons::IconView;
use crate::ui::routes::Route;
use crate::ui::screens::{Field, FormError, problem_from_error};
use crate::ui::vehicles::{VehiclesContext, group_icon, refresh_garage, use_vehicles};
use crate::ui::{tr, tr_with};

/// What went wrong, if the answer isn't a success.
fn failure(result: &Result<AssetsOutcome, ServerFnError>) -> Option<String> {
    match result {
        Ok(
            AssetsOutcome::Done
            | AssetsOutcome::Added { .. }
            | AssetsOutcome::Asset { .. }
            | AssetsOutcome::MovePreview { .. }
            | AssetsOutcome::Garage { .. },
        ) => None,
        Ok(AssetsOutcome::Invalid { problem }) => Some(tr(problem.message())),
        Ok(AssetsOutcome::NotAllowed) => Some(tr(messages::VEHICLES_NOT_ALLOWED)),
        Ok(AssetsOutcome::NotFound) => Some(tr(messages::VEHICLE_NOT_FOUND)),
        Ok(AssetsOutcome::Unavailable) => Some(tr(messages::SOMETHING_WENT_WRONG)),
        Err(error) => Some(tr(problem_from_error(error).message())),
    }
}

/// After a change: the page again, and the Garage (names and statuses show there too).
async fn changed(page: VehiclePage, vehicles: VehiclesContext) {
    page.reload();
    let _ = refresh_garage(vehicles).await;
}

/// The kinds of events a vehicle has, from its profile (all of them for an unknown kind).
fn event_kinds(asset: &Asset) -> Vec<EventKind> {
    domain::profile::event_kinds(&asset.kind)
}

/// A custom field type's name.
const fn kind_label(kind: &FieldKind) -> MessageId {
    match kind {
        FieldKind::Text => messages::KIND_TEXT,
        FieldKind::Number { .. } => messages::KIND_NUMBER,
        FieldKind::Date => messages::KIND_DATE,
        FieldKind::YesNo => messages::KIND_YES_NO,
        FieldKind::Choice { .. } => messages::KIND_CHOICE,
        FieldKind::Distance => messages::KIND_DISTANCE,
        FieldKind::Duration => messages::KIND_DURATION,
        FieldKind::Volume => messages::KIND_VOLUME,
        FieldKind::FuelEconomy => messages::KIND_FUEL_ECONOMY,
    }
}

/// The field types offered when adding a field, with their `<option>` values.
const KINDS: [(&str, MessageId); 9] = [
    ("text", messages::KIND_TEXT),
    ("number", messages::KIND_NUMBER),
    ("date", messages::KIND_DATE),
    ("yes-no", messages::KIND_YES_NO),
    ("choice", messages::KIND_CHOICE),
    ("distance", messages::KIND_DISTANCE),
    ("duration", messages::KIND_DURATION),
    ("volume", messages::KIND_VOLUME),
    ("fuel-economy", messages::KIND_FUEL_ECONOMY),
];

/// An event kind's `<option>` value, and back.
pub(super) const fn event_value(kind: EventKind) -> &'static str {
    match kind {
        EventKind::Fuel => "fuel",
        EventKind::Reading => "reading",
        EventKind::Maintenance => "maintenance",
        EventKind::Repair => "repair",
    }
}

pub(super) fn event_from_value(value: &str) -> Option<EventKind> {
    EventKind::ALL
        .into_iter()
        .find(|kind| event_value(*kind) == value)
}

/// A lifecycle status's `<option>` value.
const fn status_value(status: LifecycleStatus) -> &'static str {
    match status {
        LifecycleStatus::InService => "in-service",
        LifecycleStatus::Spare => "spare",
        LifecycleStatus::OutForService => "out-for-service",
        LifecycleStatus::Retired => "retired",
        LifecycleStatus::Disposed => "disposed",
    }
}

/// A meter unit's `<option>` value, and back.
const fn unit_value(unit: MeterUnit) -> &'static str {
    match unit {
        MeterUnit::Miles => "miles",
        MeterUnit::Kilometers => "kilometers",
        MeterUnit::Hours => "hours",
    }
}

/// A number field's range end as typed ("12.5") in its storage unit (125 for one decimal).
/// Empty means open.
///
/// # Errors
///
/// When it isn't a number with at most `decimals` places.
fn parse_scaled(text: &str, decimals: u8) -> Result<Option<i64>, ()> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (negative, digits) = text
        .strip_prefix('-')
        .map_or((false, text), |rest| (true, rest));
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let places = usize::from(decimals);
    let all_digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty())
        || !all_digits(whole)
        || !all_digits(fraction)
        || fraction.len() > places
    {
        return Err(());
    }
    // The digits with the fraction padded to `decimals` places, as one whole number.
    let scaled = format!("{whole}{fraction:0<places$}");
    let value: i64 = scaled.parse().map_err(|_| ())?;
    Ok(Some(if negative { -value } else { value }))
}

/// A stored value shown with `decimals` places (125 with one decimal: "12.5").
fn show_scaled(value: i64, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    let places = usize::from(decimals);
    let digits = format!("{:0>width$}", value.unsigned_abs(), width = places + 1);
    let (whole, fraction) = digits.split_at(digits.len() - places);
    let sign = if value < 0 { "-" } else { "" };
    format!("{sign}{whole}.{fraction}")
}

/// The Setup tab.
#[component]
pub(super) fn Setup(view: AssetView) -> Element {
    let may_change = view.can_configure || view.can_manage || view.can_move;
    rsx! {
        div { class: "vehicle-setup",
            if !may_change {
                p { class: "muted-note", {tr(messages::SETUP_VIEW_ONLY)} }
            }
            DetailsSection { view: view.clone() }
            MetersSection { view: view.clone() }
            FieldsSection { view: view.clone() }
            // The maintenance schedule moved to its own tab (step 7g, choice 15).
            if view.can_configure {
                p { class: "muted-note",
                    Link {
                        to: Route::VehicleTab { asset_id: view.id.to_string(), tab: "set-up".to_owned() },
                        {tr(messages::SETUP_SCHEDULE_MOVED)}
                    }
                }
            }
            GroupAndStatus { view }
        }
    }
}

/// A Setup section's heading, with its button (if any) on the right.
#[component]
pub(super) fn SectionHeading(title: MessageId, children: Element) -> Element {
    rsx! {
        div { class: "setup-heading",
            h3 { class: "section-heading", {tr(title).to_uppercase()} }
            {children}
        }
    }
}

// --- Details -----------------------------------------------------------------------------

/// Details: make, model, year, colour, identifiers, and description.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DetailsSection(view: AssetView) -> Element {
    let mut editing = use_signal(|| false);
    let asset = &view.asset;
    let facts: Vec<(MessageId, String)> = [
        (messages::FIELD_MAKE, asset.make.clone()),
        (messages::FIELD_MODEL, asset.model.clone()),
        (
            messages::FIELD_YEAR,
            asset.year.map(|year| year.to_string()),
        ),
        (messages::FIELD_COLOUR, asset.colour.clone()),
    ]
    .into_iter()
    .filter_map(|(label, value)| value.map(|value| (label, value)))
    .collect();
    let line = facts
        .iter()
        .map(|(label, value)| format!("{} {value}", tr(*label)))
        .collect::<Vec<_>>()
        .join(" · ");
    rsx! {
        section { class: "setup-section",
            SectionHeading { title: messages::SETUP_DETAILS,
                if view.can_manage {
                    button { class: "secondary-button", onclick: move |_| editing.set(true), {tr(messages::SETUP_EDIT_DETAILS)} }
                }
            }
            if !line.is_empty() {
                p { {line} }
            }
            for identifier in asset.identifiers.iter() {
                p { key: "{identifier.kind.as_str()}",
                    {format!("{} {}", identifier_name(asset, identifier.kind.as_str()), identifier.value)}
                }
            }
            p { class: if asset.description.as_str().is_empty() { "muted-note" } else { "vehicle-description" },
                if asset.description.as_str().is_empty() {
                    {tr(messages::SETUP_NO_DESCRIPTION)}
                } else {
                    {format!("{}: {}", tr(messages::FIELD_DESCRIPTION), asset.description.as_str())}
                }
            }
        }
        if editing() {
            EditDetailsDialog { id: view.id, asset: view.asset.clone(), on_close: move |()| editing.set(false) }
        }
    }
}

/// The identifier kinds a vehicle's form offers: its profile's, then any others it has.
fn identifier_kinds(asset: &Asset) -> Vec<String> {
    let mut kinds: Vec<String> = profile(&asset.kind)
        .map(|profile| {
            profile
                .identifiers
                .iter()
                .map(|offered| offered.kind.as_str().to_owned())
                .collect()
        })
        .unwrap_or_default();
    for identifier in &asset.identifiers {
        let kind = identifier.kind.as_str().to_owned();
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    kinds
}

/// The details form.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn EditDetailsDialog(id: AssetId, asset: Asset, on_close: EventHandler<()>) -> Element {
    let page = use_vehicle_page();
    let vehicles = use_vehicles();
    let start = AssetForm::from_asset(&asset);
    let name = use_signal(|| start.name.clone());
    let make = use_signal(|| start.make.clone());
    let model = use_signal(|| start.model.clone());
    let year = use_signal(|| start.year.map(|year| year.to_string()).unwrap_or_default());
    let colour = use_signal(|| start.colour.clone());
    let mut description = use_signal(|| start.description.clone());
    let kinds = identifier_kinds(&asset);
    let mut identifiers = use_signal(|| {
        kinds
            .iter()
            .map(|kind| {
                start
                    .identifiers
                    .iter()
                    .find(|(have, _)| have == kind)
                    .map(|(_, value)| value.clone())
                    .unwrap_or_default()
            })
            .collect::<Vec<String>>()
    });
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let form_kinds = kinds.clone();
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let year_text = year();
        let year_value = match year_text.trim() {
            "" => None,
            text => {
                let Ok(value) = text.parse::<u16>() else {
                    error.set(Some(tr(messages::ASSET_YEAR_OUT_OF_RANGE)));
                    return;
                };
                Some(value)
            }
        };
        let form = AssetForm {
            name: name(),
            make: make(),
            model: model(),
            year: year_value,
            colour: colour(),
            description: description(),
            identifiers: form_kinds.iter().cloned().zip(identifiers()).collect(),
            ..start.clone()
        };
        busy.set(true);
        spawn(async move {
            let result = api::edit_vehicle(id, form).await;
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                changed(page, vehicles).await;
                on_close.call(());
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog wide-dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(messages::SETUP_EDIT_DETAILS)} }
                Field { label: messages::ADD_VEHICLE_NAME, value: name, autofocus: true }
                div { class: "field-row",
                    Field { label: messages::FIELD_MAKE, value: make }
                    Field { label: messages::FIELD_MODEL, value: model }
                }
                div { class: "field-row",
                    Field { label: messages::FIELD_YEAR, value: year }
                    Field { label: messages::FIELD_COLOUR, value: colour }
                }
                div { class: "field-row",
                    for (index, kind) in kinds.iter().enumerate() {
                        label { key: "{kind}", class: "field",
                            span { class: "field-label", {identifier_name(&asset, kind)} }
                            input {
                                class: "field-input mono",
                                autocomplete: "off",
                                spellcheck: "false",
                                value: identifiers().get(index).cloned().unwrap_or_default(),
                                oninput: move |event| {
                                    if let Some(slot) = identifiers.write().get_mut(index) {
                                        *slot = event.value();
                                    }
                                },
                            }
                        }
                    }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::FIELD_DESCRIPTION)} }
                    textarea {
                        class: "field-input",
                        rows: "4",
                        value: description(),
                        oninput: move |event| description.set(event.value()),
                    }
                }
                p { class: "muted-note", {tr(messages::FIELD_DESCRIPTION_NOTE)} }
                FormError { message: error() }
                SaveButtons { busy: busy(), on_close }
            }
        }
    }
}

/// Cancel and Save, for the dialogs' forms.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SaveButtons(busy: bool, on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "form-buttons",
            button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
            button { class: "primary-button", r#type: "submit", disabled: busy,
                if busy { {tr(messages::ACTION_SAVING)} } else { {tr(messages::ACTION_SAVE)} }
            }
        }
    }
}

// --- Meters ------------------------------------------------------------------------------

/// What's open in the Meters or Custom fields section.
#[derive(Debug, Clone, PartialEq)]
enum Editing<I, T> {
    Adding,
    Changing(I, T),
    Removing(I, String),
}

/// Meters: each with what it measures, Edit and Remove.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MetersSection(view: AssetView) -> Element {
    let mut editing = use_signal(|| None::<Editing<MeterId, Meter>>);
    let can = view.can_configure;
    let kinds = event_kinds(&view.asset);
    rsx! {
        section { class: "setup-section",
            SectionHeading { title: messages::PANEL_METERS,
                if can {
                    button { class: "secondary-button", onclick: move |_| editing.set(Some(Editing::Adding)), {tr(messages::SETUP_ADD_METER)} }
                }
            }
            if view.meters.is_empty() {
                p { class: "muted-note", {tr(messages::METERS_NONE)} }
            }
            for (meter_id, meter) in view.meters.clone() {
                div { key: "{meter_id}", class: "setup-row",
                    div { class: "setup-row-text",
                        span { class: "setup-row-name", {meter.name.as_str().to_owned()} }
                        span { class: "setup-row-detail", {meter_summary(&meter)} }
                        span { class: "setup-row-detail", {required_text(&meter)} }
                    }
                    if can {
                        RowButtons {
                            on_edit: {
                                let meter = meter.clone();
                                move |()| editing.set(Some(Editing::Changing(meter_id, meter.clone())))
                            },
                            on_remove: {
                                let name = meter.name.as_str().to_owned();
                                move |()| editing.set(Some(Editing::Removing(meter_id, name.clone())))
                            },
                        }
                    }
                }
            }
        }
        match editing() {
            None => rsx! {},
            Some(Editing::Adding) => rsx! {
                MeterDialog { asset: view.id, kinds, current: None, on_close: move |()| editing.set(None) }
            },
            Some(Editing::Changing(id, meter)) => rsx! {
                MeterDialog { asset: view.id, kinds, current: Some((id, meter)), on_close: move |()| editing.set(None) }
            },
            Some(Editing::Removing(id, name)) => rsx! {
                ConfirmRemove { target: Removal::Meter(id), name, on_close: move |()| editing.set(None) }
            },
        }
    }
}

/// Edit and Remove on a row.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn RowButtons(on_edit: EventHandler<()>, on_remove: EventHandler<()>) -> Element {
    rsx! {
        div { class: "setup-row-buttons",
            button { class: "secondary-button", onclick: move |_| on_edit.call(()), {tr(messages::ACTION_EDIT)} }
            button { class: "secondary-button", onclick: move |_| on_remove.call(()), {tr(messages::ACTION_REMOVE)} }
        }
    }
}

/// The units that can show a meter of `dimension`.
fn units_for(dimension: MeterDimension) -> Vec<MeterUnit> {
    MeterUnit::ALL
        .into_iter()
        .filter(|unit| unit.fits(dimension))
        .collect()
}

/// Add a meter, or change one (its name, unit, decimals, description, and which kinds of
/// event require it).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MeterDialog(
    asset: AssetId,
    /// The vehicle's kinds of events, for "Required for".
    kinds: Vec<EventKind>,
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
    let description = use_signal(|| {
        existing
            .as_ref()
            .and_then(|meter| meter.description.clone())
            .unwrap_or_default()
    });
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let editing_id = current.as_ref().map(|(id, _)| *id);
    let required_for = use_signal(|| {
        existing
            .as_ref()
            .map(|meter| meter.required_for.clone())
            .unwrap_or_default()
    });
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let form = MeterForm {
            name: name(),
            dimension: dimension(),
            unit: unit(),
            precision: precision(),
            description: description(),
            required_for: required_for(),
        };
        busy.set(true);
        spawn(async move {
            let result = match editing_id {
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
    let adding = editing_id.is_none();
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(if adding { messages::METER_ADD_TITLE } else { messages::METER_EDIT_TITLE })} }
                Field { label: messages::FIELD_NAME, value: name, autofocus: true }
                div { class: "field-row",
                    label { class: "field",
                        span { class: "field-label", {tr(messages::FIELD_MEASURES)} }
                        select {
                            class: "field-input",
                            disabled: !adding,
                            onchange: move |event| {
                                let chosen = if event.value() == "duration" { MeterDimension::Duration } else { MeterDimension::Distance };
                                dimension.set(chosen);
                                if !unit().fits(chosen) {
                                    unit.set(units_for(chosen)[0]);
                                }
                            },
                            option { value: "distance", selected: dimension() == MeterDimension::Distance, {tr(messages::DIMENSION_DISTANCE)} }
                            option { value: "duration", selected: dimension() == MeterDimension::Duration, {tr(messages::DIMENSION_DURATION)} }
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
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::FIELD_PRECISION)} }
                    select {
                        class: "field-input",
                        onchange: move |event| precision.set(event.value().parse().unwrap_or(0)),
                        for places in 0..=MAX_PRECISION {
                            option { key: "{places}", value: "{places}", selected: precision() == places, {tr(precision_label(places))} }
                        }
                    }
                }
                Field { label: messages::FIELD_METER_DESCRIPTION, value: description }
                KindChecks { label: messages::METER_REQUIRED_LABEL, kinds, chosen: required_for, note: messages::METER_REQUIRED_NOTE }
                if !adding {
                    p { class: "muted-note", {tr(messages::METER_DIMENSION_FIXED)} }
                }
                FormError { message: error() }
                SaveButtons { busy: busy(), on_close }
            }
        }
    }
}

/// What a Remove confirms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Removal {
    Meter(MeterId),
    Field(CustomFieldId),
}

/// "Remove Engine hours?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ConfirmRemove(target: Removal, name: String, on_close: EventHandler<()>) -> Element {
    let page = use_vehicle_page();
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let text = match target {
        Removal::Meter(_) => messages::METER_REMOVE_TEXT,
        Removal::Field(_) => messages::FIELD_REMOVE_TEXT,
    };
    let remove = move |_| {
        busy.set(true);
        spawn(async move {
            let result = match target {
                Removal::Meter(id) => api::delete_vehicle_meter(id).await,
                Removal::Field(id) => api::delete_vehicle_field(id).await,
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
            div { class: "dialog", role: "alertdialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr_with(messages::REMOVE_TITLE, &[("name", MessageArg::from(name))])} }
                p { class: "dialog-text", {tr(text)} }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button filled", disabled: busy(), onclick: remove, {tr(messages::ACTION_REMOVE)} }
                }
            }
        }
    }
}

// --- Custom fields -----------------------------------------------------------------------

/// "Fuel-up, Reading · Duration": where it's shown, and what it holds.
fn field_summary(field: &CustomField) -> String {
    let kinds: Vec<String> = field.shown_on.iter().map(|kind| tr(kind.label())).collect();
    format!("{} · {}", kinds.join(", "), tr(kind_label(&field.kind)))
}

/// "Required for: Fuel-up", or that nothing requires it.
fn field_required_text(field: &CustomField) -> String {
    if field.required_for.is_empty() {
        return tr(messages::METER_REQUIRED_NONE);
    }
    let kinds: Vec<String> = field
        .required_for
        .iter()
        .map(|kind| tr(kind.label()))
        .collect();
    tr_with(
        messages::METER_REQUIRED_FOR,
        &[("kinds", MessageArg::from(kinds.join(", ")))],
    )
}

/// Checkboxes for kinds of events (a meter's or field's Required for, a field's Shown on).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn KindChecks(
    label: MessageId,
    kinds: Vec<EventKind>,
    chosen: Signal<Vec<EventKind>>,
    note: MessageId,
) -> Element {
    let mut chosen = chosen;
    rsx! {
        fieldset { class: "field required-for",
            legend { class: "field-label", {tr(label)} }
            for kind in kinds {
                label { key: "{event_value(kind)}", class: "switch",
                    input {
                        r#type: "checkbox",
                        checked: chosen().contains(&kind),
                        onchange: move |event| {
                            let mut list = chosen();
                            list.retain(|have| *have != kind);
                            if event.checked() {
                                list.push(kind);
                            }
                            chosen.set(list);
                        },
                    }
                    span { {tr(kind.label())} }
                }
            }
            p { class: "muted-note", {tr(note)} }
        }
    }
}

/// Custom fields: each with its event, type, and key; Edit and Remove.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FieldsSection(view: AssetView) -> Element {
    let mut editing = use_signal(|| None::<Editing<CustomFieldId, CustomField>>);
    let units = crate::ui::units::use_units();
    let can = view.can_configure;
    let kinds = event_kinds(&view.asset);
    rsx! {
        section { class: "setup-section",
            SectionHeading { title: messages::SETUP_FIELDS,
                if can {
                    button { class: "secondary-button", onclick: move |_| editing.set(Some(Editing::Adding)), {tr(messages::SETUP_ADD_FIELD)} }
                }
            }
            if view.fields.is_empty() {
                p { class: "muted-note", {tr(messages::SETUP_FIELDS_NONE)} }
            }
            for (field_id, field) in view.fields.clone() {
                div { key: "{field_id}", class: if field.enabled { "setup-row" } else { "setup-row off" },
                    div { class: "setup-row-text",
                        span { class: "setup-row-name",
                            {field.label.as_str().to_owned()}
                            if !field.enabled {
                                span { class: "badge", {tr(messages::FIELD_OFF)} }
                            }
                        }
                        span { class: "setup-row-detail", {field_summary(&field)} }
                        span { class: "setup-row-detail", {field_required_text(&field)} }
                        span { class: "setup-row-key mono", {field.key.as_str().to_owned()} }
                    }
                    if can {
                        RowButtons {
                            on_edit: {
                                let field = field.clone();
                                move |()| editing.set(Some(Editing::Changing(field_id, field.clone())))
                            },
                            on_remove: {
                                let name = field.label.as_str().to_owned();
                                move |()| editing.set(Some(Editing::Removing(field_id, name.clone())))
                            },
                        }
                    }
                }
            }
        }
        match editing() {
            None => rsx! {},
            Some(Editing::Adding) => rsx! {
                FieldDialog { asset: view.id, kinds, current: None, units: units(), on_close: move |()| editing.set(None) }
            },
            Some(Editing::Changing(id, field)) => rsx! {
                FieldDialog { asset: view.id, kinds, current: Some((id, field)), units: units(), on_close: move |()| editing.set(None) }
            },
            Some(Editing::Removing(id, name)) => rsx! {
                ConfirmRemove { target: Removal::Field(id), name, on_close: move |()| editing.set(None) }
            },
        }
    }
}

/// A field type from the add form's choices.
fn kind_from_form(value: &str, decimals: u8, choices: &str) -> Result<FieldKind, MessageId> {
    Ok(match value {
        "number" => FieldKind::Number { decimals },
        "date" => FieldKind::Date,
        "yes-no" => FieldKind::YesNo,
        "choice" => FieldKind::Choice {
            options: choices
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(Label::parse)
                .collect::<Result<_, _>>()
                .map_err(domain::label::LabelError::message)?,
        },
        "distance" => FieldKind::Distance,
        "duration" => FieldKind::Duration,
        "volume" => FieldKind::Volume,
        "fuel-economy" => FieldKind::FuelEconomy,
        _ => FieldKind::Text,
    })
}

/// A field type's `<option>` value.
fn kind_value(kind: &FieldKind) -> &'static str {
    KINDS
        .iter()
        .find(|(_, label)| *label == kind_label(kind))
        .map_or("text", |(value, _)| value)
}

/// A range end as typed for a field of `kind`, in the person's `units`, as the stored value
/// (`None` when empty). Numbers keep their decimals; distances, durations, volumes, and fuel
/// economy are typed in the person's units (#007).
fn parse_range_end(
    kind: &FieldKind,
    text: &str,
    units: Preferences,
) -> Result<Option<i64>, MessageId> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let quantity =
        |result: Result<i64, QuantityError>| result.map(Some).map_err(QuantityError::message);
    match kind {
        FieldKind::Number { decimals } => {
            parse_scaled(text, *decimals).map_err(|()| messages::FIELD_RANGE_BAD_NUMBER)
        }
        FieldKind::Distance => quantity(parse_distance(text, units.distance)),
        FieldKind::Duration => quantity(parse_duration(text)),
        FieldKind::Volume => quantity(parse_volume(text, units.volume)),
        FieldKind::FuelEconomy => quantity(parse_economy(text, units.economy)),
        FieldKind::Text | FieldKind::Date | FieldKind::YesNo | FieldKind::Choice { .. } => Ok(None),
    }
}

/// A stored range end for a field of `kind`, as the person types it in their `units`.
fn range_end_text(kind: &FieldKind, value: i64, units: Preferences) -> String {
    match kind {
        FieldKind::Number { decimals } => show_scaled(value, *decimals),
        FieldKind::Distance => distance_value(value, units.distance),
        FieldKind::Duration => show_duration(value),
        FieldKind::Volume => volume_value(value, units.volume),
        FieldKind::FuelEconomy => economy_value(value, units.economy),
        FieldKind::Text | FieldKind::Date | FieldKind::YesNo | FieldKind::Choice { .. } => {
            String::new()
        }
    }
}

/// The range as typed, for a numeric field, in the person's `units`. In L/100 km, more is
/// worse, so the typed lowest and highest swap when stored (meters per liter).
fn range_from_form(
    kind: &FieldKind,
    (min, max): (&str, &str),
    units: Preferences,
) -> Result<Option<FieldRange>, MessageId> {
    if !kind.is_numeric() {
        return Ok(None);
    }
    let mut low = parse_range_end(kind, min, units)?;
    let mut high = parse_range_end(kind, max, units)?;
    if matches!(kind, FieldKind::FuelEconomy) && economy_is_inverse(units.economy) {
        core::mem::swap(&mut low, &mut high);
    }
    if low.is_none() && high.is_none() {
        return Ok(None);
    }
    FieldRange::new(low, high)
        .map(Some)
        .map_err(domain::custom_field::CustomFieldError::message)
}

/// The field form's values, as signals.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FieldInputs {
    shown_on: Signal<Vec<EventKind>>,
    label: Signal<String>,
    kind: Signal<&'static str>,
    decimals: Signal<u8>,
    choices: Signal<String>,
    enabled: Signal<bool>,
    required_for: Signal<Vec<EventKind>>,
    min: Signal<String>,
    max: Signal<String>,
    /// The person's units, for the range.
    units: Signal<Preferences>,
}

impl FieldInputs {
    /// The form as the server takes it.
    fn form(self, kept: Option<&CustomField>) -> Result<FieldForm, MessageId> {
        let kind = match kept {
            Some(field) => field.kind.clone(),
            None => kind_from_form((self.kind)(), (self.decimals)(), &(self.choices)())?,
        };
        let range = range_from_form(&kind, (&(self.min)(), &(self.max)()), (self.units)())?;
        Ok(FieldForm {
            shown_on: (self.shown_on)(),
            label: (self.label)(),
            kind,
            enabled: (self.enabled)(),
            required_for: (self.required_for)(),
            range,
        })
    }
}

/// Add a custom field, or change one (its label, where it's shown and required, on/off, and a
/// number's range).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FieldDialog(
    asset: AssetId,
    kinds: Vec<EventKind>,
    current: Option<(CustomFieldId, CustomField)>,
    /// The person's units (loaded with Setup), for the range.
    units: Preferences,
    on_close: EventHandler<()>,
) -> Element {
    let page = use_vehicle_page();
    let existing = current.as_ref().map(|(_, field)| field.clone());
    let decimals_now = match existing.as_ref().map(|field| &field.kind) {
        Some(FieldKind::Number { decimals }) => *decimals,
        _ => 0,
    };
    // In L/100 km the stored lowest is the typed highest (more is worse).
    let inverse = existing
        .as_ref()
        .is_some_and(|field| matches!(field.kind, FieldKind::FuelEconomy))
        && economy_is_inverse(units.economy);
    let range_end = |low: bool| {
        let end = |range: &FieldRange| if low == inverse { range.max } else { range.min };
        existing
            .as_ref()
            .and_then(|field| {
                field
                    .range
                    .as_ref()
                    .and_then(end)
                    .map(|value| range_end_text(&field.kind, value, units))
            })
            .unwrap_or_default()
    };
    let start_min = range_end(true);
    let start_max = range_end(false);
    let inputs = FieldInputs {
        shown_on: use_signal(|| {
            existing.as_ref().map_or_else(
                || kinds.first().copied().into_iter().collect(),
                |field| field.shown_on.clone(),
            )
        }),
        label: use_signal(|| {
            existing
                .as_ref()
                .map(|field| field.label.as_str().to_owned())
                .unwrap_or_default()
        }),
        kind: use_signal(|| {
            existing
                .as_ref()
                .map_or("text", |field| kind_value(&field.kind))
        }),
        decimals: use_signal(|| decimals_now),
        choices: use_signal(String::new),
        enabled: use_signal(|| existing.as_ref().is_none_or(|field| field.enabled)),
        required_for: use_signal(|| {
            existing
                .as_ref()
                .map(|field| field.required_for.clone())
                .unwrap_or_default()
        }),
        min: use_signal(|| start_min),
        max: use_signal(|| start_max),
        units: use_signal(|| units),
    };
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let editing_id = current.as_ref().map(|(id, _)| *id);
    let kept = existing.clone();
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let form = match inputs.form(kept.as_ref()) {
            Ok(form) => form,
            Err(problem) => {
                error.set(Some(tr(problem)));
                return;
            }
        };
        busy.set(true);
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
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(if existing.is_none() { messages::FIELD_ADD_TITLE } else { messages::FIELD_EDIT_TITLE })} }
                Field { label: messages::FIELD_LABEL, value: inputs.label, autofocus: true }
                FieldTypeInputs { inputs, existing: existing.clone() }
                FieldOptionInputs { inputs, kinds, existing }
                FormError { message: error() }
                SaveButtons { busy: busy(), on_close }
            }
        }
    }
}

/// The field form's event and type (chosen when adding; fixed after).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FieldTypeInputs(inputs: FieldInputs, existing: Option<CustomField>) -> Element {
    let FieldInputs {
        mut kind,
        mut decimals,
        mut choices,
        ..
    } = inputs;
    if let Some(field) = existing {
        return rsx! {
            p { {tr(kind_label(&field.kind))} }
            p { class: "muted-note", {tr(messages::FIELD_KIND_FIXED)} }
        };
    }
    rsx! {
        div { class: "field-row",
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_TYPE)} }
                select {
                    class: "field-input",
                    onchange: move |event| {
                        if let Some((value, _)) = KINDS.iter().find(|(value, _)| *value == event.value()) {
                            kind.set(value);
                        }
                    },
                    for (value, label) in KINDS {
                        option { key: "{value}", value, selected: kind() == value, {tr(label)} }
                    }
                }
            }
        }
        if kind() == "number" {
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_PRECISION)} }
                select {
                    class: "field-input",
                    onchange: move |event| decimals.set(event.value().parse().unwrap_or(0)),
                    for places in 0..=MAX_DECIMALS {
                        option { key: "{places}", value: "{places}", selected: decimals() == places, "{places}" }
                    }
                }
            }
        }
        if kind() == "choice" {
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_CHOICES)} }
                textarea { class: "field-input", rows: "4", value: choices(), oninput: move |event| choices.set(event.value()) }
            }
        }
    }
}

/// The field form's on/off, a number's range, and its key.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn FieldOptionInputs(
    inputs: FieldInputs,
    /// The vehicle's kinds of events.
    kinds: Vec<EventKind>,
    existing: Option<CustomField>,
) -> Element {
    let mut enabled = inputs.enabled;
    // Only kinds it's shown on can require it.
    let shown: Vec<EventKind> = kinds
        .iter()
        .copied()
        .filter(|kind| (inputs.shown_on)().contains(kind))
        .collect();
    // The field's kind (fixed once it exists; chosen while adding), for its range.
    let kind = existing.as_ref().map_or_else(
        || kind_from_form((inputs.kind)(), (inputs.decimals)(), "x").unwrap_or(FieldKind::Text),
        |field| field.kind.clone(),
    );
    let numeric = kind.is_numeric();
    // A duration is typed as digits, with its `:` put in (phone number pads have none).
    let duration = matches!(kind, FieldKind::Duration);
    let unit = crate::ui::units::field_unit(&kind, (inputs.units)());
    let with_unit = |label| {
        unit.as_ref()
            .map_or_else(|| tr(label), |unit| format!("{} ({unit})", tr(label)))
    };
    rsx! {
        label { class: "switch",
            input { r#type: "checkbox", checked: enabled(), onchange: move |event| enabled.set(event.checked()) }
            span { {tr(messages::FIELD_ENABLED)} }
        }
        KindChecks { label: messages::FIELD_SHOWN_ON, kinds, chosen: inputs.shown_on, note: messages::FIELD_KIND_NOTE }
        if !shown.is_empty() {
            KindChecks { label: messages::METER_REQUIRED_LABEL, kinds: shown, chosen: inputs.required_for, note: messages::FIELD_REQUIRED_FOR_NOTE }
        }
        if numeric {
            div { class: "field-row",
                Field { label: messages::FIELD_RANGE_MIN, label_text: with_unit(messages::FIELD_RANGE_MIN), value: inputs.min, duration }
                Field { label: messages::FIELD_RANGE_MAX, label_text: with_unit(messages::FIELD_RANGE_MAX), value: inputs.max, duration }
            }
            p { class: "muted-note", {tr(messages::FIELD_RANGE_NOTE)} }
        }
        div { class: "field",
            span { class: "field-label", {tr(messages::FIELD_KEY)} }
            match existing {
                Some(field) => rsx! {
                    span { class: "field-key mono", {field.key.as_str().to_owned()} }
                    span { class: "muted-note", {tr(messages::FIELD_KEY_NOTE)} }
                },
                None => rsx! { span { class: "muted-note", {tr(messages::FIELD_KEY_LATER)} } },
            }
        }
    }
}

// --- Group and status --------------------------------------------------------------------

/// What's open in the Group and status section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupDialog {
    Move,
    Delete,
}

/// The group (and Move), the status menu, and Delete.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn GroupAndStatus(view: AssetView) -> Element {
    let context = use_vehicles();
    let page = use_vehicle_page();
    let mut dialog = use_signal(|| None::<GroupDialog>);
    let mut error = use_signal(|| None::<String>);
    let groups = context
        .garage
        .read()
        .as_ref()
        .map(|garage| garage.groups.clone())
        .unwrap_or_default();
    let group = groups
        .iter()
        .find(|summary| summary.id == view.asset.group)
        .cloned();
    let id = view.id;
    let asset = view.asset.clone();
    let set_status = move |event: Event<FormData>| {
        let Some(status) = LifecycleStatus::ALL
            .into_iter()
            .find(|status| status_value(*status) == event.value())
        else {
            return;
        };
        let form = AssetForm {
            status,
            ..AssetForm::from_asset(&asset)
        };
        spawn(async move {
            let result = api::edit_vehicle(id, form).await;
            error.set(failure(&result));
            changed(page, context).await;
        });
    };
    rsx! {
        section { class: "setup-section",
            SectionHeading { title: messages::SETUP_GROUP_STATUS }
            div { class: "setup-row",
                if let Some(summary) = group {
                    span { class: "badge vehicle-group-badge",
                        IconView { icon: group_icon(summary.group.icon) }
                        {summary.group.name.as_str().to_owned()}
                    }
                }
                if view.can_move {
                    button { class: "secondary-button", onclick: move |_| dialog.set(Some(GroupDialog::Move)), {tr(messages::SETUP_MOVE)} }
                }
            }
            div { class: "setup-row",
                if view.can_manage {
                    label { class: "field status-field",
                        span { class: "field-label", {tr(messages::SETUP_STATUS)} }
                        select { class: "field-input", onchange: set_status,
                            for status in LifecycleStatus::ALL {
                                option { key: "{status_value(status)}", value: status_value(status), selected: view.asset.status == status, {tr(status_label(status))} }
                            }
                        }
                    }
                    button { class: "secondary-button danger-text", onclick: move |_| dialog.set(Some(GroupDialog::Delete)), {tr(messages::SETUP_DELETE)} }
                } else {
                    span { {format!("{}: {}", tr(messages::SETUP_STATUS), tr(status_label(view.asset.status)))} }
                }
            }
            FormError { message: error() }
        }
        match dialog() {
            None => rsx! {},
            Some(GroupDialog::Move) => rsx! {
                MoveDialog { view, groups, on_close: move |()| dialog.set(None) }
            },
            Some(GroupDialog::Delete) => rsx! {
                DeleteDialog { view, on_close: move |()| dialog.set(None) }
            },
        }
    }
}

/// A role, or "no access".
fn access_text(role: Option<domain::access::Role>) -> String {
    role.map_or_else(|| tr(messages::ACCESS_NONE), |role| tr(role_label(role)))
}

/// Asks the server who'd be affected by moving `id` to `to`.
async fn preview(
    id: AssetId,
    to: GroupId,
    mut changes: Signal<Option<Vec<MoveChange>>>,
    mut error: Signal<Option<String>>,
) {
    changes.set(None);
    let result = api::preview_vehicle_move(id, to).await;
    error.set(failure(&result));
    if let Ok(AssetsOutcome::MovePreview { changes: found }) = result {
        changes.set(Some(found));
    }
}

/// Move to another group: pick it, see who gains or loses access, then move.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MoveDialog(view: AssetView, groups: Vec<GroupSummary>, on_close: EventHandler<()>) -> Element {
    let page = use_vehicle_page();
    let vehicles = use_vehicles();
    let others: Vec<GroupSummary> = groups
        .into_iter()
        .filter(|summary| summary.id != view.asset.group)
        .collect();
    let first = others.first().map(|summary| summary.id);
    let mut to = use_signal(|| first);
    let changes = use_signal(|| None::<Vec<MoveChange>>);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let id = view.id;
    use_effect(move || {
        if let Some(group) = to() {
            spawn(preview(id, group, changes, error));
        }
    });
    let do_move = move |_| {
        let Some(group) = to() else {
            return;
        };
        busy.set(true);
        spawn(async move {
            let result = api::move_vehicle(id, group).await;
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                changed(page, vehicles).await;
                on_close.call(());
            }
        });
    };
    let name = view.asset.name.as_str().to_owned();
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "dialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr_with(messages::MOVE_TITLE, &[("name", MessageArg::from(name))])} }
                if others.is_empty() {
                    p { class: "dialog-text", {tr(messages::MOVE_NO_OTHER_GROUP)} }
                } else {
                    label { class: "field",
                        span { class: "field-label", {tr(messages::MOVE_TO)} }
                        select { class: "field-input", onchange: move |event| to.set(event.value().parse().ok()),
                            for summary in others.iter() {
                                option { key: "{summary.id}", value: "{summary.id}", selected: to() == Some(summary.id), {summary.group.name.as_str().to_owned()} }
                            }
                        }
                    }
                    h3 { class: "section-heading", {tr(messages::MOVE_AFFECTED).to_uppercase()} }
                    match changes() {
                        None => rsx! { p { class: "muted-note", {tr(messages::LOADING)} } },
                        Some(list) if list.is_empty() => rsx! { p { class: "muted-note", {tr(messages::MOVE_NOBODY)} } },
                        Some(list) => rsx! {
                            ul { class: "move-changes",
                                for change in list {
                                    li { key: "{change.user}",
                                        {tr_with(messages::MOVE_CHANGE, &[
                                            ("name", MessageArg::from(change.display_name.clone())),
                                            ("before", MessageArg::from(access_text(change.before))),
                                            ("after", MessageArg::from(access_text(change.after))),
                                        ])}
                                    }
                                }
                            }
                        },
                    }
                }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    if !others.is_empty() {
                        button { class: "primary-button", disabled: busy() || changes().is_none(), onclick: do_move,
                            if busy() { {tr(messages::MOVE_MOVING)} } else { {tr(messages::MOVE_MOVE)} }
                        }
                    }
                }
            }
        }
    }
}

/// Delete, for a vehicle added by mistake: type its name to confirm.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DeleteDialog(view: AssetView, on_close: EventHandler<()>) -> Element {
    let context = use_vehicles();
    let navigator = use_navigator();
    let typed = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let id = view.id;
    let name = &view.asset.name;
    let matches = confirms_name(&typed(), name);
    let delete = move |_| {
        busy.set(true);
        spawn(async move {
            let result = api::delete_vehicle(id, typed()).await;
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                let _ = refresh_garage(context).await;
                navigator.push(Route::AllVehicles {});
            }
        });
    };
    let shown = name.as_str().to_owned();
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "alertdialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr_with(messages::DELETE_TITLE, &[("name", MessageArg::from(shown.clone()))])} }
                p { class: "dialog-text", {tr(messages::DELETE_TEXT)} }
                Field {
                    label: messages::DELETE_TYPE,
                    label_text: tr_with(messages::DELETE_TYPE, &[("name", MessageArg::from(shown))]),
                    value: typed,
                    autofocus: true,
                }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button filled", disabled: busy() || !matches, onclick: delete,
                        if busy() { {tr(messages::DELETE_DELETING)} } else { {tr(messages::DELETE_DELETE)} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::preferences::{DistanceUnit, EconomyUnit, VolumeUnit};

    #[test]
    fn range_ends_are_typed_with_the_fields_decimals() {
        assert_eq!(parse_scaled("", 1), Ok(None));
        assert_eq!(parse_scaled(" 12.5 ", 1), Ok(Some(125)));
        assert_eq!(parse_scaled("12", 2), Ok(Some(1200)));
        assert_eq!(parse_scaled("-0.25", 2), Ok(Some(-25)));
        assert_eq!(parse_scaled(".5", 1), Ok(Some(5)));
        for bad in ["1.25", "abc", "1.2.3", "-", ".", "1e3"] {
            assert_eq!(parse_scaled(bad, 1), Err(()), "{bad}");
        }
        assert_eq!(show_scaled(125, 1), "12.5");
        assert_eq!(show_scaled(5, 2), "0.05");
        assert_eq!(show_scaled(-25, 2), "-0.25");
        assert_eq!(show_scaled(40, 0), "40");
    }

    #[test]
    fn a_number_fields_range_comes_from_the_form() {
        let units = Preferences::default();
        let number = FieldKind::Number { decimals: 1 };
        assert_eq!(
            range_from_form(&number, ("0", "40.5"), units),
            Ok(Some(FieldRange::new(Some(0), Some(405)).unwrap()))
        );
        assert_eq!(range_from_form(&number, ("", ""), units), Ok(None));
        assert_eq!(
            range_from_form(&number, ("9", "1"), units),
            Err(messages::FIELD_BAD_RANGE)
        );
        assert_eq!(
            range_from_form(&FieldKind::Text, ("1", "2"), units),
            Ok(None)
        );
    }

    #[test]
    fn unit_fields_ranges_are_typed_in_the_persons_units() {
        let miles = Preferences::default();
        let metric = Preferences {
            distance: DistanceUnit::Kilometers,
            volume: VolumeUnit::Liters,
            economy: EconomyUnit::LitersPer100Km,
            ..Preferences::default()
        };
        // A trip distance up to 300 mi, stored in meters, shown back in km.
        let range = range_from_form(&FieldKind::Distance, ("", "300"), miles)
            .unwrap()
            .unwrap();
        assert_eq!(range.max, Some(482_803));
        assert_eq!(
            range_end_text(&FieldKind::Distance, 482_803, metric),
            "482.8"
        );
        // Durations as H:MM.
        let range = range_from_form(&FieldKind::Duration, ("0:05", "12:00"), miles)
            .unwrap()
            .unwrap();
        assert_eq!((range.min, range.max), (Some(300), Some(43_200)));
        // In L/100 km, the typed lowest (4) is the stored highest (25,000 m/L).
        let range = range_from_form(&FieldKind::FuelEconomy, ("4", "20"), metric)
            .unwrap()
            .unwrap();
        assert_eq!((range.min, range.max), (Some(5_000), Some(25_000)));
        assert_eq!(
            range_from_form(&FieldKind::Volume, ("lots", ""), miles),
            Err(messages::QUANTITY_NOT_A_NUMBER)
        );
        assert_eq!(
            crate::ui::units::field_unit(&FieldKind::Volume, metric).as_deref(),
            Some("L")
        );
        assert_eq!(
            crate::ui::units::field_unit(&FieldKind::Number { decimals: 0 }, metric),
            None
        );
    }

    #[test]
    fn every_field_type_round_trips_through_the_form() {
        for (value, _) in KINDS {
            let kind = kind_from_form(value, 2, "Red\nBlue").unwrap();
            assert_eq!(kind_value(&kind), value);
        }
        assert_eq!(
            kind_from_form("choice", 0, "Red\n\nBlue").unwrap(),
            FieldKind::Choice {
                options: vec![Label::parse("Red").unwrap(), Label::parse("Blue").unwrap()]
            }
        );
    }
}
