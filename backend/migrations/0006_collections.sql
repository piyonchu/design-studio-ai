-- Phase 3: collections (asset packs) — the natural unit of export + review handoff.

CREATE TABLE collections (
    id             UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id     UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name           TEXT NOT NULL,
    -- Optional pinned cover; if null the list falls back to the latest item.
    cover_asset_id UUID REFERENCES assets(id) ON DELETE SET NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX collections_project_id_idx ON collections (project_id);

-- Membership. PK (collection_id, asset_id) makes adding the same asset twice a
-- no-op; both sides CASCADE so deleting a collection or an asset stays clean.
CREATE TABLE collection_items (
    collection_id UUID NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    asset_id      UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    added_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (collection_id, asset_id)
);
CREATE INDEX collection_items_asset_idx ON collection_items (asset_id);
