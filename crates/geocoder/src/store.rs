//! Queries against the GeoPackage: R-tree candidates, then exact geometry in Rust.
//!
//! **Nearest searches widen.** A box of half-height `d` degrees around the point (and as wide
//! as `d` degrees of latitude in meters) holds everything within about `d × 111 km` of it. So
//! once the best candidate in a box is closer than that, nothing outside the box can beat it,
//! and the search stops; otherwise the box grows. The last box is the whole world.

use domain::location::Coordinates;
use sqlx::Row;
use sqlx::sqlite::{SqlitePool, SqliteRow};

use crate::geometry::{self, Geometry, Point, Polygon};
use crate::{DataInfo, FORMAT, FORMAT_VERSION, GeocoderError};

/// The search boxes' half-heights, in degrees, smallest first; the last covers the world.
const SEARCH_DEGREES: [f64; 7] = [0.05, 0.2, 1.0, 4.0, 15.0, 60.0, 360.0];
/// Meters per degree of latitude, rounded down so the "certainly inside" test stays safe.
const SAFE_METERS_PER_DEGREE: f64 = 111_000.0;

/// A search box: longitude `west..=east`, latitude `south..=north`.
#[derive(Debug, Clone, Copy)]
struct Area {
    west: f64,
    east: f64,
    south: f64,
    north: f64,
}

impl Area {
    /// The box of half-height `degrees` around `at`, wide enough to hold the circle of that
    /// many degrees of latitude (longitude degrees shrink towards the poles). One that would
    /// cross the antimeridian becomes every longitude.
    fn around(at: Point, degrees: f64) -> Self {
        let half_width = degrees / at.y.to_radians().cos().max(0.01);
        let (west, east) = if at.x - half_width < -180.0 || at.x + half_width > 180.0 {
            (-180.0, 180.0)
        } else {
            (at.x - half_width, at.x + half_width)
        };
        Self {
            west,
            east,
            south: (at.y - degrees).max(-90.0),
            north: (at.y + degrees).min(90.0),
        }
    }

    /// Just the point.
    const fn at(at: Point) -> Self {
        Self {
            west: at.x,
            east: at.x,
            south: at.y,
            north: at.y,
        }
    }
}

/// Whether a candidate at `distance_m` found in a box of half-height `degrees` is certainly
/// the nearest (nothing outside the box can be closer), or the box was the whole world.
fn settled(distance_m: f64, degrees: f64) -> bool {
    degrees >= 180.0 || distance_m <= degrees * SAFE_METERS_PER_DEGREE
}

/// One row of a layer: its geometry, country code, name, and (for cities) time zone.
struct Feature {
    geometry: Geometry,
    code: String,
    name: String,
    time_zone: String,
}

/// A row as a feature, or `None` if its geometry can't be read (logged, then skipped).
fn feature(row: &SqliteRow) -> Result<Option<Feature>, sqlx::Error> {
    let blob: Vec<u8> = row.try_get("geom")?;
    let geometry = match geometry::from_gpkg(&blob) {
        Ok(geometry) => geometry,
        Err(error) => {
            let fid: i64 = row.try_get("fid")?;
            tracing::debug!(target: domain::log_area::APP, fid, ?error, "geocoder: skipped a geometry it can't read");
            return Ok(None);
        }
    };
    Ok(Some(Feature {
        geometry,
        code: row.try_get("country_code")?,
        name: row.try_get("name")?,
        time_zone: row.try_get("time_zone").unwrap_or_default(),
    }))
}

fn features(rows: &[SqliteRow]) -> Result<Vec<Feature>, sqlx::Error> {
    Ok(rows
        .iter()
        .map(feature)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect())
}

/// A feature's polygons (none for a point).
fn polygons(feature: &Feature) -> &[Polygon] {
    match &feature.geometry {
        Geometry::Polygons(polygons) => polygons,
        Geometry::Point(_) => &[],
    }
}

/// The first feature that contains `at`.
fn containing(features: Vec<Feature>, at: Point) -> Option<Feature> {
    features
        .into_iter()
        .find(|feature| geometry::contains(polygons(feature), at))
}

