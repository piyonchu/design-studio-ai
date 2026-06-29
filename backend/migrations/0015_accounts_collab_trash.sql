-- Accounts: an editable display name (username) shown on avatars + comments.
ALTER TABLE users ADD COLUMN IF NOT EXISTS display_name TEXT;

-- Trash: soft-delete for projects. `deleted_at IS NULL` = active. A partial
-- index keeps the active-project listing fast.
ALTER TABLE projects ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;
CREATE INDEX IF NOT EXISTS projects_active_idx
    ON projects (workspace_id, created_at DESC) WHERE deleted_at IS NULL;

-- (Team collaboration reuses the existing workspace_members(workspace_id,
--  user_id, role) table — no schema change, just new endpoints.)
