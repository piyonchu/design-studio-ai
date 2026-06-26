-- Generation recipes: reusable derivation templates ("recolor to palette",
-- "side-walk pose", "matching enemy") a team can apply to any base asset.
CREATE TABLE generation_recipes (
    id          UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id  UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    instruction TEXT NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX generation_recipes_project_idx ON generation_recipes (project_id, created_at DESC);
