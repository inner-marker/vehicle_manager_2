-- Phase 2 milestone 3 step 3d: each person's read and dismissed marks on messages
-- (decision #050). Messages themselves aren't stored: they're derived from the current state.
-- Not released yet: this file may still change until v0.2.0, then never again.

CREATE TABLE message_marks (
    id        BLOB    PRIMARY KEY CHECK (length(id) = 16),
    user_id   BLOB    NOT NULL REFERENCES users (id),
    key       TEXT    NOT NULL CHECK (length(key) BETWEEN 1 AND 200),
    read      INTEGER NOT NULL DEFAULT 0 CHECK (read IN (0, 1)),
    dismissed INTEGER NOT NULL DEFAULT 0 CHECK (dismissed IN (0, 1)),
    sequence  INTEGER NOT NULL,
    deleted   INTEGER NOT NULL DEFAULT 0 CHECK (deleted IN (0, 1))
) STRICT;

-- One mark per person and message.
CREATE UNIQUE INDEX message_marks_by_user_and_key ON message_marks (user_id, key) WHERE deleted = 0;
