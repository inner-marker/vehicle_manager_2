//! A small hand-made GeoPackage for tests, with real R-tree indexes (here and in the server's
//! tests, through the `test-support` feature).
//!
//! The map (longitude, latitude in degrees):
//!
//! ```text
//!  lat 10 ┌──── AA ─────┬──── BB ────┐        DD: two islands at 30–31 and 32–33,
//!         │ North       │            │            lat 0–1; no regions
//!   lat 5 ├──── ┌CC┐ ──┤  East      │
//!         │ South└──┘  │            │        CC: an enclave (4–6, 4–6), a hole in AA
//!   lat 0 └─────────────┴────────────┘
//!       lon 0          10           20
//! ```
//!
//! Cities: Alpha (2, 2; a regional seat), its district Alpha Old Town (2.1, 2), the locality
//! Alphaville (2.12, 1.9), and Aleph (8, 8) in AA; Beta (12, 2; a capital) in BB; Middle (5, 5) in
//! CC; Dee (30.5, 0.5) in DD. A small far-away islet (40–40.1) also has AA's code.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

use crate::FORMAT_VERSION;
use crate::geometry::{Geometry, Point, Polygon, rectangle, to_gpkg};

/// The bounding box of some polygons: (west, east, south, north).
fn bounds(polygons: &[Polygon]) -> (f64, f64, f64, f64) {
    let points = polygons.iter().flat_map(|polygon| &polygon.exterior);
    points.fold(
        (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ),
        |(w, e, s, n), p| (w.min(p.x), e.max(p.x), s.min(p.y), n.max(p.y)),
    )
}

