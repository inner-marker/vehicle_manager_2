//! Official documents about a vehicle: title, registration, insurance, certificates
//! (milestone 8; plan §3 Official asset documents).
//!
//! Each [`AssetDocument`] has a type (from the kind's profile, or one of the household's own),
//! a number and issuer, its dates, notes, and its scans as attachments. **Renewing** makes a
//! new record that **supersedes** the old one ([`renewal_of`]); the old one stays as history.
//! A document's [`DocumentStatus`] (current, expired, superseded) comes from that and today's
//! date, so it's never stored.
//!
//! **A registration links to its license plate**, and its expiry becomes the plate's
//! ([`plate_follows`], decision #066).
//!
//! # Examples
//!
//! ```
//! use domain::document::DocumentType;
//!
//! assert!(DocumentType::listed("document-registration").unwrap().is_registration());
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Clearable, Record, record_patch};
use crate::dates::CalendarDate;
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;
use crate::notes::Notes;
use crate::plate::{LicensePlate, LicensePlateId, LicensePlatePatch};
use crate::time::UnixMillis;
use crate::user::UserId;

/// The ID of an official document.
pub type AssetDocumentId = Id<AssetDocument>;

/// The profile key of a registration, the one type linked to a plate.
pub const REGISTRATION: &str = "document-registration";

/// The most characters a document's number may have.
pub const NUMBER_MAX_CHARS: usize = 60;

/// The most characters a document's issuer may have.
pub const ISSUER_MAX_CHARS: usize = 100;

/// What kind of document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DocumentType {
    /// One the kind's profile lists, by its message key (e.g. `document-insurance`).
    Listed {
        /// The key.
        key: String,
    },
    /// One the household named, e.g. "Parking permit".
    Custom {
        /// Its name.
        name: Label,
    },
}

impl DocumentType {
    /// A listed type by its key: `document-` and lowercase letters and dashes.
    ///
    /// # Errors
    ///
    /// [`DocumentProblem::UnknownType`].
    pub fn listed(key: &str) -> Result<Self, DocumentProblem> {
        let name = key.strip_prefix("document-").unwrap_or_default();
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
            Err(DocumentProblem::UnknownType)
        }
    }

    /// Whether it's a registration (which links to a plate).
    #[must_use]
    pub fn is_registration(&self) -> bool {
        matches!(self, Self::Listed { key } if key == REGISTRATION)
    }
}

/// One official document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetDocument {
    /// Which vehicle.
    pub asset: AssetId,
    /// What kind.
    pub doc_type: DocumentType,
    /// Its number (registration, policy, certificate), up to 60 characters.
    pub number: Option<String>,
    /// Who issued it (a state, an insurer, an agency), up to 100 characters.
    pub issuer: Option<String>,
    /// When it was issued.
    pub issued_on: Option<CalendarDate>,
    /// When it takes effect, if not when issued (e.g. an insurance term).
    pub effective_on: Option<CalendarDate>,
    /// When it expires; none means it never does (a title).
    pub expires_on: Option<CalendarDate>,
    /// The document this one renews, if any.
    pub supersedes: Option<AssetDocumentId>,
    /// A registration's license plate.
    pub plate: Option<LicensePlateId>,
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
    /// The changeable fields of an [`AssetDocument`]; `Some` means "set to this".
    AssetDocumentPatch
    {
        /// See [`AssetDocument::asset`] (set when added).
        asset: AssetId,
        /// See [`AssetDocument::doc_type`].
        doc_type: DocumentType,
        /// See [`AssetDocument::number`].
        number: Clearable<String>,
        /// See [`AssetDocument::issuer`].
        issuer: Clearable<String>,
        /// See [`AssetDocument::issued_on`].
        issued_on: Clearable<CalendarDate>,
        /// See [`AssetDocument::effective_on`].
        effective_on: Clearable<CalendarDate>,
        /// See [`AssetDocument::expires_on`].
        expires_on: Clearable<CalendarDate>,
        /// See [`AssetDocument::supersedes`] (set when added).
        supersedes: Clearable<AssetDocumentId>,
        /// See [`AssetDocument::plate`].
        plate: Clearable<LicensePlateId>,
        /// See [`AssetDocument::notes`].
        notes: Notes,
        /// See [`AssetDocument::added_at`] (set when added).
        added_at: UnixMillis,
        /// See [`AssetDocument::added_by`] (set when added).
        added_by: UserId,
        /// See [`AssetDocument::received_at`] (set when added).
        received_at: UnixMillis,
    }
}

