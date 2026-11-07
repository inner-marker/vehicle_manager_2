//! How a requirement is complied with, and whether it applies (milestone 9; #068;
//! `docs/ui-sketches/p2m9-references-options.svg`): the two segmented choices the requirement
//! and recall dialogs share, what they typed ([`TypedCompliance`]), and a requirement's status
//! chip.

use dioxus::prelude::*;
use domain::event::Reason;
use domain::i18n::{MessageId, messages};
use domain::preferences::Preferences;
use domain::requirement::{Applicability, ComplianceMode, RequirementStatus};
use domain::time::UnixMillis;

use crate::ui::tr;

/// Applies, not applicable, or under review, as chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Applies {
    /// It applies.
    #[default]
    Yes,
    /// It doesn't (a reason is needed).
    No,
    /// Not decided yet.
    UnderReview,
}

impl Applies {
    const ALL: [Self; 3] = [Self::Yes, Self::No, Self::UnderReview];

    const fn label(self) -> MessageId {
        match self {
            Self::Yes => messages::COMPLIANCE_APPLIES,
            Self::No => messages::REQUIREMENT_STATUS_NOT_APPLICABLE,
            Self::UnderReview => messages::COMPLIANCE_UNDER_REVIEW,
        }
    }
}

/// The two choices as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TypedCompliance {
    /// One-time or recurring.
    pub mode: ComplianceMode,
    /// Whether it applies.
    pub applies: Applies,
    /// Why it doesn't, as typed.
    pub reason: String,
}

impl TypedCompliance {
    /// What a stored requirement's `mode` and `applicability` look like typed.
    pub(super) fn from_stored(mode: ComplianceMode, applicability: &Applicability) -> Self {
        let (applies, reason) = match applicability {
            Applicability::Applies => (Applies::Yes, String::new()),
            Applicability::NotApplicable { reason } => (Applies::No, reason.as_str().to_owned()),
            Applicability::UnderReview => (Applies::UnderReview, String::new()),
        };
        Self {
            mode,
            applies,
            reason,
        }
    }

    /// The applicability to save.
    ///
    /// # Errors
    ///
    /// The reason's problem, when it doesn't apply and no (or too long a) reason is given.
    pub(super) fn applicability(&self) -> Result<Applicability, MessageId> {
        Ok(match self.applies {
            Applies::Yes => Applicability::Applies,
            Applies::UnderReview => Applicability::UnderReview,
            Applies::No => Applicability::NotApplicable {
                reason: Reason::parse(&self.reason)
                    .map_err(|_| messages::COMPLIANCE_REASON_NEEDED)?,
            },
        })
    }
}

/// One-time or Recurring, and Applies / Not applicable / Under review (with its reason).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn ComplianceChoices(compliance: Signal<TypedCompliance>) -> Element {
    let mut compliance = compliance;
    let current = compliance();
    rsx! {
        div { class: "field",
            span { class: "field-label", {tr(messages::COMPLIANCE_HOW)} }
            div { class: "segmented", role: "radiogroup",
                for mode in ComplianceMode::ALL {
                    button {
                        key: "{mode:?}",
                        class: if mode == current.mode { "segment on" } else { "segment" },
                        r#type: "button",
                        role: "radio",
                        "aria-checked": mode == current.mode,
                        onclick: move |_| compliance.write().mode = mode,
                        {tr(mode.label())}
                    }
                }
            }
            if current.mode == ComplianceMode::OneTime {
                span { class: "muted-note", {tr(messages::COMPLIANCE_ONE_TIME_NOTE)} }
            }
        }
        div { class: "field",
            span { class: "field-label", {tr(messages::COMPLIANCE_APPLIES_QUESTION)} }
            div { class: "segmented", role: "radiogroup",
                for applies in Applies::ALL {
                    button {
                        key: "{applies:?}",
                        class: if applies == current.applies { "segment on" } else { "segment" },
                        r#type: "button",
                        role: "radio",
                        "aria-checked": applies == current.applies,
                        onclick: move |_| compliance.write().applies = applies,
                        {tr(applies.label())}
                    }
                }
            }
        }
        if current.applies == Applies::No {
            label { class: "field",
                span { class: "field-label", {tr(messages::COMPLIANCE_WHY_NOT)} }
                input { class: "field-input", autocomplete: "off", value: current.reason, oninput: move |event| compliance.write().reason = event.value() }
            }
        }
    }
}

/// A requirement's status chip, if it shows one (a recurring one in use shows none): its
/// text and class.
pub(super) fn status_chip(
    status: RequirementStatus,
    applicability: &Applicability,
    date: impl Fn(UnixMillis) -> String,
) -> Option<(String, &'static str)> {
    match status {
        RequirementStatus::Active => None,
        RequirementStatus::Open if *applicability == Applicability::UnderReview => {
            Some((tr(messages::COMPLIANCE_UNDER_REVIEW), "status-chip muted"))
        }
        RequirementStatus::Open => Some((tr(status.label()), "status-chip warn")),
        RequirementStatus::Complied { at, .. } => Some((
            format!("{} {}", tr(status.label()), date(at)),
            "status-chip ok",
        )),
        RequirementStatus::NotApplicable => Some((tr(status.label()), "status-chip muted")),
    }
}

/// Whether a requirement goes in the Schedule's "Done and not applicable" section.
pub(super) const fn is_settled(status: RequirementStatus) -> bool {
    matches!(
        status,
        RequirementStatus::Complied { .. } | RequirementStatus::NotApplicable
    )
}

/// A date for a chip, in the person's format, in `zone`.
pub(super) fn chip_date(
    at: UnixMillis,
    zone: &domain::location::TimeZone,
    units: &Preferences,
) -> String {
    domain::dates::format_date(
        at,
        zone,
        units.date_format,
        domain::i18n::Translator::english(),
    )
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_applicable_needs_a_reason_and_the_choices_round_trip() {
        let mut typed = TypedCompliance {
            applies: Applies::No,
            ..TypedCompliance::default()
        };
        assert_eq!(
            typed.applicability(),
            Err(messages::COMPLIANCE_REASON_NEEDED)
        );
        typed.reason = " Built after the VIN range ".into();
        let stored = typed.applicability().unwrap();
        assert_eq!(
            TypedCompliance::from_stored(ComplianceMode::OneTime, &stored),
            TypedCompliance {
                mode: ComplianceMode::OneTime,
                applies: Applies::No,
                reason: "Built after the VIN range".into(),
            }
        );
        let reviewing = TypedCompliance {
            applies: Applies::UnderReview,
            ..TypedCompliance::default()
        };
        assert_eq!(reviewing.applicability(), Ok(Applicability::UnderReview));
    }

    #[test]
    fn settled_ones_are_complied_with_or_not_applicable() {
        let done = RequirementStatus::Complied {
            event: domain::test_support::test_id(1),
            at: UnixMillis::from_millis(0),
        };
        assert!(is_settled(done));
        assert!(is_settled(RequirementStatus::NotApplicable));
        assert!(!is_settled(RequirementStatus::Open));
        assert!(!is_settled(RequirementStatus::Active));
        assert!(
            status_chip(RequirementStatus::Active, &Applicability::Applies, |_| {
                String::new()
            })
            .is_none()
        );
    }
}
