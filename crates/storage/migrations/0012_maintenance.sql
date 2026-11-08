-- Final once merged into dev (decision #062).
-- Phase 2 milestone 7 step 7d: squawks, requirements and bundles, and maintenance and repair
-- events' work: tasks, parts, repair details, end readings, and squawk dispositions (plan §3
-- Maintenance and Squawks; decisions #032, #051, #063).
--
-- A maintenance or repair event's work is part of the event, like its fuel details: kept in
-- tables of its own, rewritten whole when it changes, and never changed once finalized. Lists
-- keep their order in a `position` column (0, 1, 2, ...). Name lengths are checked in characters
-- by the domain; the limits here are in bytes.

-- Readings get a set: 'start' (every reading so far) or 'end' (a job's readings when it was
-- done, e.g. after a test drive). The key changes, so the table is rebuilt; nothing refers to it.
CREATE TABLE event_readings_new (
    event_id    BLOB    NOT NULL REFERENCES events (id),
    reading_set TEXT    NOT NULL DEFAULT 'start' CHECK (reading_set IN ('start', 'end')),
    meter_id    BLOB    NOT NULL REFERENCES meters (id),
    value       INTEGER NOT NULL CHECK (value >= 0),
    PRIMARY KEY (event_id, reading_set, meter_id)
) STRICT, WITHOUT ROWID;

INSERT INTO event_readings_new (event_id, reading_set, meter_id, value)
    SELECT event_id, 'start', meter_id, value FROM event_readings;

DROP TABLE event_readings;
ALTER TABLE event_readings_new RENAME TO event_readings;
CREATE INDEX event_readings_by_meter ON event_readings (meter_id);

-- Squawks: problems noticed on a vehicle (#051, #063). Their status comes from the
-- dispositions of finalized events, so it isn't stored here.
CREATE TABLE squawks (
    id          BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id    BLOB    NOT NULL REFERENCES assets (id),
    description TEXT    NOT NULL CHECK (length(description) BETWEEN 1 AND 2000),
    -- A Markdown subset (#055); empty means none.
    notes       TEXT    NOT NULL DEFAULT '',
    -- As opened; a disposition may change it later.
    severity    TEXT    NOT NULL CHECK (severity IN ('low', 'medium', 'high')),
    opened_ms   INTEGER NOT NULL,
    opened_by   BLOB    NOT NULL REFERENCES users (id),
    found_on_id BLOB    REFERENCES events (id),
    received_ms INTEGER NOT NULL,
    sequence    INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX squawks_by_asset ON squawks (asset_id) WHERE deleted = 0;

-- Requirements: something done to one vehicle, and how often.
CREATE TABLE requirements (
    id       BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id BLOB    NOT NULL REFERENCES assets (id),
    name     TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    notes    TEXT    NOT NULL DEFAULT '',
    enabled  INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    -- A JSON array of interval rules, e.g. [{"rule":"calendar","months":12,"end_of_month":false}].
    rules    TEXT    NOT NULL DEFAULT '[]',
    sequence INTEGER NOT NULL,
    deleted  INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX requirements_by_asset ON requirements (asset_id) WHERE deleted = 0;

-- Bundles: services done as one, e.g. "10,000-mile service".
CREATE TABLE bundles (
    id           BLOB    PRIMARY KEY CHECK (length(id) = 16),
    asset_id     BLOB    NOT NULL REFERENCES assets (id),
    name         TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    -- A JSON array of the requirements' IDs (as text), in order.
    requirements TEXT    NOT NULL DEFAULT '[]',
    sequence     INTEGER NOT NULL,
    deleted      INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

CREATE INDEX bundles_by_asset ON bundles (asset_id) WHERE deleted = 0;

-- A maintenance or repair event's work: its costs' currency and a repair's complaint and cause.
CREATE TABLE work_details (
    event_id  BLOB PRIMARY KEY REFERENCES events (id),
    currency  TEXT NOT NULL CHECK (length(currency) = 3 AND currency = upper(currency)),
    complaint TEXT,
    cause     TEXT
) STRICT, WITHOUT ROWID;

-- Each task, in order.
CREATE TABLE event_tasks (
    event_id         BLOB    NOT NULL REFERENCES work_details (event_id),
    position         INTEGER NOT NULL CHECK (position >= 0),
    title            TEXT    NOT NULL CHECK (length(title) BETWEEN 1 AND 160),
    requirement_id   BLOB    REFERENCES requirements (id),
    notes            TEXT    NOT NULL DEFAULT '',
    labor_seconds    INTEGER CHECK (labor_seconds >= 0),
    labor_cost_minor INTEGER CHECK (labor_cost_minor >= 0),
    -- Which readings reset the requirement.
    resets_from      TEXT    NOT NULL CHECK (resets_from IN ('start', 'end')),
    -- A fixed-date rule's next due date from the paperwork, e.g. '2027-11-30'.
    next_due         TEXT    CHECK (length(next_due) = 10),
    PRIMARY KEY (event_id, position)
) STRICT, WITHOUT ROWID;

CREATE INDEX event_tasks_by_requirement ON event_tasks (requirement_id)
    WHERE requirement_id IS NOT NULL;

-- Each task's parts, in order. Quantities are thousandths; prices are the currency's minor unit.
CREATE TABLE task_parts (
    event_id         BLOB    NOT NULL,
    task_position    INTEGER NOT NULL,
    position         INTEGER NOT NULL CHECK (position >= 0),
    description      TEXT    NOT NULL CHECK (length(description) BETWEEN 1 AND 400),
    part_number      TEXT    CHECK (length(part_number) BETWEEN 1 AND 160),
    quantity_milli   INTEGER NOT NULL CHECK (quantity_milli > 0),
    unit_price_minor INTEGER CHECK (unit_price_minor >= 0),
    PRIMARY KEY (event_id, task_position, position),
    FOREIGN KEY (event_id, task_position) REFERENCES event_tasks (event_id, position)
) STRICT, WITHOUT ROWID;

-- What the event does about each outstanding squawk: signed off (with notes and the task that
-- fixed it, if any) or deferred (with a reason); and a new severity, if it changed. Counts
-- once the event is finalized.
CREATE TABLE squawk_dispositions (
    event_id      BLOB    NOT NULL REFERENCES work_details (event_id),
    squawk_id     BLOB    NOT NULL REFERENCES squawks (id),
    position      INTEGER NOT NULL CHECK (position >= 0),
    action        TEXT    NOT NULL CHECK (action IN ('signed-off', 'deferred')),
    notes         TEXT    NOT NULL DEFAULT '',
    task_position INTEGER CHECK (task_position >= 0),
    reason        TEXT    CHECK (length(reason) BETWEEN 1 AND 2000),
    severity      TEXT    CHECK (severity IN ('low', 'medium', 'high')),
    PRIMARY KEY (event_id, squawk_id),
    CHECK ((action = 'deferred') = (reason IS NOT NULL)),
    CHECK (action = 'signed-off' OR (task_position IS NULL AND notes = ''))
) STRICT, WITHOUT ROWID;

CREATE INDEX squawk_dispositions_by_squawk ON squawk_dispositions (squawk_id);
