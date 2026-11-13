//! Official documents and license plates on the server, against a real database through the
//! command pipeline (milestone 8; choices 5, 7; decision #066).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod documents {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{
        AssetsOutcome, AttachmentInput, AttachmentsOutcome, DocumentInput, DocumentView,
        DocumentsOutcome, PlateInput, PlateView, UploadOutcome, UploadPurpose,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::attachment::{AttachmentOwner, Content, FileName};
    use domain::dates::CalendarDate;
    use domain::document::{
        AssetDocumentId, DocumentProblem, DocumentStatus, DocumentType, REGISTRATION,
    };
    use domain::notes::Notes;
    use domain::plate::{Authority, LicensePlateId, PlateNumber, PlateProblem};
    use domain::test_support::{ManualClock, test_id};
    use server::assets::{AssetsContext, add as add_vehicle};
    use server::attachments::{UploadContext, upload};
    use server::documents::{
        add_document, add_plate, edit_plate, list, remove_document, remove_plate, renew,
    };
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::sessions::SignedIn;
    use storage::Database;
    use storage::files::FileStore;

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

    fn date(text: &str) -> CalendarDate {
        CalendarDate::parse(text).unwrap()
    }

    async fn new_truck(database: &Database, stuart: &SignedIn, clock: &ManualClock) -> AssetId {
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
        id
    }

    fn plate_input(asset: AssetId, region: &str, number: &str, issued: &str) -> PlateInput {
        PlateInput {
            asset,
            authority: Some(Authority::region("US", region).unwrap()),
            number: PlateNumber::parse(number).unwrap(),
            issued_on: Some(date(issued)),
            expires_on: None,
            retired_on: None,
            notes: Notes::default(),
        }
    }

    fn registration(asset: AssetId, plate: LicensePlateId, expires: &str) -> DocumentInput {
        DocumentInput {
            asset,
            doc_type: DocumentType::listed(REGISTRATION).unwrap(),
            number: Some("R-55120".into()),
            issuer: Some("NC DMV".into()),
            issued_on: None,
            effective_on: None,
            expires_on: Some(date(expires)),
            plate: Some(plate),
            notes: Notes::default(),
        }
    }

    fn saved_plate(outcome: &DocumentsOutcome) -> LicensePlateId {
        let DocumentsOutcome::SavedPlate { id } = outcome else {
            panic!("expected a saved plate, got {outcome:?}");
        };
        *id
    }

    fn saved_document(outcome: &DocumentsOutcome) -> AssetDocumentId {
        let DocumentsOutcome::SavedDocument { id } = outcome else {
            panic!("expected a saved document, got {outcome:?}");
        };
        *id
    }

    /// The vehicle's documents and plates as of `today`.
    async fn papers(
        context: &AssetsContext<'_>,
        asset: AssetId,
        today: &str,
    ) -> (Vec<DocumentView>, Vec<PlateView>, bool) {
        let DocumentsOutcome::Documents {
            documents,
            plates,
            may_change,
            ..
        } = list(context, asset, &date(today)).await
        else {
            panic!("no documents");
        };
        (documents, plates, may_change)
    }

    #[tokio::test]
    async fn a_maintainer_adds_papers_and_a_driver_only_sees_them() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let driver = context(&database, &sam, &clock);

        let nc = plate_input(truck, "NC", "NCX 4471", "2024-06-01");
        assert_eq!(
            add_plate(&driver, nc.clone(), &date("2026-11-04")).await,
            DocumentsOutcome::NotAllowed
        );
        let plate = saved_plate(&add_plate(&shop, nc, &date("2026-11-04")).await);
        let registered =
            saved_document(&add_document(&shop, registration(truck, plate, "2026-10-31")).await);

        let DocumentsOutcome::Documents { listed_types, .. } =
            list(&driver, truck, &date("2026-10-31")).await
        else {
            panic!("no documents");
        };
        assert!(listed_types.contains(&REGISTRATION.to_owned()));
        let (documents, plates, may_change) = papers(&driver, truck, "2026-10-31").await;
        assert!(!may_change);
        assert_eq!(documents[0].id, registered);
        assert_eq!(
            documents[0].status,
            DocumentStatus::Current,
            "good through its last day"
        );
        assert_eq!(
            plates[0].plate.expires_on,
            Some(date("2026-10-31")),
            "the registration set its plate's expiry"
        );
        let (documents, _, _) = papers(&driver, truck, "2026-11-01").await;
        assert_eq!(documents[0].status, DocumentStatus::Expired);

        // Only a registration names a plate.
        let mut insurance = registration(truck, plate, "2027-05-01");
        insurance.doc_type = DocumentType::listed("document-insurance").unwrap();
        assert_eq!(
            add_document(&shop, insurance).await,
            DocumentsOutcome::InvalidDocument {
                problem: DocumentProblem::PlateNotRegistration
            }
        );
    }

    #[tokio::test]
    async fn renewals_supersede_once_and_the_latest_goes_first() {
        let (_temp, database, stuart, _) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let plate = saved_plate(
            &add_plate(
                &shop,
                plate_input(truck, "NC", "NCX 4471", "2024-06-01"),
                &date("2026-11-04"),
            )
            .await,
        );
        let old =
            saved_document(&add_document(&shop, registration(truck, plate, "2026-10-31")).await);
        let new =
            saved_document(&renew(&shop, old, registration(truck, plate, "2027-10-31")).await);

        let (documents, plates, _) = papers(&shop, truck, "2026-11-04").await;
        let found = |id| documents.iter().find(|view| view.id == id).unwrap();
        assert_eq!(found(old).status, DocumentStatus::Superseded);
        assert_eq!(found(old).renewed_by, Some(new));
        assert_eq!(found(new).status, DocumentStatus::Current);
        assert_eq!(plates[0].plate.expires_on, Some(date("2027-10-31")));
        assert_eq!(
            renew(&shop, old, registration(truck, plate, "2028-10-31")).await,
            DocumentsOutcome::InvalidDocument {
                problem: DocumentProblem::AlreadyRenewed
            }
        );

        assert_eq!(
            remove_document(&shop, old).await,
            DocumentsOutcome::InvalidDocument {
                problem: DocumentProblem::Renewed
            }
        );
        assert_eq!(
            remove_plate(&shop, plate).await,
            DocumentsOutcome::InvalidPlate {
                problem: PlateProblem::InUse
            }
        );
        assert_eq!(remove_document(&shop, new).await, DocumentsOutcome::Done);
        assert_eq!(remove_document(&shop, old).await, DocumentsOutcome::Done);
        assert_eq!(remove_plate(&shop, plate).await, DocumentsOutcome::Done);
        let (documents, plates, _) = papers(&shop, truck, "2026-11-04").await;
        assert!(documents.is_empty() && plates.is_empty());
    }

    #[tokio::test]
    async fn a_new_plate_retires_the_old_one_and_keeps_an_authority() {
        let (_temp, database, stuart, _) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let today = date("2026-11-04");
        let nc = saved_plate(
            &add_plate(
                &shop,
                plate_input(truck, "NC", "NCX 4471", "2024-06-01"),
                &today,
            )
            .await,
        );
        let va = saved_plate(
            &add_plate(
                &shop,
                plate_input(truck, "VA", "UVA 1819", "2026-10-15"),
                &today,
            )
            .await,
        );

        let (_, plates, _) = papers(&shop, truck, "2026-11-04").await;
        let found = |id| plates.iter().find(|view| view.id == id).unwrap();
        assert!(found(va).current);
        assert!(!found(nc).current);
        assert_eq!(found(nc).plate.retired_on, Some(date("2026-10-15")));
        assert_eq!(
            plates.last().unwrap().id,
            va,
            "the current plate comes last"
        );

        let mut unnamed = plate_input(truck, "VA", "ABC 123", "2026-11-01");
        unnamed.authority = None;
        assert_eq!(
            add_plate(&shop, unnamed.clone(), &today).await,
            DocumentsOutcome::InvalidPlate {
                problem: PlateProblem::NeedsAuthority
            }
        );
        assert_eq!(
            edit_plate(&shop, va, unnamed).await,
            DocumentsOutcome::InvalidPlate {
                problem: PlateProblem::NeedsAuthority
            },
            "a plate with an authority keeps one"
        );
        let mut corrected = plate_input(truck, "VA", "UVA 1820", "2026-10-15");
        corrected.notes = Notes::parse("Vanity plate").unwrap();
        assert_eq!(
            edit_plate(&shop, va, corrected).await,
            DocumentsOutcome::SavedPlate { id: va }
        );
        let (_, plates, _) = papers(&shop, truck, "2026-11-04").await;
        assert_eq!(plates.last().unwrap().plate.number.as_str(), "UVA 1820");
    }

    #[tokio::test]
    async fn removing_a_document_removes_its_scans() {
        let (temp, database, stuart, _) = setup().await;
        let store = FileStore::open(temp.path()).unwrap();
        let clock = ManualClock::new(NOW);
        let truck = new_truck(&database, &stuart, &clock).await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let mut title = registration(truck, test_id(1), "2099-01-01");
        title.doc_type = DocumentType::listed("document-title").unwrap();
        title.plate = None;
        title.expires_on = None;
        let document = saved_document(&add_document(&shop, title).await);

        let image = image::ImageBuffer::from_fn(800, 600, |_, _| image::Rgb([240_u8, 240, 240]));
        let mut png = std::io::Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        let upload_context = UploadContext {
            database: &database,
            store: &store,
            user: &maintainer,
            health: Health::Healthy,
            now: NOW,
        };
        let UploadOutcome::Uploaded { uploaded } = upload(
            &upload_context,
            UploadPurpose::Photo {
                size: domain::attachment::PhotoSize::Small,
            },
            png.into_inner(),
        )
        .await
        else {
            panic!("no upload");
        };
        let on_document = AttachmentOwner::Document { document };
        let scan = AttachmentInput {
            asset: truck,
            owner: on_document,
            name: FileName::from_upload("Title.jpg").unwrap(),
            file: uploaded.file,
            thumbnail: uploaded.thumbnail,
            content: Content::Photo {
                size: domain::attachment::PhotoSize::Small,
                width: 0,
                height: 0,
                metadata: domain::attachment::PhotoMetadata::default(),
                original: None,
            },
        };
        let AttachmentsOutcome::Saved { id: scan } =
            server::attachments::add(&shop, &storage::files::Files::Folder(store.clone()), scan)
                .await
        else {
            panic!("the scan wasn't added");
        };
        assert_eq!(
            remove_document(&shop, document).await,
            DocumentsOutcome::Done
        );
        assert_eq!(database.attachment(scan).await.unwrap(), None);
    }
}
