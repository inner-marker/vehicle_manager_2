-- Final once merged into dev (decision #062).
-- Phase 5 milestone 2 step 2b: what's due (decision #082; phase-5.md milestone 2).
--
-- New columns only, each NULL until set, so every existing row reads as before.
-- - `requirements.before_records`: when it was last done before the household's records
--   began, as JSON: {"on":"2026-08-02","readings":[{"meter":"…","value":78695000}]}.
-- - `requirements.warnings`: its own warnings instead of the household's, as JSON:
--   {"interval":{"coming_up":20,"due_soon":10,"very_soon":5},
--    "days":{"coming_up":60,"due_soon":30,"very_soon":14}}.
-- - `asset_documents.warn_days`: how early its expiry warns, as JSON:
--   {"coming_up":60,"due_soon":30,"very_soon":14}.
-- - `preferences.due_warnings`: the household's defaults (its record only), shaped like
--   `requirements.warnings`.
-- Interval rules gain "when a meter reaches X" inside `requirements.rules`, which is JSON
-- already: no change here.

ALTER TABLE requirements ADD COLUMN before_records TEXT;
ALTER TABLE requirements ADD COLUMN warnings TEXT;
ALTER TABLE asset_documents ADD COLUMN warn_days TEXT;
ALTER TABLE preferences ADD COLUMN due_warnings TEXT;
