//! License plates: each its own record, with an issuing authority and a history (milestone 8;
//! plan §4 Asset; decision #066).
//!
//! A vehicle that moves from state to state gets a new plate (and number) each time. Each
//! [`LicensePlate`] keeps its authority, number, and dates; the **current** one has no
//! retirement date, and the others are its history. A registration links to its plate, and
//! its expiry becomes the plate's ([`crate::document::plate_follows`]).
//!
//! Plates that were typed identifiers before milestone 8 move over with **no authority yet**
//! ([`from_identifiers`]); a new plate always has one.
//!
//! # Examples
//!
//! ```
//! use domain::plate::{Authority, PlateNumber};
//!
//! let texas = Authority::region("us", "tx").unwrap();
//! assert_eq!(texas.name(), "Texas");
//! assert_eq!(PlateNumber::parse(" abc 1234 ").unwrap().as_str(), "ABC 1234");
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::{AssetId, Identifier};
use crate::command::{Clearable, Record, record_patch};
use crate::dates::CalendarDate;
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::notes::Notes;
use crate::time::UnixMillis;
use crate::user::UserId;

/// The ID of a license plate.
pub type LicensePlateId = Id<LicensePlate>;

/// The most characters a plate's number may have.
pub const NUMBER_MAX_CHARS: usize = 15;

/// The most characters an "other" authority's name may have.
pub const AUTHORITY_MAX_CHARS: usize = 60;

/// The identifier kind plates had before they became records.
pub const PLATE_IDENTIFIER: &str = "plate";

/// The states, districts, territories, and provinces that issue plates in the US and Canada:
/// (country, code, name).
pub const REGIONS: &[(&str, &str, &str)] = &[
    ("US", "AL", "Alabama"),
    ("US", "AK", "Alaska"),
    ("US", "AZ", "Arizona"),
    ("US", "AR", "Arkansas"),
    ("US", "CA", "California"),
    ("US", "CO", "Colorado"),
    ("US", "CT", "Connecticut"),
    ("US", "DE", "Delaware"),
    ("US", "DC", "District of Columbia"),
    ("US", "FL", "Florida"),
    ("US", "GA", "Georgia"),
    ("US", "HI", "Hawaii"),
    ("US", "ID", "Idaho"),
    ("US", "IL", "Illinois"),
    ("US", "IN", "Indiana"),
    ("US", "IA", "Iowa"),
    ("US", "KS", "Kansas"),
    ("US", "KY", "Kentucky"),
    ("US", "LA", "Louisiana"),
    ("US", "ME", "Maine"),
    ("US", "MD", "Maryland"),
    ("US", "MA", "Massachusetts"),
    ("US", "MI", "Michigan"),
    ("US", "MN", "Minnesota"),
    ("US", "MS", "Mississippi"),
    ("US", "MO", "Missouri"),
    ("US", "MT", "Montana"),
    ("US", "NE", "Nebraska"),
    ("US", "NV", "Nevada"),
    ("US", "NH", "New Hampshire"),
    ("US", "NJ", "New Jersey"),
    ("US", "NM", "New Mexico"),
    ("US", "NY", "New York"),
    ("US", "NC", "North Carolina"),
    ("US", "ND", "North Dakota"),
    ("US", "OH", "Ohio"),
    ("US", "OK", "Oklahoma"),
    ("US", "OR", "Oregon"),
    ("US", "PA", "Pennsylvania"),
    ("US", "RI", "Rhode Island"),
    ("US", "SC", "South Carolina"),
    ("US", "SD", "South Dakota"),
    ("US", "TN", "Tennessee"),
    ("US", "TX", "Texas"),
    ("US", "UT", "Utah"),
    ("US", "VT", "Vermont"),
    ("US", "VA", "Virginia"),
    ("US", "WA", "Washington"),
    ("US", "WV", "West Virginia"),
    ("US", "WI", "Wisconsin"),
    ("US", "WY", "Wyoming"),
    ("US", "AS", "American Samoa"),
    ("US", "GU", "Guam"),
    ("US", "MP", "Northern Mariana Islands"),
    ("US", "PR", "Puerto Rico"),
    ("US", "VI", "U.S. Virgin Islands"),
    ("CA", "AB", "Alberta"),
    ("CA", "BC", "British Columbia"),
    ("CA", "MB", "Manitoba"),
    ("CA", "NB", "New Brunswick"),
    ("CA", "NL", "Newfoundland and Labrador"),
    ("CA", "NS", "Nova Scotia"),
    ("CA", "NT", "Northwest Territories"),
    ("CA", "NU", "Nunavut"),
    ("CA", "ON", "Ontario"),
    ("CA", "PE", "Prince Edward Island"),
    ("CA", "QC", "Quebec"),
    ("CA", "SK", "Saskatchewan"),
    ("CA", "YT", "Yukon"),
];