impl AssetDocumentPatch {
    /// Every field of `document`, for adding it.
    #[must_use]
    pub fn all(document: AssetDocument) -> Self {
        Self {
            asset: Some(document.asset),
            doc_type: Some(document.doc_type),
            number: Some(Clearable::from_option(document.number)),
            issuer: Some(Clearable::from_option(document.issuer)),
            issued_on: Some(Clearable::from_option(document.issued_on)),
            effective_on: Some(Clearable::from_option(document.effective_on)),
            expires_on: Some(Clearable::from_option(document.expires_on)),
            supersedes: Some(Clearable::from_option(document.supersedes)),
            plate: Some(Clearable::from_option(document.plate)),
            notes: Some(document.notes),
            added_at: Some(document.added_at),
            added_by: Some(document.added_by),
            received_at: Some(document.received_at),
        }
    }
}

impl Record for AssetDocument {
    const ENTITY: &'static str = "asset-document";
    type Patch = AssetDocumentPatch;
}

/// Where a document stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DocumentStatus {
    /// In force.
    Current,
    /// Past its expiry, and not renewed.
    Expired,
    /// Renewed: a later document supersedes it.
    Superseded,
}

impl DocumentStatus {
    /// The status's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Current => messages::DOCUMENT_STATUS_CURRENT,
            Self::Expired => messages::DOCUMENT_STATUS_EXPIRED,
            Self::Superseded => messages::DOCUMENT_STATUS_SUPERSEDED,
        }
    }
}

/// Whether any of `documents` renews `id`.
fn is_superseded(id: AssetDocumentId, documents: &[(AssetDocumentId, AssetDocument)]) -> bool {
    documents
        .iter()
        .any(|(_, document)| document.supersedes == Some(id))
}

/// Document `id`'s status among `documents` (all one vehicle's) on `today`: superseded if one
/// renews it, expired once past its expiry (it's good through that day), otherwise current.
#[must_use]
pub fn status(
    id: AssetDocumentId,
    document: &AssetDocument,
    documents: &[(AssetDocumentId, AssetDocument)],
    today: &CalendarDate,
) -> DocumentStatus {
    if is_superseded(id, documents) {
        DocumentStatus::Superseded
    } else if document
        .expires_on
        .as_ref()
        .is_some_and(|expires| expires < today)
    {
        DocumentStatus::Expired
    } else {
        DocumentStatus::Current
    }
}

/// A renewal of `previous` (whose ID is `previous_id`), ready to fill in: the same vehicle,
/// type, issuer, number, and plate, superseding it, with no dates or notes yet.
#[must_use]
pub fn renewal_of(
    previous_id: AssetDocumentId,
    previous: &AssetDocument,
    added_at: UnixMillis,
    added_by: UserId,
) -> AssetDocument {
    AssetDocument {
        asset: previous.asset,
        doc_type: previous.doc_type.clone(),
        number: previous.number.clone(),
        issuer: previous.issuer.clone(),
        issued_on: None,
        effective_on: None,
        expires_on: None,
        supersedes: Some(previous_id),
        plate: previous.plate,
        notes: Notes::default(),
        added_at,
        added_by,
        received_at: added_at,
    }
}

