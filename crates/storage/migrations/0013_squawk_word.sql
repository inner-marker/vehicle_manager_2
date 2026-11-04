-- Final once merged into dev (decision #062).
-- Phase 2 milestone 7 step 7f.0: what the household calls squawks on screen (decision #064).
-- Only the household's record uses it; NULL means "not chosen" (the built-in default, Issues).

ALTER TABLE preferences ADD COLUMN squawk_word TEXT
    CHECK (squawk_word IN ('issue', 'squawk', 'fault', 'write-up'));
