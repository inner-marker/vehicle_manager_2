-- Phase 2 milestone 6 step 6e: events (fuel-ups and readings), their meter readings, custom
-- field values, and fuel details (plan §3 Events; decisions #008, #030, #033, #034).
-- Not released yet: this file may still change until v0.2.0, then never again.
--
-- An event is one record, changed only through commands. Its readings, field values, and fuel
-- details are parts of it, kept in their own tables so they can be queried (a meter's readings
-- in time, a field's values), and rewritten whole when the event's list changes. Dates are UTC
-- milliseconds with the place's IANA zone; coordinates are degrees x 10^7; quantities are whole
-- numbers in their storage units (meters, seconds, milliliters, a currency's minor unit).
--
-- A finalized event never changes: the storage writer refuses (choice 9). A mistake is fixed
-- by a correction, a new event naming the one it corrects.
CREATE TABLE events (
    id                 BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id           BLOB    NOT NULL REFERENCES assets (id),
    kind               TEXT    NOT NULL CHECK (kind IN ('fuel', 'reading', 'maintenance', 'repair')),
    start_ms           INTEGER NOT NULL,
    end_ms             INTEGER CHECK (end_ms >= start_ms),
    zone               TEXT    NOT NULL CHECK (length(zone) BETWEEN 1 AND 64),

    -- Where. The place is all three or none (none only with a reason, and flagged).
    city               TEXT    CHECK (length(city) BETWEEN 1 AND 400),
    region             TEXT    CHECK (length(region) BETWEEN 1 AND 400),
    country_code       TEXT    CHECK (length(country_code) = 2 AND country_code = upper(country_code)),
    lat_e7             INTEGER CHECK (lat_e7 BETWEEN -900000000 AND 900000000),
    lon_e7             INTEGER CHECK (lon_e7 BETWEEN -1800000000 AND 1800000000),
    accuracy_m         INTEGER CHECK (accuracy_m >= 0),
    fix_ms             INTEGER,
    location_source    TEXT    NOT NULL CHECK (location_source IN ('gps', 'saved-place', 'map', 'manual')),
    place_name         TEXT    CHECK (length(place_name) BETWEEN 1 AND 160),
    -- A hint only: the saved place's values are copied in above.
    saved_place_id     BLOB    REFERENCES saved_places (id),
    geocoder_version   TEXT    CHECK (length(geocoder_version) BETWEEN 1 AND 40),
    no_place_reason    TEXT    CHECK (length(no_place_reason) BETWEEN 1 AND 2000),

    -- Required readings and fields left out: a JSON array of {"item", "reason"}.
    skipped            TEXT    NOT NULL DEFAULT '[]',
    -- A Markdown subset (#055); empty means none.
    notes              TEXT    NOT NULL DEFAULT '',
    source             TEXT    NOT NULL CHECK (source IN ('typed', 'voice', 'import')),
    logged_by          BLOB    NOT NULL REFERENCES users (id),
    -- Who did it: a user, or a name with an optional contact; or nobody recorded.
    performer_user_id  BLOB    REFERENCES users (id),
    performer_name     TEXT    CHECK (length(performer_name) BETWEEN 1 AND 400),
    performer_contact  TEXT    CHECK (length(performer_contact) BETWEEN 1 AND 400),
    reading_override   TEXT    CHECK (length(reading_override) BETWEEN 1 AND 2000),

    -- Drafts and finalizing (#034), and the groundwork for signatures (sub-plan 01).
    status             TEXT    NOT NULL CHECK (status IN ('draft', 'finalized')),
    last_edited_ms     INTEGER NOT NULL,
    received_ms        INTEGER NOT NULL,
    finalized_ms       INTEGER,
    -- 'user' (Finalize now, with finalized_by_user) or 'server' (24 hours after the last edit).
    finalized_by       TEXT    CHECK (finalized_by IN ('user', 'server')),
    finalized_by_user  BLOB    REFERENCES users (id),
    corrects_id        BLOB    REFERENCES events (id),
    correction_reason  TEXT    CHECK (length(correction_reason) BETWEEN 1 AND 2000),

    sequence           INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1)),

    CHECK ((city IS NULL) = (region IS NULL) AND (city IS NULL) = (country_code IS NULL)),
    CHECK ((lat_e7 IS NULL) = (lon_e7 IS NULL)),
    CHECK (performer_name IS NULL OR performer_user_id IS NULL),
    CHECK ((status = 'finalized') = (finalized_ms IS NOT NULL)),
    CHECK ((finalized_ms IS NULL) = (finalized_by IS NULL)),
    CHECK ((finalized_by = 'user') = (finalized_by_user IS NOT NULL))
) STRICT;

CREATE INDEX events_by_asset ON events (asset_id, start_ms) WHERE deleted = 0;
CREATE INDEX events_by_correction ON events (corrects_id) WHERE corrects_id IS NOT NULL;
CREATE INDEX events_drafts ON events (last_edited_ms) WHERE status = 'draft' AND deleted = 0;

-- Each event's meter readings, in the meter's stored unit (meters or seconds), as entered.
CREATE TABLE event_readings (
    event_id BLOB    NOT NULL REFERENCES events (id),
    meter_id BLOB    NOT NULL REFERENCES meters (id),
    value    INTEGER NOT NULL CHECK (value >= 0),
    PRIMARY KEY (event_id, meter_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX event_readings_by_meter ON event_readings (meter_id);

-- Each event's custom field values: the value as JSON, e.g. {"type":"duration","value":5400}.
CREATE TABLE event_field_values (
    event_id BLOB NOT NULL REFERENCES events (id),
    field_id BLOB NOT NULL REFERENCES custom_fields (id),
    data     TEXT NOT NULL,
    PRIMARY KEY (event_id, field_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX event_field_values_by_field ON event_field_values (field_id);

-- A fuel-up's details (#030, #033).
CREATE TABLE fuel_details (
    event_id         BLOB    PRIMARY KEY REFERENCES events (id),
    volume_ml        INTEGER NOT NULL CHECK (volume_ml > 0),
    cost_minor       INTEGER CHECK (cost_minor >= 0),
    currency         TEXT    NOT NULL CHECK (length(currency) = 3 AND currency = upper(currency)),
    filled_full      INTEGER NOT NULL CHECK (filled_full IN (0, 1)),
    missed_previous  INTEGER NOT NULL CHECK (missed_previous IN (0, 1)),
    highway_pct      INTEGER NOT NULL CHECK (highway_pct BETWEEN 0 AND 100),
    towing_pct       INTEGER NOT NULL CHECK (towing_pct BETWEEN 0 AND 100),
    hauling_pct      INTEGER NOT NULL CHECK (hauling_pct BETWEEN 0 AND 100),
    -- A JSON array of the towed vehicles' IDs (as text), never the vehicle itself.
    towed            TEXT    NOT NULL DEFAULT '[]',
    reported_economy INTEGER CHECK (reported_economy > 0)
) STRICT, WITHOUT ROWID;
