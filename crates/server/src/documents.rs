//! Official documents and license plates on the server (milestone 8; plan §3 Official asset
//! documents; decision #066).
//!
//! Who may do what (choice 7): anyone who can see a vehicle sees its documents and plates; a
//! **Maintainer** (or higher, in that mode) adds, renews, changes, and removes them.
//!
//! - **Renewing** a document makes a new one that supersedes it; only the latest of a chain may
//!   be removed, so a renewal never points at nothing.
//! - **A registration sets its plate's expiry** whenever it's saved, unless something renews it.
//! - **Adding a plate retires the current one**, on the new one's issue date (or today).
//! - Removing a document removes its scans too.

use domain::access::{AssetAction, AssetScope, permits_on_asset};
use domain::api::{
    AssetsOutcome, DocumentInput, DocumentView, DocumentsOutcome, PlateInput, PlateView,
};
use domain::asset::AssetId;
use domain::attachment::AttachmentOwner;
use domain::command::{Change, Clearable};
use domain::dates::CalendarDate;
use domain::document::{
    AssetDocument, AssetDocumentId, AssetDocumentPatch, DocumentProblem, check as check_document,
    plate_follows, status,
};
use domain::log_area;
use domain::plate::{
    LicensePlate, LicensePlateId, LicensePlatePatch, PlateProblem, check as check_plate, current,
    retire_for,
};
use storage::attachments::AttachmentWriter;
use storage::documents::{AssetDocumentWriter, LicensePlateWriter};

use crate::assets::{AssetsContext, unavailable};

