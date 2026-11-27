//! Manage → Vehicle types (decision #090; sketch part 4 of
//! `docs/ui-sketches/p6m3-vehicles-schedules-options.svg`): the household's own types, each
//! built on one of the app's kinds. Laid out like Groups: the list, then the chosen type (its
//! name, icon, kind, documents, what it starts with, template, and Delete). Everyone sees them;
//! the household's Admin, in Admin mode, changes them, and a change reaches only vehicles added
//! afterwards.

use dioxus::prelude::*;
use domain::api::{TypesOutcome, TypesView};
use domain::asset::{AssetId, AssetKind};
use domain::catalogue::{CatalogueChoice, TireAxle, catalogue};
use domain::document::DocumentType;
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::label::Label;
use domain::profile::{Profile, profile, profiles};
use domain::template::TemplateId;
use domain::vehicle_type::{TypeIcon, VehicleType, VehicleTypeId};

use super::problem_from_error;
use crate::ui::icons::IconView;
use crate::ui::vehicles::{type_icon, use_vehicles};
use crate::ui::{tr, tr_with};
use crate::{api, data};

/// A number as a Fluent argument.
fn count(value: usize) -> MessageArg {
    MessageArg::from(i64::try_from(value).unwrap_or(i64::MAX))
}

/// A message named by data (a profile's or the catalogue's).
fn tr_key(key: &str) -> String {
    Translator::english().tr_key(key)
}

/// A type icon's name.
pub(crate) const fn icon_label(icon: TypeIcon) -> MessageId {
    match icon {
        TypeIcon::Car => messages::TYPE_ICON_CAR,
        TypeIcon::Truck => messages::TYPE_ICON_TRUCK,
        TypeIcon::Motorcycle => messages::TYPE_ICON_MOTORCYCLE,
        TypeIcon::Trailer => messages::TYPE_ICON_TRAILER,
        TypeIcon::Generator => messages::TYPE_ICON_GENERATOR,
        TypeIcon::Tractor => messages::TYPE_ICON_TRACTOR,
        TypeIcon::Boat => messages::TYPE_ICON_BOAT,
        TypeIcon::Aircraft => messages::TYPE_ICON_AIRCRAFT,
        TypeIcon::Bus => messages::TYPE_ICON_BUS,
        TypeIcon::Van => messages::TYPE_ICON_VAN,
        TypeIcon::Bicycle => messages::TYPE_ICON_BICYCLE,
        TypeIcon::Flag => messages::TYPE_ICON_FLAG,
        TypeIcon::Tool => messages::TYPE_ICON_TOOL,
    }
}

/// A document type's name.
fn document_name(document: &DocumentType) -> String {
    match document {
        DocumentType::Listed { key } => tr_key(key),
        DocumentType::Custom { name } => name.as_str().to_owned(),
    }
}

/// A catalogue choice's name: a set's or field's, or "Tires: 2 axles, 6 tires".
fn choice_name(choice: &CatalogueChoice) -> String {
    let Ok(catalogue) = catalogue() else {
        return String::new();
    };
    match choice {
        CatalogueChoice::Field { key } => catalogue
            .field(key)
            .map_or_else(|| key.as_str().to_owned(), |field| tr_key(&field.label)),
        CatalogueChoice::Set { id } => catalogue
            .set(id)
            .map_or_else(|| id.clone(), |set| tr_key(&set.name)),
        CatalogueChoice::Tires { axles } => tr_with(
            messages::TYPES_TIRES,
            &[
                ("axles", count(axles.len())),
                ("tires", count(axles.iter().map(|axle| axle.tires()).sum())),
            ],
        ),
    }
}

/// The Tires layouts offered for a type: 1 to 4 axles, duals behind the front on 2 to 4.
fn tire_layouts() -> Vec<Vec<TireAxle>> {
    let mut layouts = Vec::new();
    for axles in 1..=domain::catalogue::MAX_AXLES {
        layouts.push(vec![TireAxle::Single; axles]);
        if axles > 1 {
            let mut duals = vec![TireAxle::Dual; axles];
            duals[0] = TireAxle::Single;
            layouts.push(duals);
        }
    }
    layouts
}

