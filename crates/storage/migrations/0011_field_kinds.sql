-- Phase 2 milestone 6, after the checkpoint: a custom field is shown on a list of event kinds
-- and required for some of them, like a meter (the human's choice, replacing step 6d's one
-- kind and yes-or-no). Each field's one kind becomes a one-item list, and its required flag
-- becomes that kind (or none).
-- Not released yet: this file may still change until v0.2.0, then never again.

-- JSON arrays of event kinds, e.g. ["fuel","reading"].
ALTER TABLE custom_fields ADD COLUMN shown_on TEXT NOT NULL DEFAULT '[]';
ALTER TABLE custom_fields ADD COLUMN required_for TEXT NOT NULL DEFAULT '[]';

UPDATE custom_fields SET
    shown_on = json_array(applies_to),
    required_for = CASE WHEN required = 1 THEN json_array(applies_to) ELSE '[]' END;

ALTER TABLE custom_fields DROP COLUMN required;
ALTER TABLE custom_fields DROP COLUMN applies_to;
