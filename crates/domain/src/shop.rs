//! Shops and labor rates (phase-6.md 2f; decision #089).
//!
//! - **The household's labor rate:** one rate an hour, set by Maintainers and Admins (kept with
//!   the household's preferences, [`crate::preferences::Preferences::labor_rate`]).
//! - **Shops:** the household's list of places that do work, e.g. "Joe's Auto", each with a
//!   contact and, if it differs, **its own rate**. A job's "Done by" suggests them.
//! - **Rates** are in the household's currency, in its smallest unit (cents) an hour. A job keeps
//!   the cost it was saved with, so a later change never touches old jobs.
//!
//! # Examples
//!
//! ```
//! use domain::shop::LaborRate;
//!
//! let rate = LaborRate::new(12_000).unwrap(); // $120.00 an hour
//! assert_eq!(rate.minor_per_hour(), 12_000);
//! assert!(LaborRate::new(-1).is_err());
//! ```

use serde::{Deserialize, Serialize};

use crate::command::{Clearable, Record, record_patch};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;

/// The most a contact may have, in characters (as a job's "Done by" contact).
pub const CONTACT_MAX_CHARS: usize = crate::event::PERFORMER_MAX_CHARS;

/// The highest labor rate: 100,000.00 an hour, far above any real one (a typing slip is caught).
pub const LABOR_RATE_MAX_MINOR: i64 = 10_000_000;

/// A labor rate: the household currency's smallest unit (cents) an hour, from 0 up to
/// [`LABOR_RATE_MAX_MINOR`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct LaborRate(i64);

impl LaborRate {
    /// A rate of `minor` an hour.
    ///
    /// # Errors
    ///
    /// [`ShopProblem::BadRate`] if it's negative or over [`LABOR_RATE_MAX_MINOR`].
    pub const fn new(minor: i64) -> Result<Self, ShopProblem> {
        if minor < 0 || minor > LABOR_RATE_MAX_MINOR {
            Err(ShopProblem::BadRate)
        } else {
            Ok(Self(minor))
        }
    }

    /// The rate, in the currency's smallest unit an hour.
    #[must_use]
    pub const fn minor_per_hour(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for LaborRate {
    type Error = ShopProblem;

    fn try_from(minor: i64) -> Result<Self, Self::Error> {
        Self::new(minor)
    }
}

impl From<LaborRate> for i64 {
    fn from(rate: LaborRate) -> Self {
        rate.0
    }
}

/// A shop: somewhere that does work on the household's vehicles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shop {
    /// Its name, e.g. "Joe's Auto" (unique among the household's shops, ignoring case).
    pub name: Label,
    /// How to reach it (a phone number, an email), up to [`CONTACT_MAX_CHARS`].
    pub contact: Option<String>,
    /// Its own labor rate; `None`: the household's.
    pub labor_rate: Option<LaborRate>,
}

/// The ID of a shop.
pub type ShopId = Id<Shop>;

record_patch! {
    /// The changeable fields of a [`Shop`]; `Some` means "set to this".
    ShopPatch
    {
        /// See [`Shop::name`].
        name: Label,
        /// See [`Shop::contact`].
        contact: Clearable<String>,
        /// See [`Shop::labor_rate`] (cleared: the household's).
        labor_rate: Clearable<LaborRate>,
    }
}

impl ShopPatch {
    /// Every field of `shop`, for adding it.
    #[must_use]
    pub fn all(shop: Shop) -> Self {
        Self {
            name: Some(shop.name),
            contact: Some(Clearable::from_option(shop.contact)),
            labor_rate: Some(Clearable::from_option(shop.labor_rate)),
        }
    }

    /// The fields that differ from `from` to `to`, for changing it.
    #[must_use]
    pub fn changes(from: &Shop, to: Shop) -> Self {
        Self {
            name: (from.name != to.name).then_some(to.name),
            contact: (from.contact != to.contact).then(|| Clearable::from_option(to.contact)),
            labor_rate: (from.labor_rate != to.labor_rate)
                .then(|| Clearable::from_option(to.labor_rate)),
        }
    }
}

impl Record for Shop {
    const ENTITY: &'static str = "shop";
    type Patch = ShopPatch;
}

/// Why a shop or a rate was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShopProblem {
    /// Another shop has that name.
    NameTaken,
    /// The contact is empty, has spaces at either end, or is too long.
    BadContact,
    /// A rate that's negative or far too high.
    BadRate,
}

impl ShopProblem {
    /// The message explaining it.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NameTaken => messages::SHOP_NAME_TAKEN,
            Self::BadContact => messages::SHOP_BAD_CONTACT,
            Self::BadRate => messages::SHOP_BAD_RATE,
        }
    }
}

