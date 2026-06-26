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
| 3.5 | Visual intelligence (embeddings/dedup/similar) — **spike-gated** | ⏳ (needs spend go-ahead) |
| 4 | (per PLAN) deeper RAG / asset intelligence | ⏳ |
| **5** | **Export — Godot pack first, then Unity** | 🚧 next |
| — | Nav shell (left rail + slide-overs, replace tabs) | ⏳ |

## Next: Phase 5 — Export (the wedge)
Phase 3 is complete. Next concrete, no-spend slice is **export** (Phase 3.5
visual-intelligence is parked — it needs a spike that spends on the shared key,
so it waits for a go-ahead). Export:
- Pick assets / a collection → zip + `manifest.json` (generic).
- Pre-export deterministic checks (alpha / sizes / naming) with pass/fail.
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
