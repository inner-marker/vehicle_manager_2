-- Final once merged into dev (decision #062).
-- Phase 3 milestone 3 step 3d: failed unlocks (decision #074; phase-3.md milestone 3).
--
-- A desktop unlocked after wrong PINs or passwords reports how many there were, and when the
-- last one was. Only the latest report is kept: the household Admin's message is made from it
-- (decision #050), keyed by its time, so a later report is a new message.
ALTER TABLE devices ADD COLUMN failed_unlocks INTEGER NOT NULL DEFAULT 0 CHECK (failed_unlocks >= 0);
ALTER TABLE devices ADD COLUMN last_failed_unlock_ms INTEGER;
