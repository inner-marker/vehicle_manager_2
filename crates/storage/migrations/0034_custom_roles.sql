-- Final once merged into dev (decision #062).
-- Phase 6 milestone 4 step 4c: custom roles (issue #27; sub-plan 05; decision #091;
-- phase-6.md milestone 4).
--
-- - `custom_roles`: the household's own roles, each a name and its vehicle permissions (a
--   JSON list of action names, e.g. ["log-fuel","log-reading"]).
-- - `role_grants.custom_role`: a grant of a custom role (its `role` is then 'viewer'). NULL: a
--   built-in role, so every existing grant reads as before.

CREATE TABLE custom_roles (
    id          BLOB    PRIMARY KEY CHECK (length(id) = 16),
    name        TEXT    NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    permissions TEXT    NOT NULL,
    sequence    INTEGER NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
);

CREATE INDEX custom_roles_by_sequence ON custom_roles (sequence);

ALTER TABLE role_grants ADD COLUMN custom_role BLOB
    CHECK (custom_role IS NULL OR length(custom_role) = 16);
