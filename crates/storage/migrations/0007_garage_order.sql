-- Phase 2 milestone 4 step 4e: how each person sorts the Garage, and their custom order
-- (decision #056).
-- Not released yet: this file may still change until v0.2.0, then never again.

ALTER TABLE preferences ADD COLUMN garage_sort TEXT
    CHECK (garage_sort IN ('newest-activity', 'name', 'custom'));

-- One row per person (its ID is the user's): their custom order, a JSON list of asset IDs.
-- Vehicles not listed come after the listed ones, so the list never needs to be complete.
CREATE TABLE garage_orders (
    id       BLOB    PRIMARY KEY CHECK (length(id) = 16),
    vehicles TEXT    NOT NULL DEFAULT '[]',
    sequence INTEGER NOT NULL,
    deleted  INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;
