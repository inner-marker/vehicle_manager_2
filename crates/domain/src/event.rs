//! Events: fuel-ups and meter readings (milestone 6), maintenance and repairs (milestone 7),
//! and lifecycle events (milestone 9, [`crate::lifecycle`]); plan §3 Events, decisions #006,
//! #008, #030, #033, #034, #043, #067.
//!
//! Every event shares one **spine**: its vehicle, kind, dates (UTC plus the place's zone),
//! location, readings, custom field values, notes, where it came from (its **source**: typed
//! now; voice and import later), who logged it, and who did it. A fuel-up adds its
//! [`FuelDetails`], a maintenance or repair its [`WorkDetails`], and a lifecycle event its
//! [`LifecycleDetails`].
//!
//! **Drafts and finalizing (#034):** an event is a draft until it's **finalized**: by hand
//! (**Finalize now**) or by the server 24 hours after its last edit. A finalized event never
//! changes; a mistake is fixed with a **correction**, a new event linked to the one it corrects
//! with a reason (plan §3; the human's choice, milestone 6). Who finalized it and when, and when
//! the server first received it, are kept for the signatures to come (sub-plan 01).
//!
//! The checks live in `crate::event_check`.

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Clearable, Record, record_patch};
use crate::custom_field::CustomFieldId;
use crate::dates::EventDates;
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;
use crate::lifecycle::{LifecycleDetails, LifecycleRecord};
use crate::location::{Coordinates, LocationSource, Place, SavedPlaceId};
use crate::meter::{EventKind, MeterId};
use crate::notes::Notes;
use crate::preferences::CurrencyCode;
use crate::time::UnixMillis;
use crate::user::UserId;

/// The ID of an event.
pub type EventId = Id<Event>;

/// The most characters a reason (for a skip, an override, or a correction) may have.
pub const REASON_MAX_CHARS: usize = 500;
/// The most characters a performer's name or contact may have.
pub const PERFORMER_MAX_CHARS: usize = 100;
/// How long a fuel-up or reading stays a draft after its last edit (#034).
pub const FINALIZE_AFTER_MILLIS: i64 = 24 * 60 * 60 * 1000;

/// A short text that explains a choice (a skipped reading, an override, a correction):
/// trimmed, 1–500 characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Reason(String);

impl Reason {
    /// A reason from what was typed.
    ///
    /// # Errors
    ///
    /// [`EventProblem::ReasonEmpty`] or [`EventProblem::ReasonTooLong`].
    pub fn parse(input: &str) -> Result<Self, EventProblem> {
        let reason = input.trim();
        match reason.chars().count() {
            0 => Err(EventProblem::ReasonEmpty),
            length if length > REASON_MAX_CHARS => Err(EventProblem::ReasonTooLong),
            _ => Ok(Self(reason.to_owned())),
        }
    }

    /// The reason.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Reason {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<Reason> for String {
    fn from(reason: Reason) -> Self {
        reason.0
    }
}

/// Where an event came from (idea 14's groundwork: no migration when voice arrives).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventSource {
    /// Typed in a form.
    #[default]
    Typed,
    /// Spoken (post-MVP).
    Voice,
    /// Imported from a file (later).
    Import,
}

/// Whether an event can still change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventStatus {
    /// Can be edited; finalizes by itself after 24 hours.
    #[default]
    Draft,
    /// Locked; only a correction can change what counts.
    Finalized,
}

/// Who finalized an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "by", content = "user", rename_all = "kebab-case")]
pub enum Finalizer {
    /// A person, with **Finalize now**.
    User(UserId),
    /// The server, 24 hours after the last edit.
    Server,
}

/// When and by whom an event was finalized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Finalized {
    /// When.
    pub at: UnixMillis,
    /// Who.
    pub by: Finalizer,
}

/// Who did the work: an app user, or a named person or business (plan §3 Performed by).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Performer {
    /// Someone in the household.
    User {
        /// Who.
        user: UserId,
    },
    /// Someone else, e.g. "Joe's Auto".
    Named {
        /// Their name (1–100 characters).
        name: String,
        /// How to reach them, if wanted (up to 100 characters).
        contact: Option<String>,
    },
}

