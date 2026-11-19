-- Final once merged into dev (decision #062).
-- Phase 5 milestone 7 step 7c: meters' keys and derived meters' formulas (decision #087;
-- phase-5.md milestone 7).
--
-- - `meters.key`: the stable key formulas use (`odometer`), unique on its vehicle among the
--   meters that aren't deleted. NULL for every existing meter: the server gives each one at
--   startup, made from its name and different from the vehicle's other meters' and fields'
--   keys and from the words formulas use (a slug of any name can't be made in SQL), and the
--   desktop gets them by syncing. Until then, a key is read as one made from the name.
-- - `meters.formula`: a derived meter's formula, as typed (`odometer + idle-hours × 33 mi/h`);
--   NULL for a typed meter.

ALTER TABLE meters ADD COLUMN key TEXT CHECK (key IS NULL OR length(key) BETWEEN 1 AND 40);

ALTER TABLE meters ADD COLUMN formula TEXT
    CHECK (formula IS NULL OR length(formula) BETWEEN 1 AND 500);

CREATE UNIQUE INDEX meters_keys ON meters (asset_id, key)
    WHERE key IS NOT NULL AND deleted = 0;
