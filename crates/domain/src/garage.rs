//! Ordering vehicles in the Garage (decision #056).
//!
//! Within each group, vehicles are sorted by the person's [`GarageSort`]: newest activity,
//! name, or their own **custom order** ([`GarageOrder`], one list per person, changed with
//! ▲▼ while arranging). Groups themselves stay in name order.
//!
//! **Pinned** vehicles (decision #090) are each person's own, kept with their order: they come
//! first in their own group whatever the sort, and among themselves follow it ([`pinned_first`]).
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

/// One person's custom order of vehicles, and the ones they've pinned; its ID is the person's
/// user ID.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarageOrder {
    /// Vehicles in order. Ones not listed (e.g. added later) come after, newest first.
    pub order: Vec<AssetId>,
    /// The vehicles they've pinned (#090), in no particular order; a vehicle that's gone, or
    /// they no longer see, is ignored.
    #[serde(default)]
    pub pinned: Vec<AssetId>,
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
    /// See [`GarageOrder::pinned`] (the whole list is replaced).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pinned: Option<Vec<AssetId>>,
}

impl Patch for GarageOrderPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if self.order.is_some() {
            fields.push("order");
        }
        if self.pinned.is_some() {
            fields.push("pinned");
        }
        fields
    }

    fn keep_only(&mut self, fields: &[&str]) {
        if !fields.contains(&"order") {
            self.order = None;
        }
        if !fields.contains(&"pinned") {
            self.pinned = None;
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
        // The most urgent first; equally urgent ones by name.
        GarageSort::MostUrgent => assets.sort_by(|a, b| {
            b.due
                .cmp(&a.due)
                .then_with(|| {
                    a.asset
                        .name
                        .as_str()
                        .to_lowercase()
                        .cmp(&b.asset.name.as_str().to_lowercase())
                })
                .then(b.id.cmp(&a.id))
        }),
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

/// Moves the pinned vehicles in `assets` (one group's, already sorted) to the front, keeping
/// the sort among the pinned ones and among the rest (#090).
pub fn pinned_first(assets: &mut [AssetSummary], pinned: &[AssetId]) {
    // A stable sort, so each part keeps the order it had.
    assets.sort_by_key(|item| !pinned.contains(&item.id));
}

/// `pinned` with `asset` pinned (`pin`) or not, each vehicle listed once.
#[must_use]
pub fn with_pin(pinned: &[AssetId], asset: AssetId, pin: bool) -> Vec<AssetId> {
    let mut result: Vec<AssetId> = pinned.iter().copied().filter(|id| *id != asset).collect();
    if pin {
        result.push(asset);
    }
    result
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
            permissions: crate::access::Permissions::NONE,
            vehicle_type: None,
            standing: crate::lifecycle::Standing::default(),
            id: test_id(n),
            asset: Asset {
                vehicle_type: None,
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
                main_photo: None,
                in_service: None,
            },
            role: Role::Driver,
            plate: None,
            due: None,
            ready: None,
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
    fn most_urgent_comes_first_then_by_name() {
        let mut assets = three();
        assets[0].due = Some(crate::due::DueStep::ComingUp);
        assets[1].due = Some(crate::due::DueStep::Overdue);
        assets.push(summary(4, "Boat"));
        sort_assets(&mut assets, GarageSort::MostUrgent, &[]);
        assert_eq!(names(&assets), vec!["Truck", "civic", "Boat", "Generator"]);
        assert_eq!(GarageSort::default(), GarageSort::MostUrgent, "#040");
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
    fn an_order_patch_lists_its_fields() {
        let mut patch = GarageOrderPatch {
            order: Some(vec![test_id(1)]),
            pinned: None,
        };
        assert_eq!(patch.changed_fields(), vec!["order"]);
        patch.keep_only(&[]);
        assert!(patch.changed_fields().is_empty());
        let mut pins = GarageOrderPatch {
            order: Some(vec![]),
            pinned: Some(vec![test_id(2)]),
        };
        assert_eq!(pins.changed_fields(), vec!["order", "pinned"]);
        pins.keep_only(&["pinned"]);
        assert_eq!(pins.changed_fields(), vec!["pinned"]);
    }

    #[test]
    fn pinned_vehicles_come_first_in_every_sort() {
        let truck = test_id(2);
        let generator = test_id(3);
        for sort in [
            GarageSort::Name,
            GarageSort::NewestActivity,
            GarageSort::MostUrgent,
            GarageSort::Custom,
        ] {
            let mut assets = three();
            sort_assets(&mut assets, sort, &[test_id(1), test_id(2), test_id(3)]);
            let before = names(&assets).join(",");
            pinned_first(&mut assets, &[truck]);
            assert_eq!(names(&assets)[0], "Truck", "{sort:?}");
            // The others keep the sort's order.
            let rest: Vec<&str> = before.split(',').filter(|n| *n != "Truck").collect();
            assert_eq!(names(&assets)[1..], rest[..], "{sort:?}");
        }
        // Two pinned keep the sort among themselves: by name, Generator then Truck.
        let mut assets = three();
        sort_assets(&mut assets, GarageSort::Name, &[]);
        pinned_first(&mut assets, &[truck, generator]);
        assert_eq!(names(&assets), vec!["Generator", "Truck", "civic"]);
    }

    #[test]
    fn a_pin_that_is_gone_or_elsewhere_changes_nothing() {
        let mut assets = three();
        sort_assets(&mut assets, GarageSort::Name, &[]);
        // A vehicle removed, or in another group, is pinned: this group stays as sorted.
        pinned_first(&mut assets, &[test_id(77)]);
        assert_eq!(names(&assets), vec!["civic", "Generator", "Truck"]);
        let mut none: Vec<AssetSummary> = Vec::new();
        pinned_first(&mut none, &[test_id(1)]);
        assert!(none.is_empty());
    }

    #[test]
    fn pinning_lists_each_vehicle_once() {
        let [a, b] = [test_id(1), test_id(2)];
        assert_eq!(with_pin(&[], a, true), vec![a]);
        assert_eq!(with_pin(&[a], a, true), vec![a]);
        assert_eq!(with_pin(&[a, b], a, false), vec![b]);
        assert_eq!(with_pin(&[b], a, false), vec![b]);
    }
}
