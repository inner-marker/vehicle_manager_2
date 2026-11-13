-- Final once merged into dev (decision #062).
-- Phase 3 milestone 6 step 6a: the desktop's files database (decision #077).
--
-- `files.sqlite`, beside a desktop's copy of its server's records and encrypted under the same
-- key (SQLCipher): the photos and PDFs it has downloaded, and the ones added on it and not yet
-- uploaded, each by its SHA-256. Its own schema: the records' migrations never run here.

CREATE TABLE files (
    sha256    TEXT    PRIMARY KEY CHECK (length(sha256) = 64),
    content   BLOB    NOT NULL,
    bytes     INTEGER NOT NULL CHECK (bytes > 0),
    -- What it is: a thumbnail or a PDF (downloaded by themselves, never dropped), a photo
    -- (downloaded when opened; dropped first when past the limit), an original (on request),
    -- or an upload waiting to be sent (never dropped).
    kind      TEXT    NOT NULL CHECK (kind IN ('thumbnail', 'pdf', 'photo', 'original', 'upload')),
    added_ms  INTEGER NOT NULL,
    -- When it was last shown: the photos opened longest ago are dropped first.
    opened_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX files_by_kind_and_opened ON files (kind, opened_ms);
