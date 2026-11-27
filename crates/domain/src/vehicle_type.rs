//! Household vehicle types (decision #090, idea 23).
//!
//! An Admin makes **types built on one of the app's kinds** (e.g. "Track car" on a car): a name
//! and an icon, the documents its default usage needs, which of its kind's fields start on, the
//! fields it adds from the catalogue, and optionally a **template** (#084) copied on.
//!
//! - **It keeps its kind's rules:** its event kinds, sections (fuel economy, towing), meters,
//!   and what a High squawk means come from the kind. A vehicle of a type stores its kind as
//!   any vehicle does, and the type beside it ([`crate::asset::Asset::vehicle_type`]).
//! - **Only vehicles added afterwards** start that way ([`VehicleType::new_fields`],
//!   [`VehicleType::default_usage`]); a type's later change never touches existing vehicles, and
//!   a removed type's vehicles show their kind.
//! - **Changed on the server only**, by the Admin in Admin mode, as templates are; read by
//!   everyone, from the desktop's copy offline.
//!
//! # Examples
//!
//! ```
//! use domain::vehicle_type::TypeIcon;
//!
//! assert_eq!(TypeIcon::ALL.len(), 13);
//! assert!(TypeIcon::ALL.contains(&TypeIcon::Flag));
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::{AssetId, AssetKind};
use crate::catalogue::{CatalogueChoice, CatalogueError, fields_to_add};
use crate::command::{Clearable, Record, record_patch};
use crate::custom_field::{CustomField, FieldKey};
use crate::document::DocumentType;
use crate::i18n::{MessageId, Translator, messages};
use crate::id::Id;
use crate::label::Label;
use crate::meter::Meter;
use crate::template::TemplateId;
use crate::usage::Usage;

/// The most fields a type may add from the catalogue, counting a set as one choice.
pub const MAX_CATALOGUE_CHOICES: usize = 20;

/// A type's icon (Lucide drawings in the app).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypeIcon {
    /// A car (the default).
    #[default]
    Car,
    /// A pickup or lorry.
    Truck,
    /// A motorcycle.
    Motorcycle,
    /// A trailer or caravan.
    Trailer,
    /// A generator.
    Generator,
    /// A tractor or farm machine.
    Tractor,
    /// A boat.
    Boat,
    /// An aircraft.
    Aircraft,
    /// A bus or coach.
    Bus,
    /// A van.
    Van,
    /// A bicycle.
    Bicycle,
    /// A chequered flag (a track or race vehicle).
    Flag,
    /// A spanner (a tool or other equipment).
    Tool,
}

impl TypeIcon {
    /// Every icon, in the order the picker shows them.
    pub const ALL: [Self; 13] = [
        Self::Car,
        Self::Truck,
        Self::Motorcycle,
        Self::Trailer,
        Self::Generator,
        Self::Tractor,
        Self::Boat,
        Self::Aircraft,
        Self::Bus,
        Self::Van,
        Self::Bicycle,
        Self::Flag,
        Self::Tool,
    ];
}

/// A household vehicle type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VehicleType {
    /// Its name, e.g. "Track car" (unique among the household's types, ignoring case).
    pub name: Label,
    /// Its icon, shown where a vehicle of the type has no main photo.
    pub icon: TypeIcon,
    /// The app's kind it's built on (a profile ID).
    pub kind: AssetKind,
    /// The documents its default usage needs (in place of its kind's).
    pub documents: Vec<DocumentType>,
    /// Which of its kind's fields start on (by key); the rest start off.
    pub fields_on: Vec<FieldKey>,
    /// What it adds from the catalogue, in order.
    pub catalogue: Vec<CatalogueChoice>,
    /// A template copied onto each new vehicle of the type, if any.
    pub template: Option<TemplateId>,
}

/// The ID of a vehicle type.
pub type VehicleTypeId = Id<VehicleType>;

record_patch! {
    /// The changeable fields of a [`VehicleType`]; `Some` means "set to this".
    VehicleTypePatch
    {
        /// See [`VehicleType::name`].
        name: Label,
        /// See [`VehicleType::icon`].
        icon: TypeIcon,
        /// See [`VehicleType::kind`].
        kind: AssetKind,
        /// See [`VehicleType::documents`] (the whole list).
        documents: Vec<DocumentType>,
        /// See [`VehicleType::fields_on`] (the whole list).
        fields_on: Vec<FieldKey>,
        /// See [`VehicleType::catalogue`] (the whole list).
        catalogue: Vec<CatalogueChoice>,
        /// See [`VehicleType::template`].
        template: Clearable<TemplateId>,
    }
}

impl VehicleTypePatch {
    /// Every field of `kept`, for adding it.
    #[must_use]
    pub fn all(kept: VehicleType) -> Self {
        Self {
            name: Some(kept.name),
            icon: Some(kept.icon),
            kind: Some(kept.kind),
            documents: Some(kept.documents),
            fields_on: Some(kept.fields_on),
            catalogue: Some(kept.catalogue),
            template: Some(Clearable::from_option(kept.template)),
        }
    }

    /// The fields that differ from `from` to `to`, for changing it.
    #[must_use]
    pub fn changes(from: &VehicleType, to: VehicleType) -> Self {
        Self {
            name: (from.name != to.name).then_some(to.name),
            icon: (from.icon != to.icon).then_some(to.icon),
            kind: (from.kind != to.kind).then_some(to.kind),
            documents: (from.documents != to.documents).then_some(to.documents),
            fields_on: (from.fields_on != to.fields_on).then_some(to.fields_on),
            catalogue: (from.catalogue != to.catalogue).then_some(to.catalogue),
            template: (from.template != to.template).then(|| Clearable::from_option(to.template)),
        }
    }
}

