-- Split visual_embeddings into explicit text-caption and pixel vectors so search
-- can fuse metadata similarity with true visual similarity.

ALTER TABLE visual_embeddings
    ADD COLUMN IF NOT EXISTS embedding_text  vector(768),
    ADD COLUMN IF NOT EXISTS embedding_visual vector(768),
    ADD COLUMN IF NOT EXISTS model_text  TEXT,
    ADD COLUMN IF NOT EXISTS model_visual TEXT;

-- Carry forward legacy single-column rows.
UPDATE visual_embeddings
SET embedding_text = embedding,
    model_text     = model
WHERE embedding_text IS NULL AND embedding IS NOT NULL;

DROP INDEX IF EXISTS visual_embeddings_hnsw_idx;

ALTER TABLE visual_embeddings DROP COLUMN IF EXISTS embedding;
ALTER TABLE visual_embeddings DROP COLUMN IF EXISTS model;

CREATE INDEX IF NOT EXISTS visual_embeddings_text_hnsw_idx
    ON visual_embeddings USING hnsw (embedding_text vector_cosine_ops)
    WHERE embedding_text IS NOT NULL;

CREATE INDEX IF NOT EXISTS visual_embeddings_visual_hnsw_idx
    ON visual_embeddings USING hnsw (embedding_visual vector_cosine_ops)
    WHERE embedding_visual IS NOT NULL;
