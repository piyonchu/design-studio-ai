# Architecture

Canonical record of the core architectural decisions for Design Studio AI. The README summarizes; this document is the source of truth for *why* the system is built this way.

---

## Generation Pipeline

The product is an intelligent pipeline that converts a text prompt into production-ready UI, tracking lineage, versions, and reusable assets along the way:

```
Idea → User Flow (logic) → Wireframe (layout) → Design System (global JSON styles)
     → UI Screens (Wireframe + Design System) → Assets (generated media)
```

Each stage produces an **artifact**. Artifacts are linked, so the system always knows what a screen was derived from and which assets belong to it.

---

## 1. UI-as-Code (the DSL approach)

**No vision models for layout.** The system never screenshots the canvas for the AI to "see". Instead, every structural artifact (flows, wireframes, UI screens, design systems) is a lightweight **JSON/DSL tree**.

**Modification loop:**

- **User drags an element** in tldraw → frontend mutates the JSON locally and sends a **WebSocket patch** to the Rust backend.
- **User asks the AI to change layout** → the backend feeds *the text prompt + current JSON state* to the LLM. The LLM returns a **patched JSON**. The backend persists it and pushes the new JSON to the frontend to re-render.

This keeps edits cheap, diffable, and deterministic — and lets the LLM reason about structure (not pixels).

---

## 2. Immutable versioning & lineage graph

**Git-like snapshots.** Artifacts are never overwritten. Every manual save or AI generation inserts a **new row** with a `parent_id` pointing at the previous state, forming a lineage tree.

**Action metadata.** The trigger for each change is stored alongside the snapshot — e.g. `"User dragged button"` or `"AI: switch to dark mode"` (with the originating prompt). This is what lets the LLM produce automated **version summaries** and answer "why does this exist?".

Modeled as two tables: `artifacts` (logical identity) + `artifact_versions` (immutable snapshots, `parent_id` self-reference). `artifacts.head_version_id` points at the current snapshot.

---

## 3. RAG & search intelligence (pgvector)

Three **distinct** embedding types, each indexed separately (different models, different dimensions, HNSW cosine indexes):

| Type | Source | Purpose |
|---|---|---|
| **Semantic** (text) | design briefs, chat history, rationales | Answer "why was this screen created?" |
| **Visual** (CLIP / multimodal) | binary assets (images, icons) | Dedup — flag "a similar illustration already exists" via vector distance on insert |
| **Structural** (text) | JSON layout → semantic markdown (e.g. *"screen has 1 input, 2 buttons"*) | Find structurally similar screens across projects |

The LLM is fed retrieved context (including asset relationships) so it "knows" what's attached to the current canvas **without seeing it**.

> Embedding vector dimensions in the schema are placeholders until the embedding models are chosen; reconcile in the RAG phase.

---

## 4. Artifact & asset relationships

- **Structural artifacts** (user flows, wireframes, design systems, UI screens) → stored as **JSON in Postgres** (`artifact_versions.content`).
- **Media assets** (images, audio, SVG icons) → stored as **binaries in AWS S3**; metadata in the `assets` table.
- **Binding** — `assets` rows carry foreign keys to the specific `screen_id` (a `ui_screen` artifact) and/or `project_id` they belong to. These relationships are surfaced to the LLM via RAG.
- **Pipeline edges** — `artifact_links` records cross-artifact relationships (e.g. a wireframe `derived_from` a user flow).

---

## Tech Stack

| Layer | Choice |
|---|---|
| Frontend | React, Vite, TypeScript; **tldraw** (wireframe/UI canvas), **@xyflow/react** (node-based user flows) |
| Backend | Rust, Axum; **REST + WebSockets** (real-time canvas sync) |
| Database | PostgreSQL (relational data + version graph) + **pgvector** (RAG, semantic search) |
| Storage | AWS S3 (binary media) |
| AI | Server-side LLM SDKs (Anthropic default; OpenAI optional) with prompt caching + sliding context windows |
| Deployment | Docker → AWS Fargate / ECS |
