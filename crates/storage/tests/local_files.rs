//! The desktop's files database (phase-3.md 6a, decision #077): encrypted like the copy, by
//! SHA-256, photos dropped oldest-opened first past a limit, everything else kept.

#[cfg(test)]
mod local_files {
    #[cfg(feature = "sqlcipher")]
    use domain::time::UnixMillis;
    use storage::local_files::LocalFiles;
    #[cfg(feature = "sqlcipher")]
    use storage::local_files::{FILES_FILE_NAME, LocalKind};
    use storage::{LocalKey, StorageError};

    const KEY: [u8; 32] = [7; 32];

    #[cfg(feature = "sqlcipher")]
    fn at(millis: i64) -> UnixMillis {
        UnixMillis::from_millis(1_800_000_000_000 + millis)
    }

    #[cfg(not(feature = "sqlcipher"))]
    #[tokio::test]
    async fn a_build_without_sqlcipher_refuses_and_writes_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("copy");
        let result = LocalFiles::open(&folder, &LocalKey::new(KEY)).await;
        assert!(
            matches!(result, Err(StorageError::NotEncrypted)),
            "{result:?}"
        );
        assert!(!folder.exists());
    }

    #[cfg(feature = "sqlcipher")]
    #[tokio::test]
    async fn files_are_kept_encrypted_and_opened_with_their_key_only() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("copy");
        let files = LocalFiles::open(&folder, &LocalKey::new(KEY))
            .await
            .unwrap();
        let secret = b"a registration scan, page one".repeat(40);
        let stored = files.put(&secret, LocalKind::Pdf, at(0)).await.unwrap();
        assert_eq!(stored.bytes, u64::try_from(secret.len()).unwrap());
        files.close().await;

        let bytes = std::fs::read(folder.join(FILES_FILE_NAME)).unwrap();
        assert!(!bytes.starts_with(b"SQLite format 3"));
        assert!(
            !bytes.windows(20).any(|window| window == &secret[..20]),
            "the file's contents aren't readable on disk"
        );

        let files = LocalFiles::open(&folder, &LocalKey::new(KEY))
            .await
            .unwrap();
        assert_eq!(files.get(&stored.hash).await.unwrap(), Some(secret.clone()));
        files.close().await;
        let wrong = LocalFiles::open(&folder, &LocalKey::new([8; 32])).await;
        assert!(matches!(wrong, Err(StorageError::WrongKey)), "{wrong:?}");
    }

    #[cfg(feature = "sqlcipher")]
    #[tokio::test]
    async fn only_photos_are_dropped_oldest_opened_first() {
        let temp = tempfile::tempdir().unwrap();
        let files = LocalFiles::open(&temp.path().join("copy"), &LocalKey::new(KEY))
            .await
            .unwrap();
        let file = |n: u8| vec![n; 1_000];
        let old = files.put(&file(1), LocalKind::Photo, at(1)).await.unwrap();
        let newer = files.put(&file(2), LocalKind::Photo, at(2)).await.unwrap();
        let newest = files.put(&file(3), LocalKind::Photo, at(3)).await.unwrap();
        let thumbnail = files
            .put(&file(4), LocalKind::Thumbnail, at(0))
            .await
            .unwrap();
        let upload = files.put(&file(5), LocalKind::Upload, at(0)).await.unwrap();
        // The oldest photo, opened again, is now the newest.
        files.mark_opened(&old.hash, at(10)).await.unwrap();

        let summary = files.summary().await.unwrap();
        assert_eq!(summary.files, 5);
        assert_eq!(summary.bytes, 5_000);
        assert_eq!(summary.photo_bytes, 3_000);
        assert_eq!(summary.uploads, 1);

        assert_eq!(
            files.drop_photos_over(3_000).await.unwrap(),
            0,
            "within the limit"
        );
        assert_eq!(files.drop_photos_over(1_500).await.unwrap(), 2);
        let missing = files
            .missing(&[
                old.hash.clone(),
                newer.hash.clone(),
                newest.hash.clone(),
                thumbnail.hash.clone(),
                upload.hash.clone(),
            ])
            .await
            .unwrap();
        assert_eq!(
            missing,
            vec![newer.hash, newest.hash],
            "the photos opened longest ago"
        );
        assert_eq!(files.drop_photos_over(0).await.unwrap(), 1);
        let summary = files.summary().await.unwrap();
        assert_eq!(summary.photo_bytes, 0);
        assert_eq!(summary.files, 2, "the thumbnail and the upload stay");
    }

    #[cfg(feature = "sqlcipher")]
    #[tokio::test]
    async fn a_file_stored_again_keeps_the_firmer_kind() {
        let temp = tempfile::tempdir().unwrap();
        let files = LocalFiles::open(&temp.path().join("copy"), &LocalKey::new(KEY))
            .await
            .unwrap();
        let bytes = vec![9; 500];
        let upload = files.put(&bytes, LocalKind::Upload, at(0)).await.unwrap();
        files.put(&bytes, LocalKind::Photo, at(1)).await.unwrap();
        assert_eq!(
            files.kind_of(&upload.hash).await.unwrap(),
            Some(LocalKind::Upload)
        );
        // Sent: it becomes the photo it is, and may now be dropped.
        files
            .set_kind(&upload.hash, LocalKind::Photo)
            .await
            .unwrap();
        assert_eq!(
            files.kind_of(&upload.hash).await.unwrap(),
            Some(LocalKind::Photo)
        );
        assert!(files.remove(&upload.hash).await.unwrap());
        assert_eq!(files.get(&upload.hash).await.unwrap(), None);
        assert!(matches!(
            files.put(&[], LocalKind::Photo, at(2)).await,
            Err(StorageError::InvalidValue { .. })
        ));
    }

    #[cfg(feature = "sqlcipher")]
    #[tokio::test]
    async fn an_upload_made_here_waits_with_its_bytes_until_sent() {
        use domain::api::{UploadPurpose, UploadedFile};
        use domain::attachment::{FileType, PhotoSize};

        let temp = tempfile::tempdir().unwrap();
        let files = LocalFiles::open(&temp.path().join("copy"), &LocalKey::new(KEY))
            .await
            .unwrap();
        let jpeg = files
            .put(&[1; 300], LocalKind::Upload, at(0))
            .await
            .unwrap();
        let thumbnail = files
            .put(&[2; 100], LocalKind::Upload, at(0))
            .await
            .unwrap();
        let made = UploadedFile {
            file: jpeg.clone(),
            file_type: Some(FileType::Jpeg),
            thumbnail: Some(thumbnail.clone()),
            pixels: Some((10, 10)),
            pages: None,
        };
        let purpose = UploadPurpose::Photo {
            size: PhotoSize::Small,
        };
        files
            .add_upload(&purpose, b"as picked", &made, at(1))
            .await
            .unwrap();
        for hash in [&jpeg.hash, &thumbnail.hash] {
            let upload = files.upload_making(hash).await.unwrap().unwrap();
            assert_eq!(upload.purpose, purpose);
            assert_eq!(upload.raw, b"as picked");
            assert_eq!(upload.made, made);
        }
        let other = files.put(&[3; 50], LocalKind::Photo, at(0)).await.unwrap();
        assert_eq!(files.upload_making(&other.hash).await.unwrap(), None);
        let id = files.upload_making(&jpeg.hash).await.unwrap().unwrap().id;
        files.remove_upload(id).await.unwrap();
        assert_eq!(files.upload_making(&jpeg.hash).await.unwrap(), None);
        assert_eq!(
            files.summary().await.unwrap().uploads,
            2,
            "the files stay until settled"
        );
    }
}
