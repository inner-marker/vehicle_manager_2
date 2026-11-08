-- Phase 2 milestone 1: the database foundation (phase-2.md, decisions #042-#044).
-- Every table is STRICT. IDs are 16-byte UUIDv7 BLOBs; HLCs are packed INTEGERs;
-- instants are Unix epoch milliseconds (INTEGER, UTC).
-- Not released yet: this file may still change until v0.2.0, then never again.

-- One row: which database this is, and the server's own counters.
CREATE TABLE database_meta (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    database_id      BLOB    NOT NULL CHECK (length(database_id) = 16),
    -- The server's own device ID, for changes it makes itself (e.g. automatic finalizing).
    server_device_id BLOB    NOT NULL CHECK (length(server_device_id) = 16),
    created_at_ms    INTEGER NOT NULL,
    -- Every accepted change gets the next sequence number; devices pull since a cursor.
    last_sequence    INTEGER NOT NULL DEFAULT 0 CHECK (last_sequence >= 0),
    -- The server's newest HLC, so its stamps keep increasing across restarts.
    last_hlc         INTEGER NOT NULL DEFAULT 0 CHECK (last_hlc >= 0)
) STRICT;

-- Commands already applied, so a retried command is never applied twice. `sequence` is
-- empty for a command that changed nothing (every field already newer, or the record deleted).
CREATE TABLE applied_commands (
    command_id    BLOB    PRIMARY KEY CHECK (length(command_id) = 16),
    device_id     BLOB    NOT NULL CHECK (length(device_id) = 16),
    hlc           INTEGER NOT NULL CHECK (hlc >= 0),
    sequence      INTEGER,
    applied_at_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Last-write-wins per field (#043): the stamp of the last accepted change to each field.
CREATE TABLE field_clocks (
    entity    TEXT    NOT NULL,
    record_id BLOB    NOT NULL CHECK (length(record_id) = 16),
    field     TEXT    NOT NULL,
    hlc       INTEGER NOT NULL CHECK (hlc >= 0),
    device_id BLOB    NOT NULL CHECK (length(device_id) = 16),
    PRIMARY KEY (entity, record_id, field)
) STRICT, WITHOUT ROWID;

-- Deleted records, so deletes reach other devices through pull (soft deletes).
CREATE TABLE tombstones (
    entity        TEXT    NOT NULL,
    record_id     BLOB    NOT NULL CHECK (length(record_id) = 16),
    hlc           INTEGER NOT NULL CHECK (hlc >= 0),
    device_id     BLOB    NOT NULL CHECK (length(device_id) = 16),
    sequence      INTEGER NOT NULL,
    deleted_at_ms INTEGER NOT NULL,
    PRIMARY KEY (entity, record_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX tombstones_by_sequence ON tombstones (sequence);

-- The server's scheduled jobs (#044): when each last ran, how it went, and when it runs next.
CREATE TABLE jobs (
    name             TEXT    PRIMARY KEY,
    last_started_ms  INTEGER,
    last_finished_ms INTEGER,
    last_result      TEXT    CHECK (last_result IN ('ok', 'failed', 'skipped')),
    last_message     TEXT,
    next_run_ms      INTEGER
) STRICT;
