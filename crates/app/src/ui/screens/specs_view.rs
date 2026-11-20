//! A vehicle's specs on the Maintenance tab (Phase 5 milestone 4; decision #084; sketch
//! `docs/ui-sketches/p5m4-specs-options.svg`, 1A and 2A).
//!
//! **Specs** is a view beside Schedule: each spec's name, its amount as entered with the
//! other system beside it ("5.5 qt (5.2 L)"), its detail, part number, and note. Everyone who
//! sees the schedule sees them; whoever may change it gets **+ Spec**, and opens one to change
//! or remove it. The dialog takes a name, a kind (none, volume, torque, pressure, or length),
//! an amount and its unit with the conversion shown as it's typed, a detail, a part number,
//! and a note.

use dioxus::prelude::*;
use domain::api::{AssetView, ScheduleOutcome};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::label::Label;
use domain::log_area;
use domain::notes::Notes;
use domain::spec::{Spec, SpecAmount, SpecId, SpecKind, SpecUnit};

use super::problem_from_error;
use crate::api;
use crate::ui::{tr, tr_with};

/// "5.5 qt (5.2 L) · 5W-30 full synthetic · Part FL-500S": what's known about a spec.
pub(super) fn spec_summary(spec: &Spec) -> String {
    let mut parts = Vec::new();
    if let Some(amount) = spec.amount {
        parts.push(amount.show_both());
    }
    if let Some(detail) = &spec.detail {
        parts.push(detail.clone());
    }
    if let Some(number) = &spec.part_number {
        parts.push(tr_with(
            messages::SPEC_PART_NUMBER_SHOWN,
            &[("number", MessageArg::from(number.as_str()))],
        ));
    }
    parts.join(" · ")
}

/// The Specs view: the list, **+ Spec**, and the dialog.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn SpecsView(
    vehicle: AssetView,
    specs: Vec<(SpecId, Spec)>,
    may_configure: bool,
    reloads: Signal<u32>,
) -> Element {
    // `Some(None)`: a new spec; `Some(Some(..))`: changing one.
    let mut editing = use_signal(|| None::<Option<(SpecId, Spec)>>);
    rsx! {
        section { class: "setup-section specs",
            if may_configure {
                div { class: "setup-row-buttons",
                    button { class: "secondary-button", onclick: move |_| editing.set(Some(None)), {tr(messages::SPEC_ADD)} }
                }
            }
            if specs.is_empty() {
                p { class: "muted-note", {tr(messages::SPEC_NONE_YET)} }
            }
            for (id, spec) in specs.iter() {
                if may_configure {
                    button {
                        key: "{id}",
                        class: "setup-row spec-row",
                        r#type: "button",
                        onclick: {
                            let picked = (*id, spec.clone());
                            move |_| editing.set(Some(Some(picked.clone())))
                        },
                        SpecLines { spec: spec.clone() }
                    }
                } else {
                    div { key: "{id}", class: "setup-row spec-row",
                        SpecLines { spec: spec.clone() }
                    }
                }
            }
        }
        if let Some(existing) = editing() {
            SpecDialog { vehicle, existing, reloads, on_close: move |()| editing.set(None) }
        }
    }
}

/// One spec's name, summary, and note.
#[component]
fn SpecLines(spec: Spec) -> Element {
    rsx! {
        span { class: "setup-row-text",
            span { class: "setup-row-name", {spec.name.as_str().to_owned()} }
            span { class: "setup-row-detail", {spec_summary(&spec)} }
            if !spec.notes.is_empty() {
                span { class: "setup-row-detail", {spec.notes.as_str().to_owned()} }
            }
        }
    }
}

/// A spec as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TypedSpec {
    pub name: String,
    /// `None`: no amount.
    pub kind: Option<SpecKind>,
    pub amount: String,
    pub unit: SpecUnit,
    pub detail: String,
    pub part_number: String,
    pub notes: String,
}

