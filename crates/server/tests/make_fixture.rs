//! Writes each release's migration fixture (`crates/storage/tests/fixtures/v<version>.sqlite`),
//! a small database made by that release through the server's own pipeline, with a few made-up
//! records and no personal data (the fixtures README; the release plans in `phase-2.md` and
//! `phase-3.md`).
//!
//! - **`v0.2.0`:** a truck and every kind of record Phase 2 brought.
//! - **`v0.3.0`:** the same, plus what Phase 3 brought: a paired desktop, changes it pushed
//!   (a fuel-up with two photos), one photo removed from it, and that photo's files erased by
//!   the nightly file check, leaving their audit record.
//!
//! Run once, at the release: `cargo test -p server --test make_fixture -- --ignored`. Each
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
    use domain::device::DeviceName;
    use domain::device_signature::public_key_of;
    use domain::document::{DocumentType, REGISTRATION};
    use domain::event::{
        EventId, EventInput, EventLocation, EventSource, FuelDetails, Part, Reading, ReadingSet,
        Task, WorkDetails,
    };
    use domain::hlc::Hlc;
    use domain::label::Label;
    use domain::lifecycle::{LifecycleDetails, Money, Party, SendOut};
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::operation::{Operation, OperationChange};
    use domain::plate::{Authority, PlateNumber};
    use domain::preferences::CurrencyCode;
    use domain::reference::{RECALL, ReferenceNumber, ReferenceType};
    use domain::requirement::{
        Applicability, CalendarUnit, ComplianceMode, IntervalRule, Requirement,
    };
    use domain::squawk::{Severity, SquawkText};
    use domain::sync::{OperationAnswer, PushOutcome};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use lopdf::{Document, Object, dictionary};
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::attachments::{UploadContext, add as add_attachment, upload};
    use server::events::{add, finish};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::login_limiter::LoginLimiter;
    use server::sessions::{SignedIn, SigningInDevice, sign_in_from};
    use server::sync::{PushContext, push};
    use storage::Database;
    use storage::files::FileStore;
    use storage::sync::schema_version;

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
            origin: server::origin::Origin::Online,
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
        let files = storage::files::Files::Folder(store.clone());
        let upload_context = UploadContext {
            database,
            files: &files,
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
            add_attachment(shop, &storage::files::Files::Folder(store.clone()), input).await,
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

    /// A 640×480 PNG, `red` making each one different.
    fn png_of(red: u8) -> Vec<u8> {
        let image = image::ImageBuffer::from_fn(640, 480, |x, _| {
            image::Rgba([red, 30, 30, if x < 320 { 255 } else { 0 }])
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    /// An operation made `minutes` after `NOW` on the desktop's clock.
    fn operation(n: u32, minutes: i64, change: OperationChange) -> Operation {
        Operation::new(
            test_id(7_000 + n),
            Hlc::new(
                UnixMillis::from_millis(NOW.as_millis() + minutes * 60_000),
                0,
            )
            .unwrap(),
            change,
        )
    }

    /// Sam's desktop, paired (a device key), as signing in on the desktop app does.
    async fn paired_desktop(database: &Database) -> SignedIn {
        let (_, signed_in) = sign_in_from(
            database,
            &LoginLimiter::new(),
            "sam",
            "a long password".into(),
            "laptop",
            NOW,
            SigningInDevice::Desktop {
                public_key: public_key_of(&[5; 32]),
                name: Some(DeviceName::parse("Garage Mac").unwrap()),
            },
        )
        .await
        .unwrap();
        signed_in
    }

    /// `bytes` uploaded by `user` as a small photo, as the desktop does before a push.
    async fn uploaded_photo(
        database: &Database,
        files: &storage::files::Files,
        user: &SignedIn,
        bytes: Vec<u8>,
    ) -> domain::api::UploadedFile {
        let context = UploadContext {
            database,
            files,
            user,
            health: Health::Healthy,
            now: NOW,
        };
        let UploadOutcome::Uploaded { uploaded } = upload(
            &context,
            UploadPurpose::Photo {
                size: domain::attachment::PhotoSize::Small,
            },
            bytes,
        )
        .await
        else {
            panic!("the photo upload was refused");
        };
        uploaded
    }

    fn photo_on(
        truck: AssetId,
        event: EventId,
        name: &str,
        photo: &domain::api::UploadedFile,
    ) -> AttachmentInput {
        AttachmentInput {
            asset: truck,
            owner: AttachmentOwner::Event { event, task: None },
            name: FileName::from_upload(name).unwrap(),
            file: photo.file.clone(),
            thumbnail: photo.thumbnail.clone(),
            content: Content::Photo {
                size: domain::attachment::PhotoSize::Small,
                width: 640,
                height: 480,
                metadata: domain::attachment::PhotoMetadata::default(),
                original: None,
            },
        }
    }

    /// Phase 3: Sam's paired desktop pushes a fuel-up with two photos, then removes one; past
    /// #065's 8 days, the nightly file check erases its files and keeps their audit record.
    async fn desktop_changes(
        database: &Database,
        store: &FileStore,
        snapshots: &Path,
        truck: AssetId,
        odometer: MeterId,
    ) {
        let laptop = paired_desktop(database).await;
        let files = storage::files::Files::Folder(store.clone());
        let clock = ManualClock::new(NOW);
        let pushing = PushContext {
            database,
            database_id: test_id(900),
            user: &laptop,
            health: Health::Healthy,
            clock: &clock,
            files: Some(&files),
        };
        let mut fuel_up = event(truck, odometer, EventKind::Fuel, 5, 52_300);
        fuel_up.fuel = Some(FuelDetails {
            volume_ml: 64_000,
            cost_minor: Some(22_900),
            currency: CurrencyCode::USD,
            filled_full: true,
            missed_previous: false,
            highway_pct: 80,
            towing_pct: 0,
            hauling_pct: 0,
            towed: vec![],
            reported_economy: None,
        });
        let logged = operation(1, 1, OperationChange::AddEvent { input: fuel_up });
        let event: EventId = logged.derived_id(0).unwrap();
        let kept = uploaded_photo(database, &files, &laptop, png_of(200)).await;
        let gone = uploaded_photo(database, &files, &laptop, png_of(100)).await;
        let kept = operation(
            2,
            2,
            OperationChange::AddAttachment {
                input: photo_on(truck, event, "Pump.png", &kept),
            },
        );
        let added = operation(
            3,
            3,
            OperationChange::AddAttachment {
                input: photo_on(truck, event, "Receipt.png", &gone),
            },
        );
        let removed = operation(
            4,
            4,
            OperationChange::RemoveAttachment {
                id: added.derived_id(0).unwrap(),
            },
        );
        let outcome = push(&pushing, schema_version(), &[logged, kept, added, removed]).await;
        let PushOutcome::Answers {
            answers,
            stopped: None,
        } = outcome
        else {
            panic!("the push stopped: {outcome:?}");
        };
        assert!(
            answers
                .iter()
                .all(|answer| matches!(answer, OperationAnswer::Applied { .. })),
            "{answers:?}"
        );
        erase_unused_files(database, store, snapshots).await;
        assert_eq!(
            database.erased_files().await.unwrap().len(),
            2,
            "the removed photo and its thumbnail"
        );
    }

    /// Runs the nightly file check past #065's 8 days, so removed files are erased.
    async fn erase_unused_files(database: &Database, store: &FileStore, snapshots: &Path) {
        let clock = std::sync::Arc::new(ManualClock::new(NOW));
        let jobs = server::jobs::JobContext {
            database: database.clone(),
            clock: std::sync::Arc::clone(&clock)
                as std::sync::Arc<dyn domain::time::Clock + Send + Sync>,
            snapshot_dir: server::jobs::SnapshotDir {
                path: snapshots.to_path_buf(),
                create_if_missing: true,
            },
            timers: domain::timers::Timers::NORMAL,
            server_device: test_id(901),
            file_store: Some(store.clone()),
        };
        server::jobs::run_due_jobs(&jobs, NOW).await;
        let later = UnixMillis::from_millis(
            NOW.as_millis()
                + server::jobs::keep_unused_files_for(domain::timers::Timers::NORMAL)
                + DAY,
        );
        clock.set(later);
        database
            .bring_job_forward(server::jobs::Job::FileCheck.name(), later)
            .await
            .unwrap();
        server::jobs::run_due_jobs(&jobs, later).await;
    }

    /// The fixture for `version` at `crates/storage/tests/fixtures/`, refusing one already there.
    fn fixture_path(version: &str) -> std::path::PathBuf {
        let target = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../storage/tests/fixtures/v{version}.sqlite"));
        assert!(
            !target.exists(),
            "{} exists already: a fixture is never edited",
            target.display()
        );
        target
    }

    /// Writes the database to `target`, compacted, and closes it.
    async fn write_to(database: Database, target: &Path) {
        sqlx::query("VACUUM INTO ?")
            .bind(target.display().to_string())
            .execute(database.pool())
            .await
            .unwrap();
        database.close().await;
        println!("wrote {}", target.display());
    }

    #[tokio::test]
    #[ignore = "writes the v0.2.0 migration fixture; run once at the release"]
    async fn write_the_v0_2_0_fixture() {
        let target = fixture_path("0.2.0");
        let (temp, database, jordan, sam) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let (truck, odometer) = truck(&database, &admin, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let driver = context(&database, &sam, &clock);
        everyday(&shop, &driver, truck, odometer).await;
        papers(&shop, &database, &store, &maintainer, truck).await;
        recall_and_send_out(&shop, truck, odometer).await;
        write_to(database, &target).await;
    }

    #[tokio::test]
    #[ignore = "writes the v0.3.0 migration fixture; run once at the release"]
    async fn write_the_v0_3_0_fixture() {
        let target = fixture_path("0.3.0");
        let (temp, database, jordan, sam) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let (truck, odometer) = truck(&database, &admin, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let driver = context(&database, &sam, &clock);
        everyday(&shop, &driver, truck, odometer).await;
        papers(&shop, &database, &store, &maintainer, truck).await;
        recall_and_send_out(&shop, truck, odometer).await;
        Box::pin(desktop_changes(
            &database,
            &store,
            &temp.path().join("snapshots"),
            truck,
            odometer,
        ))
        .await;
        write_to(database, &target).await;
    }
}
