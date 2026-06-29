-- Keyset pagination: the board lists assets per project ordered by
-- (created_at DESC, id DESC). The existing single-column assets(project_id)
-- index forces a sort; this composite index serves the order directly so paging
-- stays fast as a project's asset count grows.
CREATE INDEX IF NOT EXISTS assets_project_created_idx
    ON assets (project_id, created_at DESC, id DESC);
