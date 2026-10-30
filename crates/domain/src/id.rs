//! Record IDs: `UUIDv7`, generated on the device that creates the record (decision #042).
//!
//! A `UUIDv7` starts with its creation time in milliseconds, so IDs sort in creation
//! order, which keeps SQLite inserts at the end of the table and makes IDs in logs line
//! up with time. The rest is random, so devices never pick the same ID, even offline.
//!
//! [`Id<T>`] is typed by what it identifies (`Id<Device>`, later `Id<Asset>`, ...), so
//! an asset ID can't be passed where an event ID is expected. `domain` never reads the
//! clock or a random source itself: callers pass in "now" and the random bytes.
//!
//! # Examples
//!
//! ```
//! use domain::id::DeviceId;
//! use domain::time::UnixMillis;
//!
//! let now = UnixMillis::from_millis(1_700_000_000_000);
//! let device = DeviceId::generate(now, [7; 10]).unwrap();
//!
//! assert_eq!(device.created_at(), now);
//! let text = device.to_string(); // e.g. "018bcfe5-6800-7707-8707-070707070707"
//! assert_eq!(text.parse::<DeviceId>(), Ok(device));
//! ```

use core::cmp::Ordering;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::marker::PhantomData;
use core::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::{Builder, Uuid, Version};

use crate::time::UnixMillis;

/// Random bytes in a `UUIDv7`: 74 random bits, supplied as 10 bytes (the builder keeps
/// the bits the version and variant don't use).
pub const RANDOM_BYTES: usize = 10;

/// The ID of a record of kind `T`. Always a `UUIDv7`.
pub struct Id<T> {
    uuid: Uuid,
    // `fn() -> T` keeps `Id<T>` `Send`, `Sync`, and `Copy` whatever `T` is.
    kind: PhantomData<fn() -> T>,
}

/// A device: a browser session, desktop app, or the server itself.
#[derive(Debug)]
pub enum Device {}

/// A master database (plan §6 "Detecting a reset or rolled-back server").
#[derive(Debug)]
pub enum Database {}

/// The ID of a device. Breaks ties between equal HLC stamps (`crate::hlc::Stamp`).
pub type DeviceId = Id<Device>;

/// The ID of a master database. A new one means the server was recreated or restored.
pub type DatabaseId = Id<Database>;

/// Why an ID couldn't be made or read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdError {
    /// The creation time is before 1970 or past `UUIDv7`'s 48-bit limit.
    TimeOutOfRange(UnixMillis),
    /// Not a UUID in the standard text form.
    Malformed,
    /// A valid UUID, but not version 7.
    NotVersion7,
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TimeOutOfRange(time) => {
                write!(
                    formatter,
                    "time {} ms can't be stored in a UUIDv7",
                    time.as_millis()
                )
            }
            Self::Malformed => formatter.write_str("not a UUID"),
            Self::NotVersion7 => formatter.write_str("not a UUIDv7"),
        }
    }
}

impl std::error::Error for IdError {}

/// The largest creation time a `UUIDv7` can hold: 2^48 − 1 ms after 1970.
const MAX_MILLIS: u64 = (1 << 48) - 1;

impl<T> Id<T> {
    /// Makes a new ID created at `now`, with `random` supplying its random part.
    ///
    /// # Errors
    ///
    /// [`IdError::TimeOutOfRange`] if `now` is before 1970 or past the 48-bit limit.
    pub fn generate(now: UnixMillis, random: [u8; RANDOM_BYTES]) -> Result<Self, IdError> {
        let millis = u64::try_from(now.as_millis())
            .ok()
            .filter(|&millis| millis <= MAX_MILLIS)
            .ok_or(IdError::TimeOutOfRange(now))?;
        Ok(Self::from_uuid(
            Builder::from_unix_timestamp_millis(millis, &random).into_uuid(),
        ))
    }