/// The change a registration makes to its plate: the plate expires when the registration
/// does. None unless `document` (whose ID is `id`) is a registration with a plate and an
/// expiry that nothing renews yet, and the plate's expiry differs.
#[must_use]
pub fn plate_follows(
    id: AssetDocumentId,
    document: &AssetDocument,
    documents: &[(AssetDocumentId, AssetDocument)],
    plates: &[(LicensePlateId, LicensePlate)],
) -> Option<(LicensePlateId, LicensePlatePatch)> {
    let plate_id = document.plate?;
    let expires = document.expires_on.clone()?;
    if !document.doc_type.is_registration() || is_superseded(id, documents) {
        return None;
    }
    let (_, plate) = plates
        .iter()
        .find(|(candidate, _)| *candidate == plate_id)?;
    (plate.expires_on.as_ref() != Some(&expires)).then(|| {
        (
            plate_id,
            LicensePlatePatch {
                expires_on: Some(Clearable::Set(expires)),
                ..LicensePlatePatch::default()
            },
        )
    })
}

/// Checks document `id` among the vehicle's `documents` (not counting itself) and `plates`:
/// its number and issuer fit; it doesn't expire before it's issued; a renewal is of the same
/// vehicle and type, and of one nothing else renews; and only a registration has a plate, the
/// vehicle's own.
///
/// # Errors
///
/// The first [`DocumentProblem`] found.
pub fn check(
    id: AssetDocumentId,
    document: &AssetDocument,
    documents: &[(AssetDocumentId, AssetDocument)],
    plates: &[(LicensePlateId, LicensePlate)],
) -> Result<(), DocumentProblem> {
    if let DocumentType::Listed { key } = &document.doc_type {
        DocumentType::listed(key)?;
    }
    let too_long = |text: &Option<String>, max: usize| {
        text.as_ref().is_some_and(|text| text.chars().count() > max)
    };
    if too_long(&document.number, NUMBER_MAX_CHARS) {
        return Err(DocumentProblem::NumberTooLong);
    }
    if too_long(&document.issuer, ISSUER_MAX_CHARS) {
        return Err(DocumentProblem::IssuerTooLong);
    }
    let start = document
        .effective_on
        .as_ref()
        .or(document.issued_on.as_ref());
    if let (Some(start), Some(expires)) = (start, &document.expires_on)
        && expires < start
    {
        return Err(DocumentProblem::ExpiresBeforeIssued);
    }
    if let Some(previous) = document.supersedes {
        let Some((_, renewed)) = documents.iter().find(|(other, _)| *other == previous) else {
            return Err(DocumentProblem::RenewsUnknown);
        };
        if renewed.asset != document.asset || renewed.doc_type != document.doc_type {
            return Err(DocumentProblem::RenewsOtherType);
        }
        if documents
            .iter()
            .any(|(other, rival)| *other != id && rival.supersedes == Some(previous))
        {
            return Err(DocumentProblem::AlreadyRenewed);
        }
    }
    if let Some(plate) = document.plate {
        if !document.doc_type.is_registration() {
            return Err(DocumentProblem::PlateNotRegistration);
        }
        if !plates
            .iter()
            .any(|(candidate, owned)| *candidate == plate && owned.asset == document.asset)
        {
            return Err(DocumentProblem::UnknownPlate);
        }
    }
    Ok(())
}

/// Why a document was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum DocumentProblem {
    /// A listed type that isn't a document key.
    UnknownType,
    /// A number over 60 characters.
    NumberTooLong,
    /// An issuer over 100 characters.
    IssuerTooLong,
    /// An expiry before it's issued or takes effect.
    ExpiresBeforeIssued,
    /// A renewal of a document the vehicle doesn't have.
    RenewsUnknown,
    /// A renewal of another vehicle's document, or another type.
    RenewsOtherType,
    /// A renewal of a document something else already renews.
    AlreadyRenewed,
    /// A plate on a document that isn't a registration.
    PlateNotRegistration,
    /// A plate that isn't this vehicle's.
    UnknownPlate,
}

