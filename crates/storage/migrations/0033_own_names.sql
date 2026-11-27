-- Final once merged into dev (decision #062).
-- Phase 6 milestone 4 step 4b: changing names (issue #44; decision #091; phase-6.md
-- milestone 4).
--
-- - `preferences.own_names`: whether each person may change their own display name and
--   username, as a JSON boolean; the household's record only, NULL (off) until the Admin sets
--   it, so every existing row reads as before. The Admin can always change anyone's; usernames
--   and display names are already columns of `users`, so changing them needs nothing new.

ALTER TABLE preferences ADD COLUMN own_names TEXT;
