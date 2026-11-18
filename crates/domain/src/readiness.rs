//! "Ready to use?" (Phase 5 milestone 6; decisions #051, #063, #086).
//!
//! Readiness is for one of the vehicle's usages ([`crate::usage`]). Each usage stands alone:
//! - **a requirement ticked for it** (or, ticking none, for the default usage): ✗ when
//!   overdue, ! when due soon or very soon, ✓ otherwise; one not on the schedule isn't listed;
//! - **a document type it needs:** ✗ when the current one has expired **or none is on file**,
//!   ! when it's due soon or very soon, ✓ otherwise;
//! - **each open or deferred High issue** (#063): ✗, for every usage.
//!
//! The answer is ✗ if anything is, else ! if anything is, else ✓. **Advisory:** it sums up
//! the records and certifies nothing.

use serde::{Deserialize, Serialize};

use crate::api::{DueEntry, DueSubject};
use crate::document::{AssetDocument, AssetDocumentId, DocumentStatus, DocumentType};
use crate::due::{Due, DueStep};
use crate::label::Label;
use crate::requirement::{Requirement, RequirementId};
use crate::squawk::SquawkId;
use crate::usage::{Usage, UsageId};

/// How one item, or the whole, stands. Ordered: ready < warning < not ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    /// ✓
    Ready,
    /// ! (something due soon)
    Warning,
    /// ✗
    NotReady,
}

impl Mark {
    /// A due step's mark: Overdue ✗; Due soon and Due very soon !; otherwise ✓.
    #[must_use]
    pub const fn of_step(step: DueStep) -> Self {
        match step {
            DueStep::Overdue => Self::NotReady,
            DueStep::DueSoon | DueStep::DueVerySoon => Self::Warning,
            DueStep::Ok | DueStep::ComingUp => Self::Ready,
        }
    }
}

/// What one item is about, and why it has its mark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "item", rename_all = "kebab-case")]
pub enum ItemReason {
    /// A requirement, and where it stands.
    Requirement {
        /// Which.
        id: RequirementId,
        /// Its name.
        name: Label,
        /// Where it stands.
        due: Due,
    },
    /// A document type the usage needs: the current one, and where it stands (`None`: it
    /// never expires).
    Document {
        /// The type.
        doc_type: DocumentType,
        /// The current one.
        id: AssetDocumentId,
        /// Where it stands, if it expires.
        due: Option<Due>,
    },
    /// A document type the usage needs, none on file.
    MissingDocument {
        /// The type.
        doc_type: DocumentType,
    },
    /// An open or deferred High issue.
    HighIssue {
        /// Which.
        id: SquawkId,
        /// What's wrong.
        description: String,
        /// Deferred at a job (it still blocks).
        deferred: bool,
    },
}

/// One item: its mark and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessItem {
    /// How it stands.
    pub mark: Mark,
    /// What it is, and why.
    pub reason: ItemReason,
}

/// Readiness for one usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Readiness {
    /// The usage.
    pub usage: UsageId,
    /// Its name.
    pub name: Label,
    /// The vehicle's default usage.
    pub default: bool,
    /// The answer: the worst item's mark (✓ with none).
    pub mark: Mark,
    /// Every item: not ready first, then warnings, then ready.
    pub items: Vec<ReadinessItem>,
}

/// An open or deferred High issue, as readiness needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighIssue {
    /// Which.
    pub id: SquawkId,
    /// What's wrong.
    pub description: String,
    /// Deferred at a job.
    pub deferred: bool,
}

/// What readiness reads about a vehicle.
#[derive(Debug, Clone, Copy)]
pub struct VehicleRecords<'a> {
    /// Its requirements.
    pub requirements: &'a [(RequirementId, Requirement)],
    /// What's due on it (milestone 2): its requirements' and documents' steps.
    pub due: &'a [DueEntry],
    /// Its documents.
    pub documents: &'a [(AssetDocumentId, AssetDocument)],
    /// Its open and deferred High issues.
    pub high: &'a [HighIssue],
    /// Today (`2026-11-18`), for documents' status.
    pub today: &'a crate::dates::CalendarDate,
}

/// Readiness for usage `id` (`usage`) of a vehicle with these `records`.
#[must_use]
pub fn readiness(id: UsageId, usage: &Usage, records: &VehicleRecords<'_>) -> Readiness {
    let mut items = Vec::new();
    for (requirement_id, requirement) in records.requirements {
        if !requirement.needed_for(id, usage.default) {
            continue;
        }
        let Some(due) = due_of_requirement(*requirement_id, records.due) else {
            continue;
        };
        items.push(ReadinessItem {
            mark: Mark::of_step(due.step),
            reason: ItemReason::Requirement {
                id: *requirement_id,
                name: requirement.name.clone(),
                due,
            },
        });
    }
    for doc_type in &usage.documents {
        items.push(document_item(doc_type, records));
    }
    for issue in records.high {
        items.push(ReadinessItem {
            mark: Mark::NotReady,
            reason: ItemReason::HighIssue {
                id: issue.id,
                description: issue.description.clone(),
                deferred: issue.deferred,
            },
        });
    }
    // Worst first; otherwise in the order found.
    items.sort_by_key(|item| core::cmp::Reverse(item.mark));
    Readiness {
        usage: id,
        name: usage.name.clone(),
        default: usage.default,
        mark: items
            .iter()
            .map(|item| item.mark)
            .max()
            .unwrap_or(Mark::Ready),
        items,
    }
}

fn due_of_requirement(id: RequirementId, due: &[DueEntry]) -> Option<Due> {
    due.iter().find_map(|entry| match &entry.subject {
        DueSubject::Requirement { id: of, .. } if *of == id => Some(entry.due.clone()),
        DueSubject::Requirement { .. } | DueSubject::Document { .. } => None,
    })
}

/// A needed document type's item: the current document of that type (the latest to expire,
/// among those not renewed), or none on file.
fn document_item(doc_type: &DocumentType, records: &VehicleRecords<'_>) -> ReadinessItem {
    let current = records
        .documents
        .iter()
        .filter(|(_, document)| &document.doc_type == doc_type)
        .filter(|(id, document)| {
            crate::document::status(*id, document, records.documents, records.today)
                != DocumentStatus::Superseded
        })
        // One that never expires beats any that does; else the latest expiry.
        .max_by_key(|(_, document)| {
            (
                document.expires_on.is_none(),
                document
                    .expires_on
                    .as_ref()
                    .map(|date| date.as_str().to_owned()),
            )
        });
    let Some((id, _)) = current else {
        return ReadinessItem {
            mark: Mark::NotReady,
            reason: ItemReason::MissingDocument {
                doc_type: doc_type.clone(),
            },
        };
    };
    let due = records.due.iter().find_map(|entry| match &entry.subject {
        DueSubject::Document { id: of, .. } if of == id => Some(entry.due.clone()),
        DueSubject::Document { .. } | DueSubject::Requirement { .. } => None,
    });
    ReadinessItem {
        mark: due
            .as_ref()
            .map_or(Mark::Ready, |due| Mark::of_step(due.step)),
        reason: ItemReason::Document {
            doc_type: doc_type.clone(),
            id: *id,
            due,
        },
    }
}

#[cfg(test)]
mod tests;
