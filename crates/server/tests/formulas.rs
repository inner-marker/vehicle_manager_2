//! Calculated fields and derived meters on the server (Phase 5 milestone 7; decision #087):
//! saving them, with their checks; what a formula uses kept; cost per mile on fuel-ups; the
//! log form's history; a derived meter in what's due; and older meters given keys at startup.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod formulas {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{AssetsOutcome, EventsOutcome, ScheduleOutcome};
    use domain::asset::AssetId;
    use domain::asset_form::{AssetForm, FieldForm, MeterForm};
    use domain::custom_field::FieldKind;
    use domain::dates::EventDates;
    use domain::due::DueStep;
    use domain::event::{EventInput, EventLocation, EventSource, FuelDetails, Reading};
    use domain::formula::{Formula, FormulaError, NoValue, ShowsAs, in_units};
    use domain::label::Label;
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterDimension, MeterDirection, MeterId, MeterUnit};
    use domain::notes::Notes;
    use domain::preferences::{CurrencyCode, Preferences};
    use domain::requirement::{Applicability, ComplianceMode, IntervalRule, Requirement};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use server::assets::{AssetsContext, add as add_vehicle, add_field, add_meter, delete_meter};
    use server::health::Health;
    use server::sessions::SignedIn;
    use storage::Database;

    const MILE: f64 = 1_609.344;
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

    fn in_mode(user: &SignedIn, mode: ActiveMode) -> SignedIn {
        SignedIn {
            mode,
            ..user.clone()
        }
    }

    fn utc() -> TimeZone {
        TimeZone::parse("UTC").unwrap()
    }

    fn miles(n: f64) -> i64 {
        // Whole meters.
        format!("{:.0}", n * MILE).parse().unwrap()
    }

    /// A truck, and its odometer.
    async fn truck(
        database: &Database,
        jordan: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        server::groups::ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(jordan, ActiveMode::Admin);
        let form = AssetForm {
            name: "Truck".into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id, .. } =
            add_vehicle(&context(database, &admin, clock), &form).await
        else {
            panic!("no truck");
        };
        let meters = database.meters_of(id).await.unwrap();
        assert_eq!(meters[0].1.key.as_str(), "odometer", "from the profile");
        (id, meters[0].0)
    }

    fn meter_form(
        name: &str,
        dimension: MeterDimension,
        unit: MeterUnit,
        formula: Option<&str>,
    ) -> MeterForm {
        MeterForm {
            name: name.into(),
            dimension,
            unit,
            precision: 0,
            description: String::new(),
            required_for: vec![],
            direction: MeterDirection::Up,
            reset_to: None,
            formula: formula.map(|text| Formula::parse(text).unwrap()),
        }
    }

    fn calculated_form(label: &str, formula: &str, shows: ShowsAs) -> FieldForm {
        FieldForm {
            shown_on: vec![EventKind::Fuel],
            label: label.into(),
            kind: FieldKind::Calculated {
                formula: Formula::parse(formula).unwrap(),
                shows,
            },
            enabled: true,
            required_for: vec![],
            range: None,
        }
    }

    fn location() -> EventLocation {
        EventLocation {
            place: Some(Place::parse("Asheville", "North Carolina", "US").unwrap()),
            coordinates: None,
            accuracy_m: None,
            fix_time: None,
            source: LocationSource::Manual,
            place_name: None,
            saved_place: None,
            geocoder_version: None,
            missing_reason: None,
        }
    }

    /// An event of `kind` `days_ago`, with `readings` and (a fuel-up) `dollars` spent.
    fn event(
        asset: AssetId,
        kind: EventKind,
        days_ago: i64,
        readings: Vec<(MeterId, i64)>,
        dollars: i64,
    ) -> EventInput {
        EventInput {
            asset,
            kind,
            dates: EventDates {
                start: UnixMillis::from_millis(NOW.as_millis() - days_ago * DAY),
                end: None,
                zone: utc(),
            },
            location: location(),
            readings: readings
                .into_iter()
                .map(|(meter, value)| Reading {
                    meter,
                    value,
                    reset: false,
                })
                .collect(),
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: (kind == EventKind::Fuel).then(|| FuelDetails {
                volume_ml: 40_000,
                cost_minor: Some(dollars * 100),
                currency: CurrencyCode::USD,
                filled_full: true,
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

    fn derived(formula: &str) -> MeterForm {
        meter_form(
            "Service miles",
            MeterDimension::Distance,
            MeterUnit::Miles,
            Some(formula),
        )
    }

    /// Engine hours, and Service miles worked out from it and the odometer.
    async fn service_miles(shop: &AssetsContext<'_>, truck: AssetId) -> (MeterId, MeterId) {
        let hours = meter_form(
            "Engine hours",
            MeterDimension::Duration,
            MeterUnit::Hours,
            None,
        );
        assert_eq!(add_meter(shop, truck, &hours).await, AssetsOutcome::Done);
        assert_eq!(
            add_meter(shop, truck, &derived("odometer + engine-hours × 33 mi/h")).await,
            AssetsOutcome::Done
        );
        let meters = shop.database.meters_of(truck).await.unwrap();
        let find = |key: &str| {
            meters
                .iter()
                .find(|(_, meter)| meter.key.as_str() == key)
                .unwrap()
                .0
        };
        (find("engine-hours"), find("service-miles"))
    }

    #[tokio::test]
    async fn a_derived_meter_is_checked_and_keeps_what_it_uses() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, _) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        assert_eq!(
            add_meter(&shop, truck, &derived("round(odometer, 0)")).await,
            AssetsOutcome::InvalidFormula {
                problem: FormulaError::NotInDerived
            }
        );
        assert_eq!(
            add_meter(&shop, truck, &derived("odometer + tires")).await,
            AssetsOutcome::InvalidFormula {
                problem: FormulaError::UnknownKey {
                    key: "tires".into()
                }
            }
        );
        let required = MeterForm {
            required_for: vec![EventKind::Fuel],
            ..derived("odometer × 2")
        };
        assert!(matches!(
            add_meter(&shop, truck, &required).await,
            AssetsOutcome::Invalid { .. }
        ));
        let (hours_id, _) = service_miles(&shop, truck).await;

        // What it uses can't go, or be made resettable.
        assert_eq!(
            delete_meter(&shop, hours_id).await,
            AssetsOutcome::InUse {
                by: "Service miles".into()
            }
        );
        let resettable = MeterForm {
            reset_to: Some(0),
            ..meter_form(
                "Engine hours",
                MeterDimension::Duration,
                MeterUnit::Hours,
                None,
            )
        };
        assert_eq!(
            server::assets::edit_meter(&shop, hours_id, &resettable).await,
            AssetsOutcome::InUse {
                by: "Service miles".into()
            }
        );
    }

    #[tokio::test]
    async fn what_comes_due_counts_on_a_derived_meter() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let (hours_id, service) = service_miles(&shop, truck).await;
        // A requirement every 1,000 service miles counts on it: 1,000 at the first reading,
        // 1,500 + 10 h × 33 = 1,830 now, 170 left (17%: coming up).
        let sams = context(&database, &sam, &clock);
        for input in [
            event(
                truck,
                EventKind::Reading,
                30,
                vec![(odometer, miles(1_000.0)), (hours_id, 0)],
                0,
            ),
            event(
                truck,
                EventKind::Reading,
                10,
                vec![(odometer, miles(1_500.0)), (hours_id, 36_000)],
                0,
            ),
        ] {
            let saved = server::events::add(&sams, input).await;
            assert!(matches!(saved, EventsOutcome::Saved { .. }), "{saved:?}");
        }
        let requirement = Requirement {
            asset: truck,
            name: Label::parse("Service").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![IntervalRule::Meter {
                meter: service,
                every: miles(1_000.0),
                first: None,
            }],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: None,
            warnings: None,
            specs: vec![],
            usages: None,
        };
        assert!(matches!(
            server::schedule::add_requirement(&shop, requirement).await,
            ScheduleOutcome::SavedRequirement { .. }
        ));
        let ScheduleOutcome::Schedule { requirements, .. } =
            server::schedule::view(&sams, truck, &utc()).await
        else {
            panic!("no schedule");
        };
        assert_eq!(
            requirements[0].due.as_ref().map(|due| due.step),
            Some(DueStep::ComingUp)
        );
        // A typed reading of a derived meter is refused.
        let typed = event(
            truck,
            EventKind::Reading,
            5,
            vec![(service, miles(2_000.0))],
            0,
        );
        assert!(matches!(
            server::events::add(&sams, typed).await,
            EventsOutcome::Invalid { .. }
        ));
    }

    #[tokio::test]
    async fn cost_per_mile_shows_on_each_fuel_up() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        assert!(matches!(
            add_field(
                &shop,
                truck,
                &calculated_form("Bad", "cost + volume", ShowsAs::Money)
            )
            .await,
            AssetsOutcome::InvalidFormula {
                problem: FormulaError::Mismatch { .. }
            }
        ));
        let per_mile = calculated_form(
            "Cost per mile",
            "round(cost ÷ distance-since-last(odometer), 2)",
            ShowsAs::MoneyPerDistance,
        );
        assert_eq!(
            add_field(&shop, truck, &per_mile).await,
            AssetsOutcome::Done
        );
        assert_eq!(
            delete_meter(&shop, odometer).await,
            AssetsOutcome::InUse {
                by: "Cost per mile".into()
            }
        );

        let sams = context(&database, &sam, &clock);
        for input in [
            event(
                truck,
                EventKind::Fuel,
                20,
                vec![(odometer, miles(1_000.0))],
                50,
            ),
            // $60 over 400 mi: $0.15 a mile.
            event(
                truck,
                EventKind::Fuel,
                10,
                vec![(odometer, miles(1_400.0))],
                60,
            ),
        ] {
            let saved = server::events::add(&sams, input).await;
            assert!(matches!(saved, EventsOutcome::Saved { .. }), "{saved:?}");
        }
        let EventsOutcome::Events { events } = server::events::list(&sams, truck).await else {
            panic!("no events");
        };
        // Newest first.
        let units = Preferences::default();
        let newest = &events[0].calculated;
        assert_eq!(newest.len(), 1);
        let value = newest[0].value.as_ref().unwrap();
        assert!((in_units(value, &units).unwrap() - 0.15).abs() < 1e-9);
        assert_eq!(events[1].calculated[0].value, Err(NoValue::NoEarlier));
        let EventsOutcome::Event { view } = server::events::view(&sams, events[0].id).await else {
            panic!("no event");
        };
        assert_eq!(view.calculated, events[0].calculated);

        let EventsOutcome::LogContext { history, .. } =
            server::events::log_context(&sams, truck).await
        else {
            panic!("no log context");
        };
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].kind, EventKind::Fuel);
        assert_eq!(history[0].earlier.len(), 2);
    }

    #[tokio::test]
    async fn older_meters_get_their_keys_at_startup() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        // As if from before keys: no key stored.
        sqlx::query("UPDATE meters SET key = NULL")
            .execute(database.pool())
            .await
            .unwrap();
        assert_eq!(
            database.unkeyed_meters().await.unwrap(),
            vec![(odometer, truck)]
        );
        let ensure = || {
            server::formulas::ensure_meter_keys(&database, test_id(901), Health::Healthy, &clock)
        };
        assert_eq!(ensure().await, Ok(1));
        assert!(database.unkeyed_meters().await.unwrap().is_empty());
        assert_eq!(
            database
                .meter(odometer)
                .await
                .unwrap()
                .unwrap()
                .key
                .as_str(),
            "odometer"
        );
        assert_eq!(ensure().await, Ok(0), "nothing more to do");
    }
}
