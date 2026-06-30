-- Embedding QA gate: cache each generated/derived asset's visual style-fit
-- (max cosine similarity to the project's approved assets at creation time) so
-- the board can flag off-style candidates without an extra request per tile.
-- Null = not scored yet / nothing approved to compare against.
ALTER TABLE assets ADD COLUMN IF NOT EXISTS style_fit REAL;
