-- Final once merged into dev (decision #062).
-- Phase 3 milestone 6 step 6a: the audit record of erased files (decisions #072, #077).
--
-- A removed attachment now records who removed it and when, and the nightly file check
-- writes a row in `erased_files` for each file it erases (#065's 8 days after nothing uses
-- it): what the file was, what it belonged to, who removed it, and when it was erased. No
-- content. The two new columns travel with the attachment to desktops, like its others;
-- `erased_files` is server-only (never pulled).

ALTER TABLE attachments ADD COLUMN removed_by BLOB CHECK (removed_by IS NULL OR length(removed_by) = 16);
ALTER TABLE attachments ADD COLUMN removed_ms INTEGER;

CREATE TABLE erased_files (
    id            INTEGER PRIMARY KEY,
    sha256        TEXT    NOT NULL CHECK (length(sha256) = 64),
    bytes         INTEGER NOT NULL CHECK (bytes > 0),
    -- Which of the attachment's files it was: the file, its thumbnail, the photo's original,
    -- or its raw metadata.
    part          TEXT    NOT NULL CHECK (part IN ('file', 'thumbnail', 'original', 'metadata')),
    -- The attachment's kind (photo or document) and the file's type (e.g. image/jpeg), as
    -- recorded.
    kind          TEXT    NOT NULL,
    file_type     TEXT,
    name          TEXT    NOT NULL,
    -- The attachment it belonged to, and that attachment's record (owner and its ID).
    attachment_id BLOB    NOT NULL CHECK (length(attachment_id) = 16),
    owner         TEXT    NOT NULL,
    owner_id      BLOB    CHECK (owner_id IS NULL OR length(owner_id) = 16),
    removed_by    BLOB    CHECK (removed_by IS NULL OR length(removed_by) = 16),
    removed_ms    INTEGER,
    erased_ms     INTEGER NOT NULL
) STRICT;

CREATE INDEX erased_files_by_hash ON erased_files (sha256);
