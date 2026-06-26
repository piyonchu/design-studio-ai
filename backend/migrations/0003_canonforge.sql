-- CanonForge Phase 1: game language + canon + asset lifecycle.
-- Additive only — dead UI-domain tables (artifacts/*) are dropped in a later
-- migration so this one is a clean, reversible review on its own.

-- ── Game language ───────────────────────────────────────────────────────────
-- Which vertical a project targets. Lets a 2nd vertical branch later w/o rework.
ALTER TABLE projects ADD COLUMN vertical TEXT NOT NULL DEFAULT 'game_2d';

-- ── Asset review lifecycle ──────────────────────────────────────────────────
-- Everything starts 'candidate'; only 'approved' assets enter the canon and
-- influence future derivations (one bad generation can't poison identity).
CREATE TYPE asset_status AS ENUM ('candidate', 'approved', 'rejected', 'needs_review');

-- ── Canon: style rules + exemplars, versioned ───────────────────────────────
-- `data` JSONB holds { style:{...}, negative:[...], exemplar_asset_ids:[...] }.
-- Versioned via parent_id lineage so a base/style change is "this predates v2 —
-- regenerate or keep?" rather than a destructive edit.
CREATE TABLE canon (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    parent_id  UUID REFERENCES canon(id) ON DELETE SET NULL,
    version    INT  NOT NULL DEFAULT 1,
    data       JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX canon_project_id_idx ON canon (project_id);

-- ── Reshape assets around a generic asset engine ────────────────────────────
-- Vertical-specific fields live in `metadata` JSONB; the columns here are the
-- domain-neutral spine (role/status/tags) shared by every vertical.
ALTER TABLE assets ADD COLUMN role             TEXT;
ALTER TABLE assets ADD COLUMN status           asset_status NOT NULL DEFAULT 'candidate';
ALTER TABLE assets ADD COLUMN description      TEXT;
ALTER TABLE assets ADD COLUMN tags             TEXT[] NOT NULL DEFAULT '{}';
ALTER TABLE assets ADD COLUMN metadata         JSONB  NOT NULL DEFAULT '{}';
ALTER TABLE assets ADD COLUMN canon_version_id UUID REFERENCES canon(id) ON DELETE SET NULL;
-- How the asset entered the library: 'uploaded' | 'seeded' | 'derived'.
ALTER TABLE assets ADD COLUMN source_kind      TEXT NOT NULL DEFAULT 'uploaded';
