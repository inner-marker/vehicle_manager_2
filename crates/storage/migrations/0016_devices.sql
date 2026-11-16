-- Final once merged into dev (decision #062).
-- Phase 3 milestone 2 step 2a: devices (decision #073; phase-3.md milestone 2).
--
-- Every session now belongs to a device: a desktop app paired with its own key, or a browser.
-- Settings -> Devices lists a person's devices while they have a live session; signing one
-- out ends its sessions and stamps `signed_out_at_ms`. The row is kept, because changes
-- already applied are stamped with the device's ID.
--
-- `kind` has no fixed list here ('desktop', 'browser' today; checked by
-- `domain::device::DeviceKind`), so a kind can be added without rebuilding the table. `name`
-- is NULL when there's none yet (the app labels it by kind). `public_key` is a desktop's
-- Ed25519 key (32 bytes); a browser has none.

CREATE TABLE devices (
    id               BLOB    PRIMARY KEY CHECK (length(id) = 16),
    user_id          BLOB    NOT NULL REFERENCES users (id),
    kind             TEXT    NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
    name             TEXT    CHECK (name IS NULL OR length(name) BETWEEN 1 AND 400),
    public_key       BLOB    CHECK (public_key IS NULL OR length(public_key) = 32),
    added_at_ms      INTEGER NOT NULL,
    last_seen_ms     INTEGER NOT NULL,
    signed_out_at_ms INTEGER
) STRICT, WITHOUT ROWID;

CREATE INDEX devices_by_user ON devices (user_id);

-- Every existing session gets a device of its own. They can't be told apart (today's desktop
-- app keeps its token in memory and sends no key), so each is recorded as a browser, unnamed;
-- they end within 30 idle days anyway.
INSERT INTO devices (id, user_id, kind, name, public_key, added_at_ms, last_seen_ms)
SELECT device_id, user_id, 'browser', NULL, NULL, created_at_ms, last_used_ms
FROM sessions;

-- `sessions` is rebuilt so its device must exist (SQLite can't add a foreign key in place).
-- Nothing refers to `sessions`, so the usual procedure needs no change to foreign keys: create
-- the new table, copy every row, drop the old one, rename the new one, recreate its indexes.
CREATE TABLE sessions_new (
    token_hash        BLOB    PRIMARY KEY CHECK (length(token_hash) = 32),
    user_id           BLOB    NOT NULL REFERENCES users (id),
    -- The device this session belongs to; its changes are stamped with its ID.
    device_id         BLOB    NOT NULL REFERENCES devices (id),
    created_at_ms     INTEGER NOT NULL,
    last_used_ms      INTEGER NOT NULL,
    expires_at_ms     INTEGER NOT NULL,
    active_mode       TEXT    NOT NULL CHECK (active_mode IN ('driver', 'maintainer', 'admin'))
) STRICT, WITHOUT ROWID;

INSERT INTO sessions_new
    (token_hash, user_id, device_id, created_at_ms, last_used_ms, expires_at_ms, active_mode)
SELECT token_hash, user_id, device_id, created_at_ms, last_used_ms, expires_at_ms, active_mode
FROM sessions;

DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;

CREATE INDEX sessions_by_user ON sessions (user_id);
CREATE INDEX sessions_by_device ON sessions (device_id);
