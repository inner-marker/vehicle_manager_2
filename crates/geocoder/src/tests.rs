//! Lookups against the small hand-made GeoPackage in `crate::test_map` (its map is drawn
//! there), with real R-tree indexes.

use domain::location::Coordinates;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

use crate::{FORMAT, Geocoder, GeocoderError};

/// Writes the test map to a GeoPackage in a temporary folder, and opens it.
async fn test_map(format: &str) -> (tempfile::TempDir, Result<Geocoder, GeocoderError>) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("geodata-test.gpkg");
    crate::test_map::write(&path, format).await.unwrap();
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
