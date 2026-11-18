-- Final once merged into dev (decision #062).
-- Phase 5 milestone 6 step 6b: readiness's usages (decision #086; phase-5.md milestone 6).
--
-- - `usages`: each vehicle's usages ("Drivable", "Towing"), one of them its default, each with
--   the document types it needs (a JSON array, e.g. [{"type":"listed","key":"document-insurance"}]).
--   Each vehicle's default usage is made by the server, not here: its name and documents come
--   from the vehicle's profile.
-- - `requirements.usages`: the usages a requirement is needed for, a JSON array of their IDs
--   (as text); `[]` means none. NULL (every existing requirement) means the default usage.
-- - `schedule_templates.usages`: a template's usages, with the IDs they had.

CREATE TABLE usages (
    id         BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id   BLOB    NOT NULL REFERENCES assets (id),
    name       TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    documents  TEXT    NOT NULL DEFAULT '[]',
    sequence   INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX usages_by_asset ON usages (asset_id) WHERE deleted = 0;

ALTER TABLE requirements ADD COLUMN usages TEXT;

ALTER TABLE schedule_templates ADD COLUMN usages TEXT NOT NULL DEFAULT '[]';
