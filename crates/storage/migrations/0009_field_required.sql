-- Phase 2 milestone 6 step 6d: a custom field can be required on its kind of event (choice 5,
-- idea 17). Like a required reading, it's given, or skipped with a reason; a field that's
-- turned off is never required.
-- Not released yet: this file may still change until v0.2.0, then never again.

ALTER TABLE custom_fields ADD COLUMN required INTEGER NOT NULL DEFAULT 0
    CHECK (required IN (0, 1));
