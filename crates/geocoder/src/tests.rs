//! Lookups against a small hand-made GeoPackage, with real R-tree indexes.
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

use domain::location::Coordinates;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

use crate::geometry::{Geometry, Point, Polygon, rectangle, to_gpkg};
use crate::{FORMAT, FORMAT_VERSION, Geocoder, GeocoderError};

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

async fn create(pool: &SqlitePool) {
    for statement in [
        "CREATE TABLE geodata_info (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        "CREATE TABLE countries (fid INTEGER PRIMARY KEY, geom BLOB, country_code TEXT, name TEXT)",
        "CREATE TABLE regions (fid INTEGER PRIMARY KEY, geom BLOB, country_code TEXT, name TEXT)",
        "CREATE TABLE cities (fid INTEGER PRIMARY KEY, geom BLOB, name TEXT, country_code TEXT, time_zone TEXT)",
        "CREATE VIRTUAL TABLE rtree_countries_geom USING rtree(id, minx, maxx, miny, maxy)",
        "CREATE VIRTUAL TABLE rtree_regions_geom USING rtree(id, minx, maxx, miny, maxy)",
        "CREATE VIRTUAL TABLE rtree_cities_geom USING rtree(id, minx, maxx, miny, maxy)",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
}

async fn info(pool: &SqlitePool, format: &str) {
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
            .await
            .unwrap();
    }
}

async fn country(pool: &SqlitePool, fid: i64, code: &str, name: &str, shapes: Vec<Polygon>) {
    let (w, e, s, n) = bounds(&shapes);
    sqlx::query("INSERT INTO countries (fid, geom, country_code, name) VALUES (?, ?, ?, ?)")
        .bind(fid)
        .bind(to_gpkg(&Geometry::Polygons(shapes)))
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO rtree_countries_geom VALUES (?, ?, ?, ?, ?)")
        .bind(fid)
        .bind(w)
        .bind(e)
        .bind(s)
        .bind(n)
        .execute(pool)
        .await
        .unwrap();
}

async fn region(pool: &SqlitePool, fid: i64, code: &str, name: &str, shapes: Vec<Polygon>) {
    let (w, e, s, n) = bounds(&shapes);
    sqlx::query("INSERT INTO regions (fid, geom, country_code, name) VALUES (?, ?, ?, ?)")
        .bind(fid)
        .bind(to_gpkg(&Geometry::Polygons(shapes)))
        .bind(code)
        .bind(name)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO rtree_regions_geom VALUES (?, ?, ?, ?, ?)")
        .bind(fid)
        .bind(w)
        .bind(e)
        .bind(s)
        .bind(n)
        .execute(pool)
        .await
        .unwrap();
}

async fn city(pool: &SqlitePool, fid: i64, name: &str, code: &str, zone: &str, at: (f64, f64)) {
    sqlx::query(
        "INSERT INTO cities (fid, geom, name, country_code, time_zone) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(fid)
    .bind(to_gpkg(&Geometry::Point(Point { x: at.0, y: at.1 })))
    .bind(name)
    .bind(code)
    .bind(zone)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO rtree_cities_geom VALUES (?, ?, ?, ?, ?)")
        .bind(fid)
        .bind(at.0)
        .bind(at.0)
        .bind(at.1)
        .bind(at.1)
        .execute(pool)
        .await
        .unwrap();
}

/// Writes the test map to a GeoPackage in a temporary folder, and opens it.
async fn test_map(format: &str) -> (tempfile::TempDir, Result<Geocoder, GeocoderError>) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("geodata-test.gpkg");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await.unwrap();
    create(&pool).await;
    info(&pool, format).await;
    let mut aa = rectangle(0.0, 0.0, 10.0, 10.0);
    aa.holes.push(rectangle(4.0, 4.0, 6.0, 6.0).exterior);
    country(&pool, 1, "AA", "Aland", vec![aa]).await;
    country(
        &pool,
        2,
        "BB",
        "Bland",
        vec![rectangle(10.0, 0.0, 20.0, 10.0)],
    )
    .await;
    country(
        &pool,
        3,
        "CC",
        "Centre",
        vec![rectangle(4.0, 4.0, 6.0, 6.0)],
    )
    .await;
    let islands = vec![
        rectangle(30.0, 0.0, 31.0, 1.0),
        rectangle(32.0, 0.0, 33.0, 1.0),
    ];
    country(&pool, 4, "DD", "Dee Islands", islands).await;
    // A small far-away territory sharing AA's code (like Clipperton Island and France).
    let islet = vec![rectangle(40.0, 40.0, 40.1, 40.1)];
    country(&pool, 5, "AA", "Aland Islet", islet).await;
    region(
        &pool,
        1,
        "AA",
        "North",
        vec![rectangle(0.0, 5.0, 10.0, 10.0)],
    )
    .await;
    region(
        &pool,
        2,
        "AA",
        "South",
        vec![rectangle(0.0, 0.0, 10.0, 5.0)],
    )
    .await;
    region(
        &pool,
        3,
        "BB",
        "East",
        vec![rectangle(10.0, 0.0, 20.0, 10.0)],
    )
    .await;
    region(
        &pool,
        4,
        "CC",
        "Enclave",
        vec![rectangle(4.0, 4.0, 6.0, 6.0)],
    )
    .await;
    city(&pool, 1, "Alpha", "AA", "Etc/GMT", (2.0, 2.0)).await;
    city(&pool, 2, "Aleph", "AA", "Etc/GMT", (8.0, 8.0)).await;
    city(&pool, 3, "Beta", "BB", "Etc/GMT-1", (12.0, 2.0)).await;
    city(&pool, 4, "Middle", "CC", "Etc/GMT-2", (5.0, 5.0)).await;
    city(&pool, 5, "Dee", "DD", "Pacific/Fiji", (30.5, 0.5)).await;
    pool.close().await;
    let geocoder = Geocoder::open(&path).await;
    (temp, geocoder)
}