/// Every catalogue choice offered: the sets, the Tires layouts, then single fields.
fn catalogue_choices() -> Vec<CatalogueChoice> {
    let Ok(catalogue) = catalogue() else {
        return Vec::new();
    };
    catalogue
        .sets
        .iter()
        .map(|set| CatalogueChoice::Set { id: set.id.clone() })
        .chain(
            tire_layouts()
                .into_iter()
                .map(|axles| CatalogueChoice::Tires { axles }),
        )
        .chain(catalogue.fields.iter().map(|field| CatalogueChoice::Field {
            key: field.key.clone(),
        }))
        .collect()
}

/// What a type's vehicles start with: "Needs Insurance · 1 meters · 10 fields · no template".
pub(crate) fn type_summary(kept: &VehicleType, templates: &[(TemplateId, Label)]) -> String {
    let documents = if kept.documents.is_empty() {
        tr(messages::TYPE_NEEDS_NOTHING)
    } else {
        kept.documents
            .iter()
            .map(document_name)
            .collect::<Vec<_>>()
            .join(", ")
    };
    let meters = profile(&kept.kind).map_or(0, |profile| profile.meters.len());
    // Any vehicle ID will do: only the count is wanted.
    let fields = AssetId::generate(
        domain::time::UnixMillis::EPOCH,
        [0; domain::id::RANDOM_BYTES],
    )
    .ok()
    .and_then(|anyone| kept.new_fields(anyone, &[], Translator::english()).ok())
    .map_or(0, |fields| fields.len());
    let template = kept
        .template
        .and_then(|id| templates.iter().find(|(each, _)| *each == id))
        .map_or_else(
            || tr(messages::TYPE_NO_TEMPLATE),
            |(_, name)| {
                tr_with(
                    messages::TYPE_WITH_TEMPLATE,
                    &[("name", MessageArg::from(name.as_str()))],
                )
            },
        );
    tr_with(
        messages::TYPE_SUMMARY,
        &[
            ("documents", MessageArg::from(documents.as_str())),
            ("meters", count(meters)),
            ("fields", count(fields)),
            ("template", MessageArg::from(template.as_str())),
        ],
    )
}

/// A types answer that isn't what was asked for, as a message.
fn refusal(outcome: &TypesOutcome) -> MessageId {
    match outcome {
        TypesOutcome::Invalid { problem } => problem.message(),
        TypesOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        TypesOutcome::NotFound => messages::TYPES_NOT_FOUND,
        TypesOutcome::Types { .. }
        | TypesOutcome::Saved { .. }
        | TypesOutcome::Done
        | TypesOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// Which type is open on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chosen {
    /// A new one.
    Adding,
    /// One kept.
    Kept(VehicleTypeId),
}

/// `/vehicle-types`.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn VehicleTypes() -> Element {
    let context = use_vehicles();
    let mut reloads = use_signal(|| 0_u32);
    let mut chosen = use_signal(|| None::<Chosen>);
    let loaded = use_resource(move || async move {
        let _ = reloads();
        data::list_vehicle_types().await
    });
    let view: TypesView = match &*loaded.read() {
        None => return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Ok(TypesOutcome::Types { view })) => view.clone(),
        Some(Ok(other)) => {
            return rsx! { p { class: "form-error", role: "alert", {tr(refusal(other))} } };
        }
        Some(Err(error)) => {
            return rsx! { p { class: "form-error", role: "alert", {tr(problem_from_error(error).message())} } };
        }
    };
    let assets = context
        .garage
        .read()
        .as_ref()
        .map(|garage| garage.assets.clone())
        .unwrap_or_default();
    let vehicles_of = move |id: VehicleTypeId| {
        assets
            .iter()
            .filter(|item| item.asset.vehicle_type == Some(id))
            .count()
    };
    let open = match chosen() {
        // A new one starts as a car, named "New type".
        Some(Chosen::Adding) => Label::parse(&tr(messages::TYPES_NEW))
            .ok()
            .zip(AssetKind::parse("car").ok())
            .map(|(name, kind)| (None, VehicleType::from_kind(name, kind))),
        Some(Chosen::Kept(id)) => view
            .types
            .iter()
            .find(|(each, _)| *each == id)
            .map(|(id, kept)| (Some(*id), kept.clone())),
        None => None,
    };
    let layout = if open.is_some() {
        "users has-selection"
    } else {
        "users"
    };
    rsx! {
        div { class: layout,
            div { class: "users-list",
                p { class: "muted-note", {tr(messages::TYPES_INTRO)} }
                if view.may_manage {
                    button { class: "primary-button", onclick: move |_| chosen.set(Some(Chosen::Adding)), {tr(messages::TYPES_ADD)} }
                }
                if view.types.is_empty() {
                    p { class: "placeholder", {tr(messages::TYPES_NONE)} }
                }
                for (id, kept) in view.types.clone() {
                    button {
                        key: "{id}",
                        class: if chosen() == Some(Chosen::Kept(id)) { "user-row selected" } else { "user-row" },
                        onclick: move |_| chosen.set(Some(Chosen::Kept(id))),
                        span { class: "group-row",
                            IconView { icon: type_icon(kept.icon) }
                            span { class: "user-row-text",
                                strong { {kept.name.as_str().to_owned()} }
                                span { class: "user-row-role", {tr_with(messages::VEHICLES_COUNT, &[("count", count(vehicles_of(id)))])} }
                            }
                        }
                    }
                }
                p { class: "muted-note", {tr(messages::TYPES_KINDS_NOTE)} }
            }
            div { class: "users-detail",
                match open {
                    Some((id, kept)) => rsx! {
                        TypeEditor {
                            key: "{id:?}",
                            id,
                            start: kept,
                            templates: view.templates.clone(),
                            may_manage: view.may_manage,
                            on_saved: move |saved: VehicleTypeId| {
                                reloads += 1;
                                chosen.set(Some(Chosen::Kept(saved)));
                            },
                            on_removed: move |()| {
                                reloads += 1;
                                chosen.set(None);
                            },
                            on_back: move |()| chosen.set(None),
                        }
                    },
                    None => rsx! { p { class: "placeholder", {tr(messages::TYPES_SELECT_HINT)} } },
                }
            }
        }
    }
}

