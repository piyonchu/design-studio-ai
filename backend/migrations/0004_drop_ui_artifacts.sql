-- Drop the abandoned UI-as-Code domain (pre-pivot). No remaining readers after
-- routes/artifacts.rs was removed; CanonForge uses canon + assets only.

ALTER TABLE assets DROP COLUMN IF EXISTS screen_id;

-- CASCADE handles the artifacts <-> artifact_versions circular FK and the
-- structural_embeddings dependency in one shot.
DROP TABLE IF EXISTS structural_embeddings, artifact_links, artifact_versions, artifacts CASCADE;

DROP TYPE IF EXISTS artifact_kind, change_source, link_relation;