async fn create(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for statement in [
        "CREATE TABLE geodata_info (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        "CREATE TABLE countries (fid INTEGER PRIMARY KEY, geom BLOB, country_code TEXT, name TEXT)",
        "CREATE TABLE regions (fid INTEGER PRIMARY KEY, geom BLOB, country_code TEXT, name TEXT)",
        "CREATE TABLE cities (fid INTEGER PRIMARY KEY, geom BLOB, name TEXT, country_code TEXT, time_zone TEXT, feature_code TEXT, population INTEGER)",
        "CREATE VIRTUAL TABLE rtree_countries_geom USING rtree(id, minx, maxx, miny, maxy)",
        "CREATE VIRTUAL TABLE rtree_regions_geom USING rtree(id, minx, maxx, miny, maxy)",
        "CREATE VIRTUAL TABLE rtree_cities_geom USING rtree(id, minx, maxx, miny, maxy)",
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

async fn info(pool: &SqlitePool, format: &str) -> Result<(), sqlx::Error> {
    for (key, value) in [
        ("format", format),
        ("format_version", FORMAT_VERSION),
        ("data_version", "test-1"),
        ("attribution", "Test data"),
    ] {
        sqlx::query("INSERT INTO geodata_info (key, value) VALUES (?, ?)")
            .bind(key)
            .bind(value)
            .execute(pool)
            .await?;
    }
    Ok(())
}

async fn country(
    pool: &SqlitePool,
    fid: i64,
    code: &str,
    name: &str,
    shapes: Vec<Polygon>,
) -> Result<(), sqlx::Error> {
    let (w, e, s, n) = bounds(&shapes);
    sqlx::query("INSERT INTO countries (fid, geom, country_code, name) VALUES (?, ?, ?, ?)")
        .bind(fid)
        .bind(to_gpkg(&Geometry::Polygons(shapes)))
        .bind(code)
        .bind(name)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO rtree_countries_geom VALUES (?, ?, ?, ?, ?)")
        .bind(fid)
        .bind(w)
        .bind(e)
        .bind(s)
        .bind(n)
        .execute(pool)
        .await?;
    Ok(())
}

async fn region(
    pool: &SqlitePool,
    fid: i64,
    code: &str,
    name: &str,
    shapes: Vec<Polygon>,
) -> Result<(), sqlx::Error> {
    let (w, e, s, n) = bounds(&shapes);
    sqlx::query("INSERT INTO regions (fid, geom, country_code, name) VALUES (?, ?, ?, ?)")
        .bind(fid)
        .bind(to_gpkg(&Geometry::Polygons(shapes)))
        .bind(code)
        .bind(name)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO rtree_regions_geom VALUES (?, ?, ?, ?, ?)")
        .bind(fid)
        .bind(w)
        .bind(e)
        .bind(s)
        .bind(n)
        .execute(pool)
        .await?;
    Ok(())
}

async fn city(
    pool: &SqlitePool,
    fid: i64,
    name: &str,
    code: &str,
    zone: &str,
    at: (f64, f64),
    (feature_code, population): (&str, i64),
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO cities (fid, geom, name, country_code, time_zone, feature_code, population) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(fid)
    .bind(to_gpkg(&Geometry::Point(Point { x: at.0, y: at.1 })))
    .bind(name)
    .bind(code)
    .bind(zone)
    .bind(feature_code)
    .bind(population)
    .execute(pool)
    .await?;
    sqlx::query("INSERT INTO rtree_cities_geom VALUES (?, ?, ?, ?, ?)")
        .bind(fid)
        .bind(at.0)
        .bind(at.0)
        .bind(at.1)
        .bind(at.1)
        .execute(pool)
        .await?;
    Ok(())
}

/// The cities: (fid, name, country, time zone, position, (feature code, population)).
/// Near Alpha are a district of it (nearer to some points than Alpha is) and a locality.
type CityRow = (
    i64,
    &'static str,
    &'static str,
    &'static str,
    (f64, f64),
    (&'static str, i64),
);
const CITIES: [CityRow; 7] = [
    (1, "Alpha", "AA", "Etc/GMT", (2.0, 2.0), ("PPLA", 50_000)),
    (2, "Aleph", "AA", "Etc/GMT", (8.0, 8.0), ("PPL", 2_000)),
    (3, "Beta", "BB", "Etc/GMT-1", (12.0, 2.0), ("PPLC", 300_000)),
    (4, "Middle", "CC", "Etc/GMT-2", (5.0, 5.0), ("PPL", 500)),
    (5, "Dee", "DD", "Pacific/Fiji", (30.5, 0.5), ("PPL", 8_000)),
    (
        6,
        "Alpha Old Town",
        "AA",
        "Etc/GMT",
        (2.1, 2.0),
        ("PPLX", 12_000),
    ),
    (7, "Alphaville", "AA", "Etc/GMT", (2.12, 1.9), ("PPLL", 40)),
];

/// The countries and regions (the map above).
async fn shapes(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let mut aa = rectangle(0.0, 0.0, 10.0, 10.0);
    aa.holes.push(rectangle(4.0, 4.0, 6.0, 6.0).exterior);
    country(pool, 1, "AA", "Aland", vec![aa]).await?;
    country(
        pool,
        2,
        "BB",
        "Bland",
        vec![rectangle(10.0, 0.0, 20.0, 10.0)],
    )
    .await?;
    country(pool, 3, "CC", "Centre", vec![rectangle(4.0, 4.0, 6.0, 6.0)]).await?;
    let islands = vec![
        rectangle(30.0, 0.0, 31.0, 1.0),
        rectangle(32.0, 0.0, 33.0, 1.0),
    ];
    country(pool, 4, "DD", "Dee Islands", islands).await?;
    // A small far-away territory sharing AA's code (like Clipperton Island and France).
    let islet = vec![rectangle(40.0, 40.0, 40.1, 40.1)];
    country(pool, 5, "AA", "Aland Islet", islet).await?;
    region(
        pool,
        1,
        "AA",
        "North",
        vec![rectangle(0.0, 5.0, 10.0, 10.0)],
    )
    .await?;
    region(pool, 2, "AA", "South", vec![rectangle(0.0, 0.0, 10.0, 5.0)]).await?;
    region(
        pool,
        3,
        "BB",
        "East",
        vec![rectangle(10.0, 0.0, 20.0, 10.0)],
    )
    .await?;
    region(
        pool,
        4,
        "CC",
        "Enclave",
        vec![rectangle(4.0, 4.0, 6.0, 6.0)],
    )
    .await?;
    Ok(())
}

/// Writes the test map to `path`, with `format` as its `geodata_info` format (the real one is
/// [`crate::FORMAT`]).
///
/// # Errors
///
/// [`sqlx::Error`] if the file can't be written.
pub async fn write(path: &Path, format: &str) -> Result<(), sqlx::Error> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    create(&pool).await?;
    info(&pool, format).await?;
    shapes(&pool).await?;
    for (fid, name, code, zone, at, kind) in CITIES {
        city(&pool, fid, name, code, zone, at, kind).await?;
    }
    pool.close().await;
    Ok(())
}
