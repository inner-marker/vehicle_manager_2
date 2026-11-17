//! Fuel economy (Phase 5 milestone 5; decisions #080, #085).
//!
//! - **A stretch** runs from one full fill to the next, on the vehicle's fuel odometer
//!   ([`fuel_meter`]). Its figure is that distance over all the fuel bought after the first
//!   fill, up to and including the last: partial fills add in and have no figure of their own.
//! - **Dropped:** a stretch with a **Missed previous fill-up** in it, a missing reading, or no
//!   distance counts in no figure ([`DropReason`]); the chart shows a gap.
//! - **Averages** are total distance over total fuel ([`Average`]): the last 5 stretches, the
//!   last 12 months, and all time. Never an average of figures, so a short stretch doesn't
//!   weigh as much as a long one.
//! - **Breakdowns:** a stretch's highway, towing, and hauling shares are its fills' weighted
//!   by fuel, in three bands each ([`Band`]); and by towed vehicle (a stretch counts for a
//!   vehicle if any of its fills names it).
//!
//! Economy is in meters per liter, as stored (#033); the screens show each person's unit.
//!
//! # Examples
//!
//! ```
//! use domain::fuel::{Fill, summarize};
//! use domain::test_support::test_id;
//! use domain::time::UnixMillis;
//!
//! let fill = |n: u32, km: i64, liters: i64, full: bool| Fill {
//!     event: test_id(n),
//!     at: UnixMillis::from_millis(i64::from(n) * 86_400_000),
//!     reading: Some(km * 1_000),
//!     volume_ml: liters * 1_000,
//!     filled_full: full,
//!     missed_previous: false,
//!     highway_pct: 50,
//!     towing_pct: 0,
//!     hauling_pct: 0,
//!     towed: vec![],
//!     reported: None,
//! };
//! // Full at 1,000 km; 20 L partial; full with 30 L at 1,500 km: 500 km on 50 L.
//! let fills = [fill(1, 1_000, 40, true), fill(2, 1_200, 20, false), fill(3, 1_500, 30, true)];
//! let summary = summarize(&fills, UnixMillis::from_millis(10 * 86_400_000));
//! assert_eq!(summary.stretches[0].economy(), Some(10_000)); // 10 km/L
//! assert_eq!(summary.averages.all.map(|average| average.economy), Some(10_000));
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::event::{Event, EventId};
use crate::meter::{EventKind, Meter, MeterDimension, MeterId};
use crate::time::UnixMillis;

/// How many stretches "the last few" covers.
pub const LAST_STRETCHES: usize = 5;

/// How far back "the last 12 months" looks, in milliseconds (365 days).
pub const RECENT_MILLIS: i64 = 365 * 24 * 60 * 60 * 1_000;

/// The vehicle's fuel odometer: its first distance meter that fuel-ups require.
#[must_use]
pub fn fuel_meter(meters: &[(MeterId, Meter)]) -> Option<MeterId> {
    meters
        .iter()
        .find(|(_, meter)| {
            meter.dimension == MeterDimension::Distance
                && meter.required_for.contains(&EventKind::Fuel)
        })
        .map(|(id, _)| *id)
}

/// One fuel-up, as economy needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fill {
    /// Its event.
    pub event: EventId,
    /// When (the event's start).
    pub at: UnixMillis,
    /// The fuel odometer's reading, in meters, if it was read.
    pub reading: Option<i64>,
    /// How much fuel, in milliliters.
    pub volume_ml: i64,
    /// The tank was filled to full.
    pub filled_full: bool,
    /// An earlier fill-up wasn't logged.
    pub missed_previous: bool,
    /// Share of driving on highways, 0–100 %.
    pub highway_pct: u8,
    /// Share spent towing, 0–100 %.
    pub towing_pct: u8,
    /// Share spent hauling, 0–100 %.
    pub hauling_pct: u8,
    /// The vehicles towed.
    pub towed: Vec<AssetId>,
    /// The vehicle's own economy figure, in meters per liter, if noted.
    pub reported: Option<i64>,
}

