//! Offline reverse geocoding (plan §3 Reverse geocoding; decisions #038, #059): coordinates to
//! a city, a state or province, a country, and the city's time zone, from a **GeoPackage**
//! file the server opens read-only. No network, no external service.
//!
//! # The file
//!
//! A GeoPackage is a SQLite database (OGC standard); step 5c builds it with GDAL. This crate
//! reads these tables (each geometry in EPSG:4326, longitude/latitude degrees), and each
//! layer's R-tree spatial index (`rtree_<table>_geom`, which GDAL creates):
//!
//! | Table | Columns |
//! |---|---|
//! | `geodata_info` | `key`, `value`: `format` = `vehicle-manager-geodata`, `format_version` = `1`, `data_version`, `attribution` |
//! | `countries` | `fid`, `geom` (multipolygon), `country_code` (ISO alpha-2), `name` |
//! | `regions` | `fid`, `geom` (multipolygon), `country_code`, `name` (state or province) |
//! | `cities` | `fid`, `geom` (point), `name`, `country_code`, `time_zone` (IANA), `feature_code` (GeoNames, e.g. `PPLA2`), `population` |
//!
//! # A lookup
//!
//! 1. **Country:** the R-tree finds the countries whose box holds the point; an exact
//!    point-in-polygon test picks the one it's in. Outside every country (a beach, a pier: the
//!    boundaries don't trace coastlines exactly), the **nearest** country is used, found by a
//!    search box that widens until the nearest boundary is certainly inside it (#038).
//! 2. **State or province:** the same, among that country's regions where possible.
//! 3. **City:** up to five **nearby places** in that country within 25 km, nearest first,
//!    with their distance, direction, and kind (#061). The suggested city is the nearest that
//!    isn't a district; with none within 25 km, the nearest city in that country (so a point
//!    just inside a border isn't named after a town across it), found the same widening way.
//!    Its time zone comes with it.
//!
//! The answer is a **suggestion**: people can still correct an event's city, state, and
//! country (#038).

pub mod geometry;
mod store;

#[cfg(any(test, feature = "test-support"))]
pub mod test_map;
#[cfg(test)]
mod tests;

use std::fmt;
use std::path::Path;

use domain::location::{
    Compass, Coordinates, CountryCode, NearbyPlace, Place, PlaceName, TimeZone, place_kind,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::geometry::Point;

/// The `format` value this crate reads.
pub const FORMAT: &str = "vehicle-manager-geodata";
/// How many nearby places a lookup offers, and how far they may be (#061).
pub const NEARBY_LIMIT: usize = 5;
/// See [`NEARBY_LIMIT`]: 25 km.
pub const NEARBY_WITHIN_M: f64 = 25_000.0;

/// The `format_version` this crate reads.
pub const FORMAT_VERSION: &str = "2";

/// Why the geocoder couldn't open its file or answer.
#[derive(Debug)]
pub enum GeocoderError {
    /// SQLite couldn't open or read the file.
    Sqlite(sqlx::Error),
    /// The file isn't this app's geocoder data, or is a format version this build can't read.
    WrongFormat(String),
    /// The data has no country or city to give (a broken or empty file).
    Incomplete(&'static str),
}

impl fmt::Display for GeocoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(f, "geocoder data couldn't be read: {error}"),
            Self::WrongFormat(why) => write!(f, "not usable geocoder data: {why}"),
            Self::Incomplete(what) => write!(f, "geocoder data has no {what}"),
        }
    }
}

impl std::error::Error for GeocoderError {}

impl From<sqlx::Error> for GeocoderError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// About the open data file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataInfo {
    /// The data's version, e.g. `2026-11-02` (stored with every place it suggests).
    pub data_version: String,
    /// The credit to show on About, e.g. "Contains GeoNames data (CC BY 4.0) …".
    pub attribution: String,
}

/// What the geocoder suggests for a point.
#[derive(Debug, Clone, PartialEq)]
pub struct Lookup {
    /// The city, state or province, and country.
    pub place: Place,
    /// The country's name, e.g. "United States of America".
    pub country_name: String,
    /// The nearest city's time zone, if the data has a valid one.
    pub time_zone: Option<TimeZone>,
    /// How far the city is from the point, in meters.
    pub city_distance_m: f64,
    /// The point is inside its country's boundary (false: the nearest country was used).
    pub inside_country: bool,
    /// The data version it came from.
    pub data_version: String,
    /// Up to five places nearby, nearest first, for the picker to offer (#061).
    pub nearby: Vec<NearbyPlace>,
}

