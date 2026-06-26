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
| **3 · PR3** | **Smart asset board** | 🚧 next |
| 3 · PR4 | Collaboration + review queue | ⏳ |
| 3 · PR5 | Lineage graph + canon-change propagation | ⏳ |
| 3.5 | Visual intelligence (embeddings/dedup/similar) — **spike-gated** | ⏳ |
| 4 | (per PLAN) deeper RAG / asset intelligence | ⏳ |
| 5 | Export — Godot pack first, then Unity | ⏳ |
| — | Nav shell (left rail + slide-overs, replace tabs) | ⏳ |

## Next: Phase 3 PR3 — Smart asset board
Make the board manageable (no ML — that's 3.5):
- **Filters** — role · status · source_kind · collection.
- **Search** — name / tags / prompt (client-side or simple SQL).
- **Status as visual language** — candidate = dashed amber · approved = solid · needs_review = pulsing flag.
- **Multi-select** → batch approve / add-to-collection.
- Defer dedup ("2 similar") to Phase 3.5.

## Phase 3 — remaining detail
- **PR4 Collaboration** — review queue (candidates awaiting approval) + comments (`asset_comments` table) + activity feed; roles/teams already exist in the backend, just surface them.
- **PR5 Lineage graph** — canon version → assets → variants graph; "regenerate or keep" batch when a base/canon changes (uses `canon_version_id` to find stale assets). The moat made visible.

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
