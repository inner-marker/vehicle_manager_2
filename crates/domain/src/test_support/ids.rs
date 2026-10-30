//! Predictable IDs for tests.

use crate::id::{Id, RANDOM_BYTES};

/// When test ID number 0 was "created": 2023-11-14T22:13:20Z.
const BASE_MILLIS: u64 = 1_700_000_000_000;

/// The `n`th test ID of any kind: created `n` ms after a fixed time, with `n` in its
/// random part. The same `n` always gives the same ID, and higher `n` sorts later.
#[must_use]
pub fn test_id<T>(n: u32) -> Id<T> {
    let mut random = [0; RANDOM_BYTES];
    random[..4].copy_from_slice(&n.to_be_bytes());
    // BASE_MILLIS + any u32 is far inside the 48-bit range, so nothing is masked off.
    Id::from_millis_in_range(BASE_MILLIS + u64::from(n), random)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::DeviceId;

    #[test]
    fn test_ids_are_repeatable_distinct_and_ordered() {
        let first: DeviceId = test_id(1);
        assert_eq!(first, test_id(1));
        assert_ne!(first, test_id::<crate::id::Device>(2));
        assert!(first < test_id(2));
    }
}