/// A nearby city as the picker shows it (`None` if its name doesn't fit a place name).
fn nearby_place(near: &store::Near) -> Option<NearbyPlace> {
    let (kind, seat) = place_kind(&near.feature_code, near.population);
    Some(NearbyPlace {
        name: PlaceName::parse(&near.name).ok()?,
        // Within 25 km, so it fits easily.
        distance_m: u32::try_from(format!("{:.0}", near.distance_m).parse::<i64>().ok()?).ok()?,
        direction: Compass::from_bearing(near.bearing),
        kind,
        seat,
    })
}

/// The open geocoder data.
#[derive(Debug, Clone)]
pub struct Geocoder {
    pool: SqlitePool,
    info: DataInfo,
}

impl Geocoder {
    /// Opens the data file read-only and checks it's geocoder data this build can read.
    ///
    /// # Errors
    ///
    /// [`GeocoderError`] if it can't be opened, or isn't usable.
    pub async fn open(path: &Path) -> Result<Self, GeocoderError> {
        // Read-only and immutable: the file never changes while the server runs, so SQLite
        // needn't lock it or look for a journal.
        let options = SqliteConnectOptions::new()
            .filename(path)
            .read_only(true)
            .immutable(true)
            .create_if_missing(false);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        let info = store::read_info(&pool).await?;
        tracing::debug!(target: domain::log_area::APP, data_version = %info.data_version, "geocoder data opened");
        Ok(Self { pool, info })
    }

    /// About the data.
    #[must_use]
    pub const fn info(&self) -> &DataInfo {
        &self.info
    }

    /// The suggested place for `point`.
    ///
    /// # Errors
    ///
    /// [`GeocoderError`] if the file can't be read or has no country or city.
    pub async fn lookup(&self, point: Coordinates) -> Result<Lookup, GeocoderError> {
        let at = Point {
            x: point.longitude(),
            y: point.latitude(),
        };
        let country = store::country(&self.pool, at)
            .await?
            .ok_or(GeocoderError::Incomplete("country"))?;
        // A country with no states or provinces in the data is its own region.
        let region = store::region(&self.pool, at, &country.code)
            .await?
            .unwrap_or_else(|| country.name.clone());
        let near = store::nearby(
            &self.pool,
            point,
            &country.code,
            NEARBY_LIMIT,
            NEARBY_WITHIN_M,
        )
        .await?;
        // Each row with what the picker shows (rows whose name doesn't fit are left out).
        let pairs: Vec<(&store::Near, NearbyPlace)> = near
            .iter()
            .filter_map(|row| nearby_place(row).map(|place| (row, place)))
            .collect();
        // The suggestion: the nearest nearby place that isn't a district; with none, the
        // nearest city in the country, however far.
        let chosen = pairs
            .iter()
            .find(|(_, place)| !place.is_district())
            .map(|(near, _)| store::City {
                name: near.name.clone(),
                time_zone: near.time_zone.clone(),
                distance_m: near.distance_m,
            });
        let city = match chosen {
            Some(city) => city,
            None => store::city(&self.pool, point, &country.code)
                .await?
                .ok_or(GeocoderError::Incomplete("city"))?,
        };
        let bad = |what: &str| GeocoderError::WrongFormat(format!("a {what} that isn't valid"));
        let place = Place {
            city: PlaceName::parse(&city.name).map_err(|_| bad("city name"))?,
            region: PlaceName::parse(&region).map_err(|_| bad("region name"))?,
            country: CountryCode::parse(&country.code).map_err(|_| bad("country code"))?,
        };
        Ok(Lookup {
            place,
            country_name: country.name,
            time_zone: TimeZone::parse(&city.time_zone).ok(),
            city_distance_m: city.distance_m,
            inside_country: country.inside,
            data_version: self.info.data_version.clone(),
            nearby: pairs.into_iter().map(|(_, place)| place).collect(),
        })
    }

    /// Every country, by name: for typing a place by hand.
    ///
    /// # Errors
    ///
    /// [`GeocoderError`] if the file can't be read.
    pub async fn countries(&self) -> Result<Vec<(CountryCode, String)>, GeocoderError> {
        store::countries(&self.pool).await
    }
}