impl TypedSpec {
    /// A stored spec as typed again, or an empty one.
    pub(super) fn of(spec: Option<&Spec>) -> Self {
        let amount = spec.and_then(|spec| spec.amount);
        Self {
            name: spec
                .map(|spec| spec.name.as_str().to_owned())
                .unwrap_or_default(),
            kind: amount.map(|amount| amount.unit.kind()),
            amount: amount.map(SpecAmount::value_text).unwrap_or_default(),
            unit: amount.map_or(SpecUnit::Quarts, |amount| amount.unit),
            detail: spec
                .and_then(|spec| spec.detail.clone())
                .unwrap_or_default(),
            part_number: spec
                .and_then(|spec| spec.part_number.clone())
                .unwrap_or_default(),
            notes: spec
                .map(|spec| spec.notes.as_str().to_owned())
                .unwrap_or_default(),
        }
    }

    /// The amount typed so far, if it's one.
    fn amount(&self) -> Option<SpecAmount> {
        self.kind?;
        SpecAmount::parse(&self.amount, self.unit).ok()
    }

    /// The spec to save on `asset`, or the problem with what's typed.
    ///
    /// # Errors
    ///
    /// The message for a missing name, a bad amount, or a detail or part number that's too
    /// long.
    pub(super) fn parse(&self, asset: domain::asset::AssetId) -> Result<Spec, MessageId> {
        let text = |typed: &str| Some(typed.trim().to_owned()).filter(|text| !text.is_empty());
        let amount = match self.kind {
            None => None,
            Some(_) => Some(
                SpecAmount::parse(&self.amount, self.unit)
                    .map_err(|_| messages::SPEC_BAD_AMOUNT)?,
            ),
        };
        let spec = Spec {
            asset,
            name: Label::parse(&self.name).map_err(domain::label::LabelError::message)?,
            amount,
            detail: text(&self.detail),
            part_number: text(&self.part_number),
            notes: Notes::parse(&self.notes).map_err(domain::notes::NotesError::message)?,
        };
        domain::spec::check_spec(&spec).map_err(domain::spec::SpecProblem::message)?;
        Ok(spec)
    }
}