/// Where an event happened (plan §3 Event location; #059). A saved place's values are copied
/// in, so editing it later never changes the event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventLocation {
    /// The city, state or province, and country; `None` only with an override reason.
    pub place: Option<Place>,
    /// The coordinates, when known.
    pub coordinates: Option<Coordinates>,
    /// The device's accuracy, in meters.
    pub accuracy_m: Option<u32>,
    /// When the device's fix was taken.
    pub fix_time: Option<UnixMillis>,
    /// Where the location came from.
    pub source: LocationSource,
    /// A business or place name, e.g. "Joe's Auto" (from a saved place, or typed).
    pub place_name: Option<Label>,
    /// The saved place it was copied from (a hint only).
    pub saved_place: Option<SavedPlaceId>,
    /// The place data version, when the place came from the geocoder.
    pub geocoder_version: Option<String>,
    /// Why there's no place at all (the guarded override; the event is flagged).
    pub missing_reason: Option<Reason>,
}

/// One meter's reading on an event, in the stored unit (meters or seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Reading {
    /// Which meter.
    pub meter: MeterId,
    /// Its value (raw, before any later meter adjustment).
    pub value: i64,
}

/// Something required that wasn't given.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum Requirement {
    /// A meter reading.
    Meter(MeterId),
    /// A custom field.
    Field(CustomFieldId),
}

/// A required reading or field left out, and why (plan §3; the event is flagged).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Skipped {
    /// What was skipped.
    pub item: Requirement,
    /// Why.
    pub reason: Reason,
}

/// A custom field's value, in its stored form (decision #030: whole numbers in fixed units).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum FieldData {
    /// Free text.
    Text(String),
    /// A number times 10^decimals.
    Number(i64),
    /// A date, `2026-11-02`.
    Date(String),
    /// Yes or no.
    YesNo(bool),
    /// One of the field's choices.
    Choice(Label),
    /// Meters.
    Distance(i64),
    /// Seconds.
    Duration(i64),
    /// Milliliters.
    Volume(i64),
    /// Meters per liter.
    FuelEconomy(i64),
}

/// One custom field's value on an event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldValue {
    /// Which field.
    pub field: CustomFieldId,
    /// Its value.
    pub data: FieldData,
}

/// What a fuel-up adds (#030, #033).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FuelDetails {
    /// How much, in milliliters (more than zero).
    pub volume_ml: i64,
    /// What it cost, in the currency's minor unit, if known.
    pub cost_minor: Option<i64>,
    /// The cost's currency.
    pub currency: CurrencyCode,
    /// The tank was filled to full.
    pub filled_full: bool,
    /// An earlier fill-up wasn't logged.
    pub missed_previous: bool,
    /// Share of the driving on highways since the last fuel-up, 0–100 %.
    pub highway_pct: u8,
    /// Share spent towing, 0–100 %.
    pub towing_pct: u8,
    /// Share spent hauling a load, 0–100 %.
    pub hauling_pct: u8,
    /// The vehicles towed (never the vehicle itself).
    pub towed: Vec<AssetId>,
    /// The fuel economy the vehicle itself reported, in meters per liter.
    pub reported_economy: Option<i64>,
}

/// The most characters a part's description may have.
pub const PART_MAX_CHARS: usize = 100;
/// The most characters a part number may have.
pub const PART_NUMBER_MAX_CHARS: usize = 40;
/// How long a maintenance or repair draft can sit untouched before a reminder (#034).
pub const FORGOTTEN_AFTER_MILLIS: i64 = 14 * 24 * 60 * 60 * 1000;

/// Which of a maintenance event's reading sets: the start (the required one) or the end
/// (e.g. after a test drive).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReadingSet {
    /// At the start of the work.
    #[default]
    Start,
    /// At the end.
    End,
}

/// A part used on a task (#032, milestone 7 choice 3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Part {
    /// What it is, e.g. "Oil filter" (1–100 characters).
    pub description: String,
    /// Its part number, if known (up to 40 characters), e.g. "FL-500S".
    pub part_number: Option<String>,
    /// How many, in thousandths (5.5 quarts is 5,500), more than zero.
    pub quantity_milli: i64,
    /// The price of one, in the event's currency's minor unit, if known.
    pub unit_price_minor: Option<i64>,
}

