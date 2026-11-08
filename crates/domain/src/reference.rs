//! Reference documents: recalls, bulletins, and manufacturer schedules, shared by the household
//! (milestone 9; plan §3 Requirements & compliance, layer 1; decision #068).
//!
//! A [`ReferenceDocument`] is the authoritative item itself, independent of any vehicle: its
//! type and issuing authority, reference numbers, title, description and remedy, dates, a
//! revision, the applicability **as written**, a URL, and attachments of its own (not a
//! vehicle's). A vehicle's [`Requirement`](crate::requirement::Requirement) links to it; several
//! vehicles can link the same one.
//!
//! **Types are keys, not a fixed list,** so more can be added without a domain change or a
//! migration: a [`ReferenceType::Listed`] key from [`LISTED_TYPES`] (each with its own
//! message), or a [`ReferenceType::Custom`] name. Airworthiness directives join with aircraft.
//!
//! **Who may do what** ([`may_read`], [`may_change`]): anyone who can see a vehicle that links
//! it can read it; changing it needs a Maintainer of **every** vehicle that links it (a change
//! shows on all of them), or the household's Admin in Admin mode.
//!
//! Supersession is recorded ([`ReferenceDocument::supersedes`]) but not acted on until Phase 5.
//!
//! # Examples
//!
//! ```
//! use domain::reference::{RECALL, ReferenceType};
//!
//! assert_eq!(ReferenceType::listed(RECALL).unwrap(), ReferenceType::Listed { key: RECALL.into() });
//! assert!(ReferenceType::listed("recall").is_err(), "keys start with `reference-`");
//! ```

use serde::{Deserialize, Serialize};

use crate::access::{
    ActiveMode, AssetAction, AssetScope, RoleGrant, is_household_admin, permits_on_asset,
};
use crate::command::{Clearable, Record, record_patch};
use crate::dates::CalendarDate;
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;
use crate::notes::Notes;
use crate::time::UnixMillis;
use crate::user::UserId;

/// The ID of a reference document.
pub type ReferenceDocumentId = Id<ReferenceDocument>;

/// The key of a recall.
pub const RECALL: &str = messages::REFERENCE_RECALL.as_str();
/// The key of a technical service bulletin.
pub const TECHNICAL_SERVICE_BULLETIN: &str =
    messages::REFERENCE_TECHNICAL_SERVICE_BULLETIN.as_str();
/// The key of a service bulletin.
pub const SERVICE_BULLETIN: &str = messages::REFERENCE_SERVICE_BULLETIN.as_str();
/// The key of a manufacturer's schedule.
pub const MANUFACTURER_SCHEDULE: &str = messages::REFERENCE_MANUFACTURER_SCHEDULE.as_str();
/// The key of anything else.
pub const OTHER: &str = messages::REFERENCE_OTHER.as_str();

/// The types offered, in order (each key is also its message's ID). Add one here, with its
/// message, to offer another.
pub const LISTED_TYPES: [&str; 5] = [
    RECALL,
    TECHNICAL_SERVICE_BULLETIN,
    SERVICE_BULLETIN,
    MANUFACTURER_SCHEDULE,
    OTHER,
];

/// The most characters a title may have.
pub const TITLE_MAX_CHARS: usize = 200;
/// The most characters an authority may have.
pub const AUTHORITY_MAX_CHARS: usize = 100;
/// The most characters a reference number may have.
pub const NUMBER_MAX_CHARS: usize = 60;
/// The most characters a revision may have.
pub const REVISION_MAX_CHARS: usize = 20;
/// The most reference numbers one document may have.
pub const MAX_NUMBERS: usize = 10;
/// The most characters a URL may have.
pub const URL_MAX_CHARS: usize = 2_000;

/// What kind of reference document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ReferenceType {
    /// A built-in type, by its key (e.g. `reference-recall`). A key this version doesn't list
    /// (from a newer one) is kept as it is.
    Listed {
        /// The key.
        key: String,
    },
    /// One the household named, e.g. "Owner notification".
    Custom {
        /// Its name.
        name: Label,
    },
}

