-- Phase A2: per-asset version history (the headline). An asset becomes a stable
-- identity; its bytes live in versions. `asset_versions` is the source of truth
-- for history/rollback/diff. `assets.s3_key`/`mime_type` are kept as a
-- denormalized cache of the *head* version's pointer, so the file route, export,
-- mirror, and embedding paths keep working unchanged.
CREATE TABLE asset_versions (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    asset_id    UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    -- Per-asset sequence: v1, v2, … (monotonic; restore appends a new one).
    version     INT NOT NULL,
    s3_key      TEXT NOT NULL,
    mime_type   TEXT,
    prompt      TEXT,
    -- "what changed" for this version (e.g. "Regenerated", "Restored v2").
    change_note TEXT,
    -- Who produced it; null = historical/async (worker) or deleted account.
    created_by  UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (asset_id, version)
);
CREATE INDEX asset_versions_asset_idx ON asset_versions (asset_id, version DESC);

-- Head pointer: the version whose bytes the asset currently resolves to.
ALTER TABLE assets ADD COLUMN IF NOT EXISTS current_version_id UUID REFERENCES asset_versions(id);

-- Backfill: every existing asset gets a v1 from its current pointer, and its
-- head is set to that v1. Reuses the same storage key — no byte copying.
INSERT INTO asset_versions (asset_id, version, s3_key, mime_type, prompt)
SELECT id, 1, s3_key, mime_type, prompt FROM assets;

UPDATE assets a SET current_version_id = v.id
FROM asset_versions v
WHERE v.asset_id = a.id AND v.version = 1;