impl Part {
    /// What these cost together (quantity times unit price), if the price is known.
    #[must_use]
    pub fn cost_minor(&self) -> Option<i64> {
        self.unit_price_minor
            .map(|price| (i128::from(price) * i128::from(self.quantity_milli) + 500) / 1000)
            .and_then(|cost| i64::try_from(cost).ok())
    }
}

/// One task in a maintenance or repair event (#032).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Task {
    /// What was done, e.g. "Engine oil & filter".
    pub title: Label,
    /// The requirement it does, if any (a repair task often fixes something no interval
    /// tracks).
    pub requirement: Option<crate::requirement::RequirementId>,
    /// Notes.
    pub notes: Notes,
    /// The parts used.
    pub parts: Vec<Part>,
    /// How long it took, in seconds (typed as HH:MM).
    pub labor_seconds: Option<i64>,
    /// What the labor cost, in the event's currency's minor unit.
    pub labor_cost_minor: Option<i64>,
    /// Which readings reset its requirement: the start (the default) or the end.
    pub resets_from: ReadingSet,
    /// For a fixed-date rule: the next due date from the paperwork (`2027-11-30`), or none.
    pub next_due: Option<String>,
}

impl Task {
    /// What the parts cost together (the known prices).
    #[must_use]
    pub fn parts_cost_minor(&self) -> i64 {
        self.parts.iter().filter_map(Part::cost_minor).sum()
    }
}

/// What a maintenance or repair event adds (#032; milestone 7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorkDetails {
    /// The tasks, in order.
    pub tasks: Vec<Task>,
    /// Readings at the end of the work (the start ones are the event's own readings).
    pub end_readings: Vec<Reading>,
    /// What this event does about the vehicle's outstanding squawks.
    pub dispositions: Vec<crate::squawk::SquawkDisposition>,
    /// The costs' currency.
    pub currency: CurrencyCode,
    /// A repair's complaint: what was wrong, as noticed.
    pub complaint: Option<Notes>,
    /// A repair's cause: the diagnosis.
    pub cause: Option<Notes>,
}

impl WorkDetails {
    /// The end reading for `meter`, if there's one.
    #[must_use]
    pub fn end_reading(&self, meter: MeterId) -> Option<i64> {
        self.end_readings
            .iter()
            .find(|reading| reading.meter == meter)
            .map(|reading| reading.value)
    }

    /// What the visit cost: every task's parts and labor (#032).
    #[must_use]
    pub fn total_cost_minor(&self) -> i64 {
        self.tasks
            .iter()
            .map(|task| task.parts_cost_minor() + task.labor_cost_minor.unwrap_or(0))
            .sum()
    }
}

/// One event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Which vehicle.
    pub asset: AssetId,
    /// A fuel-up or a reading (maintenance and repairs arrive in milestone 7).
    pub kind: EventKind,
    /// When, and the place's zone.
    pub dates: EventDates,
    /// Where.
    pub location: EventLocation,
    /// Meter readings.
    pub readings: Vec<Reading>,
    /// Required readings and fields left out, with reasons.
    pub skipped: Vec<Skipped>,
    /// Custom field values.
    pub fields: Vec<FieldValue>,
    /// Notes (a Markdown subset, #055).
    pub notes: Notes,
    /// Typed, voice, or import.
    pub source: EventSource,
    /// Who logged it (automatic).
    pub logged_by: UserId,
    /// Who did it, if recorded.
    pub performed_by: Option<Performer>,
    /// A fuel-up's details.
    pub fuel: Option<FuelDetails>,
    /// A maintenance or repair's work: tasks, end readings, squawk dispositions.
    pub work: Option<WorkDetails>,
    /// A lifecycle event's details (milestone 9, #067).
    #[serde(default)]
    pub lifecycle: Option<LifecycleDetails>,
    /// Why a reading out of order with its neighbours was kept (the Admin's override).
    pub reading_override: Option<Reason>,
    /// Draft or finalized.
    pub status: EventStatus,
    /// When it was last changed (the 24 hours count from here).
    pub last_edited_at: UnixMillis,
    /// When the server first received it.
    pub received_at: UnixMillis,
    /// When and by whom it was finalized.
    pub finalized: Option<Finalized>,
    /// The event this one corrects, if it's a correction.
    pub corrects: Option<EventId>,
    /// Why, for a correction.
    pub correction_reason: Option<Reason>,
}

