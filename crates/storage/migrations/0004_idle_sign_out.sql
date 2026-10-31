-- Phase 2 milestone 3 step 3b: when the app signs out by itself (decision #049).
-- A per-person preference with a household default, like the units; NULL means "not chosen".
-- Not released yet: this file may still change until v0.2.0, then never again.

ALTER TABLE preferences ADD COLUMN idle_sign_out TEXT
    CHECK (idle_sign_out IN ('one-hour', 'four-hours', 'eight-hours', 'twenty-four-hours', 'never'));
