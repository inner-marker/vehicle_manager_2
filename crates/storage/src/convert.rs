//! Conversions between domain values and their SQLite column forms.
//!
//! - **IDs** ([`Id<T>`]): 16-byte `BLOB`s, which sort like the IDs.
//! - **HLCs** ([`Hlc`]): `INTEGER`. SQLite integers are signed 64-bit, so a packed HLC
//!   must stay below 2^63 (physical time before about the year 6400); anything else is
//!   refused rather than wrapped.
//! - **Named values** (enums like a lifecycle status): `TEXT`, their JSON name (e.g.
//!   `in-service`), so the tables read the same as the API.
//! - **Lists and tagged values** (identifiers, a field's kind): `TEXT` holding JSON.

use domain::command::Clearable;
use domain::hlc::Hlc;
use domain::id::Id;
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::StorageError;

/// An ID as a column value.
#[must_use]
pub fn id_to_sql<T>(id: Id<T>) -> Vec<u8> {
    id.to_bytes().to_vec()
}

/// An ID read from a column.
///
/// # Errors
///
/// [`StorageError::InvalidValue`] if the bytes aren't a 16-byte `UUIDv7`.
pub fn id_from_sql<T>(bytes: &[u8], column: &'static str) -> Result<Id<T>, StorageError> {
    let array: [u8; 16] = bytes
        .try_into()
        .map_err(|_| StorageError::InvalidValue { column })?;
    Id::from_bytes(array).map_err(|_| StorageError::InvalidValue { column })
}

/// An HLC as a column value.
///
/// # Errors
///
/// [`StorageError::InvalidValue`] if the packed value doesn't fit a signed 64-bit integer.
pub fn hlc_to_sql(hlc: Hlc, column: &'static str) -> Result<i64, StorageError> {
    i64::try_from(hlc.packed()).map_err(|_| StorageError::InvalidValue { column })
}

/// An HLC read from a column.
///
/// # Errors
///
/// [`StorageError::InvalidValue`] if the value is negative.
pub fn hlc_from_sql(value: i64, column: &'static str) -> Result<Hlc, StorageError> {
    u64::try_from(value)
        .map(Hlc::from_packed)
        .map_err(|_| StorageError::InvalidValue { column })
}

/// A value's stored name: its JSON string (for enums that serialize to a string).
#[must_use]
pub fn name_to_sql<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

/// A stored name back to its value; `None` for a name this build doesn't know.
#[must_use]
pub fn name_from_sql<T: DeserializeOwned>(name: Option<String>) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(name?)).ok()
}

/// A stored name that must be known, for a column the record can't do without.
///
/// # Errors
///
/// [`StorageError::InvalidValue`] for a name this build doesn't know.
pub fn required_name<T: DeserializeOwned>(
    name: String,
    column: &'static str,
) -> Result<T, StorageError> {
    name_from_sql(Some(name)).ok_or(StorageError::InvalidValue { column })
}

/// A value as JSON text (lists and tagged values).
///
/// # Errors
///
/// [`StorageError::InvalidValue`] if it can't be written (never for the app's own types).
pub fn json_to_sql<T: Serialize>(value: &T, column: &'static str) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(|_| StorageError::InvalidValue { column })
}

/// JSON text back to its value.
///
/// # Errors
///
/// [`StorageError::InvalidValue`] if it isn't valid for the type.
pub fn json_from_sql<T: DeserializeOwned>(
    text: &str,
    column: &'static str,
) -> Result<T, StorageError> {
    serde_json::from_str(text).map_err(|_| StorageError::InvalidValue { column })
}

/// An optional column's change in an update: `(changed?, new value)`, bound into
/// `CASE WHEN ? THEN ? ELSE column END`. Clearing a column (setting NULL) is a real change,
/// which `coalesce` can't express.
pub(crate) fn clearable<T: Clone, U>(
    change: Option<&Clearable<T>>,
    convert: impl Fn(T) -> U,
) -> (bool, Option<U>) {
    match change {
        None => (false, None),
        Some(change) => (true, change.clone().into_option().map(convert)),
    }
}

/// A patch field that must be set when creating a record.
pub(crate) fn required<T>(value: Option<T>, column: &'static str) -> Result<T, StorageError> {
    value.ok_or(StorageError::InvalidValue { column })
}

#[cfg(test)]
mod tests {
    use domain::id::DeviceId;
    use domain::test_support::test_id;
    use domain::time::UnixMillis;

    use super::*;

    #[test]
    fn ids_round_trip_and_bad_bytes_are_refused() {
        let id: DeviceId = test_id(1);
        assert_eq!(
            id_from_sql::<domain::id::Device>(&id_to_sql(id), "device_id").unwrap(),
            id
        );
        assert!(id_from_sql::<domain::id::Device>(&[0; 15], "device_id").is_err());
        assert!(id_from_sql::<domain::id::Device>(&[0; 16], "device_id").is_err());
    }

    #[test]
    fn hlcs_round_trip_and_out_of_range_values_are_refused() {
        let hlc = Hlc::new(UnixMillis::from_millis(1_700_000_000_000), 3).unwrap();
        assert_eq!(
            hlc_from_sql(hlc_to_sql(hlc, "hlc").unwrap(), "hlc").unwrap(),
            hlc
        );
        assert!(hlc_from_sql(-1, "hlc").is_err());
        assert!(hlc_to_sql(Hlc::from_packed(u64::MAX), "hlc").is_err());
    }
}
