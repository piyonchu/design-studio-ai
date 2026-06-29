-- Phase A1: folder tree — an asset's canonical home (one tree, like files).
-- Coexists with collections (cross-cutting curated sets). Root = folder_id NULL.
CREATE TABLE folders (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    -- Self-FK for the tree. A deleted parent cascades to its subtree; the assets
    -- inside fall back to root (folder_id SET NULL below).
    parent_id  UUID REFERENCES folders(id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX folders_project_idx ON folders (project_id);
CREATE INDEX folders_parent_idx ON folders (parent_id);

-- An asset's home folder. NULL = project root (unfiled). Deleting a folder
-- unfiles its assets rather than destroying them (the bytes outlive the tree).
ALTER TABLE assets ADD COLUMN IF NOT EXISTS folder_id UUID REFERENCES folders(id) ON DELETE SET NULL;
CREATE INDEX assets_folder_idx ON assets (folder_id);
