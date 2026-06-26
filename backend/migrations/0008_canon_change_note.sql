-- Smart versioning: a human-readable "what changed" note per canon version,
-- computed deterministically from the diff against the parent version.
ALTER TABLE canon ADD COLUMN change_note TEXT;
