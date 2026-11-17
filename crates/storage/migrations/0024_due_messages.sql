-- Final once merged into dev (decision #062).
-- Phase 5 milestone 3 step 3b: due messages and snooze (decision #083; phase-5.md milestone 3).
--
-- New columns only, each NULL until set, so every existing row reads as before.
-- - `message_marks.snooze`: a person's snooze on a message, as JSON:
--   {"until":1700000000000,"meter":{"meter":"…","at":135000000}} (either part may be null).
-- - `preferences.due_messages_from`: the step at which what's due starts sending messages,
--   by name ("coming-up", "due-soon", or "due-very-soon"); the household's record only.

ALTER TABLE message_marks ADD COLUMN snooze TEXT;
ALTER TABLE preferences ADD COLUMN due_messages_from TEXT;