/// The feature whose boundary is nearest `at`, with its distance in meters.
fn nearest_boundary(features: Vec<Feature>, at: Point) -> Option<(Feature, f64)> {
    features
        .into_iter()
        .map(|feature| {
            let distance = geometry::distance_to_boundary_m(polygons(&feature), at);
            (feature, distance)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

// --- Info ----------------------------------------------------------------------------------

/// Reads and checks `geodata_info`.
pub(crate) async fn read_info(pool: &SqlitePool) -> Result<DataInfo, GeocoderError> {
    let rows = sqlx::query("SELECT key, value FROM geodata_info")
        .fetch_all(pool)
        .await?;
    let value = |key: &str| -> Option<String> {
        rows.iter()
            .find(|row| row.try_get::<String, _>("key").is_ok_and(|k| k == key))
            .and_then(|row| row.try_get("value").ok())
    };
    match (value("format"), value("format_version")) {
        (Some(format), Some(version)) if format == FORMAT && version == FORMAT_VERSION => {}
        (format, version) => {
            return Err(GeocoderError::WrongFormat(format!(
                "format {format:?}, version {version:?}; this build reads {FORMAT} {FORMAT_VERSION}"
            )));
        }
    }
    Ok(DataInfo {
        data_version: value("data_version")
            .ok_or_else(|| GeocoderError::WrongFormat("no data_version".to_owned()))?,
        attribution: value("attribution").unwrap_or_default(),
    })
}

// --- Countries -----------------------------------------------------------------------------

/// The country found for a point.
pub(crate) struct Country {
    pub code: String,
    pub name: String,
    /// Inside its boundary (false: it's the nearest).
    pub inside: bool,
}

async fn countries_in(pool: &SqlitePool, area: Area) -> Result<Vec<Feature>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT c.fid, c.geom, c.country_code, c.name FROM rtree_countries_geom AS r
         JOIN countries AS c ON c.fid = r.id
         WHERE r.maxx >= ? AND r.minx <= ? AND r.maxy >= ? AND r.miny <= ?",
    )
    .bind(area.west)
    .bind(area.east)
    .bind(area.south)
    .bind(area.north)
    .fetch_all(pool)
    .await?;
    features(&rows)
}

/// The country `at` is in, or the nearest one.
pub(crate) async fn country(pool: &SqlitePool, at: Point) -> Result<Option<Country>, sqlx::Error> {
    if let Some(found) = containing(countries_in(pool, Area::at(at)).await?, at) {
        return Ok(Some(Country {
            code: found.code,
            name: found.name,
            inside: true,
        }));
    }
    for degrees in SEARCH_DEGREES {
        let candidates = countries_in(pool, Area::around(at, degrees)).await?;
        if let Some((found, distance)) = nearest_boundary(candidates, at)
            && settled(distance, degrees)
        {
            return Ok(Some(Country {
                code: found.code,
                name: found.name,
                inside: false,
            }));
        }
    }
    Ok(None)
}

/// Every country, by name: one name per code, from its largest area (e.g. `FR` covers both
/// France and Clipperton Island, and is listed as France).
pub(crate) async fn countries(
    pool: &SqlitePool,
) -> Result<Vec<(domain::location::CountryCode, String)>, GeocoderError> {
    let rows = sqlx::query(
        "SELECT c.country_code, c.name FROM countries AS c
         JOIN rtree_countries_geom AS r ON r.id = c.fid
         ORDER BY c.country_code, (r.maxx - r.minx) * (r.maxy - r.miny) DESC",
    )
    .fetch_all(pool)
    .await?;
    let mut list: Vec<(domain::location::CountryCode, String)> = Vec::new();
    for row in &rows {
        let (Ok(code), Ok(name)) = (
            row.try_get::<String, _>("country_code"),
            row.try_get::<String, _>("name"),
        ) else {
            continue;
        };
        let Ok(code) = domain::location::CountryCode::parse(&code) else {
            continue;
        };
        // The largest area comes first for each code.
        if list.last().is_none_or(|(last, _)| *last != code) {
            list.push((code, name));
        }
    }
    list.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(list)
}

// --- Regions -------------------------------------------------------------------------------

async fn regions_in(
    pool: &SqlitePool,
    area: Area,
    country_code: &str,
) -> Result<Vec<Feature>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT c.fid, c.geom, c.country_code, c.name FROM rtree_regions_geom AS r
         JOIN regions AS c ON c.fid = r.id
         WHERE r.maxx >= ? AND r.minx <= ? AND r.maxy >= ? AND r.miny <= ?
           AND c.country_code = ?",
    )
    .bind(area.west)
    .bind(area.east)
    .bind(area.south)
    .bind(area.north)
    .bind(country_code)
    .fetch_all(pool)
    .await?;
    features(&rows)
}

