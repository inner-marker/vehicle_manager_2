-- Final once merged into dev (decision #062).
-- Phase 5 milestone 4 step 4b: specs and schedule templates (decision #084; phase-5.md
-- milestone 4).
--
-- - `specs`: a vehicle's specs. An amount is kept as entered: thousandths of its unit
--   (`amount_milli`) and the unit's name (`amount_unit`, e.g. "quarts"), both or neither.
-- - `requirements.specs`: the specs a requirement's job uses, as a JSON array of their IDs (as
--   text), in order; empty for every existing requirement.
-- - `task_parts.spec_id`: the spec a job's part was pre-filled from, if any. No foreign key:
--   a job keeps what it recorded even if the spec is removed later.
-- - `schedule_templates`: the household's named templates, each part as JSON (the meters its
--   rules use, its requirements, bundles, and specs, with the IDs they had).

CREATE TABLE specs (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id     BLOB    NOT NULL REFERENCES assets (id),
    name         TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    amount_milli INTEGER CHECK (amount_milli > 0),
    amount_unit  TEXT    CHECK (length(amount_unit) BETWEEN 1 AND 40),
    detail       TEXT    CHECK (length(detail) BETWEEN 1 AND 400),
    part_number  TEXT    CHECK (length(part_number) BETWEEN 1 AND 160),
    notes        TEXT    NOT NULL DEFAULT '',
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),
    CHECK ((amount_milli IS NULL) = (amount_unit IS NULL))
) STRICT;

CREATE INDEX specs_by_asset ON specs (asset_id) WHERE deleted = 0;

ALTER TABLE requirements ADD COLUMN specs TEXT NOT NULL DEFAULT '[]';

ALTER TABLE task_parts ADD COLUMN spec_id BLOB CHECK (spec_id IS NULL OR length(spec_id) = 16);

CREATE TABLE schedule_templates (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    name         TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    meters       TEXT    NOT NULL DEFAULT '[]',
    requirements TEXT    NOT NULL DEFAULT '[]',
    bundles      TEXT    NOT NULL DEFAULT '[]',
    specs        TEXT    NOT NULL DEFAULT '[]',
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;