fn at(lon: f64, lat: f64) -> Coordinates {
    Coordinates::from_degrees(lat, lon).unwrap()
}

/// (city, region, country code, inside the country).
async fn place(geocoder: &Geocoder, lon: f64, lat: f64) -> (String, String, String, bool) {
    let found = geocoder.lookup(at(lon, lat)).await.unwrap();
    (
        found.place.city.as_str().to_owned(),
        found.place.region.as_str().to_owned(),
        found.place.country.as_str().to_owned(),
        found.inside_country,
    )
}

fn expect(city: &str, region: &str, country: &str, inside: bool) -> (String, String, String, bool) {
    (
        city.to_owned(),
        region.to_owned(),
        country.to_owned(),
        inside,
    )
}

#[tokio::test]
async fn a_point_inside_gets_its_country_region_city_and_time_zone() {
    let (_temp, geocoder) = test_map(FORMAT).await;
    let geocoder = geocoder.unwrap();
    assert_eq!(geocoder.info().data_version, "test-1");
    let found = geocoder.lookup(at(1.0, 1.0)).await.unwrap();
    assert_eq!(found.place.city.as_str(), "Alpha");
    assert_eq!(found.place.region.as_str(), "South");
    assert_eq!(found.place.country.as_str(), "AA");
    assert_eq!(found.country_name, "Aland");
    assert_eq!(
        found
            .time_zone
            .map(|zone| zone.as_str().to_owned())
            .as_deref(),
        Some("Etc/GMT")
    );
    assert!(found.inside_country);
    assert_eq!(found.data_version, "test-1");
    // 1° east and 1° north at the equator: about 157 km.
    assert!(
        (found.city_distance_m - 157_000.0).abs() < 2_000.0,
        "{}",
        found.city_distance_m
    );
}

#[tokio::test]
async fn both_sides_of_a_border_and_an_enclave() {
    let (_temp, geocoder) = test_map(FORMAT).await;
    let geocoder = geocoder.unwrap();
    // Just inside AA: Beta (in BB, 2° away) is nearer, but the city comes from the same
    // country: Aleph (about 6.3° away), not Alpha (8°).
    assert_eq!(
        place(&geocoder, 9.99, 2.0).await,
        expect("Aleph", "South", "AA", true)
    );
    assert_eq!(
        place(&geocoder, 10.01, 2.0).await,
        expect("Beta", "East", "BB", true)
    );
    // The enclave is a hole in AA, and its own country.
    assert_eq!(
        place(&geocoder, 5.0, 5.0).await,
        expect("Middle", "Enclave", "CC", true)
    );
    assert_eq!(
        place(&geocoder, 3.9, 5.0).await,
        expect("Alpha", "North", "AA", true)
    );
}

#[tokio::test]
async fn off_the_coast_the_nearest_country_and_region_are_used() {
    let (_temp, geocoder) = test_map(FORMAT).await;
    let geocoder = geocoder.unwrap();
    // 0.01° west of AA's coast (about 1.1 km), at the latitude of North.
    assert_eq!(
        place(&geocoder, -0.01, 7.0).await,
        expect("Alpha", "North", "AA", false)
    );
    // Far out at sea, nearer the Dee Islands than BB; DD has no regions, so it's its own.
    assert_eq!(
        place(&geocoder, 28.0, 0.5).await,
        expect("Dee", "Dee Islands", "DD", false)
    );
    // Between the islands.
    assert_eq!(
        place(&geocoder, 31.4, 0.5).await,
        expect("Dee", "Dee Islands", "DD", false)
    );
}

#[tokio::test]
async fn the_r_tree_is_what_finds_candidates() {
    let (temp, geocoder) = test_map(FORMAT).await;
    drop(geocoder);
    let path = temp.path().join("geodata-test.gpkg");
    // Remove AA from the countries' index only: a point in AA no longer finds it inside.
    let pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(&path))
        .await
        .unwrap();
    sqlx::query("DELETE FROM rtree_countries_geom WHERE id = 1")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;
    let geocoder = Geocoder::open(&path).await.unwrap();
    let (_, _, country, inside) = place(&geocoder, 1.0, 1.0).await;
    // Not found inside AA; the nearest indexed boundary is CC's (about 4.2° away).
    assert_eq!(country, "CC", "only the indexed countries are candidates");
    assert!(!inside);
}

#[tokio::test]
async fn other_files_are_refused() {
    let (_temp, geocoder) = test_map("something-else").await;
    assert!(matches!(geocoder, Err(GeocoderError::WrongFormat(_))));
    let missing = Geocoder::open(std::path::Path::new("/nonexistent/geodata.gpkg")).await;
    assert!(matches!(missing, Err(GeocoderError::Sqlite(_))));
}

#[tokio::test]
async fn countries_are_listed_by_name() {
    let (_temp, geocoder) = test_map(FORMAT).await;
    let names: Vec<String> = geocoder
        .unwrap()
        .countries()
        .await
        .unwrap()
        .into_iter()
        .map(|(code, name)| format!("{} {name}", code.as_str()))
        .collect();
    // One name per code, from its largest area.
    assert_eq!(
        names,
        ["AA Aland", "BB Bland", "CC Centre", "DD Dee Islands"]
    );
}
