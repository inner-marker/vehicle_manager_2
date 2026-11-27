//! The catalogue of ready-made fields and sets (decision #090, idea 16).
//!
//! The catalogue is data (`crates/domain/catalogue.ron`, compiled in). A Maintainer picks a
//! field, a set, or the Tires set from **+ Add field → From the catalogue**, and its fields are
//! **copied** onto the vehicle ([`fields_to_add`]), then edited like any other.
//!
//! - **A field already on the vehicle** (its key) is greyed out, and can't be added again.
//! - **A set** adds the fields the vehicle doesn't have, and uses (and turns on) the ones it
//!   does. **Added again** (all its fields already there), it gets " B" on its labels and `-b`
//!   on its keys, then " C", …; its calculated fields' formulas use the new keys.
//! - **Tires** asks for the axles (1–4) and the tires on each (2, or 4 for duals), and adds a
//!   pressure and a tread depth for each tire, named by position: 1L, 1R, 2L outer, 2L inner, ….
//! - Fields shown only on kinds of events the vehicle doesn't have (a trailer's fuel-ups) are
//!   left out.
//!
//! # Examples
//!
//! ```
//! use domain::catalogue::{TireAxle, tire_positions};
//!
//! let tires = tire_positions(&[TireAxle::Single, TireAxle::Dual]);
//! assert_eq!(tires.len(), 6);
//! assert_eq!(tires[2].key, "2l-outer");
//! ```

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::custom_field::{CustomField, FieldKey, FieldKind, FieldRange};
use crate::formula::Formula;
use crate::i18n::{MessageId, Translator, messages};
use crate::label::Label;
use crate::meter::EventKind;

/// The catalogue's source.
const SOURCE: &str = include_str!("../catalogue.ron");

/// The most axles the Tires set takes.
pub const MAX_AXLES: usize = 4;

/// The catalogue: its fields, its sets, and the Tires set's fields.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Catalogue {
    /// Every ready-made field, in the order offered.
    pub fields: Vec<CatalogueField>,
    /// The sets of fields (Tires apart), in the order offered.
    pub sets: Vec<CatalogueSet>,
    /// The fields the Tires set adds for each tire.
    pub tire_fields: Vec<CatalogueField>,
}

/// A ready-made field.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CatalogueField {
    /// Its stable key (#057).
    pub key: FieldKey,
    /// Its label (a message key).
    pub label: String,
    /// The kinds of events it's shown on (those the vehicle has).
    pub shown_on: Vec<EventKind>,
    /// What it holds.
    pub kind: FieldKind,
    /// Its sensible range, if any.
    #[serde(default)]
    pub range: Option<FieldRange>,
}

/// A set of ready-made fields.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct CatalogueSet {
    /// Its stable ID.
    pub id: String,
    /// Its name (a message key).
    pub name: String,
    /// Its fields' keys, in order.
    pub fields: Vec<FieldKey>,
}

/// What's being added from the catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CatalogueChoice {
    /// One field, by key.
    Field {
        /// Its key.
        key: FieldKey,
    },
    /// A set, by ID.
    Set {
        /// Its ID.
        id: String,
    },
    /// The Tires set, for these axles, front to back.
    Tires {
        /// Each axle's tires.
        axles: Vec<TireAxle>,
    },
}

/// The tires on one axle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TireAxle {
    /// Two tires, one each side.
    Single,
    /// Four: two each side (duals).
    Dual,
}

impl TireAxle {
    /// How many tires it has.
    #[must_use]
    pub const fn tires(self) -> usize {
        match self {
            Self::Single => 2,
            Self::Dual => 4,
        }
    }
}

/// One tire's position: its key part (`2l-outer`) and how it's named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TirePosition {
    /// For keys: `1l`, `2r-inner`.
    pub key: String,
    /// The axle, from 1 at the front.
    pub axle: usize,
    /// On the left.
    pub left: bool,
    /// On a dual axle: the outer tire (`Some(true)`) or the inner one.
    pub outer: Option<bool>,
}

impl TirePosition {
    /// Its name: "1L", "2R inner".
    #[must_use]
    pub fn name(&self, translator: &Translator) -> String {
        let side = translator.tr(if self.left {
            messages::TIRE_LEFT
        } else {
            messages::TIRE_RIGHT
        });
        let short = format!("{}{side}", self.axle);
        match self.outer {
            None => short,
            Some(outer) => format!(
                "{short} {}",
                translator.tr(if outer {
                    messages::TIRE_OUTER
                } else {
                    messages::TIRE_INNER
                })
            ),
        }
    }
}

