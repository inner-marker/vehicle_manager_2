-- Final once merged into dev (decision #062).
-- Phase 3 milestone 7 step 7b: rebuilding a fresh server from a desktop's copy (decision #078).
--
-- One row while a server is (or was) rebuilt: from which server's copy (its database ID),
-- when it started, when the records were in, and when its files were settled (attachments
-- whose files never came left out). Server-only: never pulled.

CREATE TABLE rebuild_state (
    id               INTEGER PRIMARY KEY CHECK (id = 1),
    from_database_id BLOB    NOT NULL CHECK (length(from_database_id) = 16),
    started_ms       INTEGER NOT NULL,
    finished_ms      INTEGER,
    settled_ms       INTEGER
) STRICT;