/// An event as the app sends it: everything but what the server sets (who logged it, the
/// times, its status, and what it corrects).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventInput {
    /// See [`Event::asset`].
    pub asset: AssetId,
    /// See [`Event::kind`].
    pub kind: EventKind,
    /// See [`Event::dates`].
    pub dates: EventDates,
    /// See [`Event::location`].
    pub location: EventLocation,
    /// See [`Event::readings`].
    pub readings: Vec<Reading>,
    /// See [`Event::skipped`].
    pub skipped: Vec<Skipped>,
    /// See [`Event::fields`].
    pub fields: Vec<FieldValue>,
    /// See [`Event::notes`].
    pub notes: Notes,
    /// See [`Event::source`].
    pub source: EventSource,
    /// See [`Event::performed_by`].
    pub performed_by: Option<Performer>,
    /// See [`Event::fuel`].
    pub fuel: Option<FuelDetails>,
    /// See [`Event::work`].
    pub work: Option<WorkDetails>,
    /// See [`Event::lifecycle`].
    #[serde(default)]
    pub lifecycle: Option<LifecycleDetails>,
    /// See [`Event::reading_override`].
    pub reading_override: Option<Reason>,
}

impl EventInput {
    /// A new draft from this input, logged by `logged_by` at `now`.
    #[must_use]
    pub fn into_draft(self, logged_by: UserId, now: UnixMillis) -> Event {
        Event {
            asset: self.asset,
            kind: self.kind,
            dates: self.dates,
            location: self.location,
            readings: self.readings,
            skipped: self.skipped,
            fields: self.fields,
            notes: self.notes,
            source: self.source,
            logged_by,
            performed_by: self.performed_by,
            fuel: self.fuel,
            work: self.work,
            lifecycle: self.lifecycle,
            reading_override: self.reading_override,
            status: EventStatus::Draft,
            last_edited_at: now,
            received_at: now,
            finalized: None,
            corrects: None,
            correction_reason: None,
        }
    }

    /// `current` with this input's changeable parts, edited at `now` (its vehicle, kind,
    /// source, and what the server set are kept).
    #[must_use]
    pub fn edit(self, current: &Event, now: UnixMillis) -> Event {
        Event {
            dates: self.dates,
            location: self.location,
            readings: self.readings,
            skipped: self.skipped,
            fields: self.fields,
            notes: self.notes,
            performed_by: self.performed_by,
            fuel: self.fuel,
            work: self.work,
            lifecycle: self.lifecycle,
            reading_override: self.reading_override,
            last_edited_at: now,
            ..current.clone()
        }
    }
}

impl EventInput {
    /// The fields of `current` this input changes, and only those (an edit made offline,
    /// #076): the form started from `current`, so a field left as it was isn't sent, and a
    /// change made meanwhile on another device to that field stands (#043). When it was
    /// edited is set when the change is applied.
    #[must_use]
    pub fn changes_from(self, current: &Event) -> EventPatch {
        let edited = self.edit(current, current.last_edited_at);
        event_changes(current, &edited)
    }
}

/// The change from `before` to `after`: only the fields that differ.
#[must_use]
pub fn event_changes(before: &Event, after: &Event) -> EventPatch {
    fn changed<T: PartialEq + Clone>(before: &T, after: &T) -> Option<T> {
        (before != after).then(|| after.clone())
    }
    fn cleared<T: PartialEq + Clone>(
        before: Option<&T>,
        after: Option<&T>,
    ) -> Option<Clearable<T>> {
        (before != after).then(|| Clearable::from_option(after.cloned()))
    }
    EventPatch {
        asset: None,
        kind: None,
        dates: changed(&before.dates, &after.dates),
        location: changed(&before.location, &after.location),
        readings: changed(&before.readings, &after.readings),
        skipped: changed(&before.skipped, &after.skipped),
        fields: changed(&before.fields, &after.fields),
        notes: changed(&before.notes, &after.notes),
        source: None,
        logged_by: None,
        performed_by: cleared(before.performed_by.as_ref(), after.performed_by.as_ref()),
        fuel: cleared(before.fuel.as_ref(), after.fuel.as_ref()),
        work: cleared(before.work.as_ref(), after.work.as_ref()),
        lifecycle: cleared(before.lifecycle.as_ref(), after.lifecycle.as_ref()),
        reading_override: cleared(
            before.reading_override.as_ref(),
            after.reading_override.as_ref(),
        ),
        status: changed(&before.status, &after.status),
        last_edited_at: changed(&before.last_edited_at, &after.last_edited_at),
        received_at: None,
        finalized: cleared(before.finalized.as_ref(), after.finalized.as_ref()),
        corrects: None,
        correction_reason: None,
    }
}