/// Every tire on these axles, front to back, left to right.
#[must_use]
pub fn tire_positions(axles: &[TireAxle]) -> Vec<TirePosition> {
    let mut positions = Vec::new();
    for (index, axle) in axles.iter().enumerate() {
        let number = index + 1;
        let tire = |left: bool, outer: Option<bool>| {
            let side = if left { "l" } else { "r" };
            let place = match outer {
                None => "",
                Some(true) => "-outer",
                Some(false) => "-inner",
            };
            TirePosition {
                key: format!("{number}{side}{place}"),
                axle: number,
                left,
                outer,
            }
        };
        match axle {
            TireAxle::Single => positions.extend([tire(true, None), tire(false, None)]),
            TireAxle::Dual => positions.extend([
                tire(true, Some(true)),
                tire(true, Some(false)),
                tire(false, Some(false)),
                tire(false, Some(true)),
            ]),
        }
    }
    positions
}

/// Why something couldn't be added from the catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogueError {
    /// Not in the catalogue (or it couldn't be read).
    Unknown,
    /// The field is already on the vehicle.
    AlreadyThere,
    /// Every field is for kinds of events the vehicle doesn't have.
    NotForThisKind,
    /// The Tires set needs 1 to 4 axles.
    BadAxles,
}

impl CatalogueError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Unknown => messages::SOMETHING_WENT_WRONG,
            Self::AlreadyThere => messages::CATALOGUE_ALREADY_THERE,
            Self::NotForThisKind => messages::CATALOGUE_NOT_FOR_THIS_KIND,
            Self::BadAxles => messages::CATALOGUE_BAD_AXLES,
        }
    }
}

/// The catalogue, read once.
static CATALOGUE: LazyLock<Result<Catalogue, String>> =
    LazyLock::new(|| ron::from_str::<Catalogue>(SOURCE).map_err(|error| error.to_string()));

/// The catalogue.
///
/// # Errors
///
/// What's wrong, if the catalogue can't be read (never in a tested build).
pub fn catalogue() -> Result<&'static Catalogue, String> {
    CATALOGUE.as_ref().map_err(Clone::clone)
}

impl Catalogue {
    /// The field with `key`.
    #[must_use]
    pub fn field(&self, key: &FieldKey) -> Option<&CatalogueField> {
        self.fields.iter().find(|field| &field.key == key)
    }

    /// The set with `id`.
    #[must_use]
    pub fn set(&self, id: &str) -> Option<&CatalogueSet> {
        self.sets.iter().find(|set| set.id == id)
    }
}

/// What adding from the catalogue does to a vehicle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Addition {
    /// The new fields, in order.
    pub new: Vec<CustomField>,
    /// Fields the vehicle already has that a set uses, to turn on (if they're off).
    pub turn_on: Vec<FieldKey>,
}

/// One field to add, before its suffix.
struct Wanted {
    key: FieldKey,
    label: String,
    kind: FieldKind,
    shown_on: Vec<EventKind>,
    range: Option<FieldRange>,
}

/// What adding `choice` to `asset` does. `event_kinds` are the vehicle's kinds of events,
/// `current` its fields, and `taken` every key it has used (fields removed too, and its
/// meters'), which are never reused.
///
/// # Errors
///
/// [`CatalogueError`] if the choice isn't in the catalogue, a single field is already on the
/// vehicle, none of it fits the vehicle's kinds of events, or the axles are wrong.
pub fn fields_to_add(
    choice: &CatalogueChoice,
    asset: AssetId,
    event_kinds: &[EventKind],
    current: &[CustomField],
    taken: &[FieldKey],
    translator: &Translator,
) -> Result<Addition, CatalogueError> {
    let catalogue = catalogue().map_err(|_| CatalogueError::Unknown)?;
    let wanted = wanted(catalogue, choice, translator)?;
    // Only the kinds of events the vehicle has.
    let wanted: Vec<Wanted> = wanted
        .into_iter()
        .filter_map(|mut field| {
            field.shown_on.retain(|kind| event_kinds.contains(kind));
            (!field.shown_on.is_empty()).then_some(field)
        })
        .collect();
    if wanted.is_empty() {
        return Err(CatalogueError::NotForThisKind);
    }
    let has = |key: &FieldKey| current.iter().find(|field| &field.key == key);
    if matches!(choice, CatalogueChoice::Field { .. })
        && wanted.iter().any(|field| has(&field.key).is_some())
    {
        return Err(CatalogueError::AlreadyThere);
    }
    // No suffix, then B, C, …: the first that isn't all on the vehicle already.
    let suffix = (b'A'..=b'Z')
        .map(|letter| (letter > b'A').then(|| char::from(letter)))
        .find(|suffix| {
            !wanted
                .iter()
                .all(|field| has(&suffixed_key(&field.key, *suffix)).is_some())
        })
        .ok_or(CatalogueError::AlreadyThere)?;
    let mut taken = taken.to_vec();
    let mut keys: Vec<(String, FieldKey)> = Vec::new();
    let mut turn_on = Vec::new();
    let mut new = Vec::new();
    for field in &wanted {
        let key = suffixed_key(&field.key, suffix);
        let label = match suffix {
            None => field.label.clone(),
            Some(letter) => format!("{} {letter}", field.label),
        };
        let label = Label::parse(&label).map_err(|_| CatalogueError::Unknown)?;
        // One the vehicle has, holding the same: used as it is.
        if let Some(existing) = has(&key)
            && same_kind(&existing.kind, &field.kind)
        {
            if !existing.enabled {
                turn_on.push(key.clone());
            }
            keys.push((field.key.as_str().to_owned(), key));
            continue;
        }
        let key = if taken.contains(&key) || crate::calculated::is_reserved(key.as_str()) {
            crate::calculated::new_key_among(&label, taken.clone())
        } else {
            key
        };
        taken.push(key.clone());
        keys.push((field.key.as_str().to_owned(), key.clone()));
        new.push(CustomField {
            asset,
            shown_on: field.shown_on.clone(),
            key,
            label,
            kind: field.kind.clone(),
            enabled: true,
            required_for: Vec::new(),
            range: field.range,
        });
    }
    // Calculated fields use the keys as they ended up.
    for field in &mut new {
        if let FieldKind::Calculated { formula, .. } = &mut field.kind {
            *formula = Formula::parse(&rekeyed(formula.as_str(), &keys))
                .map_err(|_| CatalogueError::Unknown)?;
        }
    }
    Ok(Addition { new, turn_on })
}

