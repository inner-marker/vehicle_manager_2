-- Phase 2 milestone 4 step 4c: groups, assets, meters, and custom field definitions
-- (decisions #030, #052), and role grants on a group.
-- Not released yet: this file may still change until v0.2.0, then never again.
--
-- Every table here is a record, changed only through commands; `deleted` is the soft delete
-- the command pipeline's tombstone pairs with. Values are stored by their JSON names
-- (e.g. 'in-service'), like the preferences table.

-- Where assets live (#052). The default group is made by the server through a command (record
-- IDs are UUIDv7, which SQL can't make), with the first Admin or at startup if there's none.
CREATE TABLE groups (
    id       BLOB    PRIMARY KEY CHECK (length(id) = 16),
    name     TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    icon     TEXT    NOT NULL CHECK (icon IN ('garage', 'hangar', 'boathouse', 'storage',
                                              'warehouse', 'shop', 'farm', 'house')),
    sequence INTEGER NOT NULL,
    deleted  INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

-- The tracked things ("vehicles" in the UI). Name lengths are checked in characters by the
-- domain; the limits here are in bytes, generous enough for any characters.
CREATE TABLE assets (
    id          BLOB    PRIMARY KEY CHECK (length(id) = 16),
    kind        TEXT    NOT NULL CHECK (length(kind) BETWEEN 1 AND 32),
    name        TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    make        TEXT,
    model       TEXT,
    year        INTEGER CHECK (year BETWEEN 1800 AND 2200),
    colour      TEXT,
    -- A Markdown subset (#055); empty means none.
    description TEXT    NOT NULL DEFAULT '',
    group_id    BLOB    NOT NULL REFERENCES groups (id),
    parent_id   BLOB    REFERENCES assets (id),
    status      TEXT    NOT NULL CHECK (status IN ('in-service', 'spare', 'out-for-service',
                                                   'retired', 'disposed')),
    -- A JSON array of {"kind", "value"}: the whole list changes at once.
    identifiers TEXT    NOT NULL DEFAULT '[]',
    sequence    INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX assets_by_group ON assets (group_id) WHERE deleted = 0;

-- Each asset's meters (plan §4 Meters).
CREATE TABLE meters (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id     BLOB    NOT NULL REFERENCES assets (id),
    name         TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    dimension    TEXT    NOT NULL CHECK (dimension IN ('distance', 'duration')),
    unit         TEXT    NOT NULL CHECK (unit IN ('miles', 'kilometers', 'hours')),
    precision    INTEGER NOT NULL CHECK (precision BETWEEN 0 AND 2),
    description  TEXT,
    -- A JSON array of event kinds that require a reading, e.g. ["fuel"].
    required_for TEXT    NOT NULL DEFAULT '[]',
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX meters_by_asset ON meters (asset_id) WHERE deleted = 0;

-- Each asset's custom field definitions (plan §3 Custom fields, #030). Range ends are in the
-- field's storage unit.
CREATE TABLE custom_fields (
    id         BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id   BLOB    NOT NULL REFERENCES assets (id),
    applies_to TEXT    NOT NULL CHECK (applies_to IN ('fuel', 'reading', 'maintenance', 'repair')),
    -- The stable key formulas and reports use (#057), e.g. 'trip-time'; never reused.
    key        TEXT    NOT NULL CHECK (length(key) BETWEEN 1 AND 40),
    label      TEXT    NOT NULL CHECK (length(label) BETWEEN 1 AND 160),
    -- The field's kind as JSON, e.g. {"type":"number","decimals":1}.
    kind       TEXT    NOT NULL,
    enabled    INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    has_range  INTEGER NOT NULL DEFAULT 0 CHECK (has_range IN (0, 1)),
    range_min  INTEGER,
    range_max  INTEGER,
    sequence   INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX custom_fields_by_asset ON custom_fields (asset_id) WHERE deleted = 0;

-- A key names one field of an asset forever, even once that field is removed.
CREATE UNIQUE INDEX custom_fields_keys ON custom_fields (asset_id, key);

-- Role grants, rebuilt (milestone 2 planned it): a third target kind, a group, and foreign
-- keys to the assets and groups they name. Every existing grant is on all assets (there were
-- no assets to name), so every row copies across unchanged.
CREATE TABLE role_grants_new (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    user_id      BLOB    NOT NULL REFERENCES users (id),
    target_kind  TEXT    NOT NULL CHECK (target_kind IN ('all-assets', 'asset', 'group')),
    target_asset BLOB    REFERENCES assets (id),
    target_group BLOB    REFERENCES groups (id),
    role         TEXT    NOT NULL CHECK (role IN ('viewer', 'driver', 'maintainer', 'admin')),
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    -- Each target kind names exactly what it targets.
    CHECK ((target_kind = 'asset') = (target_asset IS NOT NULL)),
    CHECK ((target_kind = 'group') = (target_group IS NOT NULL))
) STRICT;

INSERT INTO role_grants_new (id, user_id, target_kind, target_asset, role, sequence, deleted)
    SELECT id, user_id, target_kind, target_asset, role, sequence, deleted FROM role_grants;

DROP TABLE role_grants;
ALTER TABLE role_grants_new RENAME TO role_grants;
CREATE INDEX role_grants_by_user ON role_grants (user_id) WHERE deleted = 0;