/// The state or province of `country_code` that `at` is in, or the nearest one of that
/// country's; `None` if the country has none.
pub(crate) async fn region(
    pool: &SqlitePool,
    at: Point,
    country_code: &str,
) -> Result<Option<String>, sqlx::Error> {
    if let Some(found) = containing(regions_in(pool, Area::at(at), country_code).await?, at) {
        return Ok(Some(found.name));
    }
    for degrees in SEARCH_DEGREES {
        let candidates = regions_in(pool, Area::around(at, degrees), country_code).await?;
        if let Some((found, distance)) = nearest_boundary(candidates, at)
            && settled(distance, degrees)
        {
            return Ok(Some(found.name));
        }
    }
    Ok(None)
}

// --- Cities --------------------------------------------------------------------------------

/// The city found for a point.
pub(crate) struct City {
    pub name: String,
    pub time_zone: String,
    pub distance_m: f64,
}

async fn cities_in(
    pool: &SqlitePool,
    area: Area,
    country_code: Option<&str>,
) -> Result<Vec<Feature>, sqlx::Error> {
    let rows = match country_code {
        Some(code) => sqlx::query(
            "SELECT c.fid, c.geom, c.country_code, c.name, c.time_zone FROM rtree_cities_geom AS r
                 JOIN cities AS c ON c.fid = r.id
                 WHERE r.maxx >= ? AND r.minx <= ? AND r.maxy >= ? AND r.miny <= ?
                   AND c.country_code = ?",
        )
        .bind(area.west)
        .bind(area.east)
        .bind(area.south)
        .bind(area.north)
        .bind(code)
        .fetch_all(pool)
        .await?,
        None => sqlx::query(
            "SELECT c.fid, c.geom, c.country_code, c.name, c.time_zone FROM rtree_cities_geom AS r
                 JOIN cities AS c ON c.fid = r.id
                 WHERE r.maxx >= ? AND r.minx <= ? AND r.maxy >= ? AND r.miny <= ?",
        )
        .bind(area.west)
        .bind(area.east)
        .bind(area.south)
        .bind(area.north)
        .fetch_all(pool)
        .await?,
    };
    features(&rows)
}

/// The nearest city among `features`, with its distance from `point` in meters.
fn nearest_city(features: Vec<Feature>, point: Coordinates) -> Option<(Feature, f64)> {
    features
        .into_iter()
        .filter_map(|feature| {
            let Geometry::Point(at) = feature.geometry else {
                return None;
            };
            let distance = Coordinates::from_degrees(at.y, at.x)
                .ok()?
                .distance_m(point);
            Some((feature, distance))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

/// The nearest city to `point` in `country_code`, or anywhere if that country has none.
pub(crate) async fn city(
    pool: &SqlitePool,
    point: Coordinates,
    country_code: &str,
) -> Result<Option<City>, sqlx::Error> {
    let at = Point {
        x: point.longitude(),
        y: point.latitude(),
    };
    for filter in [Some(country_code), None] {
        for degrees in SEARCH_DEGREES {
            let candidates = cities_in(pool, Area::around(at, degrees), filter).await?;
            if let Some((found, distance)) = nearest_city(candidates, point)
                && settled(distance, degrees)
            {
                return Ok(Some(City {
                    name: found.name,
                    time_zone: found.time_zone,
                    distance_m: distance,
                }));
            }
        }
    }
    Ok(None)
}