impl DocumentProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::UnknownType => messages::DOCUMENT_UNKNOWN_TYPE,
            Self::NumberTooLong => messages::DOCUMENT_NUMBER_TOO_LONG,
            Self::IssuerTooLong => messages::DOCUMENT_ISSUER_TOO_LONG,
            Self::ExpiresBeforeIssued => messages::DOCUMENT_EXPIRES_BEFORE_ISSUED,
            Self::RenewsUnknown => messages::DOCUMENT_RENEWS_UNKNOWN,
            Self::RenewsOtherType => messages::DOCUMENT_RENEWS_OTHER_TYPE,
            Self::AlreadyRenewed => messages::DOCUMENT_ALREADY_RENEWED,
            Self::PlateNotRegistration => messages::DOCUMENT_PLATE_NOT_REGISTRATION,
            Self::UnknownPlate => messages::DOCUMENT_UNKNOWN_PLATE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Patch;
    use crate::plate::tests::{date, plate};
    use crate::test_support::test_id;

    fn registration(expires: &str) -> AssetDocument {
        AssetDocument {
            asset: test_id(1),
            doc_type: DocumentType::listed(REGISTRATION).unwrap(),
            number: Some("R-55120".into()),
            issuer: Some("NC DMV".into()),
            issued_on: Some(date("2025-11-01")),
            effective_on: None,
            expires_on: Some(date(expires)),
            supersedes: None,
            plate: Some(test_id(10)),
            notes: Notes::default(),
            added_at: UnixMillis::from_millis(0),
            added_by: test_id(5),
            received_at: UnixMillis::from_millis(0),
        }
    }

    fn plates() -> Vec<(LicensePlateId, LicensePlate)> {
        vec![(test_id(10), plate("NCX 4471", "2024-06-01"))]
    }

    #[test]
    fn types_are_profile_keys_or_named() {
        assert!(DocumentType::listed("document-insurance").is_ok());
        assert_eq!(
            DocumentType::listed("insurance"),
            Err(DocumentProblem::UnknownType)
        );
        assert_eq!(
            DocumentType::listed("document-"),
            Err(DocumentProblem::UnknownType)
        );
        assert!(
            !DocumentType::listed("document-title")
                .unwrap()
                .is_registration()
        );
        let custom = DocumentType::Custom {
            name: Label::parse("Parking permit").unwrap(),
        };
        assert!(!custom.is_registration());
    }

    #[test]
    fn status_comes_from_renewals_and_the_date() {
        let old = registration("2026-10-31");
        let documents = vec![(test_id(20), old.clone())];
        // Good through its expiry day.
        assert_eq!(
            status(test_id(20), &old, &documents, &date("2026-10-31")),
            DocumentStatus::Current
        );
        assert_eq!(
            status(test_id(20), &old, &documents, &date("2026-11-01")),
            DocumentStatus::Expired
        );
        let renewed = renewal_of(test_id(20), &old, UnixMillis::from_millis(9), test_id(5));
        let documents = vec![(test_id(20), old.clone()), (test_id(21), renewed.clone())];
        assert_eq!(
            status(test_id(20), &old, &documents, &date("2026-11-01")),
            DocumentStatus::Superseded
        );
        assert_eq!(
            status(test_id(21), &renewed, &documents, &date("2026-11-01")),
            DocumentStatus::Current
        );
        let mut title = registration("2026-10-31");
        title.expires_on = None;
        assert_eq!(
            status(test_id(22), &title, &[], &date("2099-01-01")),
            DocumentStatus::Current
        );
    }

    #[test]
    fn a_renewal_starts_from_the_previous_document() {
        let old = registration("2026-10-31");
        let renewed = renewal_of(test_id(20), &old, UnixMillis::from_millis(9), test_id(6));
        assert_eq!(renewed.supersedes, Some(test_id(20)));
        assert_eq!(renewed.doc_type, old.doc_type);
        assert_eq!(renewed.number, old.number);
        assert_eq!(renewed.plate, old.plate);
        assert_eq!(renewed.expires_on, None);
        assert_eq!(renewed.added_by, test_id(6));
    }

    #[test]
    fn a_registration_sets_its_plates_expiry() {
        let current = registration("2027-10-31");
        let (plate, patch) = plate_follows(test_id(21), &current, &[], &plates()).unwrap();
        assert_eq!(plate, test_id(10));
        assert_eq!(patch.expires_on, Some(Clearable::Set(date("2027-10-31"))));
        assert_eq!(patch.changed_fields(), ["expires_on"]);

        // Already the same: nothing to change.
        let mut same = plates();
        same[0].1.expires_on = Some(date("2027-10-31"));
        assert!(plate_follows(test_id(21), &current, &[], &same).is_none());

        // A registration someone renewed no longer decides.
        let renewal = renewal_of(
            test_id(21),
            &current,
            UnixMillis::from_millis(9),
            test_id(5),
        );
        let documents = vec![(test_id(22), renewal)];
        assert!(plate_follows(test_id(21), &current, &documents, &plates()).is_none());

        // Insurance doesn't touch the plate.
        let mut insurance = registration("2027-10-31");
        insurance.doc_type = DocumentType::listed("document-insurance").unwrap();
        assert!(plate_follows(test_id(23), &insurance, &[], &plates()).is_none());
    }

    #[test]
    fn documents_are_checked() {
        let ok = registration("2026-10-31");
        assert_eq!(check(test_id(20), &ok, &[], &plates()), Ok(()));

        let mut long = ok.clone();
        long.number = Some("9".repeat(61));
        assert_eq!(
            check(test_id(20), &long, &[], &plates()),
            Err(DocumentProblem::NumberTooLong)
        );
        let mut backwards = ok.clone();
        backwards.expires_on = Some(date("2025-10-31"));
        assert_eq!(
            check(test_id(20), &backwards, &[], &plates()),
            Err(DocumentProblem::ExpiresBeforeIssued)
        );
        let mut insured = ok.clone();
        insured.doc_type = DocumentType::listed("document-insurance").unwrap();
        assert_eq!(
            check(test_id(20), &insured, &[], &plates()),
            Err(DocumentProblem::PlateNotRegistration)
        );
        assert_eq!(
            check(test_id(20), &ok, &[], &[]),
            Err(DocumentProblem::UnknownPlate)
        );
    }

    #[test]
    fn a_renewal_is_of_one_document_of_the_same_type_once() {
        let old = registration("2026-10-31");
        let renewal = renewal_of(test_id(20), &old, UnixMillis::from_millis(9), test_id(5));
        let documents = vec![(test_id(20), old.clone())];
        assert_eq!(check(test_id(21), &renewal, &documents, &plates()), Ok(()));
        assert_eq!(
            check(test_id(21), &renewal, &[], &plates()),
            Err(DocumentProblem::RenewsUnknown)
        );
        let mut other = old;
        other.doc_type = DocumentType::listed("document-insurance").unwrap();
        other.plate = None;
        assert_eq!(
            check(test_id(21), &renewal, &[(test_id(20), other)], &plates()),
            Err(DocumentProblem::RenewsOtherType)
        );
        let twice = vec![documents[0].clone(), (test_id(21), renewal.clone())];
        assert_eq!(
            check(test_id(22), &renewal, &twice, &plates()),
            Err(DocumentProblem::AlreadyRenewed)
        );
    }

    #[test]
    fn a_document_round_trips() {
        let document = registration("2026-10-31");
        let json = serde_json::to_string(&document).unwrap();
        assert_eq!(
            serde_json::from_str::<AssetDocument>(&json).unwrap(),
            document
        );
        assert_eq!(AssetDocumentPatch::all(document).changed_fields().len(), 13);
    }
}
