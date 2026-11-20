-- no-transaction
-- Final once merged into dev (decision #062).
-- Phase 5 milestone 1 step 1b: percentage, counting-down, and resettable meters (decision
-- #080; phase-5.md milestone 1).
--
-- `meters` is REBUILT, because SQLite can't change a CHECK in place: its dimension listed only
-- 'distance' and 'duration', and its unit only 'miles', 'kilometers', and 'hours'. Following
-- 0015's rule, the new table lists no values for them: `domain::meter` checks them, so a new
-- one never needs a rebuild. The procedure is 0015's (SQLite's "Making Other Kinds Of Table
-- Schema Changes"): foreign keys off, then in one transaction create the new table, copy every
-- row, drop the old one, rename the new one, recreate its indexes, and check every foreign key
-- before committing. `event_readings` points at `meters` by name, so it points at the new
-- table unchanged; interval rules name meters inside their JSON, which the rebuild keeps as
-- it is. The first line keeps sqlx from wrapping this file in a transaction of its own, since
-- foreign keys can only be turned off outside one.
--
-- New columns:
-- - `meters.direction`: 'up' or 'down' (every existing meter counts up).
-- - `meters.reset_to`: what a reset sets the meter to, in its stored unit; NULL when it can't
--   be reset (every existing meter).
-- - `event_readings.reset`: 1 when the reading is a reset (the meter's new start).
--
-- A percentage is stored in hundredths of a percent (62% is 6,200).

PRAGMA foreign_keys = OFF;

BEGIN;

CREATE TABLE meters_new (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id     BLOB    NOT NULL REFERENCES assets (id),
    name         TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    dimension    TEXT    NOT NULL,
    unit         TEXT    NOT NULL,
    precision    INTEGER NOT NULL CHECK (precision BETWEEN 0 AND 2),
    description  TEXT,
    -- A JSON array of event kinds that require a reading, e.g. ["fuel"].
    required_for TEXT    NOT NULL DEFAULT '[]',
    direction    TEXT    NOT NULL DEFAULT 'up',
    reset_to     INTEGER CHECK (reset_to >= 0),
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

INSERT INTO meters_new (
    id, asset_id, name, dimension, unit, precision, description, required_for, sequence,
    deleted
)
SELECT id, asset_id, name, dimension, unit, precision, description, required_for, sequence,
       deleted
FROM meters;

DROP TABLE meters;
ALTER TABLE meters_new RENAME TO meters;

CREATE INDEX meters_by_asset ON meters (asset_id) WHERE deleted = 0;
CREATE INDEX meters_by_sequence ON meters (sequence);

ALTER TABLE event_readings ADD COLUMN reset INTEGER NOT NULL DEFAULT 0 CHECK (reset IN (0, 1));

-- Fails the migration (and rolls it back) if any row anywhere points at something missing.
CREATE TEMP TABLE migration_0022_check (
    broken_foreign_keys INTEGER NOT NULL CHECK (broken_foreign_keys = 0)
) STRICT;
INSERT INTO migration_0022_check SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE migration_0022_check;

COMMIT;

PRAGMA foreign_keys = ON;
