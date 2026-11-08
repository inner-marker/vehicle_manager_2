-- Final once merged into dev (decision #062).
-- Phase 2 milestone 8 step 8c: attachments (photos and PDFs), official documents, and license
-- plates (plan §3 Attachments, Official asset documents; decisions #036, #037, #065, #066).
--
-- Files live on disk in `data/files/`, each named by its SHA-256 (#065); `stored_files` lists
-- them, and attachments point at them. Dates without a time are 'YYYY-MM-DD' text. Name
-- lengths are checked in characters by the domain; the limits here are in bytes.

-- Every file in the store: written once, never changed.
CREATE TABLE stored_files (
    sha256    TEXT    PRIMARY KEY CHECK (length(sha256) = 64),
    bytes     INTEGER NOT NULL CHECK (bytes > 0),
    stored_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- License plates (#066): the current one has no retired_on.
CREATE TABLE license_plates (
    id          BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id    BLOB    NOT NULL REFERENCES assets (id),
    -- JSON, e.g. {"by":"region","country":"US","region":"NC"}; NULL only for a plate moved
    -- from the identifiers below, until someone chooses one.
    authority   TEXT    CHECK (authority IS NULL OR json_valid(authority)),
    number      TEXT    NOT NULL CHECK (length(number) BETWEEN 1 AND 60),
    issued_on   TEXT    CHECK (length(issued_on) = 10),
    expires_on  TEXT    CHECK (length(expires_on) = 10),
    retired_on  TEXT    CHECK (length(retired_on) = 10),
    notes       TEXT    NOT NULL DEFAULT '',
    added_ms    INTEGER NOT NULL,
    added_by    BLOB    NOT NULL REFERENCES users (id),
    received_ms INTEGER NOT NULL,
    sequence    INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX license_plates_by_asset ON license_plates (asset_id) WHERE deleted = 0;

-- Official documents: a renewal supersedes the one before; status is worked out, not stored.
CREATE TABLE asset_documents (
    id            BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id      BLOB    NOT NULL REFERENCES assets (id),
    -- A profile key (e.g. 'document-registration'), or the household's own name: one or the
    -- other.
    type_key      TEXT    CHECK (length(type_key) BETWEEN 10 AND 60),
    type_name     TEXT    CHECK (length(type_name) BETWEEN 1 AND 160),
    number        TEXT    CHECK (length(number) BETWEEN 1 AND 240),
    issuer        TEXT    CHECK (length(issuer) BETWEEN 1 AND 400),
    issued_on     TEXT    CHECK (length(issued_on) = 10),
    effective_on  TEXT    CHECK (length(effective_on) = 10),
    expires_on    TEXT    CHECK (length(expires_on) = 10),
    supersedes_id BLOB    REFERENCES asset_documents (id),
    plate_id      BLOB    REFERENCES license_plates (id),
    notes         TEXT    NOT NULL DEFAULT '',
    added_ms      INTEGER NOT NULL,
    added_by      BLOB    NOT NULL REFERENCES users (id),
    received_ms   INTEGER NOT NULL,
    sequence      INTEGER NOT NULL,
    deleted       INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    CHECK ((type_key IS NULL) <> (type_name IS NULL))
) STRICT;

CREATE INDEX asset_documents_by_asset ON asset_documents (asset_id) WHERE deleted = 0;

-- Attachments: what a file is, and what it's on. Each is about one vehicle; it may also be on
-- an event (or one of its tasks, by place), a squawk, or a document.
CREATE TABLE attachments (
    id               BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id         BLOB    NOT NULL REFERENCES assets (id),
    owner            TEXT    NOT NULL CHECK (owner IN ('vehicle', 'event', 'squawk', 'document')),
    event_id         BLOB    REFERENCES events (id),
    task_position    INTEGER CHECK (task_position >= 0),
    squawk_id        BLOB    REFERENCES squawks (id),
    document_id      BLOB    REFERENCES asset_documents (id),
    kind             TEXT    NOT NULL CHECK (kind IN ('photo', 'document')),
    -- Files in the store. Not foreign keys: a removed attachment's files are erased once
    -- nothing else uses them (#065), and its row stays as a record of the removal.
    file_sha256      TEXT    NOT NULL CHECK (length(file_sha256) = 64),
    file_bytes       INTEGER NOT NULL CHECK (file_bytes > 0),
    thumbnail_sha256 TEXT    CHECK (length(thumbnail_sha256) = 64),
    thumbnail_bytes  INTEGER CHECK (thumbnail_bytes > 0),
    name             TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 1020),
    -- JSON: a photo's size, pixels, metadata, and original; or a PDF's pages.
    content          TEXT    NOT NULL CHECK (json_valid(content)),
    added_ms         INTEGER NOT NULL,
    added_by         BLOB    NOT NULL REFERENCES users (id),
    received_ms      INTEGER NOT NULL,
    sequence         INTEGER NOT NULL,
    deleted          INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    CHECK ((event_id IS NOT NULL) = (owner = 'event')),
    CHECK ((squawk_id IS NOT NULL) = (owner = 'squawk')),
    CHECK ((document_id IS NOT NULL) = (owner = 'document')),
    CHECK (task_position IS NULL OR owner = 'event'),
    CHECK ((thumbnail_sha256 IS NULL) = (thumbnail_bytes IS NULL))
) STRICT;

CREATE INDEX attachments_by_asset ON attachments (asset_id) WHERE deleted = 0;
CREATE INDEX attachments_by_event ON attachments (event_id) WHERE deleted = 0;
CREATE INDEX attachments_by_squawk ON attachments (squawk_id) WHERE deleted = 0;
CREATE INDEX attachments_by_document ON attachments (document_id) WHERE deleted = 0;

-- Whether Maintainers and Admins may keep originals (#036): the household's record only;
-- NULL means "not chosen" (the built-in default, allowed).
ALTER TABLE preferences ADD COLUMN originals TEXT CHECK (originals IN ('allowed', 'off'));

-- Plates move out of the identifiers (#066): each 'plate' identifier of 1-15 characters
-- becomes the vehicle's first plate record, with no authority yet, added by the household's
-- first user (who set it up). Each new plate gets a UUIDv7 made from now and random bits, and
-- the next sequence number, as does each vehicle whose identifiers change.
CREATE TEMP TABLE moved_plates AS
    SELECT assets.id AS asset_id,
           upper(trim(json_extract(item.value, '$.value'))) AS number,
           row_number() OVER (ORDER BY assets.id, item.key) AS place
    FROM assets, json_each(assets.identifiers) AS item
    WHERE json_extract(item.value, '$.kind') = 'plate'
      AND length(trim(json_extract(item.value, '$.value'))) BETWEEN 1 AND 15;

INSERT INTO license_plates (id, asset_id, authority, number, notes, added_ms, added_by,
                            received_ms, sequence)
    SELECT unhex(printf('%012X', CAST(unixepoch('subsec') * 1000 AS INTEGER))
                 || '7' || substr(hex(randomblob(2)), 1, 3)
                 || '8' || substr(hex(randomblob(8)), 1, 15)),
           moved_plates.asset_id,
           NULL,
           moved_plates.number,
           '',
           CAST(unixepoch('subsec') * 1000 AS INTEGER),
           (SELECT id FROM users ORDER BY id LIMIT 1),
           CAST(unixepoch('subsec') * 1000 AS INTEGER),
           (SELECT last_sequence FROM database_meta) + moved_plates.place
    FROM moved_plates;

CREATE TEMP TABLE changed_assets AS
    SELECT asset_id, row_number() OVER (ORDER BY asset_id) AS place
    FROM (SELECT DISTINCT assets.id AS asset_id
          FROM assets, json_each(assets.identifiers) AS item
          WHERE json_extract(item.value, '$.kind') = 'plate'
            AND length(trim(json_extract(item.value, '$.value'))) BETWEEN 1 AND 15);

UPDATE assets
SET identifiers = (
        SELECT json_group_array(json(item.value))
        FROM (SELECT value FROM json_each(assets.identifiers)
              WHERE NOT (json_extract(value, '$.kind') = 'plate'
                         AND length(trim(json_extract(value, '$.value'))) BETWEEN 1 AND 15)
              ORDER BY key) AS item
    ),
    sequence = (SELECT last_sequence FROM database_meta)
               + (SELECT count(*) FROM moved_plates)
               + (SELECT place FROM changed_assets WHERE changed_assets.asset_id = assets.id)
WHERE id IN (SELECT asset_id FROM changed_assets);

UPDATE database_meta
SET last_sequence = last_sequence
                    + (SELECT count(*) FROM moved_plates)
                    + (SELECT count(*) FROM changed_assets);

DROP TABLE moved_plates;
DROP TABLE changed_assets;
