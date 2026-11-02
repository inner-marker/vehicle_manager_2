//! Saved places against a real database (decision #059).

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod places {
    use domain::label::Label;
    use domain::location::{Coordinates, Place, SavedPlace, SavedPlacePatch};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use storage::commands::RecordWriter;
    use storage::places::SavedPlaceWriter;
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const STUART: u32 = 1;
    const SAM: u32 = 2;

    /// A database with two people.
    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        for (n, username) in [(STUART, "stuart"), (SAM, "sam")] {
            let user = User {
                username: Username::parse(username).unwrap(),
                display_name: username.into(),
                disabled: false,
                must_change_password: false,
            };
            UserWriter
                .create(&mut connection, test_id(n), &UserPatch::all(user), 1)
                .await
                .unwrap();
        }
        drop(connection);
        (temp, database)
    }

    fn place(owner: u32, name: &str, lat: f64, lon: f64) -> SavedPlace {
        SavedPlace {
            owner: test_id(owner),
            name: Label::parse(name).unwrap(),
            coordinates: Coordinates::from_degrees(lat, lon).unwrap(),
            radius_m: 150,
            place: Place::parse("Asheville", "North Carolina", "US").unwrap(),
        }
    }

    async fn create(database: &Database, n: u32, saved: &SavedPlace) {
        let mut connection = database.pool().acquire().await.unwrap();
        SavedPlaceWriter
            .create(
                &mut connection,
                test_id(n),
                &SavedPlacePatch::all(saved.clone()),
                2,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn saved_places_round_trip_and_belong_to_their_owner() {
        let (_temp, database) = database().await;
        let home = place(STUART, "home garage", 35.5951, -82.5515);
        let shop = place(STUART, "Joe's Auto", 35.6, -82.56);
        let sams = place(SAM, "Sam's work", 35.7, -82.6);
        create(&database, 10, &home).await;
        create(&database, 11, &shop).await;
        create(&database, 12, &sams).await;

        // Stuart's, by name ignoring case ("home" before "Joe's"; byte order would put the
        // capital J first); never Sam's.
        let mine = database.saved_places_of(test_id(STUART)).await.unwrap();
        assert_eq!(mine, vec![(test_id(10), home.clone()), (test_id(11), shop)]);
        assert_eq!(
            database.saved_places_of(test_id(SAM)).await.unwrap().len(),
            1
        );
        assert_eq!(database.saved_place(test_id(10)).await.unwrap(), Some(home));
        assert_eq!(database.saved_place(test_id(99)).await.unwrap(), None);
    }

    #[tokio::test]
    async fn an_update_changes_only_what_it_names_and_never_the_owner() {
        let (_temp, database) = database().await;
        let home = place(STUART, "Home", 35.5951, -82.5515);
        create(&database, 10, &home).await;
        let mut connection = database.pool().acquire().await.unwrap();

        let renamed = SavedPlacePatch {
            name: Some(Label::parse("Home garage").unwrap()),
            owner: Some(test_id(SAM)),
            ..SavedPlacePatch::default()
        };
        SavedPlaceWriter
            .update(&mut connection, test_id(10), &renamed, 3)
            .await
            .unwrap();
        let after = database.saved_place(test_id(10)).await.unwrap().unwrap();
        assert_eq!(after.name.as_str(), "Home garage");
        assert_eq!(after.owner, test_id(STUART), "the owner is fixed");
        assert_eq!(after.coordinates, home.coordinates, "unchanged");

        // Moving it: the coordinates and the place each change as a whole.
        let moved = SavedPlacePatch {
            coordinates: Some(Coordinates::from_degrees(36.2168, -81.6746).unwrap()),
            radius_m: Some(300),
            place: Some(Place::parse("Boone", "North Carolina", "US").unwrap()),
            ..SavedPlacePatch::default()
        };
        SavedPlaceWriter
            .update(&mut connection, test_id(10), &moved, 4)
            .await
            .unwrap();
        let after = database.saved_place(test_id(10)).await.unwrap().unwrap();
        assert_eq!(after.coordinates.lat_e7(), 362_168_000);
        assert_eq!(after.radius_m, 300);
        assert_eq!(after.place.city.as_str(), "Boone");
        assert_eq!(after.name.as_str(), "Home garage", "unchanged");
    }

    #[tokio::test]
    async fn a_removed_place_is_gone() {
        let (_temp, database) = database().await;
        create(&database, 10, &place(STUART, "Home", 35.0, -82.0)).await;
        let mut connection = database.pool().acquire().await.unwrap();
        SavedPlaceWriter
            .delete(&mut connection, test_id(10), 3)
            .await
            .unwrap();
        assert_eq!(database.saved_place(test_id(10)).await.unwrap(), None);
        assert!(
            database
                .saved_places_of(test_id(STUART))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_place_needs_a_real_owner() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let nobody = place(77, "Home", 35.0, -82.0);
        let result = SavedPlaceWriter
            .create(
                &mut connection,
                test_id(10),
                &SavedPlacePatch::all(nobody),
                2,
            )
            .await;
        assert!(result.is_err(), "the owner's foreign key");
    }

    #[tokio::test]
    async fn the_table_refuses_values_out_of_range() {
        let (_temp, database) = database().await;
        // Straight SQL, past the domain's checks: the table's own CHECKs refuse these.
        let insert = |n: u32, lat: i64, radius: i64, country: &'static str| {
            let pool = database.pool().clone();
            async move {
                sqlx::query(
                    "INSERT INTO saved_places (id, owner_id, name, lat_e7, lon_e7, radius_m,
                                               city, region, country_code, sequence)
                     VALUES (?, ?, 'Home', ?, 0, ?, 'A', 'B', ?, 1)",
                )
                .bind(test_id::<SavedPlace>(n).to_bytes().to_vec())
                .bind(test_id::<User>(STUART).to_bytes().to_vec())
                .bind(lat)
                .bind(radius)
                .bind(country)
                .execute(&pool)
                .await
            }
        };
        assert!(
            insert(1, 350_000_000, 150, "US").await.is_ok(),
            "a good row"
        );
        assert!(insert(2, 900_000_001, 150, "US").await.is_err(), "latitude");
        assert!(insert(3, 0, 10, "US").await.is_err(), "radius under 25 m");
        assert!(insert(4, 0, 2_001, "US").await.is_err(), "radius over 2 km");
        assert!(insert(5, 0, 150, "us").await.is_err(), "lower-case country");
        assert!(insert(6, 0, 150, "USA").await.is_err(), "three letters");
    }
}
