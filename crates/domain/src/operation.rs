//! Offline changes: **operations** (phase-3.md milestone 5, decision #076).
//!
//! An [`Operation`] is one change a person makes on the desktop: the same input a screen
//! sends the server online, plus what the server used to make itself, so it can be made
//! offline, kept in the desktop's **outbox**, and sent later:
//!
//! - its own **ID** ([`OperationId`]), so a resent operation is never applied twice;
//! - its **time** on the desktop's hybrid logical clock ([`Operation::hlc`]), the
//!   last-write-wins key for every field it sets (#043);
//! - the **IDs it makes** (a new event, squawk, place, and the commands that write them),
//!   derived from its own ID ([`Operation::derived_id`]), so the desktop's copy and the
//!   server end up with the same IDs.
//!
//! An **edit carries only the fields the person changed** (compared with the record the form
//! started from), never the whole form, so a field changed meanwhile on another device isn't
//! overwritten by one this person never touched (#043). Values are absolute, never deltas
//! (#005).
//!
//! The desktop applies an operation to its copy with the server's own write code, then
//! pushes it; the server runs it through the same code again, so every check runs twice
//! (#076). The device it came from is the pushing session's, never a value in the operation.
//!
//! Only the changes Driver mode allows are operations (#076): events, squawks, and the
//! person's own preferences, saved places, and garage order. Admin work needs the server.

use core::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::api::{
    AssetsOutcome, EventsOutcome, PlacesOutcome, PreferencesOutcome, SquawkInput, SquawksOutcome,
};
use crate::asset::AssetId;
use crate::event::{EventId, EventInput, EventPatch};
use crate::hlc::{Hlc, HlcError};
use crate::id::{Id, IdError, RANDOM_BYTES};
use crate::location::{SavedPlaceForm, SavedPlaceId, SavedPlacePatch};
use crate::preferences::PreferencesPatch;
use crate::squawk::{SquawkId, SquawkPatch};

/// The operation format this build writes. The server accepts it and the one before
/// ([`accepted_formats`]), upcasting the older (#072).
pub const OPERATION_FORMAT_VERSION: u32 = 1;

/// The operation formats a server running this build accepts: this one and the one before
/// (never below 1). An app further behind is told to update, and keeps its outbox.
#[must_use]
pub const fn accepted_formats() -> RangeInclusive<u32> {
    let oldest = if OPERATION_FORMAT_VERSION > 1 {
        OPERATION_FORMAT_VERSION - 1
    } else {
        1
    };
    oldest..=OPERATION_FORMAT_VERSION
}

/// Marker for operation IDs.
#[derive(Debug)]
pub enum OperationKind {}

/// An operation's ID: a `UUIDv7` made on the desktop when the person saves.
pub type OperationId = Id<OperationKind>;

/// One change made on the desktop, to be applied to its copy and sent to the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Operation {
    /// Unique per operation, so a resent one is recognized.
    pub id: OperationId,
    /// The format it was written in ([`OPERATION_FORMAT_VERSION`] when made).
    pub format_version: u32,
    /// When, on the desktop's hybrid logical clock: every field it sets carries this time.
    pub hlc: Hlc,
    /// What it does.
    pub change: OperationChange,
}

