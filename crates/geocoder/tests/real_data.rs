//! Lookups against the real data file (step 5c), to check a build of it.
//!
//! Ignored by default (the file isn't in the repository). Run it after building the data:
//!
//! ```sh
//! GEODATA_FILE=.geodata/out/geodata-2026-11-02.gpkg cargo test -p geocoder --test real_data -- --ignored --nocapture
//! ```

use domain::location::Coordinates;
use geocoder::Geocoder;

/// (what, latitude, longitude, expected country, expected state or province).
const PLACES: &[(&str, f64, f64, &str, &str)] = &[
    ("Asheville, NC", 35.5951, -82.5515, "US", "North Carolina"),
    ("Boone, NC", 36.2168, -81.6746, "US", "North Carolina"),
    (
        "off Wrightsville Beach pier, NC",
        34.2133,
        -77.7850,
        "US",
        "North Carolina",
    ),
    (
        "off Cape Hatteras, NC",
        35.2000,
        -75.5000,
        "US",
        "North Carolina",
    ),
    (
        "in the water off Key West, FL",
        24.5400,
        -81.8000,
        "US",
        "Florida",
    ),
    ("Lake Erie, Ohio waters", 41.7500, -81.6000, "US", "Ohio"),
    ("Detroit, MI", 42.3314, -83.0458, "US", "Michigan"),
    ("Windsor, ON", 42.3149, -83.0364, "CA", "Ontario"),
    ("Point Roberts, WA", 48.9850, -123.0600, "US", "Washington"),
    ("Fiji, by the antimeridian", -16.7000, 179.9500, "FJ", ""),
    ("London", 51.5074, -0.1278, "GB", ""),
    ("Paris", 48.8566, 2.3522, "FR", ""),
    (
        "near Camp Lejeune, on the New River",
        34.6600,
        -77.3500,
        "US",
        "North Carolina",
    ),
    (
        "Kyrenia (Northern Cyprus, no code)",
        35.3400,
        33.3200,
        "",
        "",
    ),
    ("Lake Champlain", 44.4800, -73.2800, "US", ""),
    ("mid-Atlantic", 30.0000, -40.0000, "", ""),
];

#[tokio::test]
#[ignore = "needs the real data file: set GEODATA_FILE"]
async fn real_places() {
    let path = std::env::var("GEODATA_FILE").expect("set GEODATA_FILE to the .gpkg to check");
    let geocoder = Geocoder::open(path.as_ref()).await.unwrap();
    println!(
        "data {}: {}",
        geocoder.info().data_version,
        geocoder.info().attribution
    );
    let mut wrong = Vec::new();
    for &(what, lat, lon, country, region) in PLACES {
        let started = std::time::Instant::now();
        let found = geocoder
            .lookup(Coordinates::from_degrees(lat, lon).unwrap())
            .await
            .unwrap();
        let took = started.elapsed();
        println!(
            "{what:38} → {}, {}, {} ({}){} · {} · city {:.1} km · {:.1} ms",
            found.place.city.as_str(),
            found.place.region.as_str(),
            found.place.country.as_str(),
            found.country_name,
            if found.inside_country {
                ""
            } else {
                " [nearest]"
            },
            found
                .time_zone
                .as_ref()
                .map_or("no zone", |zone| zone.as_str()),
            found.city_distance_m / 1000.0,
            took.as_secs_f64() * 1000.0,
        );
        let country_ok = country.is_empty() || found.place.country.as_str() == country;
        let region_ok = region.is_empty() || found.place.region.as_str() == region;
        if !(country_ok && region_ok) {
            wrong.push(what);
        }
    }
    assert!(wrong.is_empty(), "unexpected places: {wrong:?}");
    let countries = geocoder.countries().await.unwrap();
    let name_of = |code: &str| {
        countries
            .iter()
            .find(|(each, _)| each.as_str() == code)
            .map(|(_, name)| name.as_str())
    };
    println!(
        "{} countries; FR = {:?}, AU = {:?}",
        countries.len(),
        name_of("FR"),
        name_of("AU")
    );
    assert_eq!(name_of("FR"), Some("France"));
    assert_eq!(name_of("AU"), Some("Australia"));
}