    /// Makes an ID from a creation time already known to be in range, keeping only its
    /// low 48 bits. Infallible, for test IDs (`crate::test_support::test_id`), whose time
    /// can't be out of range.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn from_millis_in_range(millis: u64, random: [u8; RANDOM_BYTES]) -> Self {
        Self::from_uuid(
            Builder::from_unix_timestamp_millis(millis & MAX_MILLIS, &random).into_uuid(),
        )
    }

    /// Reads an ID from its 16-byte form (e.g. a database `BLOB`).
    ///
    /// # Errors
    ///
    /// [`IdError::NotVersion7`] if the bytes aren't a `UUIDv7`.
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, IdError> {
        Self::checked(Uuid::from_bytes(bytes))
    }

    /// The 16-byte form, which sorts like the ID.
    #[must_use]
    pub const fn to_bytes(self) -> [u8; 16] {
        *self.uuid.as_bytes()
    }

    /// When the ID was made, to the millisecond (useful when debugging).
    #[must_use]
    pub fn created_at(self) -> UnixMillis {
        // The first 48 bits are the creation time in milliseconds.
        let [b0, b1, b2, b3, b4, b5, ..] = self.to_bytes();
        let millis = u64::from_be_bytes([0, 0, b0, b1, b2, b3, b4, b5]);
        // At most 48 bits, so it always fits in an i64.
        UnixMillis::from_millis(i64::try_from(millis).unwrap_or(i64::MAX))
    }

    /// The same ID as another kind. Only for code that genuinely re-labels an ID (e.g.
    /// storage reading a generic column); everything else keeps the type.
    #[must_use]
    pub const fn cast<U>(self) -> Id<U> {
        Id {
            uuid: self.uuid,
            kind: PhantomData,
        }
    }

    const fn from_uuid(uuid: Uuid) -> Self {
        Self {
            uuid,
            kind: PhantomData,
        }
    }

    fn checked(uuid: Uuid) -> Result<Self, IdError> {
        match uuid.get_version() {
            Some(Version::SortRand) => Ok(Self::from_uuid(uuid)),
            _ => Err(IdError::NotVersion7),
        }
    }
}

// Written by hand: deriving would require `T` itself to implement each trait, and the
// marker types (`Device`, `Database`, ...) are empty enums that never do.
impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.uuid.cmp(&other.uuid)
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.uuid.hash(state);
    }
}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Id({})", self.uuid)
    }
}

/// The standard hyphenated, lowercase text form.
impl<T> fmt::Display for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.uuid.hyphenated(), formatter)
    }
}

impl<T> FromStr for Id<T> {
    type Err = IdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::checked(Uuid::try_parse(text).map_err(|_| IdError::Malformed)?)
    }
}

/// Serialized as its text form, so commands and API responses stay readable.
impl<T> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, T> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Owned, not borrowed: not every input (e.g. JSON with escapes) can lend a `&str`.
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    #[test]
    fn generated_ids_are_version_7_and_remember_their_time() {
        let id = DeviceId::generate(NOW, [0xAB; RANDOM_BYTES]).unwrap();
        assert_eq!(
            Uuid::from_bytes(id.to_bytes()).get_version(),
            Some(Version::SortRand)
        );
        assert_eq!(id.created_at(), NOW);
    }

    #[test]
    fn ids_sort_by_creation_time() {
        let earlier = DeviceId::generate(NOW, [0xFF; RANDOM_BYTES]).unwrap();
        let later = DeviceId::generate(
            UnixMillis::from_millis(NOW.as_millis() + 1),
            [0; RANDOM_BYTES],
        )
        .unwrap();
        assert!(earlier < later);
        assert!(earlier.to_bytes() < later.to_bytes());
    }

    #[test]
    fn times_outside_48_bits_are_rejected() {
        let before_1970 = UnixMillis::from_millis(-1);
        assert_eq!(
            DeviceId::generate(before_1970, [0; RANDOM_BYTES]),
            Err(IdError::TimeOutOfRange(before_1970))
        );
        let too_late = UnixMillis::from_millis(1 << 48);
        assert!(DeviceId::generate(too_late, [0; RANDOM_BYTES]).is_err());
    }

    #[test]
    fn text_bytes_and_json_round_trip() {
        let id = DatabaseId::generate(NOW, [3; RANDOM_BYTES]).unwrap();
        assert_eq!(id.to_string().parse::<DatabaseId>(), Ok(id));
        assert_eq!(DatabaseId::from_bytes(id.to_bytes()), Ok(id));

        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{id}\""));
        assert_eq!(serde_json::from_str::<DatabaseId>(&json).unwrap(), id);
    }

    #[test]
    fn other_uuid_versions_and_garbage_are_refused() {
        // A version 4 (random) UUID.
        let v4 = "67e55044-10b1-426f-9247-bb680e5fe0c8";
        assert_eq!(v4.parse::<DeviceId>(), Err(IdError::NotVersion7));
        assert_eq!(
            DeviceId::from_bytes(*Uuid::parse_str(v4).unwrap().as_bytes()),
            Err(IdError::NotVersion7)
        );
        assert_eq!("not-a-uuid".parse::<DeviceId>(), Err(IdError::Malformed));
        assert!(serde_json::from_str::<DeviceId>("\"nope\"").is_err());
    }

    proptest! {
        #[test]
        fn any_generated_id_round_trips(millis in 0..=1_i64 << 47, random in any::<[u8; RANDOM_BYTES]>()) {
            let id = DeviceId::generate(UnixMillis::from_millis(millis), random).unwrap();
            prop_assert_eq!(id.created_at().as_millis(), millis);
            prop_assert_eq!(id.to_string().parse::<DeviceId>(), Ok(id));
            prop_assert_eq!(DeviceId::from_bytes(id.to_bytes()), Ok(id));
        }
    }
}