/// What an operation does: one per change Driver mode allows (#076).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum OperationChange {
    /// Logs an event, as a draft.
    AddEvent {
        /// The event, as the log form sends it.
        input: EventInput,
    },
    /// Changes a draft: only the fields the person changed ([`EventInput::changes_from`]).
    EditEvent {
        /// Which event.
        id: EventId,
        /// What changed.
        changes: EventPatch,
    },
    /// **Finalize now** (a fuel-up or reading).
    FinalizeEvent {
        /// Which event.
        id: EventId,
    },
    /// **Finish** (a maintenance or repair job).
    FinishEvent {
        /// Which event.
        id: EventId,
    },
    /// Deletes a draft.
    DeleteEvent {
        /// Which event.
        id: EventId,
    },
    /// Corrects a finalized event with a new draft.
    CorrectEvent {
        /// The event it corrects.
        original: EventId,
        /// The correction, as the log form sends it.
        input: EventInput,
        /// Why, as typed (checked when applied).
        reason: String,
    },
    /// Opens a squawk.
    OpenSquawk {
        /// The squawk, as the sheet sends it.
        input: SquawkInput,
    },
    /// Changes a squawk: only the fields the person changed ([`SquawkInput::changes_from`]).
    EditSquawk {
        /// Which squawk.
        id: SquawkId,
        /// What changed.
        changes: SquawkPatch,
    },
    /// Removes a squawk opened by mistake.
    DeleteSquawk {
        /// Which squawk.
        id: SquawkId,
    },
    /// Changes the person's own preferences (only the settings changed).
    SetMyPreferences {
        /// What changed.
        patch: PreferencesPatch,
    },
    /// Adds one of the person's saved places.
    AddPlace {
        /// The place, as typed.
        form: SavedPlaceForm,
    },
    /// Changes one of the person's saved places: only the fields the person changed
    /// ([`crate::location::SavedPlace::changes_to`]).
    EditPlace {
        /// Which place.
        id: SavedPlaceId,
        /// What changed.
        changes: SavedPlacePatch,
    },
    /// Removes one of the person's saved places.
    DeletePlace {
        /// Which place.
        id: SavedPlaceId,
    },
    /// Saves the person's Garage order (the whole list: it's one field).
    SetGarageOrder {
        /// The vehicles, in order.
        order: Vec<AssetId>,
    },
}

/// Why the server refused an operation: the answer its server function gave, the same one a
/// form shows online (so the desktop explains it the same way, sketch 2A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "refused-by", content = "answer", rename_all = "kebab-case")]
pub enum Refusal {
    /// An event's answer.
    Events(EventsOutcome),
    /// A squawk's answer.
    Squawks(SquawksOutcome),
    /// A saved place's answer.
    Places(PlacesOutcome),
    /// The preferences' answer.
    Preferences(PreferencesOutcome),
    /// The Garage order's answer.
    Assets(AssetsOutcome),
}

/// Why a derived ID or time couldn't be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeriveError {
    /// The operation's time can't hold another ID.
    Id(IdError),
    /// The operation's clock time can't count that far.
    Hlc(HlcError),
}

impl core::fmt::Display for DeriveError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Id(error) => write!(formatter, "can't derive an ID: {error}"),
            Self::Hlc(error) => write!(formatter, "can't derive a time: {error}"),
        }
    }
}

impl std::error::Error for DeriveError {}

impl Operation {
    /// A new operation in this build's format.
    #[must_use]
    pub const fn new(id: OperationId, hlc: Hlc, change: OperationChange) -> Self {
        Self {
            id,
            format_version: OPERATION_FORMAT_VERSION,
            hlc,
            change,
        }
    }

    /// The `n`th ID this operation makes (counting from 0, in the order its code asks for
    /// them): created when the operation was, with a random part taken from a hash of the
    /// operation's ID and `n`. The same on every device, and, since the operation's own ID is
    /// random, never one another operation makes.
    ///
    /// # Errors
    ///
    /// [`DeriveError::Id`] only if the operation's ID holds a time a `UUIDv7` can't (it never
    /// does: it is a `UUIDv7`).
    pub fn derived_id<T>(&self, n: u32) -> Result<Id<T>, DeriveError> {
        let mut hash = Sha256::new();
        hash.update(b"vehiclemanager operation id\0");
        hash.update(self.id.to_bytes());
        hash.update(n.to_be_bytes());
        let digest = hash.finalize();
        let mut random = [0; RANDOM_BYTES];
        random.copy_from_slice(&digest[..RANDOM_BYTES]);
        Id::generate(self.id.created_at(), random).map_err(DeriveError::Id)
    }