/// Something worth marking on an event, so it's easy to find later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Flag {
    /// No place could be given.
    MissingLocation,
    /// A required reading or field was skipped.
    Skipped,
    /// A reading out of order was kept by the Admin.
    ReadingOverride,
    /// It's a correction of another event.
    Correction,
}

impl Flag {
    /// The flag's label.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::MissingLocation => messages::EVENT_FLAG_MISSING_LOCATION,
            Self::Skipped => messages::EVENT_FLAG_SKIPPED,
            Self::ReadingOverride => messages::EVENT_FLAG_READING_OVERRIDE,
            Self::Correction => messages::EVENT_FLAG_CORRECTION,
        }
    }
}

impl Event {
    /// What's worth marking on it.
    #[must_use]
    pub fn flags(&self) -> Vec<Flag> {
        [
            (Flag::MissingLocation, self.location.place.is_none()),
            (Flag::Skipped, !self.skipped.is_empty()),
            (Flag::ReadingOverride, self.reading_override.is_some()),
            (Flag::Correction, self.corrects.is_some()),
        ]
        .into_iter()
        .filter_map(|(flag, on)| on.then_some(flag))
        .collect()
    }

    /// Whether it's due to be finalized by the server at `now`: a draft, `delay` after its
    /// last edit (24 hours, or the test minutes, #054).
    #[must_use]
    pub const fn finalize_due(&self, now: UnixMillis, delay_millis: i64) -> bool {
        matches!(self.status, EventStatus::Draft)
            && self.kind.finalizes_by_itself()
            && now.as_millis() - self.last_edited_at.as_millis() >= delay_millis
    }

    /// Whether it's a forgotten job at `now` (#034): a maintenance or repair draft untouched
    /// for `delay` (14 days, or the test minutes). Only a reminder; nothing locks.
    #[must_use]
    pub const fn forgotten(&self, now: UnixMillis, delay_millis: i64) -> bool {
        matches!(self.status, EventStatus::Draft)
            && !self.kind.finalizes_by_itself()
            && now.as_millis() - self.last_edited_at.as_millis() >= delay_millis
    }

    /// The tasks it did for requirements, as "last done" counts them (7b), once it's finished:
    /// each with its event's time and the readings it resets from.
    #[must_use]
    pub fn done_tasks(&self, id: EventId) -> Vec<crate::requirement::DoneTask> {
        let Some(work) = self
            .work
            .as_ref()
            .filter(|_| self.status == EventStatus::Finalized)
        else {
            return Vec::new();
        };
        work.tasks
            .iter()
            .filter_map(|task| {
                Some(crate::requirement::DoneTask {
                    requirement: task.requirement?,
                    at: self.dates.start,
                    event: id,
                    readings: match task.resets_from {
                        ReadingSet::Start => self.readings.clone(),
                        ReadingSet::End => work.end_readings.clone(),
                    },
                    next_due: task.next_due.clone(),
                })
            })
            .collect()
    }

    /// It as a lifecycle record, if it's a lifecycle event.
    #[must_use]
    pub fn lifecycle_record(&self, id: EventId) -> Option<LifecycleRecord> {
        self.lifecycle.clone().map(|details| LifecycleRecord {
            id,
            at: self.dates.start,
            status: self.status,
            details,
        })
    }

    /// The reading for `meter`, if it has one.
    #[must_use]
    pub fn reading(&self, meter: MeterId) -> Option<i64> {
        self.readings
            .iter()
            .find(|reading| reading.meter == meter)
            .map(|reading| reading.value)
    }
}

/// Which side a reading that doesn't fit clashes on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClashSide {
    /// Lower than the reading before it.
    BelowEarlier,
    /// Higher than the reading after it.
    AboveLater,
}

