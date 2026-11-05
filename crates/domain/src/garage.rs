//! Ordering vehicles in the Garage (decision #056).
//!
//! Within each group, vehicles are sorted by the person's [`GarageSort`]: newest activity,
//! name, or their own **custom order** ([`GarageOrder`], one list per person, changed with
//! ▲▼ while arranging). Groups themselves stay in name order.
//!
//! "Newest activity" means newest added until vehicles have events (milestone 6): an asset's
//! ID is a `UUIDv7`, so it carries its creation time.
//!
//! # Examples
//!
//! ```
//! use domain::garage::{Direction, move_within};
//! use domain::id::Id;
//! use domain::time::UnixMillis;
//!
//! let at = UnixMillis::from_millis(1_700_000_000_000);
//! let [a, b, c] = [1u8, 2, 3].map(|n| Id::generate(at, [n; 10]).unwrap());
//! // Moving c up past b, in a group showing a, b, c:
//! assert_eq!(move_within(&[a, b, c], &[a, b, c], c, Direction::Up), vec![a, c, b]);
//! ```

use serde::{Deserialize, Serialize};

use crate::api::AssetSummary;
use crate::asset::AssetId;
use crate::command::{Patch, Record};
use crate::id::Id;
use crate::preferences::GarageSort;

/// One person's custom order of vehicles; its ID is the person's user ID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarageOrder {
    /// Vehicles in order. Ones not listed (e.g. added later) come after, newest first.
    pub order: Vec<AssetId>,
}

/// The ID of a garage order record (the person's user ID, cast).
pub type GarageOrderId = Id<GarageOrder>;

/// The most vehicles a custom order lists (far more than a household has).
pub const MAX_ORDER_LENGTH: usize = 1_000;

/// The changeable fields of a [`GarageOrder`]; `Some` means "change this".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarageOrderPatch {
    /// See [`GarageOrder::order`] (the whole list is replaced).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<AssetId>>,
}

impl Patch for GarageOrderPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        if self.order.is_some() {
            vec!["order"]
        } else {
            Vec::new()
        }
    }

    fn keep_only(&mut self, fields: &[&str]) {
        if !fields.contains(&"order") {
            self.order = None;
        }
    }
}

impl Record for GarageOrder {
    const ENTITY: &'static str = "garage_order";
    type Patch = GarageOrderPatch;
}

/// Sorts `assets` (one group's, or any list) by `sort`, using `order` for a custom sort.
pub fn sort_assets(assets: &mut [AssetSummary], sort: GarageSort, order: &[AssetId]) {
    match sort {
        // Newest first: a later UUIDv7 was made later.
        GarageSort::NewestActivity => assets.sort_by_key(|item| core::cmp::Reverse(item.id)),
        GarageSort::Name => assets.sort_by(|a, b| {
            a.asset
                .name
                .as_str()
                .to_lowercase()
                .cmp(&b.asset.name.as_str().to_lowercase())
                .then(b.id.cmp(&a.id))
        }),
        GarageSort::Custom => assets.sort_by(|a, b| {
            // Listed vehicles by their place; unlisted ones after them, newest first.
            let place = |id: &AssetId| order.iter().position(|listed| listed == id);
            match (place(&a.id), place(&b.id)) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => core::cmp::Ordering::Less,
                (None, Some(_)) => core::cmp::Ordering::Greater,
                (None, None) => b.id.cmp(&a.id),
            }
        }),
    }
}

/// Which way a ▲▼ button moves a vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    /// ▲: one place earlier.
    Up,
    /// ▼: one place later.
    Down,
}

/// The new custom order after moving `asset` one place in `direction` within its group.
///
/// `shown` is the group's vehicles as they're shown now (in custom order), and `everything`
/// is every vehicle the person sees, in the order shown, so the result lists them all and the
/// other groups keep their places. At either end, nothing moves.
#[must_use]
pub fn move_within(
    everything: &[AssetId],
    shown: &[AssetId],
    asset: AssetId,
    direction: Direction,
) -> Vec<AssetId> {
    let mut result = everything.to_vec();
    let Some(index) = shown.iter().position(|id| *id == asset) else {
        return result;
    };
    let neighbour_index = match direction {
        Direction::Up => index.checked_sub(1),
        Direction::Down => Some(index + 1),
    };
    let Some(neighbour) = neighbour_index.and_then(|i| shown.get(i)) else {
        return result;
    };
    // Swap the two where they sit in the full list, so other groups keep their places.
    let from = result.iter().position(|id| *id == asset);
    let to = result.iter().position(|id| id == neighbour);
    if let (Some(from), Some(to)) = (from, to) {
        result.swap(from, to);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::Role;
    use crate::asset::{Asset, AssetKind, AssetName, LifecycleStatus};
    use crate::notes::Notes;
    use crate::test_support::test_id;

    fn summary(n: u32, name: &str) -> AssetSummary {
        AssetSummary {
            id: test_id(n),
            asset: Asset {
                kind: AssetKind::parse("car").unwrap(),
                name: AssetName::parse(name).unwrap(),
                make: None,
                model: None,
                year: None,
                colour: None,
                description: Notes::default(),
                group: test_id(50),
                parent: None,
                status: LifecycleStatus::InService,
                identifiers: vec![],
            },
            role: Role::Driver,
            plate: None,
        }
    }

    fn names(assets: &[AssetSummary]) -> Vec<&str> {
        assets.iter().map(|a| a.asset.name.as_str()).collect()
    }

    fn three() -> Vec<AssetSummary> {
        vec![
            summary(1, "civic"),
            summary(2, "Truck"),
            summary(3, "Generator"),
        ]
    }

    #[test]
    fn newest_activity_is_newest_added_for_now() {
        let mut assets = three();
        sort_assets(&mut assets, GarageSort::NewestActivity, &[]);
        assert_eq!(names(&assets), vec!["Generator", "Truck", "civic"]);
    }

    #[test]
    fn name_ignores_case() {
        let mut assets = three();
        sort_assets(&mut assets, GarageSort::Name, &[]);
        assert_eq!(names(&assets), vec!["civic", "Generator", "Truck"]);
    }

    #[test]
    fn custom_puts_unlisted_vehicles_last_newest_first() {
        let mut assets = three();
        assets.push(summary(4, "Boat"));
        sort_assets(&mut assets, GarageSort::Custom, &[test_id(2), test_id(1)]);
        assert_eq!(names(&assets), vec!["Truck", "civic", "Boat", "Generator"]);
    }

    #[test]
    fn moving_swaps_with_the_neighbour_in_the_same_group_only() {
        let [a, b, c, d] = [1, 2, 3, 4].map(test_id);
        // a and c are in one group, b and d in another; everything shows a, b, c, d.
        let everything = [a, b, c, d];
        assert_eq!(
            move_within(&everything, &[a, c], c, Direction::Up),
            vec![c, b, a, d]
        );
        assert_eq!(
            move_within(&everything, &[a, c], a, Direction::Up),
            everything.to_vec()
        );
        assert_eq!(
            move_within(&everything, &[b, d], d, Direction::Down),
            everything.to_vec()
        );
        assert_eq!(
            move_within(&everything, &[b, d], b, Direction::Down),
            vec![a, d, c, b]
        );
    }

    #[test]
    fn an_order_patch_lists_its_one_field() {
        let mut patch = GarageOrderPatch {
            order: Some(vec![test_id(1)]),
        };
        assert_eq!(patch.changed_fields(), vec!["order"]);
        patch.keep_only(&[]);
        assert!(patch.changed_fields().is_empty());
    }
}
