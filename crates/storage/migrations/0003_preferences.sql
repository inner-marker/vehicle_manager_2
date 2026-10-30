-- Phase 2 milestone 2 step 2i: display preferences (decisions #029, #048).
-- One row for the household (its ID is the database's) and one per user (its ID is the
-- user's); one column per setting. NULL means "not chosen": a user then follows the
-- household, and the household the built-in default.
-- Not released yet: this file may still change until v0.2.0, then never again.

CREATE TABLE preferences (
    id          BLOB    PRIMARY KEY CHECK (length(id) = 16),
    distance    TEXT    CHECK (distance IN ('miles', 'kilometers')),
    volume      TEXT    CHECK (volume IN ('us-gallons', 'imperial-gallons', 'liters')),
    economy     TEXT    CHECK (economy IN ('mpg-us', 'mpg-uk', 'liters-per-100-km', 'km-per-liter')),
    pressure    TEXT    CHECK (pressure IN ('psi', 'kpa', 'bar')),
    date_format TEXT    CHECK (date_format IN ('day-month-name-year', 'month-day-year', 'day-month-year', 'year-month-day')),
    currency    TEXT    CHECK (length(currency) = 3),
    language    TEXT    CHECK (language IN ('en-US')),
    sequence    INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;