impl Record for VehicleType {
    const ENTITY: &'static str = "vehicle_type";
    type Patch = VehicleTypePatch;
}

/// Why a vehicle type was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TypeProblem {
    /// Another type has that name.
    NameTaken,
    /// It's built on a kind the app doesn't have.
    UnknownKind,
    /// A field to start on isn't one of its kind's.
    UnknownField,
    /// Something from the catalogue isn't there, or doesn't fit its kind.
    BadCatalogue,
    /// More than [`MAX_CATALOGUE_CHOICES`] from the catalogue.
    TooMuchCatalogue,
    /// Its template has been removed.
    TemplateGone,
    /// A vehicle's type must be built on the vehicle's kind.
    WrongKind,
    /// There's no such type (or it's been removed).
    UnknownType,
}

impl TypeProblem {
    /// The message explaining it.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NameTaken => messages::TYPE_NAME_TAKEN,
            Self::UnknownKind | Self::UnknownField | Self::UnknownType => {
                messages::SOMETHING_WENT_WRONG
            }
            Self::BadCatalogue | Self::TooMuchCatalogue => messages::TYPE_BAD_CATALOGUE,
            Self::TemplateGone => messages::TYPE_TEMPLATE_GONE,
            Self::WrongKind => messages::TYPE_WRONG_KIND,
        }
    }
}

/// Checks `kept` (as `id`, if it's one already kept) against the household's `others`: a unique
/// name, a kind the app has, its kind's fields, and catalogue choices that fit its kind. (Its
/// template is checked by whoever has the templates.)
///
/// # Errors
///
/// The first [`TypeProblem`] found.
pub fn check_type<'a>(
    kept: &VehicleType,
    id: Option<VehicleTypeId>,
    others: impl IntoIterator<Item = (VehicleTypeId, &'a VehicleType)>,
) -> Result<(), TypeProblem> {
    let name = kept.name.as_str().to_lowercase();
    if others
        .into_iter()
        .any(|(other, each)| Some(other) != id && each.name.as_str().to_lowercase() == name)
    {
        return Err(TypeProblem::NameTaken);
    }
    let profile = crate::profile::profile(&kept.kind).ok_or(TypeProblem::UnknownKind)?;
    if !kept
        .fields_on
        .iter()
        .all(|key| profile.fields.iter().any(|field| &field.key == key))
    {
        return Err(TypeProblem::UnknownField);
    }
    if kept.catalogue.len() > MAX_CATALOGUE_CHOICES {
        return Err(TypeProblem::TooMuchCatalogue);
    }
    // Each choice must make fields on a new vehicle of the kind (any ID will do).
    let anyone = AssetId::generate(crate::time::UnixMillis::EPOCH, [0; crate::id::RANDOM_BYTES])
        .map_err(|_| TypeProblem::BadCatalogue)?;
    kept.new_fields(anyone, &[], Translator::english())
        .map(|_| ())
        .map_err(|_| TypeProblem::BadCatalogue)
}

impl VehicleType {
    /// A type with its kind's starting setup: the kind's default usage's documents and the
    /// fields it starts with on, nothing from the catalogue, and no template.
    #[must_use]
    pub fn from_kind(name: Label, kind: AssetKind) -> Self {
        let profile = crate::profile::profile(&kind);
        Self {
            name,
            icon: TypeIcon::default(),
            documents: profile
                .map(|profile| {
                    profile
                        .usage_documents
                        .iter()
                        .filter_map(|key| DocumentType::listed(key).ok())
                        .collect()
                })
                .unwrap_or_default(),
            fields_on: profile
                .map(|profile| {
                    profile
                        .fields
                        .iter()
                        .filter(|field| field.enabled)
                        .map(|field| field.key.clone())
                        .collect()
                })
                .unwrap_or_default(),
            catalogue: Vec::new(),
            template: None,
            kind,
        }
    }

    /// The fields a new vehicle of this type starts with: its kind's (on as the type says),
    /// then the catalogue's, in order. `meters` are the vehicle's (their keys are taken).
    ///
    /// # Errors
    ///
    /// [`CatalogueError`] if a catalogue choice no longer fits.
    pub fn new_fields(
        &self,
        asset: AssetId,
        meters: &[Meter],
        translator: &Translator,
    ) -> Result<Vec<CustomField>, CatalogueError> {
        let profile = crate::profile::profile(&self.kind).ok_or(CatalogueError::Unknown)?;
        let mut fields: Vec<CustomField> = profile
            .new_fields(asset, translator)
            .into_iter()
            .map(|field| CustomField {
                enabled: self.fields_on.contains(&field.key),
                ..field
            })
            .collect();
        for choice in &self.catalogue {
            let taken: Vec<FieldKey> = fields
                .iter()
                .map(|field| field.key.clone())
                .chain(meters.iter().map(|meter| meter.key.clone()))
                .collect();
            let added = fields_to_add(
                choice,
                asset,
                &profile.event_kinds,
                &fields,
                &taken,
                translator,
            )?;
            for field in &mut fields {
                field.enabled |= added.turn_on.contains(&field.key);
            }
            fields.extend(added.new);
        }
        Ok(fields)
    }

    /// A new vehicle's default usage: its kind's, needing this type's documents.
    #[must_use]
    pub fn default_usage(&self, asset: AssetId) -> Option<Usage> {
        crate::usage::default_usage(asset, &self.kind).map(|usage| Usage {
            documents: self.documents.clone(),
            ..usage
        })
    }
}

#[cfg(test)]
mod tests;
