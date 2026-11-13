//! Where a change comes from (phase-3.md 5c, decision #076): an **online** request, or an
//! **operation** a desktop made (offline, or online through its outbox) and pushed.
//!
//! Both run the same server function, with the same checks. Only what the server used to make
//! itself differs:
//!
//! | | Online | Pushed operation |
//! |---|---|---|
//! | New record IDs | random (`crate::ids::new_id`) | derived from the operation (`Operation::derived_id`) |
//! | Command IDs | random | derived from the operation |
//! | Command times | the server's next HLC | the operation's time and the ones after it |
//! | "Made at" (an event's last edit, a finish) | now | the operation's time |
//!
//! Derived IDs are the same on the desktop (applying the operation to its copy) and on the
//! server, so both make the same records, and a resent operation's commands are recognized by
//! `applied_commands` and skipped.

use std::sync::atomic::{AtomicU32, Ordering};

use domain::hlc::Hlc;
use domain::id::Id;
use domain::operation::{DeriveError, Operation};
use domain::time::UnixMillis;

use crate::ids::{NewIdError, new_id};

/// Command IDs are counted from here, apart from record IDs, so a server that takes a
/// different path than the desktop (one more record, one fewer) still gives the operation's
/// first records the same IDs as the desktop did.
const COMMAND_IDS_FROM: u32 = 1 << 20;

/// Where a change comes from.
#[derive(Debug)]
pub enum Origin<'a> {
    /// A request from the web or a desktop, made now.
    Online,
    /// A desktop's operation.
    Pushed(Pushed<'a>),
}

/// A desktop's operation being applied, and how many IDs and commands it has made so far.
#[derive(Debug)]
pub struct Pushed<'a> {
    operation: &'a Operation,
    ids: AtomicU32,
    commands: AtomicU32,
}

/// Why an ID couldn't be made.
#[derive(Debug)]
pub enum OriginError {
    /// No random bytes, or a clock out of range (online).
    NewId(NewIdError),
    /// The operation can't derive another (pushed).
    Derive(DeriveError),
}

impl core::fmt::Display for OriginError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NewId(error) => error.fmt(formatter),
            Self::Derive(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OriginError {}

impl<'a> Origin<'a> {
    /// A desktop's operation, before any of its IDs are made.
    #[must_use]
    pub const fn pushed(operation: &'a Operation) -> Self {
        Self::Pushed(Pushed {
            operation,
            ids: AtomicU32::new(0),
            commands: AtomicU32::new(0),
        })
    }

    /// The operation, if this is one.
    #[must_use]
    pub const fn operation(&self) -> Option<&'a Operation> {
        match self {
            Self::Online => None,
            Self::Pushed(pushed) => Some(pushed.operation),
        }
    }

    /// The next new record ID.
    ///
    /// # Errors
    ///
    /// [`OriginError`] if none can be made.
    pub fn new_id<T>(&self, now: UnixMillis) -> Result<Id<T>, OriginError> {
        match self {
            Self::Online => new_id(now).map_err(OriginError::NewId),
            Self::Pushed(pushed) => pushed
                .operation
                .derived_id(pushed.ids.fetch_add(1, Ordering::Relaxed))
                .map_err(OriginError::Derive),
        }
    }

    /// When the change was made: now online, the operation's time when pushed.
    #[must_use]
    pub fn made_at(&self, now: UnixMillis) -> UnixMillis {
        self.operation()
            .map_or(now, |operation| operation.hlc.physical())
    }

    /// The next command's ID and time, for a pushed operation; `None` online (the server
    /// makes both, `crate::commands::issue`).
    ///
    /// # Errors
    ///
    /// [`OriginError::Derive`] if the operation can't derive another.
    pub fn next_command<T>(&self) -> Result<Option<(Id<T>, Hlc)>, OriginError> {
        let Self::Pushed(pushed) = self else {
            return Ok(None);
        };
        let n = pushed.commands.fetch_add(1, Ordering::Relaxed);
        let id = pushed
            .operation
            .derived_id(COMMAND_IDS_FROM.saturating_add(n))
            .map_err(OriginError::Derive)?;
        let hlc = pushed
            .operation
            .derived_hlc(n)
            .map_err(OriginError::Derive)?;
        Ok(Some((id, hlc)))
    }
}

#[cfg(test)]
mod tests {
    use domain::operation::OperationChange;
    use domain::test_support::test_id;

    use super::*;

    fn operation() -> Operation {
        Operation::new(
            test_id(1),
            Hlc::new(UnixMillis::from_millis(1_700_000_000_000), 0).unwrap(),
            OperationChange::DeleteEvent { id: test_id(2) },
        )
    }

    #[test]
    fn a_pushed_operation_makes_the_same_ids_and_times_every_time() {
        let operation = operation();
        let first = Origin::pushed(&operation);
        let again = Origin::pushed(&operation);
        let now = UnixMillis::from_millis(1_800_000_000_000);
        for _ in 0..3 {
            let a: Id<()> = first.new_id(now).unwrap();
            let b: Id<()> = again.new_id(now).unwrap();
            assert_eq!(a, b);
            let (command_a, hlc_a) = first.next_command::<()>().unwrap().unwrap();
            let (command_b, hlc_b) = again.next_command::<()>().unwrap().unwrap();
            assert_eq!((command_a, hlc_a), (command_b, hlc_b));
            assert_ne!(command_a, a, "commands and records never share an ID");
        }
        assert_eq!(first.made_at(now), operation.hlc.physical());
    }

    #[test]
    fn online_the_server_makes_everything_now() {
        let now = UnixMillis::from_millis(1_800_000_000_000);
        let origin = Origin::Online;
        assert_eq!(origin.made_at(now), now);
        assert!(origin.next_command::<()>().unwrap().is_none());
        let a: Id<()> = origin.new_id(now).unwrap();
        let b: Id<()> = origin.new_id(now).unwrap();
        assert_ne!(a, b);
    }
}
