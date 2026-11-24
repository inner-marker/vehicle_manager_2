-- Final once merged into dev (decision #062).
-- Phase 6 milestone 2 step 2f: shops and labor rates (decision #089; phase-6.md milestone 2).
--
-- - `shops`: the household's shops, each with a contact and, if it differs from the
--   household's, its own labor rate (the household currency's smallest unit an hour).
-- - `preferences.labor_rate`: the household's labor rate, as a JSON number (e.g. 9500); the
--   household's record only, NULL until set, so every existing row reads as before.

CREATE TABLE shops (
    id         BLOB    PRIMARY KEY CHECK (length(id) = 16),
    name       TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    contact    TEXT    CHECK (length(contact) BETWEEN 1 AND 400),
    labor_rate INTEGER CHECK (labor_rate BETWEEN 0 AND 10000000),
    sequence   INTEGER NOT NULL,
    deleted    INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
);

CREATE INDEX shops_by_sequence ON shops (sequence);

ALTER TABLE preferences ADD COLUMN labor_rate TEXT;