/// Who issued a plate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "by", rename_all = "kebab-case")]
pub enum Authority {
    /// A US state (or district or territory) or a Canadian province or territory, from
    /// [`REGIONS`].
    Region {
        /// `US` or `CA`.
        country: String,
        /// Its code, e.g. `TX`.
        region: String,
    },
    /// A country, by its ISO 3166 code (e.g. `DE`).
    Country {
        /// Two capital letters.
        country: String,
    },
    /// Anyone else, e.g. a tribal nation or a military command.
    Other {
        /// Its name (up to 60 characters).
        name: String,
    },
}

impl Authority {
    /// A US or Canadian region by its codes, in any case.
    ///
    /// # Errors
    ///
    /// [`PlateProblem::UnknownRegion`].
    pub fn region(country: &str, region: &str) -> Result<Self, PlateProblem> {
        let country = country.trim().to_ascii_uppercase();
        let region = region.trim().to_ascii_uppercase();
        REGIONS
            .iter()
            .find(|(c, r, _)| *c == country && *r == region)
            .map(|_| Self::Region { country, region })
            .ok_or(PlateProblem::UnknownRegion)
    }

    /// A country by its two-letter code, in any case.
    ///
    /// # Errors
    ///
    /// [`PlateProblem::BadCountry`].
    pub fn country(code: &str) -> Result<Self, PlateProblem> {
        let code = code.trim().to_ascii_uppercase();
        if code.len() == 2 && code.bytes().all(|byte| byte.is_ascii_uppercase()) {
            Ok(Self::Country { country: code })
        } else {
            Err(PlateProblem::BadCountry)
        }
    }

    /// Another authority by name.
    ///
    /// # Errors
    ///
    /// [`PlateProblem::AuthorityEmpty`] or [`PlateProblem::AuthorityTooLong`].
    pub fn other(name: &str) -> Result<Self, PlateProblem> {
        let name = name.trim();
        match name.chars().count() {
            0 => Err(PlateProblem::AuthorityEmpty),
            length if length > AUTHORITY_MAX_CHARS => Err(PlateProblem::AuthorityTooLong),
            _ => Ok(Self::Other {
                name: name.to_owned(),
            }),
        }
    }

    /// Checks one read back (e.g. from the database or a device).
    ///
    /// # Errors
    ///
    /// The problem [`Self::region`], [`Self::country`], or [`Self::other`] would give.
    pub fn check(&self) -> Result<(), PlateProblem> {
        let again = match self {
            Self::Region { country, region } => Self::region(country, region)?,
            Self::Country { country } => Self::country(country)?,
            Self::Other { name } => Self::other(name)?,
        };
        if again == *self {
            Ok(())
        } else {
            Err(PlateProblem::UnknownRegion)
        }
    }

    /// How it's shown: a region's name ("Texas"), a country's code ("DE"), or the name given.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Region { country, region } => REGIONS
                .iter()
                .find(|(c, r, _)| c == country && r == region)
                .map_or(region.as_str(), |(_, _, name)| name),
            Self::Country { country } => country,
            Self::Other { name } => name,
        }
    }
}

/// A plate's number as it's shown on the plate: trimmed, in capitals, 1–15 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PlateNumber(String);