impl Fill {
    /// Event `id` as a fill, reading `meter`; `None` if it isn't a fuel-up.
    #[must_use]
    pub fn of(id: EventId, event: &Event, meter: Option<MeterId>) -> Option<Self> {
        let fuel = event.fuel.as_ref()?;
        Some(Self {
            event: id,
            at: event.dates.start,
            reading: meter.and_then(|meter| {
                event
                    .readings
                    .iter()
                    .find(|reading| reading.meter == meter)
                    .map(|reading| reading.value)
            }),
            volume_ml: fuel.volume_ml,
            filled_full: fuel.filled_full,
            missed_previous: fuel.missed_previous,
            highway_pct: fuel.highway_pct,
            towing_pct: fuel.towing_pct,
            hauling_pct: fuel.hauling_pct,
            towed: fuel.towed.clone(),
            reported: fuel.reported_economy,
        })
    }
}

/// Why a stretch counts in no figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DropReason {
    /// A fill in it says an earlier fill-up wasn't logged.
    MissedFillUp,
    /// Its first or last fill has no odometer reading.
    MissingReading,
    /// Its distance is zero or less.
    NoDistance,
}

/// One stretch: from a full fill to the next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stretch {
    /// The full fill it starts at.
    pub from: EventId,
    /// The full fill it ends at.
    pub to: EventId,
    /// When it ended.
    pub ended_at: UnixMillis,
    /// How far, in meters (0 when a reading is missing).
    pub distance_m: i64,
    /// How much fuel, in milliliters: every fill after the first, up to and including the
    /// last.
    pub fuel_ml: i64,
    /// Why it counts in no figure, if it doesn't.
    pub dropped: Option<DropReason>,
    /// Its share of highway driving, weighted by fuel.
    pub highway_pct: u8,
    /// Its share of towing, weighted by fuel.
    pub towing_pct: u8,
    /// Its share of hauling, weighted by fuel.
    pub hauling_pct: u8,
    /// The vehicles any of its fills towed.
    pub towed: Vec<AssetId>,
    /// The car's own figure on the fill it ends at, in meters per liter.
    pub reported: Option<i64>,
}

impl Stretch {
    /// Its figure, in meters per liter; `None` if it's dropped.
    #[must_use]
    pub fn economy(&self) -> Option<i64> {
        if self.dropped.is_some() || self.fuel_ml <= 0 {
            return None;
        }
        Some(per_liter(self.distance_m, self.fuel_ml))
    }
}

/// `distance_m` over `fuel_ml`, in meters per liter, rounded.
fn per_liter(distance_m: i64, fuel_ml: i64) -> i64 {
    let value = (i128::from(distance_m) * 1_000 + i128::from(fuel_ml) / 2) / i128::from(fuel_ml);
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// What one fill-up did for economy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "part", rename_all = "kebab-case")]
pub enum FillPart {
    /// A partial or full fill before the first full one: nothing to measure from yet.
    BeforeFirstFull,
    /// The full fill the first stretch starts from.
    Starts,
    /// A partial fill, added into the stretch it falls in (index into the stretches), or
    /// waiting for the next full fill (`None`).
    Partial {
        /// Its stretch, once there is one.
        stretch: Option<usize>,
    },
    /// The full fill that ends a stretch (index into the stretches).
    Ends {
        /// The stretch.
        stretch: usize,
    },
}

/// An average over some stretches: total distance over total fuel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Average {
    /// The figure, in meters per liter.
    pub economy: i64,
    /// How many stretches.
    pub stretches: usize,
    /// Their distance, in meters.
    pub distance_m: i64,
    /// Their fuel, in milliliters.
    pub fuel_ml: i64,
}

impl Average {
    /// The average of `stretches` (the counted ones among them); `None` if none count.
    fn of<'a>(stretches: impl IntoIterator<Item = &'a Stretch>) -> Option<Self> {
        let (count, distance_m, fuel_ml) = stretches
            .into_iter()
            .filter(|stretch| stretch.economy().is_some())
            .fold((0, 0_i64, 0_i64), |(count, distance, fuel), stretch| {
                (
                    count + 1,
                    distance.saturating_add(stretch.distance_m),
                    fuel.saturating_add(stretch.fuel_ml),
                )
            });
        (count > 0 && fuel_ml > 0).then(|| Self {
            economy: per_liter(distance_m, fuel_ml),
            stretches: count,
            distance_m,
            fuel_ml,
        })
    }
}

