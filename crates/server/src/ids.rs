//! New record IDs on the server, with real randomness from the operating system.

use domain::id::{Id, IdError, RANDOM_BYTES};
use domain::time::UnixMillis;

/// Why a new ID couldn't be made.
#[derive(Debug)]
pub enum NewIdError {
    /// The operating system's random source failed (very unusual).
    Random(getrandom::Error),
    /// The clock is outside the range a `UUIDv7` can hold.
    Id(IdError),
}

impl core::fmt::Display for NewIdError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Random(error) => write!(formatter, "no random bytes from the OS: {error}"),
            Self::Id(error) => write!(formatter, "can't make an ID: {error}"),
        }
    }
}

impl std::error::Error for NewIdError {}

/// A new `UUIDv7` created at `now`.
///
/// # Errors
///
/// [`NewIdError`] if the OS has no random bytes, or `now` is out of range.
pub fn new_id<T>(now: UnixMillis) -> Result<Id<T>, NewIdError> {
    let mut random = [0; RANDOM_BYTES];
    getrandom::fill(&mut random).map_err(NewIdError::Random)?;
    Id::generate(now, random).map_err(NewIdError::Id)
}

#[cfg(test)]
mod tests {
    use domain::id::DeviceId;

    use super::*;

    #[test]
    fn new_ids_are_unique_and_carry_the_time() {
        let now = UnixMillis::from_millis(1_700_000_000_000);
        let first: DeviceId = new_id(now).unwrap();
        let second: DeviceId = new_id(now).unwrap();
        assert_ne!(first, second, "same millisecond, different random part");
        assert_eq!(first.created_at(), now);
    }
}
