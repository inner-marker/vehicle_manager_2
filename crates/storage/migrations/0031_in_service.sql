-- Final once merged into dev (decision #062).
-- Phase 6 milestone 3 step 3c: when a vehicle went into service (idea 21; decision #090;
-- phase-6.md milestone 3).
--
-- - `assets.in_service_since`: the date (`2019-06-12`), built or first registered; a used
--   vehicle's is backdated. A chained calendar rule's first interval counts from it. NULL: not
--   known, so every existing vehicle reads as before.
-- - `assets.in_service_estimated`: 1 when that date is an estimate.
-- A chained rule's first interval needs no column: rules are stored as JSON.

ALTER TABLE assets ADD COLUMN in_service_since TEXT
    CHECK (in_service_since IS NULL OR length(in_service_since) = 10);
ALTER TABLE assets ADD COLUMN in_service_estimated INTEGER NOT NULL DEFAULT 0
    CHECK (in_service_estimated IN (0, 1));