impl core::fmt::Display for ShopProblem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = match self {
            Self::NameTaken => "another shop has that name",
            Self::BadContact => "a contact must be 1 to 100 characters",
            Self::BadRate => "a labor rate must be 0 to 100,000.00 an hour",
        };
        f.write_str(text)
    }
}

/// Checks `shop` (as `id`, if it's one already kept) against the household's `others`.
///
/// # Errors
///
/// The first [`ShopProblem`] found.
pub fn check_shop<'a>(
    shop: &Shop,
    id: Option<ShopId>,
    others: impl IntoIterator<Item = (ShopId, &'a Shop)>,
) -> Result<(), ShopProblem> {
    if let Some(contact) = &shop.contact {
        let trimmed = contact.trim();
        if trimmed != contact || trimmed.is_empty() || trimmed.chars().count() > CONTACT_MAX_CHARS {
            return Err(ShopProblem::BadContact);
        }
    }
    let name = shop.name.as_str().to_lowercase();
    if others
        .into_iter()
        .any(|(other, kept)| Some(other) != id && kept.name.as_str().to_lowercase() == name)
    {
        return Err(ShopProblem::NameTaken);
    }
    Ok(())
}

/// What `seconds` of labor cost at `rate`, in the currency's smallest unit, rounded to the
/// nearest (a half rounds up): 2:30 at 120.00 an hour is 300.00; 0:20 at 100.00 is 33.33.
#[must_use]
pub fn labor_cost(seconds: i64, rate: LaborRate) -> i64 {
    let product = i128::from(seconds.max(0)) * i128::from(rate.0);
    i64::try_from((product + 1_800) / 3_600).unwrap_or(i64::MAX)
}

/// The rate a job is priced at, and where it comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PricedAt<'a> {
    /// The rate an hour.
    pub rate: LaborRate,
    /// The shop whose own rate it is; `None`: the household's.
    pub shop: Option<&'a Shop>,
}

/// The rate for work done by `done_by` (a name as typed; `None`: someone in the household): the
/// shop of that name's own rate (ignoring capitals and spaces at either end), otherwise the
/// household's. `None` if neither has one.
#[must_use]
pub fn rate_for<'a>(
    done_by: Option<&str>,
    shops: &'a [(ShopId, Shop)],
    household: Option<LaborRate>,
) -> Option<PricedAt<'a>> {
    let named = done_by.map(|name| name.trim().to_lowercase());
    let shop = named.and_then(|name| {
        shops
            .iter()
            .map(|(_, shop)| shop)
            .find(|shop| shop.name.as_str().to_lowercase() == name)
    });
    match shop.and_then(|shop| shop.labor_rate.map(|rate| (rate, shop))) {
        Some((rate, shop)) => Some(PricedAt {
            rate,
            shop: Some(shop),
        }),
        None => household.map(|rate| PricedAt { rate, shop: None }),
    }
}

/// The most shops suggested at once.
pub const SUGGESTED_MAX: usize = 5;

