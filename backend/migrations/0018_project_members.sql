-- Phase C: per-project role overrides + a reviewer gate. Layered ON TOP of
-- workspace roles — absent → the member's workspace role applies. A `reviewer`
-- (and `owner`) may approve; an `editor` may submit for review but not
-- self-approve. NOT per-folder ACLs (too heavy for a small-team product).
CREATE TYPE project_role AS ENUM ('viewer', 'editor', 'reviewer', 'owner');

CREATE TABLE project_members (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role       project_role NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, user_id)
);
CREATE INDEX project_members_user_idx ON project_members (user_id);