/// The fields `choice` adds, labelled, before suffixes and the vehicle's kinds of events.
fn wanted(
    catalogue: &Catalogue,
    choice: &CatalogueChoice,
    translator: &Translator,
) -> Result<Vec<Wanted>, CatalogueError> {
    let plain = |field: &CatalogueField| Wanted {
        key: field.key.clone(),
        label: translator.tr_key(&field.label),
        kind: field.kind.clone(),
        shown_on: field.shown_on.clone(),
        range: field.range,
    };
    Ok(match choice {
        CatalogueChoice::Field { key } => {
            vec![plain(catalogue.field(key).ok_or(CatalogueError::Unknown)?)]
        }
        CatalogueChoice::Set { id } => catalogue
            .set(id)
            .ok_or(CatalogueError::Unknown)?
            .fields
            .iter()
            .map(|key| catalogue.field(key).map(plain))
            .collect::<Option<_>>()
            .ok_or(CatalogueError::Unknown)?,
        CatalogueChoice::Tires { axles } => {
            if axles.is_empty() || axles.len() > MAX_AXLES {
                return Err(CatalogueError::BadAxles);
            }
            let mut fields = Vec::new();
            for tire in tire_positions(axles) {
                for field in &catalogue.tire_fields {
                    let key = FieldKey::parse(&format!("{}-{}", field.key.as_str(), tire.key))
                        .map_err(|_| CatalogueError::Unknown)?;
                    fields.push(Wanted {
                        key,
                        label: format!(
                            "{} {}",
                            translator.tr_key(&field.label),
                            tire.name(translator)
                        ),
                        ..plain(field)
                    });
                }
            }
            fields
        }
    })
}

/// `key` with `-b` (and so on) for a suffix.
fn suffixed_key(key: &FieldKey, suffix: Option<char>) -> FieldKey {
    match suffix {
        None => key.clone(),
        Some(letter) => {
            FieldKey::parse(&format!("{}-{}", key.as_str(), letter.to_ascii_lowercase()))
                .unwrap_or_else(|_| key.clone())
        }
    }
}

/// Whether a field the vehicle has holds what the catalogue's would (any formula counts).
fn same_kind(have: &FieldKind, want: &FieldKind) -> bool {
    match (have, want) {
        (FieldKind::Calculated { .. }, FieldKind::Calculated { .. }) => true,
        _ => have == want,
    }
}

/// `formula` with each key in `keys` (catalogue key, key on the vehicle) changed. Keys are
/// runs of lowercase letters, digits, and `-`.
fn rekeyed(formula: &str, keys: &[(String, FieldKey)]) -> String {
    let mut out = String::new();
    let mut word = String::new();
    let flush = |word: &mut String, out: &mut String| {
        let replaced = keys
            .iter()
            .find(|(from, _)| from == word)
            .map_or(word.as_str(), |(_, to)| to.as_str());
        out.push_str(replaced);
        word.clear();
    };
    for c in formula.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
            word.push(c);
        } else {
            flush(&mut word, &mut out);
            out.push(c);
        }
    }
    flush(&mut word, &mut out);
    out
}

#[cfg(test)]
mod tests;
