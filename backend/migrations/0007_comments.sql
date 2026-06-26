-- Phase 3 PR4: collaboration — comments on assets. The review queue + inspector
-- thread surface these so a team can discuss a candidate before it's approved.

CREATE TABLE asset_comments (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    asset_id   UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    -- Author kept even-handedly: deleting a user nulls the attribution rather
    -- than vanishing the discussion (RESTRICT would block account deletion).
    author_id  UUID REFERENCES users(id) ON DELETE SET NULL,
    body       TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX asset_comments_asset_idx ON asset_comments (asset_id, created_at);