/// The reading a new one doesn't fit with, so the form can say which ("lower than 52,310 mi on
/// 1 Mar").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadingClash {
    /// The meter.
    pub meter: MeterId,
    /// Below the one before, or above the one after.
    pub side: ClashSide,
    /// When the other reading was.
    pub at: UnixMillis,
    /// The other reading's value, in the meter's stored unit.
    pub value: i64,
}

/// Why an event (or a change to one) was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", content = "item", rename_all = "kebab-case")]
pub enum EventProblem {
    /// A reason with nothing in it.
    ReasonEmpty,
    /// A reason over 500 characters.
    ReasonTooLong,
    /// A performer's name or contact that's empty or too long.
    BadPerformer,
    /// No place, and no reason why.
    LocationRequired,
    /// A required reading or field missing, with no skip reason.
    Required(Requirement),
    /// A reading lower than the one before it, or higher than the one after (which one).
    ReadingOutOfOrder(ReadingClash),
    /// A reading for a meter that isn't this vehicle's.
    UnknownMeter(MeterId),
    /// A value for a field that isn't this vehicle's (or isn't on this kind of event, or is off).
    UnknownField(CustomFieldId),
    /// A value of the wrong type for its field, or a choice that isn't one of its options.
    WrongValue(CustomFieldId),
    /// A fuel-up without its details, or a reading with them.
    FuelDetails,
    /// A fuel-up of zero (or less).
    NoVolume,
    /// A negative cost, or a reported fuel economy of zero.
    BadAmount,
    /// A percentage over 100.
    BadPercent,
    /// A vehicle towing itself.
    TowsItself,
    /// A kind of event this vehicle doesn't have (e.g. a fuel-up for a trailer).
    WrongKind,
    /// **Finish** on a fuel-up or reading, or **Finalize now** on a maintenance or repair:
    /// jobs are finished (with their checks), the others finalized.
    NotAJob,
    /// A change to a finalized event.
    Finalized,
    /// A correction of an event that isn't finalized yet (edit the draft instead).
    NotFinalized,
    /// A correction of an event that was already corrected (correct the correction instead).
    AlreadyCorrected,
    /// A maintenance or repair without its work details, or another kind with them.
    WorkDetails,
    /// A complaint or cause on something other than a repair.
    RepairOnly,
    /// A task for a requirement that isn't this vehicle's.
    UnknownRequirement,
    /// A part with no description, too long a description or part number, or a quantity of
    /// zero or less.
    BadPart,
    /// A negative price, labor time, or labor cost.
    BadCost,
    /// A next due date on a task whose requirement has no fixed-date rule, or that isn't a
    /// date.
    BadNextDue,
    /// A task resetting from the end readings when there are none for its meters.
    NoEndReadings,
    /// An end reading lower than the start one (the meter in question).
    EndBelowStart(MeterId),
    /// A signed-off squawk naming a task that isn't in the list.
    UnknownTask,
    /// A squawk disposition problem (finishing with one left, one twice, one not outstanding).
    Squawk(crate::squawk::SquawkProblem),
    /// A lifecycle event without its details, or another kind with them (#067).
    LifecycleDetails,
    /// A work order, carrier, or tracking number over 100 characters.
    TextTooLong,
    /// A second acquisition (correct the first instead).
    AlreadyAcquired,
    /// A send-out while the vehicle is already out.
    AlreadyOut,
    /// A return when the vehicle isn't out, or naming another send-out.
    NotOut,
    /// A return dated before its send-out.
    ReturnBeforeSendOut,
    /// A second disposal.
    AlreadyDisposed,
    /// A withdrawn disposal that isn't a correction.
    WithdrawnNotCorrection,
    /// Something new for a vehicle whose disposal is finalized (corrections only, #067).
    VehicleDisposed,
}