/// One type: everything about it, Save, and Delete (read only unless the person may manage).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn TypeEditor(
    id: Option<VehicleTypeId>,
    start: VehicleType,
    templates: Vec<(TemplateId, Label)>,
    may_manage: bool,
    on_saved: EventHandler<VehicleTypeId>,
    on_removed: EventHandler<()>,
    on_back: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| start.name.as_str().to_owned());
    let mut kept = use_signal(|| start.clone());
    let mut status = use_signal(|| None::<Result<(), String>>);
    let mut busy = use_signal(|| false);
    let mut deleting = use_signal(|| false);
    let current = kept();
    let kind_profile: Option<&'static Profile> = profile(&current.kind);
    let usage_name =
        kind_profile.map_or_else(String::new, |profile| tr_key(&profile.default_usage));
    let save = move |event: FormEvent| {
        event.prevent_default();
        let Ok(label) = Label::parse(&name()) else {
            status.set(Some(Err(tr(messages::LABEL_EMPTY))));
            return;
        };
        let form = VehicleType {
            name: label,
            ..kept()
        };
        busy.set(true);
        spawn(async move {
            let result = match id {
                Some(id) => api::edit_vehicle_type(id, form).await,
                None => api::add_vehicle_type(form).await,
            };
            busy.set(false);
            match result {
                Ok(TypesOutcome::Saved { id }) => {
                    status.set(Some(Ok(())));
                    on_saved.call(id);
                }
                Ok(other) => status.set(Some(Err(tr(refusal(&other))))),
                Err(error) => status.set(Some(Err(tr(problem_from_error(&error).message())))),
            }
        });
    };
    rsx! {
        button { class: "link-button users-back", onclick: move |_| on_back.call(()), {tr(messages::TYPES_BACK)} }
        form { class: "type-editor", onsubmit: save,
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_NAME)} }
                input { class: "field-input", autocomplete: "off", disabled: !may_manage, value: name(), oninput: move |event| name.set(event.value()) }
            }
            div { class: "field",
                span { class: "field-label", {tr(messages::GROUPS_ICON)} }
                div { class: "icon-picker", role: "group", "aria-label": tr(messages::GROUPS_ICON),
                    for each in TypeIcon::ALL {
                        button {
                            key: "{each:?}",
                            r#type: "button",
                            disabled: !may_manage,
                            class: if each == current.icon { "icon-choice chosen" } else { "icon-choice" },
                            "aria-pressed": if each == current.icon { "true" } else { "false" },
                            onclick: move |_| kept.write().icon = each,
                            IconView { icon: type_icon(each) }
                            span { {tr(icon_label(each))} }
                        }
                    }
                }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::TYPES_BUILT_ON)} }
                select {
                    class: "field-input",
                    disabled: !may_manage,
                    onchange: move |event: FormEvent| {
                        if let Ok(kind) = AssetKind::parse(&event.value()) {
                            // A new kind: its own documents and fields, nothing else kept.
                            let renamed = kept().name;
                            kept.set(VehicleType { icon: kept().icon, template: kept().template, ..VehicleType::from_kind(renamed, kind) });
                        }
                    },
                    for each in profiles().unwrap_or_default() {
                        option { key: "{each.id.as_str()}", value: each.id.as_str(), selected: each.id == current.kind, {tr_key(&each.name)} }
                    }
                }
                span { class: "muted-note", {tr(messages::TYPES_KEEPS)} }
            }
            h3 { class: "section-heading", {tr_with(messages::TYPES_DOCUMENTS, &[("usage", MessageArg::from(usage_name.as_str()))]).to_uppercase()} }
            for key in kind_profile.map(|profile| profile.documents.clone()).unwrap_or_default() {
                {
                    let document = DocumentType::listed(&key).ok();
                    let ticked = document.as_ref().is_some_and(|document| current.documents.contains(document));
                    rsx! {
                        label { key: "{key}", class: "check",
                            input {
                                r#type: "checkbox",
                                disabled: !may_manage,
                                checked: ticked,
                                onchange: move |event: FormEvent| {
                                    if let Some(document) = document.clone() {
                                        let mut write = kept.write();
                                        write.documents.retain(|each| *each != document);
                                        if event.checked() {
                                            write.documents.push(document);
                                        }
                                    }
                                },
                            }
                            span { {tr_key(&key)} }
                        }
                    }
                }
            }
            if current.documents.is_empty() {
                p { class: "muted-note", {tr(messages::TYPES_NO_DOCUMENTS)} }
            }
            h3 { class: "section-heading", {tr(messages::TYPES_STARTS_WITH).to_uppercase()} }
            p { class: "muted-note",
                {tr_with(messages::TYPES_METERS, &[("list", MessageArg::from(kind_profile.map(|profile| profile.meters.iter().map(|meter| tr_key(&meter.name)).collect::<Vec<_>>().join(", ")).unwrap_or_default().as_str()))])}
            }
            if let Some(kind_profile) = kind_profile.filter(|profile| !profile.fields.is_empty()) {
                span { class: "field-label", {tr(messages::TYPES_KIND_FIELDS)} }
                for field in kind_profile.fields.clone() {
                    label { key: "{field.key.as_str()}", class: "check",
                        input {
                            r#type: "checkbox",
                            disabled: !may_manage,
                            checked: current.fields_on.contains(&field.key),
                            onchange: {
                                let key = field.key.clone();
                                move |event: FormEvent| {
                                    let mut write = kept.write();
                                    write.fields_on.retain(|each| *each != key);
                                    if event.checked() {
                                        write.fields_on.push(key.clone());
                                    }
                                }
                            },
                        }
                        span { {tr_key(&field.label)} }
                    }
                }
            }
            span { class: "field-label", {tr(messages::TYPES_FROM_CATALOGUE)} }
            if current.catalogue.is_empty() {
                p { class: "muted-note", {tr(messages::TYPES_CATALOGUE_NONE)} }
            }
            for (index, choice) in current.catalogue.iter().enumerate() {
                div { key: "{index}", class: "setup-row",
                    span { class: "setup-row-name", {choice_name(choice)} }
                    if may_manage {
                        button {
                            class: "secondary-button",
                            r#type: "button",
                            onclick: move |_| {
                                kept.write().catalogue.remove(index);
                            },
                            {tr(messages::SPEC_REMOVE)}
                        }
                    }
                }
            }
            if may_manage {
                select {
                    class: "field-input",
                    "aria-label": tr(messages::TYPES_CATALOGUE_PICK),
                    onchange: move |event: FormEvent| {
                        let picked: Option<usize> = event.value().parse().ok();
                        if let Some(choice) = picked.and_then(|index| catalogue_choices().get(index).cloned()) {
                            kept.write().catalogue.push(choice);
                        }
                    },
                    option { value: "", selected: true, {tr(messages::TYPES_CATALOGUE_PICK)} }
                    for (index, choice) in catalogue_choices().iter().enumerate() {
                        option { key: "{index}", value: "{index}", {choice_name(choice)} }
                    }
                }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::TYPES_TEMPLATE)} }
                select {
                    class: "field-input",
                    disabled: !may_manage,
                    onchange: move |event: FormEvent| kept.write().template = event.value().parse().ok(),
                    option { value: "", selected: current.template.is_none(), {tr(messages::TYPES_TEMPLATE_NONE)} }
                    for (template, label) in templates.clone() {
                        option { key: "{template}", value: "{template}", selected: current.template == Some(template), {label.as_str().to_owned()} }
                    }
                }
                span { class: "muted-note", {tr(messages::TYPES_TEMPLATE_NOTE)} }
            }
            p { class: "muted-note", {type_summary(&current, &templates)} }
            match status() {
                Some(Ok(())) => rsx! { p { class: "muted-note", role: "status", {tr(messages::TYPES_SAVED)} } },
                Some(Err(message)) => rsx! { p { class: "form-error", role: "alert", {message} } },
                None => rsx! {},
            }
            if may_manage {
                div { class: "form-buttons",
                    button { class: "primary-button", r#type: "submit", disabled: busy(),
                        if busy() { {tr(messages::ACTION_SAVING)} } else { {tr(messages::ACTION_SAVE)} }
                    }
                }
                if id.is_some() {
                    div { class: "account-actions",
                        button { class: "danger-button", r#type: "button", onclick: move |_| deleting.set(true), {tr(messages::TYPES_DELETE)} }
                    }
                    p { class: "muted-note", {tr(messages::TYPES_DELETE_NOTE)} }
                }
            } else {
                p { class: "muted-note", {tr(messages::TYPES_READ_ONLY)} }
            }
        }
        if let (true, Some(id)) = (deleting(), id) {
            DeleteTypeDialog { id, name: start.name.as_str().to_owned(), on_removed, on_close: move |()| deleting.set(false) }
        }
    }
}