/// Add or change a spec (2A), and remove one.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SpecDialog(
    vehicle: AssetView,
    existing: Option<(SpecId, Spec)>,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let start = existing.clone();
    let mut typed = use_signal(|| TypedSpec::of(start.as_ref().map(|(_, spec)| spec)));
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let asset = vehicle.id;
    let editing_id = existing.as_ref().map(|(id, _)| *id);
    let mut finish = move |result: Result<ScheduleOutcome, ServerFnError>| match result {
        Ok(ScheduleOutcome::SavedSpec { .. } | ScheduleOutcome::Done) => {
            reloads += 1;
            on_close.call(());
        }
        Ok(ScheduleOutcome::InvalidSpec { problem: refused }) => {
            problem.set(Some(refused.message()));
        }
        Ok(ScheduleOutcome::NotAllowed) => problem.set(Some(messages::VEHICLES_NOT_ALLOWED)),
        Ok(other) => {
            tracing::debug!(target: log_area::UI, ?other, "spec: unexpected answer");
            problem.set(Some(messages::SOMETHING_WENT_WRONG));
        }
        Err(error) => problem.set(Some(problem_from_error(&error).message())),
    };
    let save = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        let spec = match typed().parse(asset) {
            Ok(spec) => spec,
            Err(message) => {
                problem.set(Some(message));
                return;
            }
        };
        busy.set(true);
        spawn(async move {
            let result = match editing_id {
                Some(id) => api::edit_spec(id, spec).await,
                None => api::add_spec(spec).await,
            };
            busy.set(false);
            finish(result);
        });
    };
    let remove = move |_| {
        let Some(id) = editing_id else {
            return;
        };
        busy.set(true);
        spawn(async move {
            let result = api::delete_spec(id).await;
            busy.set(false);
            finish(result);
        });
    };
    let shown = typed();
    let converted = shown.amount().map(SpecAmount::other_text);
    let title = existing.as_ref().map_or_else(
        || tr(messages::SPEC_ADD_TITLE),
        |(_, spec)| spec.name.as_str().to_owned(),
    );
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            form {
                class: "dialog spec-dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                onsubmit: save,
                h2 { class: "dialog-title", {title} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SPEC_NAME)} }
                    input { class: "field-input", value: shown.name.clone(), maxlength: "40", oninput: move |event| typed.write().name = event.value() }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SPEC_KIND)} }
                    select {
                        class: "field-input",
                        onchange: move |event| {
                            let kind = SpecKind::ALL.into_iter().find(|kind| format!("{kind:?}") == event.value());
                            let mut spec = typed.write();
                            spec.kind = kind;
                            if let Some(first) = kind.and_then(|kind| kind.units().first().copied()) {
                                spec.unit = first;
                            }
                        },
                        option { value: "", selected: shown.kind.is_none(), {tr(messages::SPEC_KIND_NONE)} }
                        for kind in SpecKind::ALL {
                            option { key: "{kind:?}", value: "{kind:?}", selected: shown.kind == Some(kind), {tr(kind.label())} }
                        }
                    }
                }
                if let Some(kind) = shown.kind {
                    div { class: "check-row spec-amount",
                        label { class: "field",
                            span { class: "field-label", {tr(messages::SPEC_AMOUNT)} }
                            input { class: "field-input", inputmode: "decimal", value: shown.amount.clone(), oninput: move |event| typed.write().amount = event.value() }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::SPEC_UNIT)} }
                            select {
                                class: "field-input",
                                onchange: move |event| {
                                    if let Some(unit) = SpecUnit::ALL.into_iter().find(|unit| format!("{unit:?}") == event.value()) {
                                        typed.write().unit = unit;
                                    }
                                },
                                for unit in kind.units() {
                                    option { key: "{unit:?}", value: "{unit:?}", selected: shown.unit == unit, {tr(unit.abbr())} }
                                }
                            }
                        }
                        if let Some(other) = converted {
                            span { class: "muted-note spec-converted", {format!("= {other}")} }
                        }
                    }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SPEC_DETAIL)} }
                    input { class: "field-input", value: shown.detail.clone(), maxlength: "100", oninput: move |event| typed.write().detail = event.value() }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SPEC_PART_NUMBER)} }
                    input { class: "field-input", value: shown.part_number.clone(), maxlength: "40", oninput: move |event| typed.write().part_number = event.value() }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SQUAWK_NOTES_LABEL)} }
                    input { class: "field-input", value: shown.notes, oninput: move |event| typed.write().notes = event.value() }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::ACTION_SAVE)} }
                    if editing_id.is_some() {
                        button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: remove, {tr(messages::SPEC_REMOVE)} }
                    }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::test_support::test_id;

    use super::*;

    #[test]
    fn a_spec_reads_as_one_line() {
        let mut oil = TypedSpec::of(None);
        oil.name = "Engine oil".into();
        oil.kind = Some(SpecKind::Volume);
        oil.amount = "5.5".into();
        oil.detail = "5W-30 full synthetic".into();
        let spec = oil.parse(test_id(1)).unwrap();
        assert_eq!(spec_summary(&spec), "5.5 qt (5.2 L) · 5W-30 full synthetic");
        let filter = Spec {
            part_number: Some("FL-500S".into()),
            amount: None,
            detail: None,
            ..spec.clone()
        };
        assert_eq!(spec_summary(&filter), "Part FL-500S");
        // Typed again, it's the same spec.
        assert_eq!(TypedSpec::of(Some(&spec)).parse(test_id(1)), Ok(spec));
    }

    #[test]
    fn a_typed_spec_is_checked() {
        let mut typed = TypedSpec::of(None);
        assert!(typed.parse(test_id(1)).is_err(), "no name");
        typed.name = "Lug nut torque".into();
        typed.kind = Some(SpecKind::Torque);
        typed.unit = SpecUnit::FootPounds;
        typed.amount = "lots".into();
        assert_eq!(typed.parse(test_id(1)), Err(messages::SPEC_BAD_AMOUNT));
        typed.amount = "150".into();
        assert_eq!(
            typed
                .parse(test_id(1))
                .map(|spec| spec.amount.map(SpecAmount::show)),
            Ok(Some("150 ft-lb".into()))
        );
        // No kind: no amount, whatever was typed.
        typed.kind = None;
        assert_eq!(typed.parse(test_id(1)).map(|spec| spec.amount), Ok(None));
        typed.part_number = "x".repeat(41);
        assert_eq!(typed.parse(test_id(1)), Err(messages::SPEC_BAD_PART_NUMBER));
    }
}
