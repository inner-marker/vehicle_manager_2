-- Final once merged into dev (decision #062).
-- Phase 6 milestone 2 step 2h: a vehicle's main photo (#24; phase-6.md milestone 2).
--
-- - `assets.main_photo`: the ID of one of the vehicle's own photos, shown on the Garage card
--   and the Overview; NULL for none, so every existing vehicle reads as before. Not a foreign
--   key (attachments already point at assets); a removed photo is ignored where it's read.

ALTER TABLE assets ADD COLUMN main_photo BLOB CHECK (main_photo IS NULL OR length(main_photo) = 16);
