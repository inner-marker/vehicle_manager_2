//! **+ From the catalogue** on a vehicle's Setup tab (decision #090; sketch part 3 of
//! `docs/ui-sketches/p6m3-vehicles-schedules-options.svg`): ready-made sets and fields, each
//! copied into the vehicle's own fields. A field it already has is greyed out; **Tires** asks for
//! the axles and the tires on each first.

use dioxus::prelude::*;
use domain::api::AssetView;
use domain::catalogue::{
    CatalogueChoice, CatalogueField, CatalogueSet, MAX_AXLES, TireAxle, catalogue, tire_positions,
};
use domain::i18n::{MessageArg, Translator, messages};
use domain::meter::EventKind;

use super::vehicle::use_vehicle_page;
use super::vehicle_setup::{failure, kind_label};
use crate::api;
use crate::ui::screens::FormError;
use crate::ui::{tr, tr_with};

/// Whether a catalogue field would show on any of the vehicle's kinds of events.
fn fits(field: &CatalogueField, kinds: &[EventKind]) -> bool {
    field.shown_on.iter().any(|kind| kinds.contains(kind))
}

/// A set's summary: "5 fields: Trip distance, Trip time, …".
fn set_summary(set: &CatalogueSet) -> String {
    let Ok(catalogue) = catalogue() else {
        return String::new();
    };
    let labels: Vec<String> = set
        .fields
        .iter()
        .filter_map(|key| catalogue.field(key))
        .map(|field| tr_key(&field.label))
        .collect();
    tr_with(
        messages::CATALOGUE_SET_SUMMARY,
        &[
            (
                "count",
                MessageArg::from(i64::try_from(labels.len()).unwrap_or(i64::MAX)),
            ),
            ("fields", MessageArg::from(labels.join(", ").as_str())),
        ],
    )
}

/// A message named by the catalogue's data.
fn tr_key(key: &str) -> String {
    Translator::english().tr_key(key)
}

/// The catalogue dialog: sets, then fields, or the Tires form.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn CatalogueDialog(
    view: AssetView,
    kinds: Vec<EventKind>,
    on_close: EventHandler<()>,
) -> Element {
    let page = use_vehicle_page();
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let tires = use_signal(|| None::<Vec<TireAxle>>);
    let asset = view.id;
    let mut add = move |choice: CatalogueChoice| {
        busy.set(true);
        spawn(async move {
            let result = api::add_catalogue_fields(asset, choice).await;
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                page.reload();
                on_close.call(());
            }
        });
    };
    let Ok(catalogue) = catalogue() else {
        return rsx! {};
    };
    let title = tr_with(
        messages::CATALOGUE_TITLE,
        &[("name", MessageArg::from(view.asset.name.as_str()))],
    );
    if let Some(axles) = tires() {
        return rsx! {
            TiresForm { title, axles: tires, busy: busy(), error: error(),
                on_add: move |()| add(CatalogueChoice::Tires { axles: axles.clone() }),
                on_close,
            }
        };
    }
    let has = |key: &str| {
        view.fields
            .iter()
            .any(|(_, field)| field.key.as_str() == key)
    };
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog catalogue", role: "dialog", "aria-modal": "true",
                h2 { class: "dialog-title", {title} }
                p { class: "muted-note", {tr(messages::CATALOGUE_INTRO)} }
                h3 { class: "catalogue-heading", {tr(messages::CATALOGUE_SETS)} }
                for set in catalogue.sets.clone() {
                    {
                        let fit = set.fields.iter().filter_map(|key| catalogue.field(key)).any(|field| fits(field, &kinds));
                        let id = set.id.clone();
                        rsx! {
                            CatalogueRow {
                                key: "{set.id}",
                                name: tr_key(&set.name),
                                detail: if fit { set_summary(&set) } else { tr(messages::CATALOGUE_NOT_FOR_VEHICLE) },
                                enabled: fit && !busy(),
                                on_add: move |()| add(CatalogueChoice::Set { id: id.clone() }),
                            }
                        }
                    }
                }
                CatalogueRow {
                    name: tr(messages::CATALOGUE_SET_TIRES),
                    detail: tr(messages::CATALOGUE_SET_TIRES_NOTE),
                    enabled: !busy(),
                    on_add: {
                        let mut tires = tires;
                        move |()| tires.set(Some(vec![TireAxle::Single, TireAxle::Single]))
                    },
                }
                h3 { class: "catalogue-heading", {tr(messages::CATALOGUE_FIELDS)} }
                for field in catalogue.fields.clone() {
                    {
                        let there = has(field.key.as_str());
                        let fit = fits(&field, &kinds);
                        let kind = tr(kind_label(&field.kind));
                        let detail = if there {
                            format!("{kind} · {}", tr(messages::CATALOGUE_ON_VEHICLE))
                        } else if !fit {
                            format!("{kind} · {}", tr(messages::CATALOGUE_NOT_FOR_VEHICLE))
                        } else {
                            kind
                        };
                        let key = field.key.clone();
                        rsx! {
                            CatalogueRow {
                                key: "{field.key.as_str()}",
                                name: tr_key(&field.label),
                                detail,
                                enabled: fit && !there && !busy(),
                                on_add: move |()| add(CatalogueChoice::Field { key: key.clone() }),
                            }
                        }
                    }
                }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CLOSE)} }
                }
            }
        }
    }
}