impl ReferenceType {
    /// A listed type by its key: `reference-` and lowercase letters and dashes.
    ///
    /// # Errors
    ///
    /// [`ReferenceProblem::UnknownType`].
    pub fn listed(key: &str) -> Result<Self, ReferenceProblem> {
        let name = key.strip_prefix("reference-").unwrap_or_default();
        if !name.is_empty()
            && key.len() <= 60
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        {
            Ok(Self::Listed {
                key: key.to_owned(),
            })
        } else {
            Err(ReferenceProblem::UnknownType)
        }
    }

    /// The types offered, in order.
    #[must_use]
    pub fn offered() -> Vec<Self> {
        LISTED_TYPES
            .iter()
            .map(|key| Self::Listed {
                key: (*key).to_owned(),
            })
            .collect()
    }
}

/// One of a document's reference numbers, e.g. NHTSA campaign `24V-123`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReferenceNumber {
    /// Whose number it is, e.g. "NHTSA campaign" or "Ford recall", if wanted.
    pub scheme: Option<Label>,
    /// The number (1–60 characters).
    pub number: String,
}

/// One recall, bulletin, or schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceDocument {
    /// What kind.
    pub doc_type: ReferenceType,
    /// Who issued it, e.g. "NHTSA" or the manufacturer (up to 100 characters).
    pub authority: Option<String>,
    /// Its reference numbers (up to 10).
    pub numbers: Vec<ReferenceNumber>,
    /// Its title (1–200 characters).
    pub title: String,
    /// What it's about.
    pub description: Notes,
    /// The remedy or required action.
    pub remedy: Notes,
    /// When it was issued.
    pub issued_on: Option<CalendarDate>,
    /// When it takes effect, if not when issued.
    pub effective_on: Option<CalendarDate>,
    /// Its revision, e.g. "B" (up to 20 characters).
    pub revision: Option<String>,
    /// The document it replaces (recorded; acted on in Phase 5).
    pub supersedes: Option<ReferenceDocumentId>,
    /// Which vehicles it applies to, as written (model, years, VIN range); matching is
    /// post-MVP.
    pub applicability: Notes,
    /// Where it's published (`http` or `https`).
    pub url: Option<String>,
    /// When it was added.
    pub added_at: UnixMillis,
    /// Who added it.
    pub added_by: UserId,
    /// When the server first received it.
    pub received_at: UnixMillis,
}

record_patch! {
    /// The changeable fields of a [`ReferenceDocument`]; `Some` means "set to this".
    ReferenceDocumentPatch
    {
        /// See [`ReferenceDocument::doc_type`].
        doc_type: ReferenceType,
        /// See [`ReferenceDocument::authority`].
        authority: Clearable<String>,
        /// See [`ReferenceDocument::numbers`] (the whole list).
        numbers: Vec<ReferenceNumber>,
        /// See [`ReferenceDocument::title`].
        title: String,
        /// See [`ReferenceDocument::description`].
        description: Notes,
        /// See [`ReferenceDocument::remedy`].
        remedy: Notes,
        /// See [`ReferenceDocument::issued_on`].
        issued_on: Clearable<CalendarDate>,
        /// See [`ReferenceDocument::effective_on`].
        effective_on: Clearable<CalendarDate>,
        /// See [`ReferenceDocument::revision`].
        revision: Clearable<String>,
        /// See [`ReferenceDocument::supersedes`].
        supersedes: Clearable<ReferenceDocumentId>,
        /// See [`ReferenceDocument::applicability`].
        applicability: Notes,
        /// See [`ReferenceDocument::url`].
        url: Clearable<String>,
        /// See [`ReferenceDocument::added_at`] (set when added).
        added_at: UnixMillis,
        /// See [`ReferenceDocument::added_by`] (set when added).
        added_by: UserId,
        /// See [`ReferenceDocument::received_at`] (set when added).
        received_at: UnixMillis,
    }
}

impl ReferenceDocumentPatch {
    /// Every field of `document`, for adding it.
    #[must_use]
    pub fn all(document: ReferenceDocument) -> Self {
        Self {
            doc_type: Some(document.doc_type),
            authority: Some(Clearable::from_option(document.authority)),
            numbers: Some(document.numbers),
            title: Some(document.title),
            description: Some(document.description),
            remedy: Some(document.remedy),
            issued_on: Some(Clearable::from_option(document.issued_on)),
            effective_on: Some(Clearable::from_option(document.effective_on)),
            revision: Some(Clearable::from_option(document.revision)),
            supersedes: Some(Clearable::from_option(document.supersedes)),
            applicability: Some(document.applicability),
            url: Some(Clearable::from_option(document.url)),
            added_at: Some(document.added_at),
            added_by: Some(document.added_by),
            received_at: Some(document.received_at),
        }
    }
}

