-- Phase 2 milestone 2: users, role grants, passwords, and sessions (decisions #028, #045).
-- Not released yet: this file may still change until v0.2.0, then never again.

-- People who can sign in. A record (changed only through commands); `deleted` is the soft
-- delete the command pipeline's tombstone pairs with.
CREATE TABLE users (
    id                   BLOB    PRIMARY KEY CHECK (length(id) = 16),
    username             TEXT    NOT NULL CHECK (length(username) BETWEEN 3 AND 32),
    display_name         TEXT    NOT NULL,
    disabled             INTEGER NOT NULL CHECK (disabled IN (0, 1)),
    must_change_password INTEGER NOT NULL CHECK (must_change_password IN (0, 1)),
    sequence             INTEGER NOT NULL,
    deleted              INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

-- Usernames are unique among users that aren't deleted (stored lowercase by the domain).
CREATE UNIQUE INDEX users_username ON users (username) WHERE deleted = 0;

-- Password hashes: server-only, never a record, never synced or sent to a device.
CREATE TABLE user_passwords (
    user_id       BLOB    PRIMARY KEY REFERENCES users (id),
    password_hash TEXT    NOT NULL,
    changed_at_ms INTEGER NOT NULL
) STRICT;

-- Role grants (#045): a role on one asset, or on all assets. Created and deleted, never edited.
-- `target_asset` will reference `assets` once that table exists (milestone 4).
CREATE TABLE role_grants (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    user_id      BLOB    NOT NULL REFERENCES users (id),
    target_kind  TEXT    NOT NULL CHECK (target_kind IN ('all-assets', 'asset')),
    target_asset BLOB    CHECK (length(target_asset) = 16),
    role         TEXT    NOT NULL CHECK (role IN ('viewer', 'driver', 'maintainer', 'admin')),
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    -- An asset grant names its asset; an all-assets grant doesn't.
    CHECK ((target_kind = 'asset') = (target_asset IS NOT NULL))
) STRICT;

CREATE INDEX role_grants_by_user ON role_grants (user_id) WHERE deleted = 0;

-- Signed-in sessions: server-only. Only the SHA-256 of each token is stored, so a copy of
-- the database (e.g. a snapshot) can't be used to sign in.
CREATE TABLE sessions (
    token_hash        BLOB    PRIMARY KEY CHECK (length(token_hash) = 32),
    user_id           BLOB    NOT NULL REFERENCES users (id),
    -- The device this session acts as, for command stamps.
    device_id         BLOB    NOT NULL CHECK (length(device_id) = 16),
    created_at_ms     INTEGER NOT NULL,
    last_used_ms      INTEGER NOT NULL,
    expires_at_ms     INTEGER NOT NULL,
    active_mode       TEXT    NOT NULL CHECK (active_mode IN ('driver', 'maintainer', 'admin'))
) STRICT, WITHOUT ROWID;

CREATE INDEX sessions_by_user ON sessions (user_id);