impl PlateNumber {
    /// A number from what was typed.
    ///
    /// # Errors
    ///
    /// [`PlateProblem::NumberEmpty`] or [`PlateProblem::NumberTooLong`].
    pub fn parse(input: &str) -> Result<Self, PlateProblem> {
        let number = input.trim().to_uppercase();
        match number.chars().count() {
            0 => Err(PlateProblem::NumberEmpty),
            length if length > NUMBER_MAX_CHARS => Err(PlateProblem::NumberTooLong),
            _ => Ok(Self(number)),
        }
    }

    /// The number.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for PlateNumber {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|problem| format!("{problem:?}"))
    }
}

impl From<PlateNumber> for String {
    fn from(number: PlateNumber) -> Self {
        number.0
    }
}

/// One license plate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicensePlate {
    /// Which vehicle.
    pub asset: AssetId,
    /// Who issued it; none only for a plate moved over from the identifiers.
    pub authority: Option<Authority>,
    /// Its number.
    pub number: PlateNumber,
    /// When it was issued (or put on the vehicle).
    pub issued_on: Option<CalendarDate>,
    /// When it expires: its registration's expiry, when one is linked.
    pub expires_on: Option<CalendarDate>,
    /// When it stopped being used (a new plate replaced it, or the vehicle was sold); none
    /// while it's current.
    pub retired_on: Option<CalendarDate>,
    /// Notes.
    pub notes: Notes,
    /// When it was added.
    pub added_at: UnixMillis,
    /// Who added it.
    pub added_by: UserId,
    /// When the server first received it.
    pub received_at: UnixMillis,
}

record_patch! {
    /// The changeable fields of a [`LicensePlate`]; `Some` means "set to this".
    LicensePlatePatch
    {
        /// See [`LicensePlate::asset`] (set when added).
        asset: AssetId,
        /// See [`LicensePlate::authority`].
        authority: Clearable<Authority>,
        /// See [`LicensePlate::number`].
        number: PlateNumber,
        /// See [`LicensePlate::issued_on`].
        issued_on: Clearable<CalendarDate>,
        /// See [`LicensePlate::expires_on`].
        expires_on: Clearable<CalendarDate>,
        /// See [`LicensePlate::retired_on`].
        retired_on: Clearable<CalendarDate>,
        /// See [`LicensePlate::notes`].
        notes: Notes,
        /// See [`LicensePlate::added_at`] (set when added).
        added_at: UnixMillis,
        /// See [`LicensePlate::added_by`] (set when added).
        added_by: UserId,
        /// See [`LicensePlate::received_at`] (set when added).
        received_at: UnixMillis,
    }
}

impl LicensePlatePatch {
    /// Every field of `plate`, for adding it.
    #[must_use]
    pub fn all(plate: LicensePlate) -> Self {
        Self {
            asset: Some(plate.asset),
            authority: Some(Clearable::from_option(plate.authority)),
            number: Some(plate.number),
            issued_on: Some(Clearable::from_option(plate.issued_on)),
            expires_on: Some(Clearable::from_option(plate.expires_on)),
            retired_on: Some(Clearable::from_option(plate.retired_on)),
            notes: Some(plate.notes),
            added_at: Some(plate.added_at),
            added_by: Some(plate.added_by),
            received_at: Some(plate.received_at),
        }
    }
}

impl Record for LicensePlate {
    const ENTITY: &'static str = "license-plate";
    type Patch = LicensePlatePatch;
}

/// The vehicle's current plate among `plates` (all one vehicle's): one not retired, the most
/// recently issued (then the newest ID) if there are several.
#[must_use]
pub fn current(
    plates: &[(LicensePlateId, LicensePlate)],
) -> Option<&(LicensePlateId, LicensePlate)> {
    plates
        .iter()
        .filter(|(_, plate)| plate.retired_on.is_none())
        .max_by(|(a_id, a), (b_id, b)| (&a.issued_on, a_id).cmp(&(&b.issued_on, b_id)))
}

