-- Final once merged into dev (decision #062).
-- Phase 6 milestone 3 step 3e: household vehicle types (idea 23; decision #090; phase-6.md
-- milestone 3).
--
-- - `vehicle_types`: the household's types, each built on one of the app's kinds (a profile
--   ID), with an icon, the documents its default usage needs, which of its kind's fields start
--   on, what it adds from the catalogue (JSON lists), and a template copied on, or none.
-- - `assets.vehicle_type`: the type a vehicle was added as. NULL: none, so every existing
--   vehicle reads as before. A removed type's vehicles keep the ID and show their kind.

CREATE TABLE vehicle_types (
    id        BLOB    PRIMARY KEY CHECK (length(id) = 16),
    name      TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    icon      TEXT    NOT NULL,
    kind      TEXT    NOT NULL CHECK (length(kind) BETWEEN 1 AND 40),
    documents TEXT    NOT NULL,
    fields_on TEXT    NOT NULL,
    catalogue TEXT    NOT NULL,
    template  BLOB    CHECK (template IS NULL OR length(template) = 16),
    sequence  INTEGER NOT NULL,
    deleted   INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
);

CREATE INDEX vehicle_types_by_sequence ON vehicle_types (sequence);

ALTER TABLE assets ADD COLUMN vehicle_type BLOB
    CHECK (vehicle_type IS NULL OR length(vehicle_type) = 16);