impl EventProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::ReasonEmpty => messages::EVENT_REASON_EMPTY,
            Self::ReasonTooLong => messages::EVENT_REASON_TOO_LONG,
            Self::BadPerformer => messages::EVENT_BAD_PERFORMER,
            Self::LocationRequired => messages::EVENT_LOCATION_REQUIRED,
            Self::Required(_) => messages::EVENT_REQUIRED,
            Self::ReadingOutOfOrder(_) => messages::EVENT_READING_OUT_OF_ORDER,
            Self::UnknownMeter(_)
            | Self::UnknownField(_)
            | Self::FuelDetails
            | Self::WorkDetails
            | Self::RepairOnly
            | Self::UnknownRequirement
            | Self::UnknownTask
            | Self::LifecycleDetails
            | Self::WithdrawnNotCorrection => messages::SOMETHING_WENT_WRONG,
            Self::WrongValue(_) => messages::EVENT_WRONG_VALUE,
            Self::NoVolume => messages::EVENT_NO_VOLUME,
            Self::BadAmount | Self::BadCost => messages::EVENT_BAD_AMOUNT,
            Self::BadPercent => messages::EVENT_BAD_PERCENT,
            Self::TowsItself => messages::EVENT_TOWS_ITSELF,
            Self::WrongKind => messages::EVENT_WRONG_KIND,
            Self::NotAJob => messages::EVENT_FINISH_JOBS_ONLY,
            Self::Finalized => messages::EVENT_FINALIZED,
            Self::NotFinalized => messages::EVENT_NOT_FINALIZED,
            Self::AlreadyCorrected => messages::EVENT_ALREADY_CORRECTED,
            Self::BadPart => messages::EVENT_BAD_PART,
            Self::BadNextDue => messages::EVENT_BAD_NEXT_DUE,
            Self::NoEndReadings => messages::EVENT_NO_END_READINGS,
            Self::EndBelowStart(_) => messages::EVENT_END_BELOW_START,
            Self::Squawk(problem) => problem.message(),
            Self::TextTooLong => messages::EVENT_TEXT_TOO_LONG,
            Self::AlreadyAcquired => messages::EVENT_ALREADY_ACQUIRED,
            Self::AlreadyOut => messages::EVENT_ALREADY_OUT,
            Self::NotOut => messages::EVENT_NOT_OUT,
            Self::ReturnBeforeSendOut => messages::EVENT_RETURN_BEFORE_SEND_OUT,
            Self::AlreadyDisposed => messages::EVENT_ALREADY_DISPOSED,
            Self::VehicleDisposed => messages::EVENT_VEHICLE_DISPOSED,
        }
    }
}

record_patch! {
    /// The changeable fields of an [`Event`]; `Some` means "set to this".
    EventPatch
    {
    /// See [`Event::asset`] (set when created).
    asset: AssetId,
    /// See [`Event::kind`] (set when created).
    kind: EventKind,
    /// See [`Event::dates`].
    dates: EventDates,
    /// See [`Event::location`].
    location: EventLocation,
    /// See [`Event::readings`] (the whole list).
    readings: Vec<Reading>,
    /// See [`Event::skipped`] (the whole list).
    skipped: Vec<Skipped>,
    /// See [`Event::fields`] (the whole list).
    fields: Vec<FieldValue>,
    /// See [`Event::notes`].
    notes: Notes,
    /// See [`Event::source`] (set when created).
    source: EventSource,
    /// See [`Event::logged_by`] (set when created).
    logged_by: UserId,
    /// See [`Event::performed_by`].
    performed_by: Clearable<Performer>,
    /// See [`Event::fuel`].
    fuel: Clearable<FuelDetails>,
    /// See [`Event::work`].
    work: Clearable<WorkDetails>,
    /// See [`Event::lifecycle`].
    lifecycle: Clearable<LifecycleDetails>,
    /// See [`Event::reading_override`].
    reading_override: Clearable<Reason>,
    /// See [`Event::status`].
    status: EventStatus,
    /// See [`Event::last_edited_at`].
    last_edited_at: UnixMillis,
    /// See [`Event::received_at`] (set when created).
    received_at: UnixMillis,
    /// See [`Event::finalized`].
    finalized: Clearable<Finalized>,
    /// See [`Event::corrects`] (set when created).
    corrects: Clearable<EventId>,
    /// See [`Event::correction_reason`] (set when created).
    correction_reason: Clearable<Reason>,
    }
}

impl EventPatch {
    /// The fields a person edits in a draft (the log form's): a pushed edit keeps only these
    /// (`keep_only`), so it can never set what the server sets (status, who logged it, when it
    /// was finalized, what it corrects).
    pub const EDITABLE: [&'static str; 11] = [
        "dates",
        "location",
        "readings",
        "skipped",
        "fields",
        "notes",
        "performed_by",
        "fuel",
        "work",
        "lifecycle",
        "reading_override",
    ];

