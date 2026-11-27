-- Final once merged into dev (decision #062).
-- Phase 6 milestone 4 step 4e: a PIN for Admin mode (sub-plan 04 §4.2; decision #071;
-- phase-6.md milestone 4).
--
-- - `user_passwords.admin_pin_hash`: the Argon2id hash of the PIN an Admin chose to enter Admin
--   mode with, instead of their password; NULL (the password, as before) until they choose one.
--   Kept beside the password hash, so like it never synced to a desktop.

ALTER TABLE user_passwords ADD COLUMN admin_pin_hash TEXT;