impl Record for ReferenceDocument {
    const ENTITY: &'static str = "reference-document";
    type Patch = ReferenceDocumentPatch;
}

/// Why a reference document was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum ReferenceProblem {
    /// A listed type that isn't a reference key.
    UnknownType,
    /// No title, or one over 200 characters.
    BadTitle,
    /// An authority over 100 characters.
    AuthorityTooLong,
    /// A reference number that's empty or over 60 characters, the same one twice, or more
    /// than 10.
    BadNumber,
    /// A revision over 20 characters.
    RevisionTooLong,
    /// A URL that isn't `http` or `https`, has spaces, or is too long.
    BadUrl,
    /// Taking effect before it was issued.
    EffectiveBeforeIssued,
    /// Superseding itself, or one that isn't a reference document.
    BadSupersedes,
}

impl ReferenceProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::UnknownType | Self::BadSupersedes => messages::SOMETHING_WENT_WRONG,
            Self::BadTitle => messages::REFERENCE_BAD_TITLE,
            Self::AuthorityTooLong => messages::REFERENCE_AUTHORITY_TOO_LONG,
            Self::BadNumber => messages::REFERENCE_BAD_NUMBER,
            Self::RevisionTooLong => messages::REFERENCE_REVISION_TOO_LONG,
            Self::BadUrl => messages::REFERENCE_BAD_URL,
            Self::EffectiveBeforeIssued => messages::REFERENCE_EFFECTIVE_BEFORE_ISSUED,
        }
    }
}

/// Checks reference document `id` against the household's other ones (`known`: their IDs).
///
/// # Errors
///
/// The first [`ReferenceProblem`] found.
pub fn check(
    id: ReferenceDocumentId,
    document: &ReferenceDocument,
    known: &[ReferenceDocumentId],
) -> Result<(), ReferenceProblem> {
    if let ReferenceType::Listed { key } = &document.doc_type {
        ReferenceType::listed(key)?;
    }
    let length = |text: &str| text.chars().count();
    let title = length(document.title.trim());
    if title == 0 || title > TITLE_MAX_CHARS {
        return Err(ReferenceProblem::BadTitle);
    }
    if document
        .authority
        .as_deref()
        .is_some_and(|authority| length(authority) > AUTHORITY_MAX_CHARS)
    {
        return Err(ReferenceProblem::AuthorityTooLong);
    }
    check_numbers(&document.numbers)?;
    if document
        .revision
        .as_deref()
        .is_some_and(|revision| length(revision) > REVISION_MAX_CHARS)
    {
        return Err(ReferenceProblem::RevisionTooLong);
    }
    if let Some(url) = &document.url {
        check_url(url)?;
    }
    if let (Some(issued), Some(effective)) = (&document.issued_on, &document.effective_on)
        && effective < issued
    {
        return Err(ReferenceProblem::EffectiveBeforeIssued);
    }
    if let Some(replaced) = document.supersedes
        && (replaced == id || !known.contains(&replaced))
    {
        return Err(ReferenceProblem::BadSupersedes);
    }
    Ok(())
}

fn check_numbers(numbers: &[ReferenceNumber]) -> Result<(), ReferenceProblem> {
    if numbers.len() > MAX_NUMBERS {
        return Err(ReferenceProblem::BadNumber);
    }
    for (index, number) in numbers.iter().enumerate() {
        let length = number.number.trim().chars().count();
        if length == 0
            || length > NUMBER_MAX_CHARS
            || numbers[..index].iter().any(|earlier| earlier == number)
        {
            return Err(ReferenceProblem::BadNumber);
        }
    }
    Ok(())
}

