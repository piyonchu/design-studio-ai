-- Smart naming: an optional human display name per asset. Null = fall back to an
-- auto-derived name (role + prompt) in the UI; set = an explicit override that
-- also drives the export filename.
ALTER TABLE assets ADD COLUMN name TEXT;
