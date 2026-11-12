-- Final once merged into dev (decision #062).
-- Phase 3 milestone 4 step 4a: pulling changes to the desktop's copy (decision #075;
-- phase-3.md milestone 4).
--
-- On the server: an index on each record table's `sequence`, so a pull ("every row changed
-- since the device's cursor") reads only what changed.
--
-- On a desktop's copy (the same migrations run there): `pull_state`, one row saying which
-- server the copy is of, how far it has pulled, and when. Empty on the server.

CREATE INDEX users_by_sequence               ON users (sequence);
CREATE INDEX role_grants_by_sequence         ON role_grants (sequence);
CREATE INDEX preferences_by_sequence         ON preferences (sequence);
CREATE INDEX message_marks_by_sequence       ON message_marks (sequence);
CREATE INDEX groups_by_sequence              ON groups (sequence);
CREATE INDEX assets_by_sequence              ON assets (sequence);
CREATE INDEX meters_by_sequence              ON meters (sequence);
CREATE INDEX custom_fields_by_sequence       ON custom_fields (sequence);
CREATE INDEX garage_orders_by_sequence       ON garage_orders (sequence);
CREATE INDEX saved_places_by_sequence        ON saved_places (sequence);
CREATE INDEX events_by_sequence              ON events (sequence);
CREATE INDEX squawks_by_sequence             ON squawks (sequence);
CREATE INDEX requirements_by_sequence        ON requirements (sequence);
CREATE INDEX bundles_by_sequence             ON bundles (sequence);
CREATE INDEX license_plates_by_sequence      ON license_plates (sequence);
CREATE INDEX asset_documents_by_sequence     ON asset_documents (sequence);
CREATE INDEX attachments_by_sequence         ON attachments (sequence);
CREATE INDEX reference_documents_by_sequence ON reference_documents (sequence);

CREATE TABLE pull_state (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    -- The server's database ID and instance name ('' for the household's main server).
    server_database_id BLOB    NOT NULL CHECK (length(server_database_id) = 16),
    server_instance    TEXT    NOT NULL DEFAULT '',
    -- The server's highest change number this copy has (0 before the first pull).
    cursor             INTEGER NOT NULL DEFAULT 0 CHECK (cursor >= 0),
    -- The server's newest change number at the last pull, for the first download's progress.
    server_sequence    INTEGER NOT NULL DEFAULT 0 CHECK (server_sequence >= 0),
    -- When the last pull finished (Unix milliseconds), if one has.
    last_pull_ms       INTEGER
) STRICT;