/// One set or field: its name, what it is, and Add (greyed out when it can't be added).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn CatalogueRow(name: String, detail: String, enabled: bool, on_add: EventHandler<()>) -> Element {
    rsx! {
        div { class: if enabled { "setup-row" } else { "setup-row off" },
            div { class: "setup-row-text",
                span { class: "setup-row-name", {name} }
                span { class: "setup-row-detail", {detail} }
            }
            button { class: "secondary-button", r#type: "button", disabled: !enabled, onclick: move |_| on_add.call(()), {tr(messages::CATALOGUE_ADD)} }
        }
    }
}

/// The Tires set: how many axles, the tires on each, and what that adds.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn TiresForm(
    title: String,
    axles: Signal<Option<Vec<TireAxle>>>,
    busy: bool,
    error: Option<String>,
    on_add: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let mut axles = axles;
    let current = axles().unwrap_or_default();
    let positions = tire_positions(&current);
    let english = Translator::english();
    let names: Vec<String> = positions.iter().map(|tire| tire.name(english)).collect();
    let count = |n: usize| MessageArg::from(i64::try_from(n).unwrap_or(i64::MAX));
    let field_count =
        positions.len() * catalogue().map_or(2, |catalogue| catalogue.tire_fields.len());
    let mut set_count = move |n: usize| {
        if let Some(list) = axles.write().as_mut() {
            list.resize(n, TireAxle::Single);
        }
    };
    rsx! {
        div { class: "dialog-scrim",
            form {
                class: "dialog",
                role: "dialog",
                "aria-modal": "true",
                onsubmit: move |event: FormEvent| {
                    event.prevent_default();
                    on_add.call(());
                },
                h2 { class: "dialog-title", {title} }
                h3 { class: "catalogue-heading", {tr(messages::TIRES_TITLE)} }
                div { class: "field",
                    span { class: "field-label", {tr(messages::TIRES_AXLES)} }
                    div { class: "segmented compact", role: "radiogroup",
                        for n in 1..=MAX_AXLES {
                            button {
                                key: "{n}",
                                class: if n == current.len() { "segment on" } else { "segment" },
                                r#type: "button",
                                role: "radio",
                                "aria-checked": n == current.len(),
                                onclick: move |_| set_count(n),
                                "{n}"
                            }
                        }
                    }
                }
                span { class: "field-label", {tr(messages::TIRES_ON_EACH)} }
                for (index, axle) in current.iter().copied().enumerate() {
                    div { key: "{index}", class: "tire-axle",
                        span { {tr_with(messages::TIRES_AXLE, &[("number", count(index + 1))])} }
                        div { class: "segmented compact", role: "radiogroup",
                            for (choice, label) in [(TireAxle::Single, messages::TIRES_SINGLE), (TireAxle::Dual, messages::TIRES_DUAL)] {
                                button {
                                    key: "{choice:?}",
                                    class: if choice == axle { "segment on" } else { "segment" },
                                    r#type: "button",
                                    role: "radio",
                                    "aria-checked": choice == axle,
                                    onclick: move |_| {
                                        if let Some(list) = axles.write().as_mut()
                                            && let Some(each) = list.get_mut(index)
                                        {
                                            *each = choice;
                                        }
                                    },
                                    {tr(label)}
                                }
                            }
                        }
                    }
                }
                p { class: "muted-note",
                    {tr_with(messages::TIRES_SUMMARY, &[
                        ("tires", count(positions.len())),
                        ("fields", count(field_count)),
                        ("names", MessageArg::from(names.join(", ").as_str())),
                    ])}
                }
                p { class: "muted-note", {tr(messages::TIRES_UNITS_NOTE)} }
                FormError { message: error }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| axles.set(None), {tr(messages::ACTION_BACK)} }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", disabled: busy,
                        {tr_with(messages::TIRES_ADD, &[("fields", count(field_count))])}
                    }
                }
            }
        }
    }
}
