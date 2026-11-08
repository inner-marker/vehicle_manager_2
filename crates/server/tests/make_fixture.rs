//! Writes the `v0.2.0` migration fixture (`crates/storage/tests/fixtures/v0.2.0.sqlite`), a
//! small database made by this release through the server's own pipeline, with a few made-up
//! records and no personal data (the fixtures README; the `v0.2.0` release plan).
//!
//! Run once, at the release: `cargo test -p server --test make_fixture -- --ignored`. It
//! refuses to overwrite a fixture that's already there: once committed, a fixture is never
//! edited.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod make_fixture {
    use std::path::Path;

    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{
        AssetsOutcome, AttachmentInput, DocumentInput, EventsOutcome, PlateInput, ReferenceInput,
        ReferenceLink, ReferencesOutcome, ScheduleOutcome, SquawkInput, SquawksOutcome,
        UploadOutcome, UploadPurpose,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::attachment::{AttachmentOwner, Content, FileName};
    use domain::dates::{CalendarDate, EventDates};
    use domain::document::{DocumentType, REGISTRATION};
    use domain::event::{
        EventId, EventInput, EventLocation, EventSource, FuelDetails, Part, Reading, ReadingSet,
        Task, WorkDetails,
    };
    use domain::label::Label;
    use domain::lifecycle::{LifecycleDetails, Money, Party, SendOut};
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::plate::{Authority, PlateNumber};
    use domain::preferences::CurrencyCode;
    use domain::reference::{RECALL, ReferenceNumber, ReferenceType};
    use domain::requirement::{
        Applicability, CalendarUnit, ComplianceMode, IntervalRule, Requirement,
    };
    use domain::squawk::{Severity, SquawkText};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use lopdf::{Document, Object, dictionary};
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::attachments::{UploadContext, add as add_attachment, upload};
    use server::events::{add, finish};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::sessions::SignedIn;
    use storage::Database;
    use storage::files::FileStore;

    const DAY: i64 = 24 * 60 * 60 * 1000;
    const MILE: i64 = 1_609;

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
        }
    }

    fn in_mode(user: &SignedIn, mode: ActiveMode) -> SignedIn {
        SignedIn {
            mode,
            ..user.clone()
        }
    }

    fn at(days: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + days * DAY)
    }

    /// An event of `kind` on `asset`, `days` after `NOW`, at `miles`.
    fn event(
        asset: AssetId,
        odometer: MeterId,
        kind: EventKind,
        days: i64,
        miles: i64,
    ) -> EventInput {
        EventInput {
            asset,
            kind,
            dates: EventDates {
                start: at(days),
                end: None,
                zone: TimeZone::parse("America/New_York").unwrap(),
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
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::parse("Made-up record for the migration fixture.").unwrap(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: None,
            work: None,
            lifecycle: None,
            reading_override: None,
        }
    }

    fn saved(outcome: &EventsOutcome) -> EventId {
        let EventsOutcome::Saved { id, .. } = outcome else {
            panic!("not saved: {outcome:?}");
        };
        *id
    }

    /// A one-page blank PDF.
    fn pdf() -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let page = document.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page.into()],
                "Count" => 1,
            }),
        );
        let catalog = document.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        document.trailer.set("Root", catalog);
        let mut bytes = Vec::new();
        document.save_to(&mut bytes).unwrap();
        bytes
    }

    /// A truck with an odometer, added by the Admin.
    async fn truck(
        database: &Database,
        admin: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let form = AssetForm {
            name: "Fixture truck".into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id } =
            add_vehicle(&context(database, admin, clock), &form).await
        else {
            panic!("the truck wasn't added");
        };
        let AssetsOutcome::Asset { view } =
            vehicle_page(&context(database, admin, clock), id).await
        else {
            panic!("no truck page");
        };
        (id, view.meters[0].0)
    }

    /// A fuel-up, a requirement and a finished job doing it (with a part), and a squawk.
    async fn everyday(
        shop: &AssetsContext<'_>,
        driver: &AssetsContext<'_>,
        truck: AssetId,
        odometer: MeterId,
    ) {
        let mut fuel_up = event(truck, odometer, EventKind::Fuel, 1, 52_000);
        fuel_up.fuel = Some(FuelDetails {
            volume_ml: 68_000,
            cost_minor: Some(24_500),
            currency: CurrencyCode::USD,
            filled_full: true,
            missed_previous: false,
            highway_pct: 60,
            towing_pct: 0,
            hauling_pct: 0,
            towed: vec![],
            reported_economy: None,
        });
        saved(&add(driver, fuel_up).await);
        let oil = Requirement {
            asset: truck,
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![
                IntervalRule::Meter {
                    meter: odometer,
                    every: 5_000 * MILE,
                },
                IntervalRule::Calendar {
                    every: 6,
                    unit: CalendarUnit::Months,
                    end_of_month: false,
                },
            ],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
        };
        let ScheduleOutcome::SavedRequirement { id: oil } =
            server::schedule::add_requirement(shop, oil).await
        else {
            panic!("the requirement wasn't added");
        };
        let mut job = event(truck, odometer, EventKind::Maintenance, 2, 52_100);
        job.work = Some(WorkDetails {
            tasks: vec![Task {
                title: Label::parse("Engine oil & filter").unwrap(),
                requirement: Some(oil),
                notes: Notes::default(),
                parts: vec![Part {
                    description: "Oil filter".into(),
                    part_number: Some("FL-500S".into()),
                    quantity_milli: 1_000,
                    unit_price_minor: Some(1_299),
                }],
                labor_seconds: Some(1_800),
                labor_cost_minor: None,
                resets_from: ReadingSet::Start,
                next_due: None,
            }],
            end_readings: vec![],
            dispositions: vec![],
            currency: CurrencyCode::USD,
            complaint: None,
            cause: None,
        });
        let job = saved(&add(shop, job).await);
        assert_eq!(finish(shop, job).await, EventsOutcome::Done);
        let squawk = SquawkInput {
            asset: truck,
            description: SquawkText::parse("Rattle from the rear on bumps").unwrap(),
            notes: Notes::default(),
            severity: Severity::Low,
            opened_at: at(3),
            found_on: None,
        };
        assert!(matches!(
            server::squawks::open(driver, squawk).await,
            SquawksOutcome::Saved { .. }
        ));
    }

    /// A registration with its PDF, and a plate.
    async fn papers(
        shop: &AssetsContext<'_>,
        database: &Database,
        store: &FileStore,
        maintainer: &SignedIn,
        truck: AssetId,
    ) {
        let today = CalendarDate::parse("2023-11-15").unwrap();
        assert!(matches!(
            server::documents::add_plate(
                shop,
                PlateInput {
                    asset: truck,
                    authority: Some(Authority::Region {
                        country: "US".into(),
                        region: "NC".into(),
                    }),
                    number: PlateNumber::parse("FIX 0001").unwrap(),
                    issued_on: Some(CalendarDate::parse("2023-01-10").unwrap()),
                    expires_on: None,
                    retired_on: None,
                    notes: Notes::default(),
                },
                &today,
            )
            .await,
            domain::api::DocumentsOutcome::SavedPlate { .. }
        ));
        let domain::api::DocumentsOutcome::SavedDocument { id: document } =
            server::documents::add_document(
                shop,
                DocumentInput {
                    asset: truck,
                    doc_type: DocumentType::listed(REGISTRATION).unwrap(),
                    number: Some("R-00001".into()),
                    issuer: Some("Made-up DMV".into()),
                    issued_on: Some(CalendarDate::parse("2023-01-10").unwrap()),
                    effective_on: None,
                    expires_on: Some(CalendarDate::parse("2024-01-31").unwrap()),
                    plate: None,
                    notes: Notes::default(),
                },
            )
            .await
        else {
            panic!("the document wasn't added");
        };
        let upload_context = UploadContext {
            database,
            store,
            user: maintainer,
            health: Health::Healthy,
            now: NOW,
        };
        let UploadOutcome::Uploaded { uploaded } =
            upload(&upload_context, UploadPurpose::Document, pdf()).await
        else {
            panic!("the upload was refused");
        };
        let input = AttachmentInput {
            asset: truck,
            owner: AttachmentOwner::Document { document },
            name: FileName::from_upload("Registration.pdf").unwrap(),
            file: uploaded.file,
            thumbnail: None,
            content: Content::Document { pages: 1 },
        };
        assert!(matches!(
            add_attachment(shop, store, input).await,
            domain::api::AttachmentsOutcome::Saved { .. }
        ));
    }

    /// A recall with its requirement, and a send-out.
    async fn recall_and_send_out(shop: &AssetsContext<'_>, truck: AssetId, odometer: MeterId) {
        let input = ReferenceInput {
            doc_type: ReferenceType::listed(RECALL).unwrap(),
            authority: Some("Made-up authority".into()),
            numbers: vec![ReferenceNumber {
                scheme: Some(Label::parse("Campaign").unwrap()),
                number: "00V-000".into(),
            }],
            title: "Made-up recall for the fixture".into(),
            description: Notes::default(),
            remedy: Notes::parse("Replace the part.").unwrap(),
            issued_on: Some(CalendarDate::parse("2023-06-01").unwrap()),
            effective_on: None,
            revision: None,
            supersedes: None,
            applicability: Notes::default(),
            url: None,
        };
        let link = ReferenceLink {
            asset: truck,
            name: Label::parse("Made-up recall").unwrap(),
            mode: ComplianceMode::OneTime,
            applicability: Applicability::Applies,
        };
        assert!(matches!(
            server::references::add(shop, input, link).await,
            ReferencesOutcome::Saved { .. }
        ));
        let mut sent = event(truck, odometer, EventKind::SentOut, 4, 52_200);
        sent.readings = vec![];
        sent.lifecycle = Some(LifecycleDetails::SentOut(SendOut {
            shop: Party {
                name: "Made-up Garage".into(),
                contact: None,
            },
            work_order: Some("WO-0001".into()),
            expected_back: Some(CalendarDate::parse("2023-11-30").unwrap()),
            carrier: None,
            tracking: None,
            estimate: Some(Money {
                minor: 40_000,
                currency: CurrencyCode::USD,
            }),
        }));
        saved(&add(shop, sent).await);
    }

    #[tokio::test]
    #[ignore = "writes the v0.2.0 migration fixture; run once at the release"]
    async fn write_the_v0_2_0_fixture() {
        let target =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../storage/tests/fixtures/v0.2.0.sqlite");
        assert!(
            !target.exists(),
            "{} exists already: a fixture is never edited",
            target.display()
        );
        let (temp, database, stuart, sam) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&stuart, ActiveMode::Admin);
        let (truck, odometer) = truck(&database, &admin, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let driver = context(&database, &sam, &clock);
        everyday(&shop, &driver, truck, odometer).await;
        papers(&shop, &database, &store, &maintainer, truck).await;
        recall_and_send_out(&shop, truck, odometer).await;

        sqlx::query("VACUUM INTO ?")
            .bind(target.display().to_string())
            .execute(database.pool())
            .await
            .unwrap();
        database.close().await;
        println!("wrote {}", target.display());
    }
}
