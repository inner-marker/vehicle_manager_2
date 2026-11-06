//! Reference documents on the server: recalls, bulletins, and schedules shared by the
//! household (milestone 9; decision #068).
//!
//! Who may do what (`domain::reference`):
//! - **Read** one, and its attachments: anyone who can see a vehicle whose requirement links
//!   it, or the household's Admin.
//! - **Add** one: a Maintainer of the vehicle it's added from (in that mode), which also gives
//!   that vehicle its requirement for it, in one go. Another vehicle links the same one
//!   through its schedule (a requirement naming it).
//! - **Change or remove** one, and add or remove its attachments: a Maintainer of every
//!   vehicle that links it, or the household's Admin in Admin mode, since a change shows on
//!   all of them. One is removed (with its attachments) only once no requirement links it.

use domain::access::{AssetAction, AssetScope, permits_on_asset};
use domain::api::{AssetsOutcome, ReferenceInput, ReferenceLink, ReferenceView, ReferencesOutcome};
use domain::command::{Change, Patch};
use domain::log_area;
use domain::reference::{
    ReferenceDocument, ReferenceDocumentId, ReferenceDocumentPatch, check, may_change, may_read,
};
use domain::requirement::{Requirement, RequirementId, RequirementPatch, check_compliance};
use storage::StorageError;
use storage::attachments::AttachmentWriter;
use storage::references::ReferenceDocumentWriter;
use storage::requirements::RequirementWriter;

use crate::assets::{AssetsContext, unavailable};