/// The Fuel tab's three averages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Averages {
    /// The last 5 counted stretches.
    pub last: Option<Average>,
    /// The counted stretches ending in the last 12 months.
    pub year: Option<Average>,
    /// Every counted stretch.
    pub all: Option<Average>,
}

/// A condition's band: for highway, mostly city / mixed / mostly highway; for towing and
/// hauling, none / some / mostly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Band {
    /// Mostly city (30% or less highway), or none (0%).
    Low,
    /// Mixed, or some (under 50%).
    Middle,
    /// Mostly highway (70% or more), or mostly (50% or more).
    High,
}

impl Band {
    /// Every band, in order.
    pub const ALL: [Self; 3] = [Self::Low, Self::Middle, Self::High];

    /// A highway share's band.
    #[must_use]
    pub const fn highway(pct: u8) -> Self {
        match pct {
            0..=30 => Self::Low,
            31..=69 => Self::Middle,
            _ => Self::High,
        }
    }

    /// A towing or hauling share's band.
    #[must_use]
    pub const fn load(pct: u8) -> Self {
        match pct {
            0 => Self::Low,
            1..=49 => Self::Middle,
            _ => Self::High,
        }
    }
}

/// The breakdowns: each condition's bands (in [`Band::ALL`]'s order), and each towed vehicle.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Breakdowns {
    /// Mostly city, mixed, mostly highway.
    pub highway: [Option<Average>; 3],
    /// No towing, some, mostly.
    pub towing: [Option<Average>; 3],
    /// No hauling, some, mostly.
    pub hauling: [Option<Average>; 3],
    /// Each vehicle towed (`None`: the stretches that tow nothing), with its average.
    pub towed: Vec<(Option<AssetId>, Average)>,
}

/// Everything economy says about a vehicle's fill-ups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FuelSummary {
    /// Its stretches, oldest first.
    pub stretches: Vec<Stretch>,
    /// What each fill-up did, oldest first.
    pub parts: Vec<(EventId, FillPart)>,
    /// The three averages.
    pub averages: Averages,
    /// The breakdowns.
    pub breakdowns: Breakdowns,
}

/// `fills`' shares weighted by fuel: highway, towing, hauling.
fn weighted(fills: &[&Fill]) -> (u8, u8, u8) {
    let fuel: i128 = fills.iter().map(|fill| i128::from(fill.volume_ml)).sum();
    if fuel <= 0 {
        return (0, 0, 0);
    }
    let share = |pct: fn(&Fill) -> u8| {
        let total: i128 = fills
            .iter()
            .map(|fill| i128::from(pct(fill)) * i128::from(fill.volume_ml))
            .sum();
        u8::try_from((total + fuel / 2) / fuel).unwrap_or(100)
    };
    (
        share(|fill| fill.highway_pct),
        share(|fill| fill.towing_pct),
        share(|fill| fill.hauling_pct),
    )
}

/// The stretch from full fill `first` through `rest` (the fills after it, the last one full).
fn stretch(first: &Fill, rest: &[&Fill]) -> Option<Stretch> {
    let last = rest.last()?;
    let fuel_ml = rest.iter().map(|fill| fill.volume_ml).sum();
    let distance_m = match (first.reading, last.reading) {
        (Some(from), Some(to)) => to - from,
        _ => 0,
    };
    let dropped = if rest.iter().any(|fill| fill.missed_previous) {
        Some(DropReason::MissedFillUp)
    } else if first.reading.is_none() || last.reading.is_none() {
        Some(DropReason::MissingReading)
    } else if distance_m <= 0 {
        Some(DropReason::NoDistance)
    } else {
        None
    };
    let (highway_pct, towing_pct, hauling_pct) = weighted(rest);
    let mut towed: Vec<AssetId> = Vec::new();
    for vehicle in rest.iter().flat_map(|fill| &fill.towed) {
        if !towed.contains(vehicle) {
            towed.push(*vehicle);
        }
    }
    Some(Stretch {
        from: first.event,
        to: last.event,
        ended_at: last.at,
        distance_m: distance_m.max(0),
        fuel_ml,
        dropped,
        highway_pct,
        towing_pct,
        hauling_pct,
        towed,
        reported: last.reported,
    })
}

