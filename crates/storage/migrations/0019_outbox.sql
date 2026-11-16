-- Final once merged into dev (decision #062).
-- Phase 3 milestone 5 step 5b: offline changes (decision #076; phase-3.md milestone 5).
--
-- On a desktop's copy (the same migrations run there): the `outbox` of changes made on the
-- desktop and not yet acknowledged by the server, and the desktop's own clock (`device_clock`).
-- Both empty on the server.
--
-- On the server: `applied_operations`, each operation a desktop pushed and its answer, so a
-- resent one gets the same answer and is never applied twice; and a flag on `devices` for a
-- desktop whose clock was found far ahead. Empty (and unused) on a copy.
--
-- None of these are pulled: the pull sends only the record tables (`storage::sync`).

CREATE TABLE outbox (
    -- The order the changes were made in, and are sent in (never reused).
    position      INTEGER PRIMARY KEY AUTOINCREMENT,
    operation_id  BLOB    NOT NULL UNIQUE CHECK (length(operation_id) = 16),
    -- The operation (`domain::operation::Operation`), as JSON.
    operation     TEXT    NOT NULL CHECK (json_valid(operation)),
    -- Waiting to be sent, or refused by the server (kept for Messages until opened or
    -- discarded; sketch 2A). Acknowledged changes are removed.
    state         TEXT    NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'refused')),
    -- Why it was refused (`domain::operation::Refusal`), as JSON.
    refusal       TEXT    CHECK (refusal IS NULL OR json_valid(refusal)),
    made_at_ms    INTEGER NOT NULL,
    refused_at_ms INTEGER,
    CHECK ((state = 'refused') = (refusal IS NOT NULL AND refused_at_ms IS NOT NULL))
) STRICT;

CREATE TABLE device_clock (
    id       INTEGER PRIMARY KEY CHECK (id = 1),
    -- The newest hybrid logical clock time this desktop has made or received (packed).
    last_hlc INTEGER NOT NULL CHECK (last_hlc >= 0)
) STRICT;

CREATE TABLE applied_operations (
    operation_id   BLOB    PRIMARY KEY CHECK (length(operation_id) = 16),
    -- Who sent it, from which device (the session's, never a value in the operation).
    device_id      BLOB    NOT NULL REFERENCES devices (id),
    user_id        BLOB    NOT NULL REFERENCES users (id),
    hlc            INTEGER NOT NULL CHECK (hlc >= 0),
    format_version INTEGER NOT NULL CHECK (format_version >= 1),
    -- Why it was refused (`domain::operation::Refusal`, as JSON); NULL when it was applied.
    refusal        TEXT    CHECK (refusal IS NULL OR json_valid(refusal)),
    received_at_ms INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- A desktop whose change came with a clock more than 5 minutes ahead of the server's: how far
-- ahead, and when (a message for the Admin). Cleared once its clock is right again.
ALTER TABLE devices ADD COLUMN clock_ahead_ms    INTEGER CHECK (clock_ahead_ms IS NULL OR clock_ahead_ms > 0);
ALTER TABLE devices ADD COLUMN clock_ahead_at_ms INTEGER;