/// A vehicle request's refusal as a documents answer.
fn from_assets(outcome: &AssetsOutcome) -> DocumentsOutcome {
    match outcome {
        AssetsOutcome::NotFound => DocumentsOutcome::NotFound,
        AssetsOutcome::NotAllowed => DocumentsOutcome::NotAllowed,
        AssetsOutcome::Garage { .. }
        | AssetsOutcome::Asset { .. }
        | AssetsOutcome::Added { .. }
        | AssetsOutcome::MovePreview { .. }
        | AssetsOutcome::Done
        | AssetsOutcome::Invalid { .. }
        | AssetsOutcome::Unavailable => DocumentsOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> DocumentsOutcome {
    from_assets(&unavailable(what, error))
}

/// Turns `Result<DocumentsOutcome, DocumentsOutcome>` into the outcome either way.
fn either(result: Result<DocumentsOutcome, DocumentsOutcome>) -> DocumentsOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

const fn document_problem(problem: DocumentProblem) -> DocumentsOutcome {
    DocumentsOutcome::InvalidDocument { problem }
}

const fn plate_problem(problem: PlateProblem) -> DocumentsOutcome {
    DocumentsOutcome::InvalidPlate { problem }
}

type Documents = Vec<(AssetDocumentId, AssetDocument)>;
type Plates = Vec<(LicensePlateId, LicensePlate)>;

impl AssetsContext<'_> {
    fn may_manage_documents(&self, scope: AssetScope) -> bool {
        permits_on_asset(
            &self.user.grants,
            self.user.mode,
            AssetAction::ManageDocuments,
            scope,
        )
    }

    /// The vehicle's scope, if the person may manage its documents.
    async fn document_scope(&self, asset: AssetId) -> Result<AssetScope, DocumentsOutcome> {
        let scope = self
            .visible(asset)
            .await
            .map_err(|outcome| from_assets(&outcome))?;
        if self.may_manage_documents(scope) {
            Ok(scope)
        } else {
            Err(DocumentsOutcome::NotAllowed)
        }
    }

    /// The vehicle's documents and plates.
    async fn papers(&self, asset: AssetId) -> Result<(Documents, Plates), DocumentsOutcome> {
        let documents = self
            .database
            .documents_of(asset)
            .await
            .map_err(|error| failed("documents", &error))?;
        let plates = self
            .database
            .plates_of(asset)
            .await
            .map_err(|error| failed("plates", &error))?;
        Ok((documents, plates))
    }

    async fn save_document(&self, change: Change<AssetDocument>) -> Result<(), DocumentsOutcome> {
        self.save(&AssetDocumentWriter, change)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    async fn save_plate(&self, change: Change<LicensePlate>) -> Result<(), DocumentsOutcome> {
        self.save(&LicensePlateWriter, change)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    /// Checks document `id` among the vehicle's others, saves `change`, then brings its
    /// plate's expiry in line.
    async fn checked_document(
        &self,
        id: AssetDocumentId,
        document: &AssetDocument,
        change: Change<AssetDocument>,
    ) -> Result<(), DocumentsOutcome> {
        let (documents, plates) = self.papers(document.asset).await?;
        let others: Documents = documents
            .into_iter()
            .filter(|(other, _)| *other != id)
            .collect();
        check_document(id, document, &others, &plates).map_err(document_problem)?;
        self.save_document(change).await?;
        if let Some((plate, patch)) = plate_follows(id, document, &others, &plates) {
            self.save_plate(Change::Update { id: plate, patch }).await?;
        }
        Ok(())
    }
}

/// A change to an optional field, if it differs.
fn changed<T: Clone + PartialEq>(from: Option<&T>, to: Option<&T>) -> Option<Clearable<T>> {
    (from != to).then(|| Clearable::from_option(to.cloned()))
}

/// A document from what was typed (with what it renews, if anything).
fn document_from(
    context: &AssetsContext<'_>,
    input: DocumentInput,
    supersedes: Option<AssetDocumentId>,
) -> AssetDocument {
    let now = context.clock.now();
    AssetDocument {
        asset: input.asset,
        doc_type: input.doc_type,
        number: input.number.filter(|text| !text.trim().is_empty()),
        issuer: input.issuer.filter(|text| !text.trim().is_empty()),
        issued_on: input.issued_on,
        effective_on: input.effective_on,
        expires_on: input.expires_on,
        supersedes,
        plate: input.plate,
        notes: input.notes,
        added_at: now,
        added_by: context.user.user,
        received_at: now,
    }
}

/// A plate from what was typed.
fn plate_from(context: &AssetsContext<'_>, input: PlateInput) -> LicensePlate {
    let now = context.clock.now();
    LicensePlate {
        asset: input.asset,
        authority: input.authority,
        number: input.number,
        issued_on: input.issued_on,
        expires_on: input.expires_on,
        retired_on: input.retired_on,
        notes: input.notes,
        added_at: now,
        added_by: context.user.user,
        received_at: now,
    }
}

// --- Seeing ------------------------------------------------------------------------------

/// One vehicle's documents and plates, each document's status as of `today` (the person's
/// date).
pub async fn list(
    context: &AssetsContext<'_>,
    asset: AssetId,
    today: &CalendarDate,
) -> DocumentsOutcome {
    either(list_papers(context, asset, today).await)
}

async fn list_papers(
    context: &AssetsContext<'_>,
    asset: AssetId,
    today: &CalendarDate,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    let scope = context
        .visible(asset)
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    let vehicle = context
        .database
        .asset(asset)
        .await
        .map_err(|error| failed("vehicle", &error))?
        .ok_or(DocumentsOutcome::NotFound)?;
    let (documents, plates) = context.papers(asset).await?;
    let current_id = current(&plates).map(|(id, _)| *id);
    let mut plate_views: Vec<PlateView> = plates
        .into_iter()
        .map(|(id, plate)| PlateView {
            id,
            plate,
            current: Some(id) == current_id,
        })
        .collect();
    plate_views.sort_by_key(|view| view.current);
    let document_views = documents
        .iter()
        .map(|(id, document)| DocumentView {
            id: *id,
            document: document.clone(),
            status: status(*id, document, &documents, today),
            renewed_by: documents
                .iter()
                .find(|(_, other)| other.supersedes == Some(*id))
                .map(|(other, _)| *other),
        })
        .collect();
    Ok(DocumentsOutcome::Documents {
        documents: document_views,
        plates: plate_views,
        listed_types: domain::profile::profile(&vehicle.kind)
            .map(|profile| profile.documents.clone())
            .unwrap_or_default(),
        may_change: context.may_manage_documents(scope),
    })
}

// --- Documents ---------------------------------------------------------------------------

/// Adds a document.
pub async fn add_document(context: &AssetsContext<'_>, input: DocumentInput) -> DocumentsOutcome {
    either(add_new_document(context, input, None).await)
}

/// Renews document `previous`: a new document of the same type that supersedes it.
pub async fn renew(
    context: &AssetsContext<'_>,
    previous: AssetDocumentId,
    input: DocumentInput,
) -> DocumentsOutcome {
    either(add_new_document(context, input, Some(previous)).await)
}

async fn add_new_document(
    context: &AssetsContext<'_>,
    input: DocumentInput,
    supersedes: Option<AssetDocumentId>,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    context.document_scope(input.asset).await?;
    let document = document_from(context, input, supersedes);
    let id: AssetDocumentId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    let change = Change::Create {
        id,
        values: AssetDocumentPatch::all(document.clone()),
    };
    context.checked_document(id, &document, change).await?;
    tracing::debug!(target: log_area::APP, document = %id, asset = %document.asset, renews = ?supersedes, "document added");
    Ok(DocumentsOutcome::SavedDocument { id })
}

/// Changes a document (its vehicle and what it renews stay as they were).
pub async fn edit_document(
    context: &AssetsContext<'_>,
    id: AssetDocumentId,
    input: DocumentInput,
) -> DocumentsOutcome {
    either(edit_existing_document(context, id, input).await)
}

/// The change from `current` to `next`: only what differs.
fn document_changes(current: &AssetDocument, next: &AssetDocument) -> AssetDocumentPatch {
    AssetDocumentPatch {
        doc_type: (current.doc_type != next.doc_type).then(|| next.doc_type.clone()),
        number: changed(current.number.as_ref(), next.number.as_ref()),
        issuer: changed(current.issuer.as_ref(), next.issuer.as_ref()),
        issued_on: changed(current.issued_on.as_ref(), next.issued_on.as_ref()),
        effective_on: changed(current.effective_on.as_ref(), next.effective_on.as_ref()),
        expires_on: changed(current.expires_on.as_ref(), next.expires_on.as_ref()),
        plate: changed(current.plate.as_ref(), next.plate.as_ref()),
        notes: (current.notes != next.notes).then(|| next.notes.clone()),
        ..AssetDocumentPatch::default()
    }
}

async fn edit_existing_document(
    context: &AssetsContext<'_>,
    id: AssetDocumentId,
    input: DocumentInput,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    let current = context
        .database
        .document(id)
        .await
        .map_err(|error| failed("document", &error))?
        .ok_or(DocumentsOutcome::NotFound)?;
    context.document_scope(current.asset).await?;
    let next = AssetDocument {
        asset: current.asset,
        supersedes: current.supersedes,
        added_at: current.added_at,
        added_by: current.added_by,
        received_at: current.received_at,
        ..document_from(context, input, current.supersedes)
    };
    let patch = document_changes(&current, &next);
    context
        .checked_document(id, &next, Change::Update { id, patch })
        .await?;
    Ok(DocumentsOutcome::SavedDocument { id })
}

/// Removes a document (and its scans), if nothing renews it.
pub async fn remove_document(context: &AssetsContext<'_>, id: AssetDocumentId) -> DocumentsOutcome {
    either(remove_existing_document(context, id).await)
}

async fn remove_existing_document(
    context: &AssetsContext<'_>,
    id: AssetDocumentId,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    let current = context
        .database
        .document(id)
        .await
        .map_err(|error| failed("document", &error))?
        .ok_or(DocumentsOutcome::NotFound)?;
    context.document_scope(current.asset).await?;
    let (documents, _) = context.papers(current.asset).await?;
    if documents
        .iter()
        .any(|(_, other)| other.supersedes == Some(id))
    {
        return Err(document_problem(DocumentProblem::Renewed));
    }
    let scans = context
        .database
        .attachments_on(current.asset, AttachmentOwner::Document { document: id })
        .await
        .map_err(|error| failed("scans", &error))?;
    for (scan, _) in scans {
        context
            .save(&AttachmentWriter, Change::Delete { id: scan })
            .await
            .map_err(|outcome| from_assets(&outcome))?;
    }
    context.save_document(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, document = %id, "document removed");
    Ok(DocumentsOutcome::Done)
}

// --- Plates ------------------------------------------------------------------------------

/// Adds a plate, retiring the vehicle's current one on the new one's issue date (or `today`).
pub async fn add_plate(
    context: &AssetsContext<'_>,
    input: PlateInput,
    today: &CalendarDate,
) -> DocumentsOutcome {
    either(add_new_plate(context, input, today).await)
}

async fn add_new_plate(
    context: &AssetsContext<'_>,
    input: PlateInput,
    today: &CalendarDate,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    context.document_scope(input.asset).await?;
    let plate = plate_from(context, input);
    check_plate(&plate, true).map_err(plate_problem)?;
    let (_, plates) = context.papers(plate.asset).await?;
    let retiring = if plate.retired_on.is_none() {
        retire_for(&plates, &plate, today)
    } else {
        None
    };
    let id: LicensePlateId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save_plate(Change::Create {
            id,
            values: LicensePlatePatch::all(plate.clone()),
        })
        .await?;
    if let Some((old, patch)) = retiring {
        context
            .save_plate(Change::Update { id: old, patch })
            .await?;
    }
    tracing::debug!(target: log_area::APP, plate = %id, asset = %plate.asset, "plate added");
    Ok(DocumentsOutcome::SavedPlate { id })
}

/// Changes a plate. One that has an authority must keep one.
pub async fn edit_plate(
    context: &AssetsContext<'_>,
    id: LicensePlateId,
    input: PlateInput,
) -> DocumentsOutcome {
    either(edit_existing_plate(context, id, input).await)
}

async fn edit_existing_plate(
    context: &AssetsContext<'_>,
    id: LicensePlateId,
    input: PlateInput,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    let current = context
        .database
        .plate(id)
        .await
        .map_err(|error| failed("plate", &error))?
        .ok_or(DocumentsOutcome::NotFound)?;
    context.document_scope(current.asset).await?;
    let next = LicensePlate {
        asset: current.asset,
        added_at: current.added_at,
        added_by: current.added_by,
        received_at: current.received_at,
        ..plate_from(context, input)
    };
    check_plate(&next, current.authority.is_some()).map_err(plate_problem)?;
    let patch = LicensePlatePatch {
        authority: changed(current.authority.as_ref(), next.authority.as_ref()),
        number: (current.number != next.number).then(|| next.number.clone()),
        issued_on: changed(current.issued_on.as_ref(), next.issued_on.as_ref()),
        expires_on: changed(current.expires_on.as_ref(), next.expires_on.as_ref()),
        retired_on: changed(current.retired_on.as_ref(), next.retired_on.as_ref()),
        notes: (current.notes != next.notes).then(|| next.notes.clone()),
        ..LicensePlatePatch::default()
    };
    context.save_plate(Change::Update { id, patch }).await?;
    Ok(DocumentsOutcome::SavedPlate { id })
}

/// Removes a plate added by mistake, if no registration names it.
pub async fn remove_plate(context: &AssetsContext<'_>, id: LicensePlateId) -> DocumentsOutcome {
    either(remove_existing_plate(context, id).await)
}

async fn remove_existing_plate(
    context: &AssetsContext<'_>,
    id: LicensePlateId,
) -> Result<DocumentsOutcome, DocumentsOutcome> {
    let current = context
        .database
        .plate(id)
        .await
        .map_err(|error| failed("plate", &error))?
        .ok_or(DocumentsOutcome::NotFound)?;
    context.document_scope(current.asset).await?;
    let (documents, _) = context.papers(current.asset).await?;
    if documents
        .iter()
        .any(|(_, document)| document.plate == Some(id))
    {
        return Err(plate_problem(PlateProblem::InUse));
    }
    context.save_plate(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, plate = %id, "plate removed");
    Ok(DocumentsOutcome::Done)
}
