# Roadmap — CanonForge

Execution status + what's next. Full product vision: [PLAN.md](PLAN.md). Practical
setup + gotchas: [HANDOFF.md](HANDOFF.md).

**Legend:** ✅ done · 🚧 in progress · ⏳ planned · 💡 stretch

## Status

| Phase | What | Status |
|-------|------|--------|
| 0 | Scaffolding (monorepo, Docker, auth) | ✅ |
| 1 | Generic core + canon + bring-a-base (generate/upload) | ✅ |
| — | Cleanup: dropped dead UI-as-Code domain + CRLF/.gitattributes fix | ✅ |
| 2 | Reference-driven derivation (img2img) + canon-bound prompts + review gate | ✅ |
| 3 · PR1 | Asset inspector + lineage strip + delete | ✅ |
| 3 · PR2 | Collections (packs) + env-bool fix | ✅ |
| 3 · PR3 | Smart asset board (filters · search · status language · batch) | ✅ |
| 3 · PR4 | Collaboration: review queue + comment threads | ✅ |
| 3 · PR5 | Lineage graph + canon-change propagation | ✅ |
| 3.5 | Visual intelligence — embedding pipeline (search · dedup · find-similar) | ✅ (real text embedder `text-embedding-3-small`, cached, behind `EMBED_MOCK`; pixel-CLIP later) |
| 4 | Audio modality — provider boundary + mock WAV + generate/play | ✅ (mock; no hosted provider yet) |
| 5 · PR1 | Export — generic zip + manifest + pre-export checks | ✅ |
| 5 · PR2 | Export — role/tag-grouped pack (vertical-neutral) | ✅ |
| 3.5 PR2 | Semantic context RAG — "Ask this project" over briefs/prompts/comments/canon | ✅ |
| 3.5 PR3 | LLM answer-synthesis over retrieval + disk cache (cheap real spend) | ✅ (google/gemini-2.5-flash; cached) |
| — | Smart versioning — auto canon change-notes (deterministic diff) + history | ✅ |
| — | Asset naming — editable name + auto-derived display labels (drives export filename) | ✅ |
| — | **Exemplar loop** — approved assets condition future generation (moat closed) | ✅ |
| — | Generation recipes — reusable derivation templates (apply/save in derive panel) | ✅ |
| — | Batch derive — "Derive all" runs every preset → a whole set in one click | ✅ |
| — | Style-fit score — embedding similarity to approved assets, shown at review | ✅ |
| 6 | 2nd vertical — **manhwa/webtoon** (config-only; proves core generalizes) | ✅ |
| — | **Vertical-adapter framework** — registry per side (BE prompt rules + validation; FE picker single-source); 3rd vertical **illustration** | ✅ |
| — | Test suite (18 unit tests over core logic) + GitHub Actions CI | ✅ |
| — | Activity feed — merged asset/comment/canon timeline (Activity tab) | ✅ |
| — | Engine adapters (Godot/Unity) consuming the grouped manifest | ⏳ |
| — | Nav shell — left rail replaces the tab bar | ✅ (slide-overs later) |

### Done — Phase 3.5 (visual intelligence, mock embeddings)
Mock-first feature-hashed embedder (`ai/embeddings.rs`, `EMBED_MOCK` default) —
a real text/CLIP model swaps in behind the same signature (gated on spend).
- Embed-on-insert across generate / derive / **upload (imports)** / audio;
  `/embeddings/backfill` indexes anything missing.
- **Smart search** (`/assets/search?q`) — semantic/keyword ranking in the board.
- **Pre-generate dedup nudge** (`/assets/similar-check`) — "N similar already
  exist" with thumbnails before you spend a generation.
- **Find similar** (`/assets/:id/similar`). `visual_embeddings` store, cosine.
Verified: query ranks matches over non-matches; dup prompt flagged ~0.89; novel
prompt → 0 false positives.

### Done — Phase 3.5 PR2 (semantic context, mock)
"Ask this project" retrieves the most relevant snippets (brief / asset prompt /
comment / canon) for a question — `semantic_embeddings`, same mock embedder.
`/context?q` + `/context/backfill`; box atop the Canon tab. Retrieval-only; an
LLM synthesis layer (true generated answers) can sit on top later.