    /// The time for the `n`th command this operation writes (counting from 0): its own time,
    /// then the next ones after it, so two commands in one operation never tie.
    ///
    /// # Errors
    ///
    /// [`DeriveError::Hlc`] if counting on passes the clock's 48-bit limit.
    pub fn derived_hlc(&self, n: u32) -> Result<Hlc, DeriveError> {
        let mut hlc = self.hlc;
        for _ in 0..n {
            hlc = hlc.successor().map_err(DeriveError::Hlc)?;
        }
        Ok(hlc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Patch;
    use crate::event::Event;
    use crate::test_support::test_id;
    use crate::time::UnixMillis;

    fn operation(seed: u32) -> Operation {
        Operation::new(
            test_id(seed),
            Hlc::new(UnixMillis::from_millis(1_700_000_000_000), 3).unwrap(),
            OperationChange::DeleteEvent { id: test_id(99) },
        )
    }

    #[test]
    fn derived_ids_are_the_same_everywhere_and_never_repeat() {
        let first = operation(1);
        let again = operation(1);
        let other = operation(2);
        let ids: Vec<EventId> = (0..50).map(|n| first.derived_id(n).unwrap()).collect();
        for (n, id) in ids.iter().enumerate() {
            let n = u32::try_from(n).unwrap();
            assert_eq!(
                *id,
                again.derived_id(n).unwrap(),
                "the same on every device"
            );
            assert_eq!(id.created_at(), first.id.created_at());
            assert_ne!(*id, other.derived_id::<Event>(n).unwrap());
            assert_ne!(id.cast::<OperationKind>(), first.id);
            // A UUIDv7, so it round-trips through its stored form.
            assert_eq!(EventId::from_bytes(id.to_bytes()), Ok(*id));
        }
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn each_command_of_an_operation_has_its_own_later_time() {
        let operation = operation(1);
        assert_eq!(operation.derived_hlc(0), Ok(operation.hlc));
        let times: Vec<Hlc> = (0..5).map(|n| operation.derived_hlc(n).unwrap()).collect();
        assert!(times.windows(2).all(|pair| pair[0] < pair[1]));
    }

    /// A reading of `miles` on `meter`, as the log form sends it.
    fn reading(miles: i64) -> EventInput {
        use crate::dates::EventDates;
        use crate::event::{EventLocation, EventSource, Reading};
        use crate::location::LocationSource;
        use crate::location::TimeZone;
        use crate::meter::EventKind;
        use crate::notes::Notes;
        EventInput {
            asset: test_id(10),
            kind: EventKind::Reading,
            dates: EventDates {
                start: UnixMillis::from_millis(1_700_000_000_000),
                end: None,
                zone: TimeZone::utc(),
            },
            location: EventLocation {
                place: None,
                coordinates: None,
                accuracy_m: None,
                fix_time: None,
                source: LocationSource::Manual,
                place_name: None,
                saved_place: None,
                geocoder_version: None,
                missing_reason: None,
            },
            readings: vec![Reading {
                meter: test_id(11),
                value: miles * 1_609_344 / 1_000,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: None,
            work: None,
            lifecycle: None,
            reading_override: None,
        }
    }

    #[test]
    fn an_edit_made_offline_carries_only_what_the_person_changed() {
        let logged = reading(84_150).into_draft(test_id(20), UnixMillis::from_millis(1));
        // On the desktop: only the notes change.
        let mut typed = reading(84_150);
        typed.notes = crate::notes::Notes::parse("Checked at the pump").unwrap();
        let changes = typed.clone().changes_from(&logged);
        assert_eq!(changes.changed_fields(), vec!["notes"]);
        // Meanwhile, on the web, the reading changed; the edit leaves it as the web set it.
        let mut on_server = logged.clone();
        on_server.readings = reading(84_210).readings;
        on_server.apply(changes);
        assert_eq!(on_server.readings, reading(84_210).readings);
        assert_eq!(on_server.notes, typed.notes);
        // Nothing changed: nothing sent.
        assert!(
            reading(84_150)
                .changes_from(&logged)
                .changed_fields()
                .is_empty()
        );
    }

    #[test]
    fn a_server_accepts_this_format_and_the_one_before() {
        let formats = accepted_formats();
        assert!(formats.contains(&OPERATION_FORMAT_VERSION));
        assert!(*formats.start() >= 1);
        assert!(OPERATION_FORMAT_VERSION - *formats.start() <= 1);
        assert!(!formats.contains(&(OPERATION_FORMAT_VERSION + 1)));
    }

    #[test]
    fn an_operation_round_trips_through_its_stored_form() {
        let mut operation = operation(1);
        operation.change = OperationChange::SetGarageOrder {
            order: vec![test_id(5), test_id(6)],
        };
        let text = serde_json::to_string(&operation).unwrap();
        assert!(
            text.contains("\"operation\":\"set-garage-order\""),
            "{text}"
        );
        let back: Operation = serde_json::from_str(&text).unwrap();
        assert_eq!(back, operation);
    }
}