    /// Every field of `event`, for creating it.
    #[must_use]
    pub fn all(event: Event) -> Self {
        Self {
            asset: Some(event.asset),
            kind: Some(event.kind),
            dates: Some(event.dates),
            location: Some(event.location),
            readings: Some(event.readings),
            skipped: Some(event.skipped),
            fields: Some(event.fields),
            notes: Some(event.notes),
            source: Some(event.source),
            logged_by: Some(event.logged_by),
            performed_by: Some(Clearable::from_option(event.performed_by)),
            fuel: Some(Clearable::from_option(event.fuel)),
            work: Some(Clearable::from_option(event.work)),
            lifecycle: Some(Clearable::from_option(event.lifecycle)),
            reading_override: Some(Clearable::from_option(event.reading_override)),
            status: Some(event.status),
            last_edited_at: Some(event.last_edited_at),
            received_at: Some(event.received_at),
            finalized: Some(Clearable::from_option(event.finalized)),
            corrects: Some(Clearable::from_option(event.corrects)),
            correction_reason: Some(Clearable::from_option(event.correction_reason)),
        }
    }

    /// The event a create sets up, if every field it needs is set (the name of the first
    /// missing one otherwise).
    ///
    /// # Errors
    ///
    /// The missing field's name.
    pub fn into_event(self) -> Result<Event, &'static str> {
        Ok(Event {
            asset: self.asset.ok_or("asset")?,
            kind: self.kind.ok_or("kind")?,
            dates: self.dates.ok_or("dates")?,
            location: self.location.ok_or("location")?,
            readings: self.readings.unwrap_or_default(),
            skipped: self.skipped.unwrap_or_default(),
            fields: self.fields.unwrap_or_default(),
            notes: self.notes.unwrap_or_default(),
            source: self.source.unwrap_or_default(),
            logged_by: self.logged_by.ok_or("logged_by")?,
            performed_by: self.performed_by.and_then(Clearable::into_option),
            fuel: self.fuel.and_then(Clearable::into_option),
            work: self.work.and_then(Clearable::into_option),
            lifecycle: self.lifecycle.and_then(Clearable::into_option),
            reading_override: self.reading_override.and_then(Clearable::into_option),
            status: self.status.unwrap_or_default(),
            last_edited_at: self.last_edited_at.ok_or("last_edited_at")?,
            received_at: self.received_at.ok_or("received_at")?,
            finalized: self.finalized.and_then(Clearable::into_option),
            corrects: self.corrects.and_then(Clearable::into_option),
            correction_reason: self.correction_reason.and_then(Clearable::into_option),
        })
    }
}

/// Sets `target` to `value`, if there is one.
fn set<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

/// Sets or clears `target` as `value` says, if there is one.
fn clear<T>(target: &mut Option<T>, value: Option<Clearable<T>>) {
    if let Some(value) = value {
        *target = value.into_option();
    }
}

impl Event {
    /// Applies `patch`. What's fixed once an event exists (its vehicle, kind, source, who
    /// logged it, when it was received, and what it corrects) is ignored.
    pub fn apply(&mut self, patch: EventPatch) {
        let EventPatch {
            dates,
            location,
            readings,
            skipped,
            fields,
            notes,
            performed_by,
            fuel,
            work,
            lifecycle,
            reading_override,
            status,
            last_edited_at,
            finalized,
            ..
        } = patch;
        set(&mut self.dates, dates);
        set(&mut self.location, location);
        set(&mut self.readings, readings);
        set(&mut self.skipped, skipped);
        set(&mut self.fields, fields);
        set(&mut self.notes, notes);
        clear(&mut self.performed_by, performed_by);
        clear(&mut self.fuel, fuel);
        clear(&mut self.work, work);
        clear(&mut self.lifecycle, lifecycle);
        clear(&mut self.reading_override, reading_override);
        set(&mut self.status, status);
        set(&mut self.last_edited_at, last_edited_at);
        clear(&mut self.finalized, finalized);
    }
}

impl Record for Event {
    const ENTITY: &'static str = "event";
    type Patch = EventPatch;
}
