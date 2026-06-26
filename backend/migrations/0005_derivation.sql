-- Phase 2: reference-driven derivation provenance.

-- Edge: a derivative points back to the base it was derived from. This is the
-- core of the product — every derivative records where it came from, and under
-- which canon version, so a base/style change can flag stale derivatives later.
CREATE TYPE asset_relation AS ENUM ('derived_from', 'variant_of');

CREATE TABLE asset_links (
    id         UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    from_asset UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,  -- the derivative
    to_asset   UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,  -- the base
    relation   asset_relation NOT NULL DEFAULT 'derived_from',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (from_asset, to_asset, relation)
);
CREATE INDEX asset_links_from_idx ON asset_links (from_asset);
CREATE INDEX asset_links_to_idx   ON asset_links (to_asset);

-- How a derivative was produced. `method='deterministic'` is reserved for
-- pixel ops (recolor/resize) that must NOT go through the model — the spike
-- showed generative recolor drifts identity.
ALTER TABLE assets ADD COLUMN derivation TEXT;                              -- preset id / instruction
ALTER TABLE assets ADD COLUMN method     TEXT NOT NULL DEFAULT 'generative'; -- 'generative' | 'deterministic'
