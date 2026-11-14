//! Photos and PDFs on the server: processing, uploads, and who may add, move, see, and remove
//! attachments, against a real database and file store (milestone 8; choices 2, 3, 6, 7).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod attachments {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{
        AssetsOutcome, AttachmentInput, AttachmentPart, AttachmentsOutcome, EventsOutcome,
        PreferencesOutcome, SquawkInput, SquawksOutcome, UploadOutcome, UploadPurpose,
        UploadedFile,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::attachment::{
        AttachmentId, AttachmentOwner, AttachmentProblem, Content, FileName, Original,
        PhotoMetadata, PhotoSize,
    };
    use domain::dates::EventDates;
    use domain::event::{
        EventId, EventInput, EventLocation, EventSource, Reading, ReadingSet, Task, WorkDetails,
    };
    use domain::label::Label;
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::{Choice, CurrencyCode, Originals, PreferencesPatch};
    use domain::squawk::{Severity, SquawkText};
    use domain::test_support::{ManualClock, test_id};
    use image::{ImageBuffer, Rgba};
    use lopdf::{Document, Object, dictionary};
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::attachments::{
        UploadContext, count_pages, download, list, move_to, process_photo, remove, upload,
    };
    use server::events::{add as add_event, correct, edit, finish};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::preferences::{PreferencesContext, set_household};
    use server::sessions::SignedIn;
    use server::squawks;
    use storage::Database;
    use storage::files::{FileStore, Files};

    const MILE: i64 = 1_609;

    /// Adds an attachment with the server's own files (its folder).
    async fn add(
        context: &AssetsContext<'_>,
        store: &FileStore,
        input: AttachmentInput,
    ) -> AttachmentsOutcome {
        server::attachments::add(context, &Files::Folder(store.clone()), input).await
    }

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

    /// A 1600 × 1200 PNG, half transparent (small, so the tests stay quick in debug builds).
    fn png() -> Vec<u8> {
        let image = ImageBuffer::from_fn(1600, 1200, |x, _| {
            Rgba([200_u8, 30, 30, if x < 800 { 255 } else { 0 }])
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    /// A small JPEG carrying an EXIF block (as a camera's would, with its GPS).
    fn camera_jpeg() -> Vec<u8> {
        let image = ImageBuffer::from_fn(640, 480, |_, _| image::Rgb([10_u8, 120, 200]));
        let mut plain = std::io::Cursor::new(Vec::new());
        image
            .write_to(&mut plain, image::ImageFormat::Jpeg)
            .unwrap();
        let plain = plain.into_inner();
        // An APP1 segment: "Exif", then an empty big-endian TIFF directory.
        let tiff = b"MM\0\x2a\0\0\0\x08\0\0\0\0\0\0";
        let mut app1 = b"Exif\0\0".to_vec();
        app1.extend_from_slice(tiff);
        let length = u16::try_from(app1.len() + 2).unwrap().to_be_bytes();
        let mut jpeg = plain[..2].to_vec();
        jpeg.extend_from_slice(&[0xFF, 0xE1, length[0], length[1]]);
        jpeg.extend_from_slice(&app1);
        jpeg.extend_from_slice(&plain[2..]);
        jpeg
    }

    /// A PDF with `pages` blank Letter pages.
    fn pdf(pages: u32) -> Vec<u8> {
        let mut document = Document::with_version("1.5");
        let pages_id = document.new_object_id();
        let kids: Vec<Object> = (0..pages)
            .map(|_| {
                document
                    .add_object(dictionary! {
                        "Type" => "Page",
                        "Parent" => pages_id,
                        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                    })
                    .into()
            })
            .collect();
        document.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => i64::from(pages),
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

    /// A truck and its odometer.
    async fn new_truck(
        database: &Database,
        stuart: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(stuart, ActiveMode::Admin);
        let form = AssetForm {
            name: "Truck".into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id } =
            add_vehicle(&context(database, &admin, clock), &form).await
        else {
            panic!("the truck wasn't added");
        };
        let AssetsOutcome::Asset { view } =
            vehicle_page(&context(database, &admin, clock), id).await
        else {
            panic!("no truck page");
        };
        (id, view.meters[0].0)
    }

    /// A brake job with two tasks, as a draft.
    fn brake_job(asset: AssetId, odometer: MeterId) -> EventInput {
        let task = |title: &str| Task {
            title: Label::parse(title).unwrap(),
            requirement: None,
            notes: Notes::default(),
            parts: vec![],
            labor_seconds: None,
            labor_cost_minor: None,
            resets_from: ReadingSet::Start,
            next_due: None,
        };
        EventInput {
            asset,
            kind: EventKind::Repair,
            dates: EventDates {
                start: NOW,
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
                value: 80_000 * MILE,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: None,
            work: Some(WorkDetails {
                tasks: vec![task("Front pads"), task("Rear pads")],
                end_readings: vec![],
                dispositions: vec![],
                currency: CurrencyCode::USD,
                complaint: None,
                cause: None,
            }),
            lifecycle: None,
            reading_override: None,
        }
    }

    async fn uploaded(
        database: &Database,
        store: &FileStore,
        user: &SignedIn,
        purpose: UploadPurpose,
        bytes: Vec<u8>,
    ) -> UploadedFile {
        let files = Files::Folder(store.clone());
        let upload_context = UploadContext {
            database,
            files: &files,
            user,
            health: Health::Healthy,
            now: NOW,
        };
        let UploadOutcome::Uploaded { uploaded } = upload(&upload_context, purpose, bytes).await
        else {
            panic!("the upload was refused");
        };
        uploaded
    }

    fn photo_input(
        asset: AssetId,
        owner: AttachmentOwner,
        photo: &UploadedFile,
    ) -> AttachmentInput {
        AttachmentInput {
            asset,
            owner,
            name: FileName::from_upload("IMG_0412.HEIC").unwrap(),
            file: photo.file.clone(),
            thumbnail: photo.thumbnail.clone(),
            content: Content::Photo {
                size: PhotoSize::Small,
                // Whatever the app says, the server reads the file.
                width: 1,
                height: 1,
                metadata: PhotoMetadata::default(),
                original: None,
            },
        }
    }

    fn saved(outcome: &AttachmentsOutcome) -> AttachmentId {
        let AttachmentsOutcome::Saved { id } = outcome else {
            panic!("expected a saved attachment, got {outcome:?}");
        };
        *id
    }

    fn event_saved(outcome: &EventsOutcome) -> EventId {
        let EventsOutcome::Saved { id, .. } = outcome else {
            panic!("expected a saved event, got {outcome:?}");
        };
        *id
    }

    fn refused(problem: AttachmentProblem) -> AttachmentsOutcome {
        AttachmentsOutcome::Invalid { problem }
    }

    async fn open_squawk(driver: &AssetsContext<'_>, truck: AssetId) -> domain::squawk::SquawkId {
        let SquawksOutcome::Saved { id } = squawks::open(
            driver,
            SquawkInput {
                asset: truck,
                description: SquawkText::parse("Cracked mirror").unwrap(),
                notes: Notes::default(),
                severity: Severity::Low,
                opened_at: NOW,
                found_on: None,
            },
        )
        .await
        else {
            panic!("no squawk");
        };
        id
    }

    async fn small_photo(database: &Database, store: &FileStore, user: &SignedIn) -> UploadedFile {
        let purpose = UploadPurpose::Photo {
            size: PhotoSize::Small,
        };
        uploaded(database, store, user, purpose, png()).await
    }

    /// Each attachment's owner and details, from a list.
    fn listed(outcome: AttachmentsOutcome) -> Vec<(AttachmentOwner, Content)> {
        let AttachmentsOutcome::Attachments { attachments, .. } = outcome else {
            panic!("no list: {outcome:?}");
        };
        attachments
            .iter()
            .map(|view| (view.attachment.owner, view.attachment.content.clone()))
            .collect()
    }

    /// A receipt (a one-page PDF) for `owner`, uploaded.
    async fn receipt(
        database: &Database,
        store: &FileStore,
        user: &SignedIn,
        truck: AssetId,
        owner: AttachmentOwner,
    ) -> AttachmentInput {
        let file = uploaded(database, store, user, UploadPurpose::Document, pdf(1)).await;
        AttachmentInput {
            asset: truck,
            owner,
            name: FileName::from_upload("Receipt.pdf").unwrap(),
            file: file.file,
            thumbnail: None,
            content: Content::Document { pages: 99 },
        }
    }

    #[test]
    fn photos_are_shrunk_flattened_and_saved_without_their_metadata() {
        let processed = process_photo(&png(), PhotoSize::Small).unwrap();
        assert_eq!(processed.pixels, (1280, 960));
        assert!(processed.jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
        let thumbnail = image::load_from_memory(&processed.thumbnail).unwrap();
        assert_eq!((thumbnail.width(), thumbnail.height()), (400, 300));
        // The transparent half is laid over white.
        let kept = image::load_from_memory(&processed.jpeg).unwrap().to_rgb8();
        assert!(
            kept.get_pixel(1200, 400)
                .0
                .iter()
                .all(|channel| *channel > 240)
        );

        let camera = camera_jpeg();
        assert!(camera.windows(4).any(|window| window == b"Exif"));
        let processed = process_photo(&camera, PhotoSize::Large).unwrap();
        assert_eq!(processed.pixels, (640, 480), "never enlarged");
        assert!(
            !processed.jpeg.windows(4).any(|window| window == b"Exif"),
            "the camera's metadata is gone"
        );
        assert_eq!(
            process_photo(b"\xFF\xD8\xFF not really", PhotoSize::Small),
            Err(AttachmentProblem::Unreadable)
        );
    }

    #[test]
    fn a_pdfs_pages_are_counted() {
        assert_eq!(count_pages(&pdf(3)), Ok(3));
        assert_eq!(
            count_pages(b"%PDF-1.7 broken"),
            Err(AttachmentProblem::Unreadable)
        );
    }

    #[tokio::test]
    async fn uploads_need_a_driver_and_a_file_of_the_right_kind() {
        let (temp, database, stuart, sam) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let photo = uploaded(
            &database,
            &store,
            &sam,
            UploadPurpose::Photo {
                size: PhotoSize::Medium,
            },
            png(),
        )
        .await;
        assert_eq!(photo.pixels, Some((1600, 1200)), "Medium never enlarges");
        assert!(database.is_stored(&photo.file.hash).await.unwrap());
        assert!(store.get(&photo.thumbnail.unwrap().hash).is_ok());

        let pdf_upload =
            uploaded(&database, &store, &stuart, UploadPurpose::Document, pdf(2)).await;
        assert_eq!(pdf_upload.pages, Some(2));

        let refuse = async |user: &SignedIn, purpose, bytes| {
            let files = Files::Folder(store.clone());
            let upload_context = UploadContext {
                database: &database,
                files: &files,
                user,
                health: Health::Healthy,
                now: NOW,
            };
            upload(&upload_context, purpose, bytes).await
        };
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0 ...".to_vec();
        assert_eq!(
            refuse(
                &sam,
                UploadPurpose::Photo {
                    size: PhotoSize::Small
                },
                heic.clone()
            )
            .await,
            UploadOutcome::Invalid {
                problem: AttachmentProblem::HeicPhoto
            }
        );
        assert!(matches!(
            refuse(&sam, UploadPurpose::Original, heic).await,
            UploadOutcome::Uploaded { .. }
        ));
        assert_eq!(
            refuse(&sam, UploadPurpose::Document, png()).await,
            UploadOutcome::Invalid {
                problem: AttachmentProblem::NotAPdf
            }
        );
        let viewer = SignedIn {
            grants: vec![],
            ..sam.clone()
        };
        assert_eq!(
            refuse(&viewer, UploadPurpose::Document, pdf(1)).await,
            UploadOutcome::NotAllowed
        );
    }

    #[tokio::test]
    async fn a_driver_attaches_photos_to_their_squawk_but_not_to_the_vehicle() {
        let (temp, database, stuart, sam) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let (truck, _) = new_truck(&database, &stuart, &clock).await;
        let driver = context(&database, &sam, &clock);
        let squawk = open_squawk(&driver, truck).await;
        let photo = small_photo(&database, &store, &sam).await;
        let on_squawk = AttachmentOwner::Squawk { squawk };
        let id = saved(&add(&driver, &store, photo_input(truck, on_squawk, &photo)).await);
        assert_eq!(
            add(
                &driver,
                &store,
                photo_input(truck, AttachmentOwner::Vehicle, &photo)
            )
            .await,
            AttachmentsOutcome::NotAllowed,
            "a vehicle's own photos are an Admin's"
        );

        let AttachmentsOutcome::Attachments {
            attachments,
            may_add,
            may_keep_originals,
        } = list(&driver, truck, on_squawk).await
        else {
            panic!("no list");
        };
        assert!(may_add);
        assert!(
            !may_keep_originals,
            "only Maintainers and Admins keep originals"
        );
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].added_by_name, "sam");
        let Content::Photo { width, height, .. } = attachments[0].attachment.content else {
            panic!("not a photo");
        };
        assert_eq!((width, height), (1280, 960), "the server read the file");

        let file = download(&driver, &store, id, AttachmentPart::File)
            .await
            .unwrap();
        assert_eq!(file.mime, "image/jpeg");
        assert_eq!(file.bytes, store.get(&photo.file.hash).unwrap());
        assert_eq!(
            download(&driver, &store, id, AttachmentPart::Original).await,
            Err(AttachmentsOutcome::NotAllowed)
        );

        // A file that was never uploaded, or not with that size, is refused.
        let mut forged = photo_input(truck, on_squawk, &photo);
        forged.file.bytes += 1;
        assert_eq!(
            add(&driver, &store, forged).await,
            refused(AttachmentProblem::NotUploaded)
        );

        assert_eq!(remove(&driver, id).await, AttachmentsOutcome::Done);
        assert_eq!(
            download(&driver, &store, id, AttachmentPart::File).await,
            Err(AttachmentsOutcome::NotFound)
        );
    }

    #[tokio::test]
    async fn originals_need_a_maintainer_and_the_households_say_so() {
        let (temp, database, stuart, _) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let job = event_saved(&add_event(&shop, brake_job(truck, odometer)).await);
        let on_job = AttachmentOwner::Event {
            event: job,
            task: None,
        };
        let photo = uploaded(
            &database,
            &store,
            &maintainer,
            UploadPurpose::Photo {
                size: PhotoSize::Small,
            },
            png(),
        )
        .await;
        let heic = b"\0\0\0\x18ftypheic\0\0\0\0 as taken".to_vec();
        let original = uploaded(
            &database,
            &store,
            &maintainer,
            UploadPurpose::Original,
            heic,
        )
        .await;
        let mut input = photo_input(truck, on_job, &photo);
        input.content = Content::Photo {
            size: PhotoSize::Small,
            width: 0,
            height: 0,
            metadata: PhotoMetadata::default(),
            original: Some(Original {
                file: original.file.clone(),
                file_type: original.file_type.unwrap(),
            }),
        };
        let id = saved(&add(&shop, &store, input.clone()).await);
        let kept = download(&shop, &store, id, AttachmentPart::Original)
            .await
            .unwrap();
        assert_eq!(kept.mime, "image/heic");

        // The household turns originals off.
        let admin = in_mode(&stuart, ActiveMode::Admin);
        let preferences = PreferencesContext {
            database: &database,
            database_id: test_id(900),
            user: &admin,
            health: Health::Healthy,
            clock: &clock,
            origin: server::origin::Origin::Online,
        };
        let off = PreferencesPatch {
            originals: Some(Choice::Set(Originals::Off)),
            ..PreferencesPatch::default()
        };
        assert!(matches!(
            set_household(&preferences, off).await,
            PreferencesOutcome::Preferences { .. }
        ));
        assert_eq!(
            add(&shop, &store, input).await,
            refused(AttachmentProblem::OriginalsOff)
        );
    }

    #[tokio::test]
    async fn a_drafts_attachments_follow_its_tasks() {
        let (temp, database, stuart, _) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let job = event_saved(&add_event(&shop, brake_job(truck, odometer)).await);
        let on = |task| AttachmentOwner::Event { event: job, task };
        let input = receipt(&database, &store, &maintainer, truck, on(Some(1))).await;
        let id = saved(&add(&shop, &store, input.clone()).await);
        let beyond = AttachmentInput {
            owner: on(Some(2)),
            ..input.clone()
        };
        assert_eq!(
            add(&shop, &store, beyond).await,
            refused(AttachmentProblem::NoSuchTask)
        );
        assert_eq!(
            listed(list(&shop, truck, on(None)).await),
            [(on(Some(1)), Content::Document { pages: 1 })],
            "the server counted the pages"
        );

        // Moved to the first task, it stays when the second task goes; one left on the second
        // task moves to the job itself.
        assert_eq!(
            move_to(&shop, id, Some(0)).await,
            AttachmentsOutcome::Saved { id }
        );
        let second = saved(&add(&shop, &store, input).await);
        let mut one_task = brake_job(truck, odometer);
        one_task.work.as_mut().unwrap().tasks.truncate(1);
        event_saved(&edit(&shop, job, one_task).await);
        let owners: Vec<_> = listed(list(&shop, truck, on(None)).await)
            .into_iter()
            .map(|(owner, _)| owner)
            .collect();
        assert_eq!(owners.len(), 2);
        assert!(owners.contains(&on(Some(0))) && owners.contains(&on(None)));
        assert!(database.attachment(second).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_finished_jobs_attachments_are_locked_and_its_correction_keeps_them() {
        let (temp, database, stuart, _) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let job = event_saved(&add_event(&shop, brake_job(truck, odometer)).await);
        let on = |task| AttachmentOwner::Event { event: job, task };
        let input = receipt(&database, &store, &maintainer, truck, on(Some(0))).await;
        let id = saved(&add(&shop, &store, input.clone()).await);
        saved(
            &add(
                &shop,
                &store,
                AttachmentInput {
                    owner: on(None),
                    ..input
                },
            )
            .await,
        );

        assert_eq!(finish(&shop, job).await, EventsOutcome::Done);
        assert_eq!(
            remove(&shop, id).await,
            refused(AttachmentProblem::EventFinished)
        );
        let AttachmentsOutcome::Attachments { may_add, .. } = list(&shop, truck, on(None)).await
        else {
            panic!("no list");
        };
        assert!(!may_add);

        let mut corrected = brake_job(truck, odometer);
        corrected.work.as_mut().unwrap().tasks.truncate(1);
        let correction = event_saved(&correct(&shop, job, corrected, "Wrong pads listed").await);
        let in_correction = |task| AttachmentOwner::Event {
            event: correction,
            task,
        };
        let copied: Vec<_> = listed(list(&shop, truck, in_correction(None)).await)
            .into_iter()
            .map(|(owner, _)| owner)
            .collect();
        assert_eq!(copied.len(), 2);
        assert!(copied.contains(&in_correction(Some(0))) && copied.contains(&in_correction(None)));
        assert_eq!(
            listed(list(&shop, truck, on(None)).await).len(),
            2,
            "the original keeps its own"
        );
    }

    #[tokio::test]
    async fn a_recalls_pdf_is_on_no_vehicle_and_read_by_anyone_who_sees_a_linked_one() {
        use domain::api::{ReferenceInput, ReferenceLink, ReferencesOutcome};
        use domain::reference::{RECALL, ReferenceType};
        use domain::requirement::{Applicability, ComplianceMode};

        let (temp, database, stuart, sam) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let (truck, _) = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let maintain = context(&database, &maintainer, &clock);
        let input = ReferenceInput {
            doc_type: ReferenceType::listed(RECALL).unwrap(),
            authority: Some("NHTSA".into()),
            numbers: vec![],
            title: "Rear axle nut may loosen".into(),
            description: Notes::default(),
            remedy: Notes::default(),
            issued_on: None,
            effective_on: None,
            revision: None,
            supersedes: None,
            applicability: Notes::default(),
            url: None,
        };
        let link = ReferenceLink {
            asset: truck,
            name: Label::parse("Axle nut recall").unwrap(),
            mode: ComplianceMode::OneTime,
            applicability: Applicability::Applies,
        };
        let ReferencesOutcome::Saved { id: recall, .. } =
            server::references::add(&maintain, input, link).await
        else {
            panic!("the recall wasn't added");
        };
        let owner = AttachmentOwner::Reference { reference: recall };
        let notice = receipt(&database, &store, &maintainer, truck, owner).await;
        let id = saved(&add(&maintain, &store, notice).await);
        let stored = database.attachment(id).await.unwrap().unwrap();
        assert_eq!(stored.asset, None, "on no vehicle");
        assert_eq!(stored.content, Content::Document { pages: 1 });

        // Sam can see the truck, so he reads the recall and its PDF, but can't change them.
        let driver = context(&database, &sam, &clock);
        let AttachmentsOutcome::Attachments {
            attachments,
            may_add,
            ..
        } = list(&driver, truck, owner).await
        else {
            panic!("no list");
        };
        assert_eq!(attachments.len(), 1);
        assert!(!may_add);
        assert!(
            download(&driver, &store, id, AttachmentPart::File)
                .await
                .is_ok()
        );
        assert_eq!(remove(&driver, id).await, AttachmentsOutcome::NotAllowed);

        // Someone who sees no linked vehicle doesn't find it at all.
        let stranger = SignedIn {
            grants: vec![],
            ..sam.clone()
        };
        let outside = context(&database, &stranger, &clock);
        assert_eq!(
            list(&outside, truck, owner).await,
            AttachmentsOutcome::NotFound
        );
        assert_eq!(
            download(&outside, &store, id, AttachmentPart::File).await,
            Err(AttachmentsOutcome::NotFound)
        );
        assert_eq!(remove(&maintain, id).await, AttachmentsOutcome::Done);
    }
}
