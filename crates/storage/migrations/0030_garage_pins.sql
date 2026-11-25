-- Final once merged into dev (decision #062).
-- Phase 6 milestone 3 step 3b: pinned vehicles (idea 15; decision #090; phase-6.md milestone 3).
--
-- - `garage_orders.pinned`: the person's pinned vehicles, a JSON list of asset IDs, which come
--   first in their own group whatever the sort. Empty for everyone to start with, so every
--   Garage reads as before. Kept beside the custom order: both are the person's own.

ALTER TABLE garage_orders ADD COLUMN pinned TEXT NOT NULL DEFAULT '[]';
