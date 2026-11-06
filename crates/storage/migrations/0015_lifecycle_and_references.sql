-- no-transaction
-- Final once merged into dev (decision #062).
-- Phase 2 milestone 9 step 9c: lifecycle events, reference documents, and requirements'
-- compliance (plan §3 Asset lifecycle & components, Requirements & compliance; decisions
-- #067, #068).
--
-- Two tables are REBUILT, because SQLite can't change a CHECK in place: `events` (its kind
-- listed only the four everyday kinds) and `attachments` (its owner listed four owners, and
-- every attachment needed a vehicle; a reference document's has none). This follows SQLite's
-- documented procedure (https://www.sqlite.org/lang_altertable.html, "Making Other Kinds Of
-- Table Schema Changes"): foreign keys off, then in one transaction create the new table, copy
-- every row, drop the old one, rename the new one, recreate its indexes, and check every
-- foreign key before committing. Tables that point at `events` or `attachments` do so by name,
-- so they point at the new tables unchanged. The first line keeps sqlx from wrapping this file
-- in a transaction of its own, since foreign keys can only be turned off outside one.
--
-- From here on, a column that holds a kind, type, owner, or mode gets NO check listing its
-- values: the domain checks them, so a new one never needs a rebuild (the human, 2026-11-05).

PRAGMA foreign_keys = OFF;

BEGIN;

-- --- events: any kind -------------------------------------------------------------------

CREATE TABLE events_new (
    id                 BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id           BLOB    NOT NULL REFERENCES assets (id),
    -- The domain checks the kind, so a new one needs no rebuild (milestone 9).
    kind               TEXT    NOT NULL CHECK (length(kind) BETWEEN 1 AND 40),
    start_ms           INTEGER NOT NULL,
    end_ms             INTEGER CHECK (end_ms >= start_ms),
    zone               TEXT    NOT NULL CHECK (length(zone) BETWEEN 1 AND 64),

    -- Where. The place is all three or none (none only with a reason, and flagged).
    city               TEXT    CHECK (length(city) BETWEEN 1 AND 400),
    region             TEXT    CHECK (length(region) BETWEEN 1 AND 400),
    country_code       TEXT    CHECK (length(country_code) = 2 AND country_code = upper(country_code)),
    lat_e7             INTEGER CHECK (lat_e7 BETWEEN -900000000 AND 900000000),
    lon_e7             INTEGER CHECK (lon_e7 BETWEEN -1800000000 AND 1800000000),
    accuracy_m         INTEGER CHECK (accuracy_m >= 0),
    fix_ms             INTEGER,
    location_source    TEXT    NOT NULL CHECK (location_source IN ('gps', 'saved-place', 'map', 'manual')),
    place_name         TEXT    CHECK (length(place_name) BETWEEN 1 AND 160),
    -- A hint only: the saved place's values are copied in above.
    saved_place_id     BLOB    REFERENCES saved_places (id),
    geocoder_version   TEXT    CHECK (length(geocoder_version) BETWEEN 1 AND 40),
    no_place_reason    TEXT    CHECK (length(no_place_reason) BETWEEN 1 AND 2000),

    -- Required readings and fields left out: a JSON array of {"item", "reason"}.
    skipped            TEXT    NOT NULL DEFAULT '[]',
    -- A Markdown subset (#055); empty means none.
    notes              TEXT    NOT NULL DEFAULT '',
    source             TEXT    NOT NULL CHECK (source IN ('typed', 'voice', 'import')),
    logged_by          BLOB    NOT NULL REFERENCES users (id),
    -- Who did it: a user, or a name with an optional contact; or nobody recorded.
    performer_user_id  BLOB    REFERENCES users (id),
    performer_name     TEXT    CHECK (length(performer_name) BETWEEN 1 AND 400),
    performer_contact  TEXT    CHECK (length(performer_contact) BETWEEN 1 AND 400),
    reading_override   TEXT    CHECK (length(reading_override) BETWEEN 1 AND 2000),

    -- Drafts and finalizing (#034), and the groundwork for signatures (sub-plan 01).
    status             TEXT    NOT NULL CHECK (status IN ('draft', 'finalized')),
    last_edited_ms     INTEGER NOT NULL,
    received_ms        INTEGER NOT NULL,
    finalized_ms       INTEGER,
    -- 'user' (Finalize now, with finalized_by_user) or 'server' (24 hours after the last edit).
    finalized_by       TEXT    CHECK (finalized_by IN ('user', 'server')),
    finalized_by_user  BLOB    REFERENCES users (id),
    corrects_id        BLOB    REFERENCES events (id),
    correction_reason  TEXT    CHECK (length(correction_reason) BETWEEN 1 AND 2000),

    sequence           INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),

    CHECK ((city IS NULL) = (region IS NULL) AND (city IS NULL) = (country_code IS NULL)),
    CHECK ((lat_e7 IS NULL) = (lon_e7 IS NULL)),
    CHECK (performer_name IS NULL OR performer_user_id IS NULL),
    CHECK ((status = 'finalized') = (finalized_ms IS NOT NULL)),
    CHECK ((finalized_ms IS NULL) = (finalized_by IS NULL)),
    CHECK ((finalized_by = 'user') = (finalized_by_user IS NOT NULL))
) STRICT;

INSERT INTO events_new (
    id, asset_id, kind, start_ms, end_ms, zone, city, region, country_code, lat_e7, lon_e7,
    accuracy_m, fix_ms, location_source, place_name, saved_place_id, geocoder_version,
    no_place_reason, skipped, notes, source, logged_by, performer_user_id, performer_name,
    performer_contact, reading_override, status, last_edited_ms, received_ms, finalized_ms,
    finalized_by, finalized_by_user, corrects_id, correction_reason, sequence, deleted
)
SELECT
    id, asset_id, kind, start_ms, end_ms, zone, city, region, country_code, lat_e7, lon_e7,
    accuracy_m, fix_ms, location_source, place_name, saved_place_id, geocoder_version,
    no_place_reason, skipped, notes, source, logged_by, performer_user_id, performer_name,
    performer_contact, reading_override, status, last_edited_ms, received_ms, finalized_ms,
    finalized_by, finalized_by_user, corrects_id, correction_reason, sequence, deleted
FROM events;

DROP TABLE events;
ALTER TABLE events_new RENAME TO events;

CREATE INDEX events_by_asset ON events (asset_id, start_ms) WHERE deleted = 0;
CREATE INDEX events_by_correction ON events (corrects_id) WHERE corrects_id IS NOT NULL;
CREATE INDEX events_drafts ON events (last_edited_ms) WHERE status = 'draft' AND deleted = 0;

-- --- Lifecycle events (#067) -------------------------------------------------------------

-- A lifecycle event's details: the domain's `LifecycleDetails` as JSON, e.g.
-- {"kind":"sent-out","shop":{"name":"Joe's Auto","contact":null},...}. A return also names
-- the send-out it ends, so open send-outs can be found.
CREATE TABLE lifecycle_details (
    event_id    BLOB PRIMARY KEY REFERENCES events (id),
    details     TEXT NOT NULL CHECK (json_valid(details)),
    sent_out_id BLOB REFERENCES events (id)
) STRICT, WITHOUT ROWID;

CREATE INDEX lifecycle_details_by_send_out ON lifecycle_details (sent_out_id)
    WHERE sent_out_id IS NOT NULL;

-- --- Reference documents (#068) ----------------------------------------------------------

-- Recalls, bulletins, and schedules, shared by the household. The type is the domain's
-- `ReferenceType` as JSON: {"type":"listed","key":"reference-recall"} or
-- {"type":"custom","name":"Owner notification"}.
CREATE TABLE reference_documents (
    id            BLOB    PRIMARY KEY CHECK (length(id) = 16),
    doc_type      TEXT    NOT NULL CHECK (json_valid(doc_type)),
    authority     TEXT    CHECK (length(authority) BETWEEN 1 AND 400),
    -- A JSON array of {"scheme", "number"}.
    numbers       TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(numbers)),
    title         TEXT    NOT NULL CHECK (length(title) BETWEEN 1 AND 800),
    -- Markdown subsets (#055); empty means none.
    description   TEXT    NOT NULL DEFAULT '',
    remedy        TEXT    NOT NULL DEFAULT '',
    issued_on     TEXT    CHECK (length(issued_on) = 10),
    effective_on  TEXT    CHECK (length(effective_on) = 10),
    revision      TEXT    CHECK (length(revision) BETWEEN 1 AND 80),
    supersedes_id BLOB    REFERENCES reference_documents (id),
    -- Which vehicles it applies to, as written.
    applicability TEXT    NOT NULL DEFAULT '',
    url           TEXT    CHECK (length(url) BETWEEN 1 AND 8000),
    added_ms      INTEGER NOT NULL,
    added_by      BLOB    NOT NULL REFERENCES users (id),
    received_ms   INTEGER NOT NULL,
    sequence      INTEGER NOT NULL,
    deleted       INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

-- --- Requirements: the recall they're for, their mode, and whether they apply ------------

ALTER TABLE requirements ADD COLUMN reference_id BLOB REFERENCES reference_documents (id);
-- 'recurring' or 'one-time' (Phase 5 adds more; the domain checks them).
ALTER TABLE requirements ADD COLUMN mode TEXT NOT NULL DEFAULT 'recurring'
    CHECK (length(mode) BETWEEN 1 AND 40);
-- The domain's `Applicability` as JSON: {"applies":"applies"}, {"applies":"under-review"},
-- or {"applies":"not-applicable","reason":"..."}.
ALTER TABLE requirements ADD COLUMN applicability TEXT NOT NULL DEFAULT '{"applies":"applies"}'
    CHECK (json_valid(applicability));

CREATE INDEX requirements_by_reference ON requirements (reference_id)
    WHERE reference_id IS NOT NULL AND deleted = 0;

-- --- attachments: any owner, and a reference document's on no vehicle --------------------

CREATE TABLE attachments_new (
    id               BLOB    PRIMARY KEY CHECK (length(id) = 16),
    -- NULL only for a reference document's (shared by vehicles).
    asset_id         BLOB    REFERENCES assets (id),
    -- 'vehicle', 'event', 'squawk', 'document', or 'reference' (the domain checks it).
    owner            TEXT    NOT NULL CHECK (length(owner) BETWEEN 1 AND 40),
    event_id         BLOB    REFERENCES events (id),
    task_position    INTEGER CHECK (task_position >= 0),
    squawk_id        BLOB    REFERENCES squawks (id),
    document_id      BLOB    REFERENCES asset_documents (id),
    reference_id     BLOB    REFERENCES reference_documents (id),
    -- 'photo' or 'document' (the domain checks it).
    kind             TEXT    NOT NULL CHECK (length(kind) BETWEEN 1 AND 40),
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
    CHECK ((reference_id IS NOT NULL) = (owner = 'reference')),
    CHECK ((asset_id IS NULL) = (owner = 'reference')),
    CHECK (task_position IS NULL OR owner = 'event'),
    CHECK ((thumbnail_sha256 IS NULL) = (thumbnail_bytes IS NULL))
) STRICT;

INSERT INTO attachments_new (
    id, asset_id, owner, event_id, task_position, squawk_id, document_id, kind, file_sha256,
    file_bytes, thumbnail_sha256, thumbnail_bytes, name, content, added_ms, added_by,
    received_ms, sequence, deleted
)
SELECT
    id, asset_id, owner, event_id, task_position, squawk_id, document_id, kind, file_sha256,
    file_bytes, thumbnail_sha256, thumbnail_bytes, name, content, added_ms, added_by,
    received_ms, sequence, deleted
FROM attachments;

DROP TABLE attachments;
ALTER TABLE attachments_new RENAME TO attachments;

CREATE INDEX attachments_by_asset ON attachments (asset_id) WHERE deleted = 0;
CREATE INDEX attachments_by_event ON attachments (event_id) WHERE deleted = 0;
CREATE INDEX attachments_by_squawk ON attachments (squawk_id) WHERE deleted = 0;
CREATE INDEX attachments_by_document ON attachments (document_id) WHERE deleted = 0;
CREATE INDEX attachments_by_reference ON attachments (reference_id) WHERE deleted = 0;

-- --- Check, then commit -----------------------------------------------------------------

-- Fails the migration (and rolls it back) if any row anywhere points at something missing.
CREATE TEMP TABLE migration_0015_check (
    broken_foreign_keys INTEGER NOT NULL CHECK (broken_foreign_keys = 0)
) STRICT;
INSERT INTO migration_0015_check SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE migration_0015_check;

COMMIT;

PRAGMA foreign_keys = ON;
