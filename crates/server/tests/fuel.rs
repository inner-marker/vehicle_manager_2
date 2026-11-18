//! Fuel economy on the server (Phase 5 milestone 5; decision #085): a truck's fuel-ups, logged
//! as a Driver (so still drafts), become stretches and averages; a missed fill-up drops one;
//! a towed vehicle is named; a vehicle the person can't see isn't found.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod fuel {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{AssetsOutcome, EventsOutcome, FuelOutcome};
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::dates::EventDates;
    use domain::event::{EventInput, EventLocation, EventSource, FuelDetails, Reading};
    use domain::fuel::{DropReason, FillPart};
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::health::Health;
    use server::sessions::SignedIn;
    use storage::Database;

    const MILE: i64 = 1_609;
    const GALLON: i64 = 3_785;
    const DAY: i64 = 24 * 60 * 60 * 1_000;

    fn context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> AssetsContext<'a> {
        AssetsContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            clock,
            origin: server::origin::Origin::Online,
        }
    }

    /// A vehicle of `kind` named `name`, and its first meter.
    async fn vehicle(
        database: &Database,
        jordan: &SignedIn,
        clock: &ManualClock,
        kind: &str,
        name: &str,
    ) -> (AssetId, Option<MeterId>) {
        server::groups::ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = SignedIn {
            mode: ActiveMode::Admin,
            ..jordan.clone()
        };
        let form = AssetForm {
            name: name.into(),
            ..AssetForm::new(kind, group)
        };
        let AssetsOutcome::Added { id } =
            add_vehicle(&context(database, &admin, clock), &form).await
        else {
            panic!("{name} wasn't added");
        };
        let AssetsOutcome::Asset { view } =
            vehicle_page(&context(database, &admin, clock), id).await
        else {
            panic!("no page for {name}");
        };
        (id, view.meters.first().map(|(id, _)| *id))
    }

    /// A fuel-up `days_ago` at `miles`, `gallons` bought, full or not.
    fn fuel_up(
        asset: AssetId,
        odometer: MeterId,
        days_ago: i64,
        miles: i64,
        gallons: i64,
        full: bool,
    ) -> EventInput {
        EventInput {
            asset,
            kind: EventKind::Fuel,
            dates: EventDates {
                start: UnixMillis::from_millis(NOW.as_millis() - days_ago * DAY),
                end: None,
                zone: TimeZone::parse("UTC").unwrap(),
            },
            location: EventLocation {
                place: Some(Place::parse("Asheville", "North Carolina", "US").unwrap()),
                coordinates: None,
                accuracy_m: None,
                fix_time: None,
                source: LocationSource::Manual,
                place_name: None,
                saved_place: None,
                geocoder_version: None,
                missing_reason: None,
            },
            readings: vec![Reading {
                meter: odometer,
                value: miles * MILE,
                reset: false,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: Some(FuelDetails {
                volume_ml: gallons * GALLON,
                cost_minor: None,
                currency: CurrencyCode::USD,
                filled_full: full,
                missed_previous: false,
                highway_pct: 50,
                towing_pct: 0,
                hauling_pct: 0,
                towed: vec![],
                reported_economy: None,
            }),
            work: None,
            lifecycle: None,
            reading_override: None,
        }
    }

    #[tokio::test]
    async fn a_trucks_fuel_ups_become_stretches_and_averages() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = vehicle(&database, &jordan, &clock, "truck", "Truck").await;
        let odometer = odometer.unwrap();
        let (camper, _) = vehicle(&database, &jordan, &clock, "trailer", "Camper").await;
        let sams = context(&database, &sam, &clock);
        let mut towing = fuel_up(truck, odometer, 10, 1_600, 25, true);
        if let Some(fuel) = towing.fuel.as_mut() {
            fuel.towing_pct = 80;
            fuel.towed = vec![camper];
        }
        let mut missed = fuel_up(truck, odometer, 5, 2_000, 20, true);
        if let Some(fuel) = missed.fuel.as_mut() {
            fuel.missed_previous = true;
        }
        for input in [
            fuel_up(truck, odometer, 40, 1_000, 15, true),
            fuel_up(truck, odometer, 30, 1_150, 5, false),
            // 1,000 → 1,300 mi on 5 + 10 gallons: 20 mpg.
            fuel_up(truck, odometer, 20, 1_300, 10, true),
            // 1,300 → 1,600 mi on 25 gallons, towing the camper: 12 mpg.
            towing,
            missed,
        ] {
            let saved = server::events::add(&sams, input).await;
            assert!(matches!(saved, EventsOutcome::Saved { .. }), "{saved:?}");
        }

        let FuelOutcome::Fuel { view } = server::fuel::summary(&sams, truck).await else {
            panic!("no fuel economy");
        };
        let summary = &view.summary;
        assert_eq!(summary.stretches.len(), 3);
        let mpg = |meters_per_liter: i64| {
            f64::from(i32::try_from(meters_per_liter).unwrap()) * 3.785_411_784 / 1_609.344
        };
        assert!((mpg(summary.stretches[0].economy().unwrap()) - 20.0).abs() < 0.05);
        assert!((mpg(summary.stretches[1].economy().unwrap()) - 12.0).abs() < 0.05);
        assert_eq!(summary.stretches[2].dropped, Some(DropReason::MissedFillUp));
        assert!(matches!(
            summary.parts[1].1,
            FillPart::Partial { stretch: Some(0) }
        ));
        // Both counted stretches, distance over fuel: 600 mi on 40 gallons.
        let all = summary.averages.all.unwrap();
        assert_eq!(all.stretches, 2);
        assert!((mpg(all.economy) - 15.0).abs() < 0.05);
        // The camper is named; "not towing" is the first stretch.
        assert_eq!(view.towed_names, vec![(camper, "Camper".to_owned())]);
        assert_eq!(summary.breakdowns.towed.len(), 2);
        assert_eq!(
            view.last_fill,
            Some(UnixMillis::from_millis(NOW.as_millis() - 5 * DAY))
        );

        assert_eq!(
            server::fuel::summary(&sams, test_id(777)).await,
            FuelOutcome::NotFound
        );
    }
}
