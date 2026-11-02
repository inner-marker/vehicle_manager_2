//! Place lookup and each person's saved places, against a real database (decision #059).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod places {
    use crate::common::{NOW, setup};
    use domain::api::PlacesOutcome;
    use domain::location::{Coordinates, LocationError, SavedPlaceForm, SavedPlaceId};
    use domain::test_support::ManualClock;
    use geocoder::Geocoder;
    use server::health::{Health, HealthProblem};
    use server::places::{PlacesContext, add, countries, data_info, delete, edit, list, locate};
    use server::sessions::SignedIn;
    use storage::Database;

    fn context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
        geocoder: Option<&'a Geocoder>,
    ) -> PlacesContext<'a> {
        PlacesContext {
            database,
            user,
            health: Health::Healthy,
            clock,
            geocoder,
        }
    }

    fn form(name: &str, lat: &str, lon: &str) -> SavedPlaceForm {
        SavedPlaceForm {
            name: name.into(),
            latitude: lat.into(),
            longitude: lon.into(),
            radius_m: 150,
            city: "Alpha".into(),
            region: "South".into(),
            country: "AA".into(),
        }
    }

    fn added(outcome: &PlacesOutcome) -> SavedPlaceId {
        let PlacesOutcome::Added { id } = outcome else {
            panic!("expected a new place, got {outcome:?}");
        };
        *id
    }

    fn names(outcome: PlacesOutcome) -> Vec<String> {
        let PlacesOutcome::Places { places } = outcome else {
            panic!("expected places, got {outcome:?}");
        };
        places
            .into_iter()
            .map(|(_, place)| place.name.as_str().to_owned())
            .collect()
    }

    /// The small test map from `geocoder::test_map`, opened.
    async fn test_map() -> (tempfile::TempDir, Geocoder) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("geodata-test.gpkg");
        geocoder::test_map::write(&path, geocoder::FORMAT)
            .await
            .unwrap();
        let geocoder = Geocoder::open(&path).await.unwrap();
        (temp, geocoder)
    }

    #[tokio::test]
    async fn each_person_has_their_own_saved_places() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        // No role or mode is needed: Driver mode adds its own places.
        let as_stuart = context(&database, &stuart, &clock, None);
        let as_sam = context(&database, &sam, &clock, None);
        let home = added(&add(&as_stuart, &form("Home garage", "2.0", "2.0")).await);
        added(&add(&as_sam, &form("Sam's work", "8.0", "8.0")).await);

        assert_eq!(names(list(&as_stuart).await), ["Home garage"]);
        assert_eq!(names(list(&as_sam).await), ["Sam's work"]);
        // Someone else's place is "not found", whatever is asked.
        let rename = form("Mine now", "2.0", "2.0");
        assert_eq!(edit(&as_sam, home, &rename).await, PlacesOutcome::NotFound);
        assert_eq!(delete(&as_sam, home).await, PlacesOutcome::NotFound);
        assert_eq!(names(list(&as_stuart).await), ["Home garage"]);
    }

    #[tokio::test]
    async fn a_saved_place_is_checked_changed_and_removed() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let ctx = context(&database, &stuart, &clock, None);
        let too_big = SavedPlaceForm {
            radius_m: 5_000,
            ..form("Home", "2.0", "2.0")
        };
        assert_eq!(
            add(&ctx, &too_big).await,
            PlacesOutcome::Invalid {
                problem: LocationError::RadiusOutOfRange
            }
        );
        let id = added(&add(&ctx, &form("Home", "2.0", "2.0")).await);
        let moved = SavedPlaceForm {
            radius_m: 300,
            ..form("Home garage", "2.5", "2.0")
        };
        assert_eq!(edit(&ctx, id, &moved).await, PlacesOutcome::Done);
        let saved = database.saved_place(id).await.unwrap().unwrap();
        assert_eq!(saved.name.as_str(), "Home garage");
        assert_eq!(saved.radius_m, 300);
        assert_eq!(saved.coordinates.lat_e7(), 25_000_000);
        assert_eq!(delete(&ctx, id).await, PlacesOutcome::Done);
        assert!(names(list(&ctx).await).is_empty());
    }

    #[tokio::test]
    async fn changes_are_refused_while_the_server_is_unhealthy() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let unhealthy = PlacesContext {
            health: Health::Unhealthy(HealthProblem::IntegrityCheckFailed),
            ..context(&database, &stuart, &clock, None)
        };
        assert_eq!(
            add(&unhealthy, &form("Home", "2.0", "2.0")).await,
            PlacesOutcome::Unavailable
        );
    }

    fn located(outcome: PlacesOutcome) -> domain::api::Located {
        let PlacesOutcome::Located { located } = outcome else {
            panic!("expected a location, got {outcome:?}");
        };
        *located
    }

    #[tokio::test]
    async fn locating_finds_your_saved_place_and_the_suggested_place() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (_map, geocoder) = test_map().await;
        let as_stuart = context(&database, &stuart, &clock, Some(&geocoder));
        let home = added(&add(&as_stuart, &form("Home garage", "2.0", "2.0")).await);

        // 50 m north of home: inside its 150 m radius.
        let here = Coordinates::from_degrees(2.000_45, 2.0).unwrap();
        let found = located(locate(&as_stuart, here).await);
        assert_eq!(found.saved.map(|(id, _)| id), Some(home));
        let suggestion = found.suggestion.unwrap();
        assert_eq!(suggestion.place.city.as_str(), "Alpha");
        assert_eq!(suggestion.place.region.as_str(), "South");
        assert_eq!(suggestion.place.country.as_str(), "AA");
        assert_eq!(suggestion.data_version, "test-1");
        assert!(suggestion.inside_country);

        // Sam's lookup never matches Stuart's place.
        let as_sam = context(&database, &sam, &clock, Some(&geocoder));
        assert_eq!(located(locate(&as_sam, here).await).saved, None);

        // Without place data, the saved place still matches; there's just no suggestion.
        let no_data = context(&database, &stuart, &clock, None);
        let found = located(locate(&no_data, here).await);
        assert_eq!(found.saved.map(|(id, _)| id), Some(home));
        assert_eq!(found.suggestion, None);
    }

    #[tokio::test]
    async fn countries_and_data_info_come_from_the_place_data() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (_map, geocoder) = test_map().await;
        let PlacesOutcome::Countries { countries: list } =
            countries(&context(&database, &stuart, &clock, Some(&geocoder))).await
        else {
            panic!("expected countries");
        };
        assert_eq!(list.len(), 4);
        assert_eq!(
            countries(&context(&database, &stuart, &clock, None)).await,
            PlacesOutcome::Countries { countries: vec![] }
        );
        let PlacesOutcome::DataInfo { info: Some(info) } = data_info(Some(&geocoder)) else {
            panic!("expected data info");
        };
        assert_eq!(info.data_version, "test-1");
        assert_eq!(data_info(None), PlacesOutcome::DataInfo { info: None });
    }
}
