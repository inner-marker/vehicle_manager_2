-- Phase 2 milestone 5 step 5d: each person's saved places (decision #059).
-- Not released yet: this file may still change until v0.2.0, then never again.
--
-- A saved place is one person's alias, e.g. "Home garage": choosing it copies its name and
-- location into an event (milestone 6), so editing or removing a place never changes past
-- events. Coordinates are whole degrees x 10^7, like every stored quantity (plan §3).
-- Name lengths are checked in characters by the domain; the limits here are in bytes.
CREATE TABLE saved_places (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    owner_id     BLOB    NOT NULL REFERENCES users (id),
    name         TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    lat_e7       INTEGER NOT NULL CHECK (lat_e7 BETWEEN -900000000 AND 900000000),
    lon_e7       INTEGER NOT NULL CHECK (lon_e7 BETWEEN -1800000000 AND 1800000000),
    radius_m     INTEGER NOT NULL CHECK (radius_m BETWEEN 25 AND 2000),
    city         TEXT    NOT NULL CHECK (length(city) BETWEEN 1 AND 400),
    region       TEXT    NOT NULL CHECK (length(region) BETWEEN 1 AND 400),
    -- ISO 3166-1 alpha-2, upper case (e.g. 'US').
    country_code TEXT    NOT NULL CHECK (length(country_code) = 2 AND country_code = upper(country_code)),
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX saved_places_by_owner ON saved_places (owner_id) WHERE deleted = 0;
