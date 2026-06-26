-- The moat loop: approved assets can be flagged as style exemplars. Generation
-- then conditions on them (reference img2img), so new assets inherit the
-- approved art direction — "the project remembers your style".
ALTER TABLE assets ADD COLUMN exemplar BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX assets_exemplar_idx ON assets (project_id) WHERE exemplar;
