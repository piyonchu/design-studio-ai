-- Authentication & workspace access control.
-- Users, team membership (workspace_members with roles), and server-side
-- sessions (opaque token in an httpOnly cookie). See ARCHITECTURE.md / ROADMAP.

CREATE TYPE workspace_role AS ENUM ('owner', 'editor', 'viewer');

CREATE TABLE users (
    id            UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    email         TEXT NOT NULL,
    password_hash TEXT NOT NULL,   -- argon2id PHC string
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- Case-insensitive unique email (avoids the citext extension).
CREATE UNIQUE INDEX users_email_lower_idx ON users (lower(email));

-- Team membership: which users may access which workspaces, and at what role.
CREATE TABLE workspace_members (
    workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    user_id      UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role         workspace_role NOT NULL DEFAULT 'editor',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (workspace_id, user_id)
);
CREATE INDEX workspace_members_user_id_idx ON workspace_members (user_id);

-- Server-side sessions. `token` is a 256-bit random value (hex) stored in the
-- ds_session cookie; rows are deleted on logout and ignored once expired.
CREATE TABLE sessions (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id    UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token      TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX sessions_user_id_idx ON sessions (user_id);