### Done — Smart versioning (canon change notes)
Canon versions now carry an auto-generated **deterministic diff** note ("palette:
'…' → '…'; set perspective to '…'; +1 negative") — no LLM, more honest than a
guess. `GET /canon/history` + a version-history list in the Canon tab.
Asset-level autoname (display names) is a possible follow-up.

## Next: Nav shell (left rail) — or pick the next vertical
Export is generic and grouped; engine-specific packers (Godot/Unity) are
**deferred** — they're a per-vertical adapter, and per PLAN we don't build the
adapter layer until 2–3 verticals exist (rule of three). The grouped manifest
(`groups[]` keyed by role/tag) is the seam an adapter will consume.

### Done — Phase 4 (audio modality, mock)
- `ai/audio.rs` mirrors the image boundary: `AUDIO_MOCK=true` synthesizes a
  deterministic WAV; real mode 503 until a hosted provider is wired.
- `POST /projects/:id/audio` stores `kind='audio'` assets; the board has an
  image/audio toggle and plays clips inline. Second modality from the brief now
  in the product. (Waveform/duration display + audio-specific checks: later.)

Good next moves (no spend, no new vertical lock-in):
- **Nav shell** — replace the growing tab bar (Canon/Assets/Review/Lineage/
  Collections) with a left rail + slide-overs, matching the design mockups.
- Export-from-board (multi-select → export) and approved-assets-feed-canon are
  small high-value follow-ups.
- Phase 3.5 visual-intelligence stays parked (its spike spends on the shared key).

### Done — Phase 5 (export, vertical-neutral)
- **Pre-export checks** — `POST /export/check`: per-asset filename,
  format/dimensions/alpha (png/jpeg), issues. Blocking = rejected / undecodable.
- **Grouped pack** — `POST /export`: a zip of `manifest.json` (project, canon
  version, `groups[]` by role/tag, per-asset metadata + skipped list) +
  `assets/<group>/<file>`. Frontend: an Export dialog from a collection shows
  the grouped report, then downloads. Engine adapters consume `groups[]` later.
- Then a **Godot package** (Unity later).

### Done — Phase 3 PR5 (lineage + canon propagation)
- **Lineage tab** — roots → derivatives as a connector-lined tree; click to inspect.
- **Canon drift** — assets predating the current canon flagged stale; `GET /lineage`
  (assets + `derived_from` edges) + `POST /reconcile` (rebind to current canon).
  Per-node Keep (reconcile) / Regenerate (re-run generate/derive under new canon)
  + a "Keep all" banner action.

### Done — Phase 3 PR4 (collaboration)
- **Review queue** — a "Review" tab: candidate + needs-review backlog as a
  worklist; focused candidate shows preview + approve / needs-review / reject
  with its discussion side-by-side, decision advances to the next.
- **Comment threads** (`asset_comments`, migration 0007) — per-asset discussion
  in both the inspector and the queue; author + relative time, delete-own
  (Owner can moderate). Activity feed deferred (no events table yet).

### Done — Phase 3 PR3 (smart asset board)
Client-side over existing endpoints, no backend changes:
- **Filters** — status · role · source_kind · collection (counts + lazy collection membership).
- **Search** — role / prompt / derivation / kind / tags.
- **Status as visual language** — candidate = amber ring · approved = solid teal · needs_review = rose + pulsing flag · rejected = dimmed.
- **Multi-select** → batch approve / reject / add-to-collection.
- Dedup ("2 similar") deferred to Phase 3.5.

## Phase 3.5 — Visual intelligence (gate before building)
1. **Spike first:** pick an embedding model (CLIP/multimodal), embed a few assets, test whether cosine similarity reliably catches near-dups + "similar", and at what cost. No-go → don't build.
2. If it holds: embed-on-insert (`visual_embeddings` table exists, no pipeline yet) → dedup nudge + "find visually similar" + reuse recommendations.

## Phase 5 — Export (the wedge)
Pick assets / a collection → zip + `manifest.json` (generic), sprite atlas, then a
**Godot package** (Unity later). Pre-export deterministic checks (alpha / sizes /
naming) with pass/fail before download.

## Known follow-ups / debt
- Prompt polish: custom-instruction identity guard; label canon style fields; clean the double-negative in negatives.
- Deterministic recolor (the `assets.method` column is reserved for it — generative recolor drifts identity, per the spike).
- Approved assets don't yet feed back into the canon as exemplars.
- Tech-check details in the inspector (alpha present, dimensions) are stubs.
