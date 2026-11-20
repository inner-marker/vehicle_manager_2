//! A vehicle's usages (Phase 5 milestone 6; decision #086): what it can be ready for.
//!
//! Every vehicle has a **default usage**, named for its kind ("Drivable", "Towable",
//! "Usable", from its profile), and Admins and Maintainers add more ("Local", "Highway",
//! "Towing"; for aircraft later VFR, IFR, oceanic). **Each usage stands alone:** it lists the
//! document types it needs, and requirements tick the usages they're needed for
//! ([`crate::requirement::Requirement::usages`]; none ticked means the default usage).
//! Readiness for a usage is worked out in [`crate::readiness`].

use serde::{Deserialize, Serialize};

use crate::asset::{AssetId, AssetKind};
use crate::command::{Record, record_patch};
use crate::document::DocumentType;
use crate::i18n::{MessageId, Translator, messages};
use crate::id::Id;
use crate::label::Label;

/// One of a vehicle's usages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Which vehicle.
    pub asset: AssetId,
    /// Its name, e.g. "Drivable", "Towing".
    pub name: Label,
    /// The vehicle's default usage (one per vehicle).
    pub default: bool,
    /// The document types it needs: one that's expired, or not on file, makes the vehicle not
    /// ready for it.
    pub documents: Vec<DocumentType>,
}

/// The ID of a usage.
pub type UsageId = Id<Usage>;

record_patch! {
    /// The changeable fields of a [`Usage`]; `Some` means "set to this".
    UsagePatch
    {
        /// See [`Usage::asset`] (set when created).
        asset: AssetId,
        /// See [`Usage::name`].
        name: Label,
        /// See [`Usage::default`] (set when created).
        default: bool,
        /// See [`Usage::documents`] (the whole list).
        documents: Vec<DocumentType>,
    }
}

impl UsagePatch {
    /// Every field of `usage`, for creating it.
    #[must_use]
    pub fn all(usage: Usage) -> Self {
        Self {
            asset: Some(usage.asset),
            name: Some(usage.name),
            default: Some(usage.default),
            documents: Some(usage.documents),
        }
    }
}

impl Record for Usage {
    const ENTITY: &'static str = "usage";
    type Patch = UsagePatch;
}

/// Why a usage was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageProblem {
    /// Another of the vehicle's usages has that name (ignoring case).
    NameTaken,
    /// A document type listed twice.
    RepeatedDocument,
    /// The default usage can't be removed.
    DefaultStays,
}

impl UsageProblem {
    /// The message explaining it.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NameTaken => messages::USAGE_NAME_TAKEN,
            Self::RepeatedDocument => messages::SOMETHING_WENT_WRONG,
            Self::DefaultStays => messages::USAGE_DEFAULT_STAYS,
        }
    }
}

/// Checks `usage` (`id`, if it's one being changed) against its vehicle's `others`: a name of
/// its own (ignoring case), and each document type once.
///
/// # Errors
///
/// The first [`UsageProblem`] found.
pub fn check_usage(
    id: Option<UsageId>,
    usage: &Usage,
    others: &[(UsageId, Usage)],
) -> Result<(), UsageProblem> {
    let name = usage.name.as_str().to_lowercase();
    if others
        .iter()
        .any(|(other, them)| Some(*other) != id && them.name.as_str().to_lowercase() == name)
    {
        return Err(UsageProblem::NameTaken);
    }
    for (index, document) in usage.documents.iter().enumerate() {
        if usage.documents[..index].contains(document) {
            return Err(UsageProblem::RepeatedDocument);
        }
    }
    Ok(())
}

/// The default usage for vehicle `asset` of `kind`: its profile's name and document types
/// (#086); "Usable" with none for a kind without a profile. `None` only if a profile's name
/// isn't a label (the profiles' tests rule that out).
#[must_use]
pub fn default_usage(asset: AssetId, kind: &AssetKind) -> Option<Usage> {
    let profile = crate::profile::profile(kind);
    let english = Translator::english();
    let name = profile.map_or_else(
        || english.tr(messages::USAGE_USABLE),
        |profile| english.tr_key(&profile.default_usage),
    );
    Some(Usage {
        asset,
        name: Label::parse(&name).ok()?,
        default: true,
        documents: profile
            .map(|profile| {
                profile
                    .usage_documents
                    .iter()
                    .filter_map(|key| DocumentType::listed(key).ok())
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// The vehicle's default usage among `usages`.
#[must_use]
pub fn default_of(usages: &[(UsageId, Usage)]) -> Option<UsageId> {
    usages
        .iter()
        .find(|(_, usage)| usage.default)
        .map(|(id, _)| *id)
}

/// Copying a schedule's usages onto another vehicle (#086), matched by name (ignoring case):
/// each of `from` the copied requirements tick, with the new vehicle's usage of the same name
/// if it has one (`Some`), or a copy to create (`None`). The default usage maps to the new
/// vehicle's default whatever its name.
#[must_use]
pub fn match_usages(
    ticked: &[UsageId],
    from: &[(UsageId, Usage)],
    to: &[(UsageId, Usage)],
) -> Vec<(UsageId, Option<UsageId>)> {
    let mut matched: Vec<(UsageId, Option<UsageId>)> = Vec::new();
    for old in ticked {
        if matched.iter().any(|(was, _)| was == old) {
            continue;
        }
        let Some((_, usage)) = from.iter().find(|(id, _)| id == old) else {
            continue;
        };
        let name = usage.name.as_str().to_lowercase();
        let found = to
            .iter()
            .find(|(_, theirs)| {
                (usage.default && theirs.default)
                    || (!usage.default && theirs.name.as_str().to_lowercase() == name)
            })
            .map(|(id, _)| *id);
        matched.push((*old, found));
    }
    matched
}

#[cfg(test)]
mod tests;