/// Economy from a vehicle's `fills` (in any order), with "the last 12 months" up to `now`.
#[must_use]
pub fn summarize(fills: &[Fill], now: UnixMillis) -> FuelSummary {
    let mut ordered: Vec<&Fill> = fills.iter().collect();
    ordered.sort_by_key(|fill| (fill.at, fill.event));
    let mut stretches = Vec::new();
    let mut parts = Vec::new();
    // The full fill the current stretch started at, and the fills since.
    let mut start: Option<&Fill> = None;
    let mut since: Vec<&Fill> = Vec::new();
    for fill in ordered {
        let Some(first) = start else {
            if fill.filled_full {
                start = Some(fill);
                parts.push((fill.event, FillPart::Starts));
            } else {
                parts.push((fill.event, FillPart::BeforeFirstFull));
            }
            continue;
        };
        since.push(fill);
        if !fill.filled_full {
            parts.push((fill.event, FillPart::Partial { stretch: None }));
            continue;
        }
        if let Some(made) = stretch(first, &since) {
            let index = stretches.len();
            for (event, part) in &mut parts {
                if since.iter().any(|in_it| in_it.event == *event) {
                    *part = FillPart::Partial {
                        stretch: Some(index),
                    };
                }
            }
            parts.push((fill.event, FillPart::Ends { stretch: index }));
            stretches.push(made);
        }
        start = Some(fill);
        since.clear();
    }
    let counted: Vec<&Stretch> = stretches
        .iter()
        .filter(|stretch| stretch.economy().is_some())
        .collect();
    let recent_from = now.as_millis().saturating_sub(RECENT_MILLIS);
    let averages = Averages {
        last: Average::of(counted.iter().rev().take(LAST_STRETCHES).copied()),
        year: Average::of(
            counted
                .iter()
                .filter(|stretch| stretch.ended_at.as_millis() >= recent_from)
                .copied(),
        ),
        all: Average::of(counted.iter().copied()),
    };
    let breakdowns = breakdowns(&counted);
    FuelSummary {
        stretches,
        parts,
        averages,
        breakdowns,
    }
}

/// The breakdowns of the `counted` stretches.
fn breakdowns(counted: &[&Stretch]) -> Breakdowns {
    let banded = |band_of: &dyn Fn(&Stretch) -> Band| {
        Band::ALL.map(|band| {
            Average::of(
                counted
                    .iter()
                    .filter(|stretch| band_of(stretch) == band)
                    .copied(),
            )
        })
    };
    let mut vehicles: Vec<AssetId> = Vec::new();
    for vehicle in counted.iter().flat_map(|stretch| &stretch.towed) {
        if !vehicles.contains(vehicle) {
            vehicles.push(*vehicle);
        }
    }
    let mut towed: Vec<(Option<AssetId>, Average)> = Vec::new();
    if let Some(none) = Average::of(
        counted
            .iter()
            .filter(|stretch| stretch.towed.is_empty())
            .copied(),
    ) {
        towed.push((None, none));
    }
    for vehicle in vehicles {
        if let Some(average) = Average::of(
            counted
                .iter()
                .filter(|stretch| stretch.towed.contains(&vehicle))
                .copied(),
        ) {
            towed.push((Some(vehicle), average));
        }
    }
    Breakdowns {
        highway: banded(&|stretch| Band::highway(stretch.highway_pct)),
        towing: banded(&|stretch| Band::load(stretch.towing_pct)),
        hauling: banded(&|stretch| Band::load(stretch.hauling_pct)),
        towed,
    }
}

#[cfg(test)]
mod tests;