/// The shops to suggest for a "Done by" name typed so far: those whose name has it in it
/// (ignoring capitals), by name, up to [`SUGGESTED_MAX`]; every shop (up to that) while nothing
/// is typed; none once it's a shop's whole name.
#[must_use]
pub fn suggested<'a>(typed: &str, shops: &'a [(ShopId, Shop)]) -> Vec<&'a Shop> {
    let typed = typed.trim().to_lowercase();
    if shops
        .iter()
        .any(|(_, shop)| shop.name.as_str().to_lowercase() == typed)
    {
        return Vec::new();
    }
    shops
        .iter()
        .map(|(_, shop)| shop)
        .filter(|shop| shop.name.as_str().to_lowercase().contains(&typed))
        .take(SUGGESTED_MAX)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    fn rated(name: &str, minor: Option<i64>) -> (ShopId, Shop) {
        (
            test_id(name.len().try_into().unwrap()),
            Shop {
                name: Label::parse(name).unwrap(),
                contact: None,
                labor_rate: minor.map(|minor| LaborRate::new(minor).unwrap()),
            },
        )
    }

    #[test]
    fn labor_costs_its_time_at_the_rate_to_the_nearest_cent() {
        let rate = |minor| LaborRate::new(minor).unwrap();
        assert_eq!(labor_cost(9_000, rate(12_000)), 30_000);
        assert_eq!(labor_cost(1_200, rate(10_000)), 3_333);
        // 0:40 at 100.00 is 66.666…: up to 66.67; 0:30 at 0.01 is half a cent: up.
        assert_eq!(labor_cost(2_400, rate(10_000)), 6_667);
        assert_eq!(labor_cost(1_800, rate(1)), 1);
        assert_eq!(labor_cost(0, rate(12_000)), 0);
        assert_eq!(labor_cost(-60, rate(12_000)), 0);
        // The longest time at the highest rate doesn't overflow.
        assert_eq!(labor_cost(i64::MAX, rate(LABOR_RATE_MAX_MINOR)), i64::MAX);
    }

    #[test]
    fn a_shops_own_rate_comes_first_then_the_households() {
        let shops = [rated("Joe's Auto", Some(12_000)), rated("Tire Barn", None)];
        let household = Some(LaborRate::new(9_500).unwrap());
        let joes = rate_for(Some(" joe's auto "), &shops, household).unwrap();
        assert_eq!(joes.rate.minor_per_hour(), 12_000);
        assert_eq!(joes.shop.map(|shop| shop.name.as_str()), Some("Joe's Auto"));
        // A shop without its own rate, someone else, or the household: the household's.
        for done_by in [Some("Tire Barn"), Some("Pat"), None] {
            let priced = rate_for(done_by, &shops, household).unwrap();
            assert_eq!((priced.rate.minor_per_hour(), priced.shop), (9_500, None));
        }
        assert_eq!(rate_for(Some("Tire Barn"), &shops, None), None);
        assert!(rate_for(Some("Joe's Auto"), &shops, None).is_some());
    }

    #[test]
    fn done_by_suggests_the_shops_it_could_be() {
        let shops = [
            rated("Dealer service", None),
            rated("Joe's Auto", None),
            rated("Tire Barn", None),
        ];
        let names = |typed| -> Vec<&str> {
            suggested(typed, &shops)
                .iter()
                .map(|shop| shop.name.as_str())
                .collect()
        };
        assert_eq!(names(""), ["Dealer service", "Joe's Auto", "Tire Barn"]);
        assert_eq!(names("AR"), ["Tire Barn"]);
        assert_eq!(names("e"), ["Dealer service", "Joe's Auto", "Tire Barn"]);
        assert!(names("joe's auto").is_empty());
        assert!(names("Pat").is_empty());
        let many: Vec<(ShopId, Shop)> = (1..=8)
            .map(|n| rated(&format!("Shop {}", "x".repeat(n)), None))
            .collect();
        assert_eq!(suggested("shop", &many).len(), SUGGESTED_MAX);
    }

    fn shop(name: &str, contact: Option<&str>) -> Shop {
        Shop {
            name: Label::parse(name).unwrap(),
            contact: contact.map(str::to_owned),
            labor_rate: None,
        }
    }

    #[test]
    fn rates_are_never_negative_or_absurd() {
        assert_eq!(LaborRate::new(0).map(LaborRate::minor_per_hour), Ok(0));
        assert!(LaborRate::new(LABOR_RATE_MAX_MINOR).is_ok());
        assert_eq!(LaborRate::new(-1), Err(ShopProblem::BadRate));
        assert_eq!(
            LaborRate::new(LABOR_RATE_MAX_MINOR + 1),
            Err(ShopProblem::BadRate)
        );
        // Stored as a plain number; a bad one isn't read back.
        assert_eq!(
            serde_json::to_string(&LaborRate::new(12_000).unwrap()).unwrap(),
            "12000"
        );
        assert!(serde_json::from_str::<LaborRate>("-5").is_err());
    }

    #[test]
    fn a_shops_name_is_its_own_and_its_contact_is_tidy() {
        let joes = shop("Joe's Auto", Some("541-555-0100"));
        let kept = [(test_id(1), &joes)];
        assert_eq!(check_shop(&shop("Tire Barn", None), None, kept), Ok(()));
        assert_eq!(
            check_shop(&shop("joe's auto", None), None, kept),
            Err(ShopProblem::NameTaken)
        );
        // Changing Joe's itself keeps its name.
        assert_eq!(check_shop(&joes, Some(test_id(1)), kept), Ok(()));
        for contact in ["", " 555", &"x".repeat(CONTACT_MAX_CHARS + 1)] {
            assert_eq!(
                check_shop(&shop("New", Some(contact)), None, kept),
                Err(ShopProblem::BadContact)
            );
        }
    }

    #[test]
    fn a_change_names_only_what_changed() {
        let before = shop("Joe's Auto", None);
        let after = Shop {
            labor_rate: Some(LaborRate::new(12_000).unwrap()),
            ..before.clone()
        };
        let patch = ShopPatch::changes(&before, after);
        assert_eq!(patch.name, None);
        assert_eq!(patch.contact, None);
        assert_eq!(
            patch.labor_rate,
            Some(Clearable::Set(LaborRate::new(12_000).unwrap()))
        );
    }
}