/// A URL is `http://` or `https://` with something after it, no spaces, up to 2,000
/// characters.
///
/// # Errors
///
/// [`ReferenceProblem::BadUrl`].
pub fn check_url(url: &str) -> Result<(), ReferenceProblem> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"));
    match rest {
        Some(rest)
            if !rest.is_empty()
                && url.chars().count() <= URL_MAX_CHARS
                && !url.chars().any(char::is_whitespace) =>
        {
            Ok(())
        }
        Some(_) | None => Err(ReferenceProblem::BadUrl),
    }
}

/// Whether the person may read a reference document that the vehicles `linked` link to: one of
/// them they can see, or they're the household's Admin.
#[must_use]
pub fn may_read(grants: &[RoleGrant], mode: ActiveMode, linked: &[AssetScope]) -> bool {
    is_household_admin(grants)
        || linked
            .iter()
            .any(|asset| permits_on_asset(grants, mode, AssetAction::View, *asset))
}

/// Whether the person, in `mode`, may change (or remove) a reference document that the vehicles
/// `linked` link to: a Maintainer of every one of them, or the household's Admin in Admin mode.
/// One linked by no vehicle is the household Admin's alone.
#[must_use]
pub fn may_change(grants: &[RoleGrant], mode: ActiveMode, linked: &[AssetScope]) -> bool {
    (mode == ActiveMode::Admin && is_household_admin(grants))
        || (!linked.is_empty()
            && linked.iter().all(|asset| {
                permits_on_asset(grants, mode, AssetAction::ConfigureMaintenance, *asset)
            }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::{GrantTarget, Role};
    use crate::command::Patch;
    use crate::test_support::test_id;

    fn recall() -> ReferenceDocument {
        ReferenceDocument {
            doc_type: ReferenceType::listed(RECALL).unwrap(),
            authority: Some("NHTSA".to_owned()),
            numbers: vec![
                ReferenceNumber {
                    scheme: Some(Label::parse("NHTSA campaign").unwrap()),
                    number: "24V-123".to_owned(),
                },
                ReferenceNumber {
                    scheme: Some(Label::parse("Toyota").unwrap()),
                    number: "24TA05".to_owned(),
                },
            ],
            title: "Rear axle nut may loosen".to_owned(),
            description: Notes::parse("The nut may not be torqued.").unwrap(),
            remedy: Notes::parse("Dealers will retorque the nut, free.").unwrap(),
            issued_on: Some(CalendarDate::parse("2024-03-01").unwrap()),
            effective_on: Some(CalendarDate::parse("2024-03-15").unwrap()),
            revision: None,
            supersedes: None,
            applicability: Notes::parse("2022–2024 Tacoma").unwrap(),
            url: Some("https://www.nhtsa.gov/recalls?nhtsaId=24V123".to_owned()),
            added_at: UnixMillis::from_millis(0),
            added_by: test_id(5),
            received_at: UnixMillis::from_millis(0),
        }
    }

    fn checked(document: &ReferenceDocument) -> Result<(), ReferenceProblem> {
        check(test_id(1), document, &[test_id(1), test_id(2)])
    }

    #[test]
    fn a_good_recall_passes() {
        assert_eq!(checked(&recall()), Ok(()));
    }

    #[test]
    fn types_are_keys_so_more_can_be_added() {
        for key in LISTED_TYPES {
            assert!(ReferenceType::listed(key).is_ok(), "{key}");
        }
        assert_eq!(ReferenceType::offered().len(), LISTED_TYPES.len());
        let mut newer = recall();
        newer.doc_type = ReferenceType::listed("reference-airworthiness-directive").unwrap();
        assert_eq!(
            checked(&newer),
            Ok(()),
            "a key from a newer version is kept"
        );
        assert_eq!(
            ReferenceType::listed("recall"),
            Err(ReferenceProblem::UnknownType)
        );
        let mut custom = recall();
        custom.doc_type = ReferenceType::Custom {
            name: Label::parse("Owner notification").unwrap(),
        };
        assert_eq!(checked(&custom), Ok(()));
    }

    #[test]
    fn every_listed_type_has_an_english_name() {
        let english = crate::i18n::Translator::english();
        for key in LISTED_TYPES {
            assert!(english.has_key(key), "{key}");
        }
    }

    #[test]
    fn titles_numbers_and_lengths_are_checked() {
        let with = |change: fn(&mut ReferenceDocument)| {
            let mut document = recall();
            change(&mut document);
            checked(&document)
        };
        assert_eq!(
            with(|d| d.title = "  ".into()),
            Err(ReferenceProblem::BadTitle)
        );
        assert_eq!(
            with(|d| d.title = "x".repeat(TITLE_MAX_CHARS + 1)),
            Err(ReferenceProblem::BadTitle)
        );
        assert_eq!(
            with(|d| d.authority = Some("x".repeat(AUTHORITY_MAX_CHARS + 1))),
            Err(ReferenceProblem::AuthorityTooLong)
        );
        assert_eq!(
            with(|d| d.numbers[1] = d.numbers[0].clone()),
            Err(ReferenceProblem::BadNumber),
            "the same number twice"
        );
        assert_eq!(
            with(|d| d.numbers[0].number = String::new()),
            Err(ReferenceProblem::BadNumber)
        );
        assert_eq!(
            with(|d| d.revision = Some("x".repeat(REVISION_MAX_CHARS + 1))),
            Err(ReferenceProblem::RevisionTooLong)
        );
        assert_eq!(
            with(|d| d.effective_on = Some(CalendarDate::parse("2024-01-01").unwrap())),
            Err(ReferenceProblem::EffectiveBeforeIssued)
        );
    }

    #[test]
    fn urls_are_web_addresses() {
        assert_eq!(check_url("https://example.com/a"), Ok(()));
        assert_eq!(check_url("http://example.com"), Ok(()));
        for bad in [
            "example.com",
            "https://",
            "javascript:alert(1)",
            "https://a b",
        ] {
            assert_eq!(check_url(bad), Err(ReferenceProblem::BadUrl), "{bad}");
        }
    }

    #[test]
    fn it_supersedes_only_another_known_one() {
        let mut document = recall();
        document.supersedes = Some(test_id(2));
        assert_eq!(checked(&document), Ok(()));
        document.supersedes = Some(test_id(1));
        assert_eq!(checked(&document), Err(ReferenceProblem::BadSupersedes));
        document.supersedes = Some(test_id(9));
        assert_eq!(checked(&document), Err(ReferenceProblem::BadSupersedes));
    }

    fn scope(n: u32) -> AssetScope {
        AssetScope {
            id: test_id(n),
            group: test_id(100),
        }
    }

    fn grant(asset: u32, role: Role) -> RoleGrant {
        RoleGrant {
            user: test_id(5),
            target: GrantTarget::Asset(test_id(asset)),
            role,
        }
    }

    #[test]
    fn changing_one_needs_a_maintainer_of_every_vehicle_that_links_it() {
        let linked = [scope(1), scope(2)];
        let of_one = [grant(1, Role::Maintainer), grant(2, Role::Viewer)];
        assert!(may_read(&of_one, ActiveMode::Maintainer, &linked));
        assert!(!may_change(&of_one, ActiveMode::Maintainer, &linked));
        let of_both = [grant(1, Role::Maintainer), grant(2, Role::Maintainer)];
        assert!(may_change(&of_both, ActiveMode::Maintainer, &linked));
        assert!(
            !may_change(&of_both, ActiveMode::Driver, &linked),
            "the mode caps it"
        );
        assert!(!may_read(
            &[grant(3, Role::Admin)],
            ActiveMode::Admin,
            &linked
        ));
    }

    #[test]
    fn the_household_admin_may_change_any_in_admin_mode() {
        let admin = [RoleGrant {
            user: test_id(5),
            target: GrantTarget::AllAssets,
            role: Role::Admin,
        }];
        assert!(may_change(&admin, ActiveMode::Admin, &[]));
        assert!(may_change(&admin, ActiveMode::Admin, &[scope(1)]));
        assert!(may_read(&admin, ActiveMode::Driver, &[]));
        assert!(
            !may_change(&admin, ActiveMode::Maintainer, &[]),
            "one linked by nothing is the Admin's, in Admin mode"
        );
    }

    #[test]
    fn the_patch_covers_every_field_and_survives_the_trip() {
        let patch = ReferenceDocumentPatch::all(recall());
        assert_eq!(patch.changed_fields().len(), 15);
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(
            serde_json::from_str::<ReferenceDocumentPatch>(&json).unwrap(),
            patch
        );
    }
}