/// The change that retires the vehicle's current plate when `new` is added: retired on the
/// day the new one was issued, or `today`. None if there's no current plate.
#[must_use]
pub fn retire_for(
    plates: &[(LicensePlateId, LicensePlate)],
    new: &LicensePlate,
    today: &CalendarDate,
) -> Option<(LicensePlateId, LicensePlatePatch)> {
    current(plates).map(|(id, _)| {
        let on = new.issued_on.clone().unwrap_or_else(|| today.clone());
        (
            *id,
            LicensePlatePatch {
                retired_on: Some(Clearable::Set(on)),
                ..LicensePlatePatch::default()
            },
        )
    })
}

/// Splits a vehicle's identifiers into its plate numbers (to move into plate records, with no
/// authority) and the rest. A plate number that no longer fits is kept as an identifier.
#[must_use]
pub fn from_identifiers(identifiers: Vec<Identifier>) -> (Vec<PlateNumber>, Vec<Identifier>) {
    let mut plates = Vec::new();
    let mut rest = Vec::new();
    for identifier in identifiers {
        match PlateNumber::parse(&identifier.value) {
            Ok(number) if identifier.kind.as_str() == PLATE_IDENTIFIER => plates.push(number),
            Ok(_) | Err(_) => rest.push(identifier),
        }
    }
    (plates, rest)
}

/// Checks a plate: its authority (required on a `new` one), and that it isn't retired before
/// it was issued.
///
/// # Errors
///
/// [`PlateProblem::NeedsAuthority`], [`PlateProblem::RetiredBeforeIssued`], or the
/// authority's problem.
pub fn check(plate: &LicensePlate, new: bool) -> Result<(), PlateProblem> {
    match &plate.authority {
        Some(authority) => authority.check()?,
        None if new => return Err(PlateProblem::NeedsAuthority),
        None => {}
    }
    if let (Some(issued), Some(retired)) = (&plate.issued_on, &plate.retired_on)
        && retired < issued
    {
        return Err(PlateProblem::RetiredBeforeIssued);
    }
    Ok(())
}

/// Why a plate was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum PlateProblem {
    /// A number with nothing in it.
    NumberEmpty,
    /// A number over 15 characters.
    NumberTooLong,
    /// A new plate with no authority.
    NeedsAuthority,
    /// A US or Canadian region that isn't in the list.
    UnknownRegion,
    /// A country code that isn't two letters.
    BadCountry,
    /// An "other" authority with no name.
    AuthorityEmpty,
    /// An "other" authority's name over 60 characters.
    AuthorityTooLong,
    /// Retired before it was issued.
    RetiredBeforeIssued,
    /// Removing a plate a registration names.
    InUse,
}