/// A vehicle request's refusal as a reference documents answer.
fn from_assets(outcome: &AssetsOutcome) -> ReferencesOutcome {
    match outcome {
        AssetsOutcome::NotFound => ReferencesOutcome::NotFound,
        AssetsOutcome::NotAllowed => ReferencesOutcome::NotAllowed,
        AssetsOutcome::Garage { .. }
        | AssetsOutcome::Asset { .. }
        | AssetsOutcome::Added { .. }
        | AssetsOutcome::MovePreview { .. }
        | AssetsOutcome::Done
        | AssetsOutcome::Invalid { .. }
        | AssetsOutcome::Unavailable => ReferencesOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> ReferencesOutcome {
    from_assets(&unavailable(what, error))
}

/// Turns `Result<ReferencesOutcome, ReferencesOutcome>` into the outcome either way.
fn either(result: Result<ReferencesOutcome, ReferencesOutcome>) -> ReferencesOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

/// The vehicles whose requirements link `id`, as the permission check sees them.
///
/// # Errors
///
/// The storage error if they can't be read.
pub(crate) async fn linked(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
) -> Result<Vec<AssetScope>, StorageError> {
    let mut scopes: Vec<AssetScope> = Vec::new();
    for (_, requirement) in context.database.requirements_linking(id).await? {
        if scopes.iter().any(|scope| scope.id == requirement.asset) {
            continue;
        }
        if let Some(scope) = context.database.asset_scope(requirement.asset).await? {
            scopes.push(scope);
        }
    }
    Ok(scopes)
}

/// Whether reference document `id` exists and the person may read it.
///
/// # Errors
///
/// The storage error if it can't be read.
pub(crate) async fn readable(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
) -> Result<bool, StorageError> {
    if context.database.reference_document(id).await?.is_none() {
        return Ok(false);
    }
    let scopes = linked(context, id).await?;
    Ok(may_read(&context.user.grants, context.user.mode, &scopes))
}

/// Whether the person may change reference document `id` (and its attachments).
///
/// # Errors
///
/// The storage error if it can't be read.
pub(crate) async fn changeable(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
) -> Result<bool, StorageError> {
    let scopes = linked(context, id).await?;
    Ok(may_change(&context.user.grants, context.user.mode, &scopes))
}

/// The person's view of `document`, if they may read it.
async fn view_of(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
    document: ReferenceDocument,
) -> Result<Option<ReferenceView>, ReferencesOutcome> {
    let scopes = linked(context, id)
        .await
        .map_err(|error| failed("linked vehicles", &error))?;
    let (grants, mode) = (&context.user.grants, context.user.mode);
    if !may_read(grants, mode, &scopes) {
        return Ok(None);
    }
    let mut vehicles = Vec::new();
    for scope in &scopes {
        if domain::access::role_on(grants, *scope).is_none() {
            continue;
        }
        let name = context
            .database
            .asset(scope.id)
            .await
            .map_err(|error| failed("asset", &error))?
            .map(|asset| asset.name.as_str().to_owned())
            .unwrap_or_default();
        vehicles.push((scope.id, name));
    }
    Ok(Some(ReferenceView {
        id,
        document,
        vehicles,
        may_change: may_change(grants, mode, &scopes),
    }))
}

/// `input`'s values, with who added `document` and when.
fn with_input(document: &ReferenceDocument, input: ReferenceInput) -> ReferenceDocument {
    ReferenceDocument {
        doc_type: input.doc_type,
        authority: input.authority.filter(|text| !text.trim().is_empty()),
        numbers: input.numbers,
        title: input.title.trim().to_owned(),
        description: input.description,
        remedy: input.remedy,
        issued_on: input.issued_on,
        effective_on: input.effective_on,
        revision: input.revision.filter(|text| !text.trim().is_empty()),
        supersedes: input.supersedes,
        applicability: input.applicability,
        url: input.url.filter(|text| !text.trim().is_empty()),
        added_at: document.added_at,
        added_by: document.added_by,
        received_at: document.received_at,
    }
}

/// Checks `document` (as `id`) against the household's others.
async fn checked(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
    document: &ReferenceDocument,
) -> Result<(), ReferencesOutcome> {
    let known: Vec<ReferenceDocumentId> = context
        .database
        .reference_documents()
        .await
        .map_err(|error| failed("reference documents", &error))?
        .into_iter()
        .map(|(known, _)| known)
        .collect();
    check(id, document, &known).map_err(|problem| ReferencesOutcome::Invalid { problem })
}

// --- Reading ------------------------------------------------------------------------------

/// The reference documents the person can read, by title (to link one to another vehicle).
pub async fn list(context: &AssetsContext<'_>) -> ReferencesOutcome {
    either(list_references(context).await)
}

async fn list_references(
    context: &AssetsContext<'_>,
) -> Result<ReferencesOutcome, ReferencesOutcome> {
    let mut references = Vec::new();
    for (id, document) in context
        .database
        .reference_documents()
        .await
        .map_err(|error| failed("reference documents", &error))?
    {
        references.extend(view_of(context, id, document).await?);
    }
    Ok(ReferencesOutcome::References { references })
}

/// One reference document.
pub async fn view(context: &AssetsContext<'_>, id: ReferenceDocumentId) -> ReferencesOutcome {
    either(view_reference(context, id).await)
}

async fn view_reference(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
) -> Result<ReferencesOutcome, ReferencesOutcome> {
    let document = context
        .database
        .reference_document(id)
        .await
        .map_err(|error| failed("reference document", &error))?
        .ok_or(ReferencesOutcome::NotFound)?;
    let view = view_of(context, id, document)
        .await?
        .ok_or(ReferencesOutcome::NotFound)?;
    Ok(ReferencesOutcome::Reference {
        view: Box::new(view),
    })
}

// --- Adding, changing, removing -----------------------------------------------------------

/// Adds a reference document, and its first vehicle's requirement for it (`link`).
pub async fn add(
    context: &AssetsContext<'_>,
    input: ReferenceInput,
    link: ReferenceLink,
) -> ReferencesOutcome {
    either(add_reference(context, input, link).await)
}

async fn add_reference(
    context: &AssetsContext<'_>,
    input: ReferenceInput,
    link: ReferenceLink,
) -> Result<ReferencesOutcome, ReferencesOutcome> {
    let scope = context
        .visible(link.asset)
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    if !permits_on_asset(
        &context.user.grants,
        context.user.mode,
        AssetAction::ConfigureMaintenance,
        scope,
    ) {
        return Err(ReferencesOutcome::NotAllowed);
    }
    if !crate::lifecycle::takes_new_records(context.database, link.asset)
        .await
        .map_err(|error| failed("lifecycle", &error))?
    {
        return Err(ReferencesOutcome::NotAllowed);
    }
    let now = context.clock.now();
    let id: ReferenceDocumentId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    let blank = ReferenceDocument {
        doc_type: input.doc_type.clone(),
        authority: None,
        numbers: Vec::new(),
        title: String::new(),
        description: domain::notes::Notes::default(),
        remedy: domain::notes::Notes::default(),
        issued_on: None,
        effective_on: None,
        revision: None,
        supersedes: None,
        applicability: domain::notes::Notes::default(),
        url: None,
        added_at: now,
        added_by: context.user.user,
        received_at: now,
    };
    let document = with_input(&blank, input);
    checked(context, id, &document).await?;
    let requirement = Requirement {
        asset: link.asset,
        name: link.name,
        notes: domain::notes::Notes::default(),
        enabled: true,
        rules: Vec::new(),
        reference: Some(id),
        mode: link.mode,
        applicability: link.applicability,
    };
    check_compliance(&requirement, &[id])
        .map_err(|problem| ReferencesOutcome::InvalidRequirement { problem })?;
    context
        .save(
            &ReferenceDocumentWriter,
            Change::Create {
                id,
                values: ReferenceDocumentPatch::all(document),
            },
        )
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    let requirement_id: RequirementId =
        context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save(
            &RequirementWriter,
            Change::Create {
                id: requirement_id,
                values: RequirementPatch::all(requirement),
            },
        )
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    tracing::debug!(target: log_area::APP, reference = %id, requirement = %requirement_id, asset = %link.asset, "reference document added");
    Ok(ReferencesOutcome::Saved {
        id,
        requirement: Some(requirement_id),
    })
}

/// The stored document, if the person may change it.
async fn changeable_document(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
) -> Result<ReferenceDocument, ReferencesOutcome> {
    let current = context
        .database
        .reference_document(id)
        .await
        .map_err(|error| failed("reference document", &error))?
        .ok_or(ReferencesOutcome::NotFound)?;
    if !readable(context, id)
        .await
        .map_err(|error| failed("linked vehicles", &error))?
    {
        return Err(ReferencesOutcome::NotFound);
    }
    if !changeable(context, id)
        .await
        .map_err(|error| failed("linked vehicles", &error))?
    {
        return Err(ReferencesOutcome::NotAllowed);
    }
    Ok(current)
}

/// Changes a reference document (who added it and when stay).
pub async fn edit(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
    input: ReferenceInput,
) -> ReferencesOutcome {
    either(edit_reference(context, id, input).await)
}

async fn edit_reference(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
    input: ReferenceInput,
) -> Result<ReferencesOutcome, ReferencesOutcome> {
    let current = changeable_document(context, id).await?;
    let document = with_input(&current, input);
    checked(context, id, &document).await?;
    let mut patch = ReferenceDocumentPatch::all(document);
    // Only what differs, so the fields' clocks show what really changed.
    let differ = changed_fields(&current, &patch);
    patch.keep_only(&differ);
    if !differ.is_empty() {
        context
            .save(&ReferenceDocumentWriter, Change::Update { id, patch })
            .await
            .map_err(|outcome| from_assets(&outcome))?;
    }
    tracing::debug!(target: log_area::APP, reference = %id, "reference document changed");
    Ok(ReferencesOutcome::Saved {
        id,
        requirement: None,
    })
}

/// The fields of `patch` that differ from `current` (the ones to keep).
fn changed_fields(
    current: &ReferenceDocument,
    patch: &ReferenceDocumentPatch,
) -> Vec<&'static str> {
    use domain::command::Clearable;
    let was = ReferenceDocumentPatch::all(current.clone());
    let same = |a: &Option<Clearable<String>>, b: &Option<Clearable<String>>| a == b;
    let mut fields = Vec::new();
    let mut differs = |name: &'static str, differ: bool| {
        if differ {
            fields.push(name);
        }
    };
    differs("doc_type", was.doc_type != patch.doc_type);
    differs("authority", !same(&was.authority, &patch.authority));
    differs("numbers", was.numbers != patch.numbers);
    differs("title", was.title != patch.title);
    differs("description", was.description != patch.description);
    differs("remedy", was.remedy != patch.remedy);
    differs("issued_on", was.issued_on != patch.issued_on);
    differs("effective_on", was.effective_on != patch.effective_on);
    differs("revision", !same(&was.revision, &patch.revision));
    differs("supersedes", was.supersedes != patch.supersedes);
    differs("applicability", was.applicability != patch.applicability);
    differs("url", !same(&was.url, &patch.url));
    fields
}

/// Removes a reference document no requirement links, with its attachments.
pub async fn remove(context: &AssetsContext<'_>, id: ReferenceDocumentId) -> ReferencesOutcome {
    either(remove_reference(context, id).await)
}

async fn remove_reference(
    context: &AssetsContext<'_>,
    id: ReferenceDocumentId,
) -> Result<ReferencesOutcome, ReferencesOutcome> {
    changeable_document(context, id).await?;
    if !context
        .database
        .requirements_linking(id)
        .await
        .map_err(|error| failed("requirements", &error))?
        .is_empty()
    {
        return Err(ReferencesOutcome::InUse);
    }
    for (attachment, _) in context
        .database
        .attachments_of_reference(id)
        .await
        .map_err(|error| failed("attachments", &error))?
    {
        context
            .save(&AttachmentWriter, Change::Delete { id: attachment })
            .await
            .map_err(|outcome| from_assets(&outcome))?;
    }
    context
        .save(&ReferenceDocumentWriter, Change::Delete { id })
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    tracing::debug!(target: log_area::APP, reference = %id, "reference document removed");
    Ok(ReferencesOutcome::Done)
}