/// "Delete the type Track car?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DeleteTypeDialog(
    id: VehicleTypeId,
    name: String,
    on_removed: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let mut problem = use_signal(|| None::<MessageId>);
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div {
                class: "dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr_with(messages::TYPES_DELETE_TITLE, &[("name", MessageArg::from(name.as_str()))])} }
                p { {tr(messages::TYPES_DELETE_NOTE)} }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button {
                        class: "primary-button danger",
                        onclick: move |_| {
                            spawn(async move {
                                match api::delete_vehicle_type(id).await {
                                    Ok(TypesOutcome::Done) => {
                                        on_close.call(());
                                        on_removed.call(());
                                    }
                                    Ok(other) => problem.set(Some(refusal(&other))),
                                    Err(error) => problem.set(Some(problem_from_error(&error).message())),
                                }
                            });
                        },
                        {tr(messages::TYPES_DELETE)}
                    }
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_a_name_and_a_drawing() {
        for icon in TypeIcon::ALL {
            assert!(Translator::english().has(icon_label(icon)), "{icon:?}");
            let _ = type_icon(icon);
        }
    }

    #[test]
    fn the_tires_layouts_go_up_to_four_axles_with_duals_behind() {
        let layouts = tire_layouts();
        assert_eq!(layouts.len(), 7);
        assert_eq!(
            choice_name(&CatalogueChoice::Tires {
                axles: layouts[2].clone()
            }),
            "Tires: 2 axles, 6 tires"
        );
        assert!(catalogue_choices().len() > layouts.len());
    }

    #[test]
    fn a_type_says_what_it_starts_with() {
        let mut kept = VehicleType::from_kind(
            Label::parse("Track car").unwrap(),
            AssetKind::parse("car").unwrap(),
        );
        kept.documents = vec![DocumentType::listed("document-insurance").unwrap()];
        kept.catalogue = vec![CatalogueChoice::Tires {
            axles: vec![TireAxle::Single; 2],
        }];
        let template: TemplateId = domain::test_support::test_id(5);
        kept.template = Some(template);
        assert_eq!(
            type_summary(&kept, &[(template, Label::parse("Track day").unwrap())]),
            "Needs Insurance · 1 meters · 10 fields · the Track day schedule"
        );
        kept.documents.clear();
        kept.template = None;
        assert!(type_summary(&kept, &[]).starts_with("Needs no documents"));
        assert!(type_summary(&kept, &[]).ends_with("no template"));
    }
}
