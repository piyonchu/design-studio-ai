-- Async generation jobs: a DB-backed queue an in-process worker drains, so
-- long / batch generation is decoupled from the request (the client enqueues a
-- job and polls its status). `status` is TEXT + CHECK rather than a Postgres
-- enum to avoid custom-enum decode friction with runtime sqlx.
CREATE TABLE jobs (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,                       -- 'generate' (more later)
    status      TEXT NOT NULL DEFAULT 'queued'
                CHECK (status IN ('queued', 'running', 'succeeded', 'failed')),
    payload     JSONB NOT NULL DEFAULT '{}'::jsonb,  -- request params
    result      JSONB,                               -- {asset_ids:[...]} on success
    error       TEXT,                                -- message on failure
    attempts    INT NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);
CREATE INDEX jobs_project_idx ON jobs (project_id, created_at DESC);
-- Partial index for the worker's "next queued job" claim.
CREATE INDEX jobs_queued_idx ON jobs (created_at) WHERE status = 'queued';