impl PlateProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NumberEmpty => messages::PLATE_NUMBER_EMPTY,
            Self::NumberTooLong => messages::PLATE_NUMBER_TOO_LONG,
            Self::NeedsAuthority => messages::PLATE_NEEDS_AUTHORITY,
            Self::UnknownRegion => messages::PLATE_UNKNOWN_REGION,
            Self::BadCountry => messages::PLATE_BAD_COUNTRY,
            Self::AuthorityEmpty => messages::PLATE_AUTHORITY_EMPTY,
            Self::AuthorityTooLong => messages::PLATE_AUTHORITY_TOO_LONG,
            Self::RetiredBeforeIssued => messages::PLATE_RETIRED_BEFORE_ISSUED,
            Self::InUse => messages::PLATE_IN_USE,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::asset::IdentifierKind;
    use crate::test_support::test_id;

    pub(crate) fn date(text: &str) -> CalendarDate {
        CalendarDate::parse(text).unwrap()
    }

    pub(crate) fn plate(number: &str, issued: &str) -> LicensePlate {
        LicensePlate {
            asset: test_id(1),
            authority: Some(Authority::region("US", "NC").unwrap()),
            number: PlateNumber::parse(number).unwrap(),
            issued_on: Some(date(issued)),
            expires_on: None,
            retired_on: None,
            notes: Notes::default(),
            added_at: UnixMillis::from_millis(0),
            added_by: test_id(5),
            received_at: UnixMillis::from_millis(0),
        }
    }

    #[test]
    fn authorities_are_regions_countries_or_named() {
        assert_eq!(
            Authority::region(" us ", "tx"),
            Ok(Authority::Region {
                country: "US".into(),
                region: "TX".into()
            })
        );
        assert_eq!(Authority::region("CA", "ON").unwrap().name(), "Ontario");
        assert_eq!(
            Authority::region("US", "ON"),
            Err(PlateProblem::UnknownRegion)
        );
        assert_eq!(Authority::country("de").unwrap().name(), "DE");
        assert_eq!(Authority::country("DEU"), Err(PlateProblem::BadCountry));
        assert_eq!(
            Authority::other("Cherokee Nation").unwrap().name(),
            "Cherokee Nation"
        );
        assert_eq!(Authority::other(" "), Err(PlateProblem::AuthorityEmpty));
        assert_eq!(
            Authority::other(&"x".repeat(61)),
            Err(PlateProblem::AuthorityTooLong)
        );
        // One read back must be as it would be made.
        let lower = Authority::Region {
            country: "us".into(),
            region: "tx".into(),
        };
        assert_eq!(lower.check(), Err(PlateProblem::UnknownRegion));
        assert_eq!(REGIONS.len(), 56 + 13);
    }

    #[test]
    fn numbers_are_trimmed_and_in_capitals() {
        assert_eq!(
            PlateNumber::parse(" ncx-4471 ").unwrap().as_str(),
            "NCX-4471"
        );
        assert_eq!(PlateNumber::parse(""), Err(PlateProblem::NumberEmpty));
        assert_eq!(
            PlateNumber::parse(&"9".repeat(16)),
            Err(PlateProblem::NumberTooLong)
        );
    }

    #[test]
    fn the_current_plate_is_the_one_not_retired() {
        let mut texas = plate("TX 100", "2019-03-01");
        texas.retired_on = Some(date("2024-06-01"));
        let plates = vec![
            (test_id(10), texas),
            (test_id(11), plate("NCX 4471", "2024-06-01")),
        ];
        assert_eq!(current(&plates).map(|(id, _)| *id), Some(test_id(11)));
        assert_eq!(current(&plates[..1]), None);
    }

    #[test]
    fn a_new_plate_retires_the_current_one() {
        let plates = vec![(test_id(10), plate("NCX 4471", "2024-06-01"))];
        let today = date("2026-11-04");
        let (id, patch) = retire_for(&plates, &plate("VA 22", "2026-10-15"), &today).unwrap();
        assert_eq!(id, test_id(10));
        assert_eq!(patch.retired_on, Some(Clearable::Set(date("2026-10-15"))));

        let mut undated = plate("VA 22", "2026-10-15");
        undated.issued_on = None;
        let (_, patch) = retire_for(&plates, &undated, &today).unwrap();
        assert_eq!(patch.retired_on, Some(Clearable::Set(today)));
        assert!(retire_for(&[], &undated, &date("2026-11-04")).is_none());
    }

    #[test]
    fn a_new_plate_needs_an_authority_but_a_moved_one_may_wait() {
        let mut moved = plate("NCX 4471", "2024-06-01");
        moved.authority = None;
        assert_eq!(check(&moved, true), Err(PlateProblem::NeedsAuthority));
        assert_eq!(check(&moved, false), Ok(()));
        let mut backwards = plate("NCX 4471", "2024-06-01");
        backwards.retired_on = Some(date("2024-05-31"));
        assert_eq!(
            check(&backwards, false),
            Err(PlateProblem::RetiredBeforeIssued)
        );
    }

    #[test]
    fn plate_identifiers_move_out_and_the_rest_stay() {
        let kind = |text: &str| IdentifierKind::parse(text).unwrap();
        let identifiers = vec![
            Identifier::new(kind("vin"), "1FTFW1E50PFA00001").unwrap(),
            Identifier::new(kind("plate"), "ncx 4471").unwrap(),
            Identifier::new(kind("plate"), &"9".repeat(20)).unwrap(),
        ];
        let (plates, rest) = from_identifiers(identifiers);
        assert_eq!(plates, [PlateNumber::parse("NCX 4471").unwrap()]);
        assert_eq!(rest.len(), 2);
        assert_eq!(rest[0].kind.as_str(), "vin");
    }
}
