-- Phase 1 schema for Design Studio AI.
-- Immutable artifact versioning + lineage graph, S3 asset binding, and three
-- pgvector embedding stores (semantic / visual / structural). See ARCHITECTURE.md.
--
-- Self-contained: re-declares extensions so it applies against any fresh DB,
-- not just the docker-compose container (which also enables them via init script).

CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- ── Enums ──────────────────────────────────────────────────────────────────
CREATE TYPE artifact_kind AS ENUM ('idea', 'user_flow', 'wireframe', 'design_system', 'ui_screen');
CREATE TYPE asset_kind    AS ENUM ('image', 'icon', 'illustration', 'audio', 'svg');
CREATE TYPE change_source AS ENUM ('manual', 'ai', 'import');
CREATE TYPE link_relation AS ENUM ('derived_from', 'references', 'contains');

-- ── Tenancy ────────────────────────────────────────────────────────────────
CREATE TABLE workspaces (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE projects (
    id           UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    brief        TEXT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX projects_workspace_id_idx ON projects (workspace_id);

-- ── Artifacts: logical identity + immutable version snapshots ───────────────
CREATE TABLE artifacts (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id      UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind            artifact_kind NOT NULL,
    name            TEXT NOT NULL,
    head_version_id UUID,  -- current snapshot; FK added after artifact_versions exists
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX artifacts_project_id_idx ON artifacts (project_id);

-- Every save/generation is a new row; lineage tree via parent_id. `content` is
-- the UI-as-Code DSL tree. change_summary/prompt power AI version summaries.
CREATE TABLE artifact_versions (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    artifact_id    UUID NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    parent_id      UUID REFERENCES artifact_versions(id) ON DELETE SET NULL,
    content        JSONB NOT NULL,
    change_source  change_source NOT NULL,
    change_summary TEXT,
    prompt         TEXT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX artifact_versions_artifact_id_idx ON artifact_versions (artifact_id);
CREATE INDEX artifact_versions_parent_id_idx   ON artifact_versions (parent_id);

ALTER TABLE artifacts
    ADD CONSTRAINT artifacts_head_version_fk
    FOREIGN KEY (head_version_id) REFERENCES artifact_versions(id) ON DELETE SET NULL;

-- Pipeline edges across artifacts (Idea → Flow → Wireframe → …).
CREATE TABLE artifact_links (
    id               UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    from_artifact_id UUID NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    to_artifact_id   UUID NOT NULL REFERENCES artifacts(id) ON DELETE CASCADE,
    relation         link_relation NOT NULL DEFAULT 'derived_from',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (from_artifact_id, to_artifact_id, relation)
);

-- ── Media assets (binaries in S3, metadata here) ────────────────────────────
CREATE TABLE assets (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    screen_id  UUID REFERENCES artifacts(id) ON DELETE SET NULL,  -- the ui_screen it belongs to
    kind       asset_kind NOT NULL,
    s3_key     TEXT NOT NULL,
    mime_type  TEXT,
    width      INT,
    height     INT,
    prompt     TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX assets_project_id_idx ON assets (project_id);
CREATE INDEX assets_screen_id_idx  ON assets (screen_id);

-- ── RAG: three embedding stores ─────────────────────────────────────────────
-- NOTE: vector dimensions are PLACEHOLDERS (1024 text / 768 CLIP). Reconcile
-- with the chosen embedding models before the RAG phase ships.

-- 1. Semantic context: briefs, chat, rationales.
CREATE TABLE semantic_embeddings (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL,        -- 'brief' | 'chat' | 'rationale' | ...
    source_id   UUID,                 -- soft reference to the originating row
    content     TEXT NOT NULL,
    embedding   vector(1024) NOT NULL,
    model       TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX semantic_embeddings_hnsw_idx
    ON semantic_embeddings USING hnsw (embedding vector_cosine_ops);

-- 2. Visual dedup: CLIP/multimodal embeddings over assets.
CREATE TABLE visual_embeddings (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    asset_id   UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    embedding  vector(768) NOT NULL,
    model      TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX visual_embeddings_hnsw_idx
    ON visual_embeddings USING hnsw (embedding vector_cosine_ops);

-- 3. Layout topology: structural markdown over artifact versions.
CREATE TABLE structural_embeddings (
    id                  UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    artifact_version_id UUID NOT NULL REFERENCES artifact_versions(id) ON DELETE CASCADE,
    markdown            TEXT NOT NULL,  -- e.g. "Screen has 1 input, 2 buttons"
    embedding           vector(1024) NOT NULL,
    model               TEXT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX structural_embeddings_hnsw_idx
    ON structural_embeddings USING hnsw (embedding vector_cosine_ops);
