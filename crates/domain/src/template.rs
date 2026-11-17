//! Household schedule templates (Phase 5 milestone 4; decisions #031, #084): a vehicle's
//! requirements, bundles, and specs saved under a name ("F-150 factory schedule"), and copied
//! onto another vehicle when applied. Never linked: changing a template later doesn't reach
//! the vehicles it was applied to.
//!
//! A template keeps the meters its rules use (by their name and kind, as they were on the
//! vehicle it came from), so applying it works like Copy schedule
//! ([`crate::requirement::copy_schedule`]): each meter rule moves to the new vehicle's meter
//! with the same name and kind, or is left out and listed.

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Record, record_patch};
use crate::id::Id;
use crate::label::Label;
use crate::meter::{Meter, MeterId};
use crate::requirement::{Bundle, CopiedSchedule, IntervalRule, Requirement, RequirementId};
use crate::spec::{Spec, SpecId};

/// A named household template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleTemplate {
    /// Its name, e.g. "F-150 factory schedule".
    pub name: Label,
    /// The meters its rules use, as they were on the vehicle it came from.
    pub meters: Vec<(MeterId, Meter)>,
    /// Its requirements, with the IDs they had (its bundles and their specs name these).
    pub requirements: Vec<(RequirementId, Requirement)>,
    /// Its bundles.
    pub bundles: Vec<Bundle>,
    /// Its specs, with the IDs they had.
    pub specs: Vec<(SpecId, Spec)>,
}

/// The ID of a template.
pub type TemplateId = Id<ScheduleTemplate>;

record_patch! {
    /// The changeable fields of a [`ScheduleTemplate`]; `Some` means "set to this".
    ScheduleTemplatePatch
    {
        /// See [`ScheduleTemplate::name`].
        name: Label,
        /// See [`ScheduleTemplate::meters`].
        meters: Vec<(MeterId, Meter)>,
        /// See [`ScheduleTemplate::requirements`].
        requirements: Vec<(RequirementId, Requirement)>,
        /// See [`ScheduleTemplate::bundles`].
        bundles: Vec<Bundle>,
        /// See [`ScheduleTemplate::specs`].
        specs: Vec<(SpecId, Spec)>,
    }
}

impl ScheduleTemplatePatch {
    /// Every field of `template`, for creating it.
    #[must_use]
    pub fn all(template: ScheduleTemplate) -> Self {
        Self {
            name: Some(template.name),
            meters: Some(template.meters),
            requirements: Some(template.requirements),
            bundles: Some(template.bundles),
            specs: Some(template.specs),
        }
    }
}

impl Record for ScheduleTemplate {
    const ENTITY: &'static str = "schedule_template";
    type Patch = ScheduleTemplatePatch;
}

/// What a template holds, for its card: requirements, bundles, specs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemplateCounts {
    /// Requirements.
    pub requirements: usize,
    /// Bundles.
    pub bundles: usize,
    /// Specs.
    pub specs: usize,
}

impl ScheduleTemplate {
    /// A template named `name` from one vehicle's schedule: its requirements (each one's
    /// recall or bulletin, and when it was last done before the records, left behind: they're
    /// that vehicle's), bundles, and specs, with the meters the requirements' rules use.
    #[must_use]
    pub fn from_vehicle(
        name: Label,
        meters: &[(MeterId, Meter)],
        requirements: &[(RequirementId, Requirement)],
        bundles: &[Bundle],
        specs: &[(SpecId, Spec)],
    ) -> Self {
        let used = |meter: MeterId| {
            requirements.iter().any(|(_, requirement)| {
                requirement.rules.iter().any(|rule| match rule {
                    IntervalRule::Meter { meter: on, .. }
                    | IntervalRule::Reaches { meter: on, .. } => *on == meter,
                    IntervalRule::Calendar { .. } | IntervalRule::FixedDate { .. } => false,
                })
            })
        };
        Self {
            name,
            meters: meters.iter().filter(|(id, _)| used(*id)).cloned().collect(),
            requirements: requirements
                .iter()
                .map(|(id, requirement)| {
                    (
                        *id,
                        Requirement {
                            reference: None,
                            before_records: None,
                            ..requirement.clone()
                        },
                    )
                })
                .collect(),
            bundles: bundles.to_vec(),
            specs: specs.to_vec(),
        }
    }

    /// What it holds.
    #[must_use]
    pub fn counts(&self) -> TemplateCounts {
        TemplateCounts {
            requirements: self.requirements.len(),
            bundles: self.bundles.len(),
            specs: self.specs.len(),
        }
    }

    /// It, copied onto `to_asset`, whose meters are `to_meters` (#031): as Copy schedule.
    #[must_use]
    pub fn apply(&self, to_meters: &[(MeterId, Meter)], to_asset: AssetId) -> CopiedSchedule {
        crate::requirement::copy_schedule(
            &self.requirements,
            &self.bundles,
            &self.specs,
            &self.meters,
            to_meters,
            to_asset,
        )
    }
}

#[cfg(test)]
mod tests;
