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
- 🚧 Workspace-based auth & access control *(landed: email+password (argon2id), httpOnly session cookies, users + workspace_members roles, AuthUser extractor + role-gated handlers)*
- 🚧 Core CRUD API for projects and artifacts *(landed: workspaces/projects/artifacts + immutable version append + pipeline links; assets deferred to Phase 3)*
- 🚧 Rate limiting + Turnstile bot protection *(landed: per-IP global + stricter auth tier via tower_governor; Turnstile verify on signup/login with dev bypass. Per-user/workspace quotas deferred to Phase 3)*
- ⏳ AI reliability scaffolding: timeouts, retries, graceful degradation

## Phase 2 — Artifact Lifecycle 🚧

The linked workflow that is the product's core differentiator.

- 🚧 Idea → Flow → Wireframe → Design System → UI Screens pipeline *(generate-from-parent landed; full guided pipeline pending)*
- 🚧 UI-as-Code DSL tree + AI patch loop *(landed: typed DSL + `validate`, Anthropic client (reqwest, opus-4-8, adaptive thinking, retry/backoff, AI_MOCK dev mode), `POST …/generate` + `POST /artifacts/:id/ai-edit`)*
- 🚧 Frontend foundation *(landed: dark-glassmorphism app shell + auth (login/signup) + Workspace Hub wired to the REST API; React 19 + Vite + Tailwind v4 + Geist + Phosphor; `/api` dev proxy for same-origin cookies)*
- 🚧 Project workspace: **User Flow canvas (xyflow) + AI chat** *(landed: `/projects/:id` with chat panel driving generate/ai-edit, flow rendered as a node graph, REST save→manual version with node positions; matches design-screens/02)*
- 🚧 Wireframe canvas *(landed: device-agnostic Element-tree renderer (web/tablet/phone), kind-aware workspace with artifact tabs + New menu, AI generate/edit; matches design-screens/03. tldraw deferred as a future freeform layer)*
- 🚧 Design System + hi-fi screen theming *(landed: design_system artifact → swatches/type-scale/components view (Screen 4); Low-fi/Hi-fi toggle paints the Element tree with project tokens (Screen 5); ui_screen kind)*
- ⏳ WebSocket canvas sync (live multi-client patches) — currently REST save→version
- ⏳ Direct-manipulation editing + multi-screen layouts; asset panel (Screen 6)
- 🚧 Automatic artifact linking *(generate records `derived_from` edges; richer Design-Memory relationships pending)*
- 🚧 Version snapshots per edit, with action/prompt metadata *(AI edits append immutable versions with `change_source='ai'` + prompt)*

Also delivered the deferred Phase 1 **AI-reliability scaffolding** (timeouts, retries, graceful 503 degradation) here, since the LLM client now exists.

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
