# Roadmap

Phased delivery plan for Design Studio AI. Phases are sequenced so each builds on the last; scope within a phase can be adjusted at review.

**Legend:** ✅ done · 🚧 in progress · ⏳ planned · 💡 stretch

> **Current status:** Phase 0 complete. Phases 1+ are **pending proposal approval** — no feature code yet.

---

## Phase 0 — Scaffolding ✅

Foundational dev environment. Reversible groundwork, no product features.

- ✅ Monorepo layout (`backend/`, `frontend/`, `infra/`)
- ✅ Rust + Axum backend skeleton (`/health`, CORS, tracing), stable toolchain pinned
- ✅ React + Vite + TypeScript frontend
- ✅ Postgres 16 + pgvector via Docker Compose (`vector`, `uuid-ossp` enabled)
- ✅ `.env.example`, `.gitignore`, getting-started docs

## Phase 1 — Foundation 🚧

The data model and access layer everything else depends on.

- 🚧 Schema + migrations (sqlx): immutable `artifacts`/`artifact_versions` lineage graph (`parent_id`), `artifact_links` pipeline edges, `assets` S3 binding, 3 embedding tables *(landed in `0001_init`)*
- 🚧 DB pool + migrate-on-boot, DB-backed `/health` *(landed)*
- ⏳ Workspace-based auth & access control
- ⏳ Core CRUD API for projects and artifacts
- ⏳ Rate limiting (workspace / user / IP), Turnstile bot protection
- ⏳ AI reliability scaffolding: timeouts, retries, graceful degradation

## Phase 2 — Artifact Lifecycle ⏳

The linked workflow that is the product's core differentiator.

- ⏳ Idea → Flow → Wireframe → Design System → UI Screens pipeline
- ⏳ UI-as-Code DSL tree + AI patch loop (LLM returns patched JSON)
- ⏳ WebSocket canvas sync (tldraw / xyflow ↔ backend patches)
- ⏳ Automatic artifact linking (Design Memory relationships persisted)
- ⏳ Version snapshots per edit, with action/prompt metadata

## Phase 3 — AI Generation ⏳

- ⏳ Text/structured generation (flows, wireframes, design systems) via Claude
- ⏳ Image generation (screens, illustrations, icons)
- ⏳ Audio generation
- ⏳ S3 storage for generated assets
- ⏳ Auto-generate missing states (error / empty / loading / offline from a success state)

## Phase 4 — RAG & Asset Intelligence ⏳

The retrieval-augmented layer — three embedding pipelines (see [ARCHITECTURE.md](./ARCHITECTURE.md)).

- ⏳ Confirm embedding models + reconcile vector dims (schema currently uses placeholders)
- ⏳ **Semantic** pipeline: embed briefs/chat/rationales → Design Memory Q&A
- ⏳ **Visual** pipeline: CLIP/multimodal embeds of assets → duplicate detection on insert
- ⏳ **Structural** pipeline: JSON layout → markdown → embed → "similar screens"
- ⏳ Auto-tagging & categorization; reuse recommendations

## Phase 5 — Version Intelligence ⏳

- ⏳ AI-summarized version diffs with rationale and historical context

## Stretch & Nice-to-have 💡

- 💡 Asset Lineage Graph — visual artifact dependency graph
- 💡 Collaboration — team workspaces, shared libraries, review workflows
- 💡 Video generation — keyframes, simple animations, motion design

---

## Infrastructure & Quality (cross-cutting)

Tracked alongside feature phases, hardened before launch.

- ⏳ Deployed on AWS (Docker → Fargate, scaling on ECS)
- ⏳ Production-ready UX & error handling
- ⏳ AI fail-safe mechanisms
- ⏳ Persistent storage (Postgres + S3)
