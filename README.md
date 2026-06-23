# Design Studio AI

> An AI-powered design workspace that generates, organizes, versions, and reuses every design artifact — from user flows and wireframes to UI screens and assets — so product designers spend less time managing files and more time designing.

> **Status:** 📋 Proposal — direction approved in principle, full implementation plan pending sign-off. The repo currently contains scaffolding only ([see Roadmap](./ROADMAP.md)).

---

## Vision

Build an AI-powered Product Design Studio that turns product ideas into complete design projects while automatically organizing, versioning, and reusing every artifact created along the way.

Unlike tools that focus only on UI generation, the platform manages the **entire design lifecycle** — from idea exploration to asset management — keeping design knowledge, assets, and decisions connected and reusable.

## The Problem

Today's design workflow is fragmented across tools:

```
Idea → Miro → Figma → Midjourney → Icon Libraries → Google Drive → Notion
```

Artifacts scatter across these tools, causing:

- Lost design context
- Duplicate asset creation
- Poor discoverability
- Difficult handoffs
- Weak version tracking
- Repetitive, manual work

Designers spend significant time managing files instead of designing.

## The Solution

An AI Product Design Studio combining three layers:

**1. AI Design Generation** — turn ideas into user flows, wireframes, design systems, UI screens, and assets (images, icons, illustrations, audio).

**2. Workflow Intelligence** — every artifact is automatically linked, so the system understands relationships and maintains context across the lifecycle:

```
Idea → User Flow → Wireframe → Design System → UI Screens → Assets
```

**3. Asset Management** — auto-tag, categorize, link assets to screens and projects, store reusable components, and enable semantic search.

---

## Key Features

### Must-have

- **AI design workflow** — Idea → Flow → Wireframe → Design System → UI Screens
- **Asset generation** — images and audio
- **Reusable asset library** — store and reuse images, icons, components, illustrations
- **Version history** — track design evolution across a project
- **Search & tagging** — semantic search, auto-tagging, filtering

### Differentiating AI features

- **Design Memory** — answer "why does this screen exist?", "which flow generated it?", "which assets belong here?"
- **Asset Intelligence** — auto-categorize, detect duplicates, recommend reuse (*"a similar onboarding illustration already exists"*)
- **Version Intelligence** — AI summaries of what changed between versions, with rationale
- **Auto-generate missing states** — given a success state, generate error / empty / loading / offline states
- **Asset Lineage Graph** *(stretch)* — visual graph tracing artifact origins and dependencies

### Nice-to-have

- Collaboration (team workspaces, shared libraries, review workflows)
- Video generation (keyframes, simple animations, motion design)

---

## Architecture

| Layer | Choice |
|---|---|
| Frontend | React + Vite + TypeScript |
| Backend | Rust + Axum |
| Database | PostgreSQL |
| Vector / RAG store | pgvector |
| Object storage | AWS S3 |
| Deployment | Docker → AWS Fargate / ECS |

### Retrieval (RAG)

Several AI features are retrieval-augmented, not pure generation: **semantic search**, **duplicate detection**, **reuse recommendations**, **Design Memory**, and **Version Intelligence** all retrieve relevant context before the model responds. `pgvector` is the retrieval store.

Because assets are largely *visual*, "find a similar illustration" relies on **multimodal embeddings** (image + text), not text alone. Embedding model selection and pgvector index strategy (HNSW vs IVFFlat) are addressed in the implementation plan.

### Security & reliability

- **Auth** — workspace-based access control
- **Rate limiting** — per workspace / user / IP
- **Bot protection** — Cloudflare Turnstile
- **Privacy** — PDPA compliance
- **AI reliability** — timeout recovery, model-failure handling, input validation, retries, graceful degradation

---

## Getting Started

**Prerequisites:** Rust (stable), Node.js 20+, Docker + Docker Compose.

```bash
# 1. Configure environment
cp .env.example .env        # fill in API keys / S3 creds as needed

# 2. Start Postgres + pgvector
docker compose up -d        # DB on localhost:5432, extensions auto-enabled

# 3. Backend (http://localhost:8080)
cd backend && cargo run     # GET /health → {"status":"ok"}

# 4. Frontend (http://localhost:5173)
cd frontend && npm install && npm run dev
```

## Project Structure

```
design-studio-ai/
├── backend/                # Rust + Axum API
│   ├── src/main.rs         # entrypoint (/health, CORS, tracing)
│   ├── Cargo.toml
│   └── rust-toolchain.toml # pinned to stable
├── frontend/               # React + Vite + TypeScript
├── infra/
│   └── db/init/            # Postgres init (enables vector, uuid-ossp)
├── docker-compose.yml      # Postgres 16 + pgvector
├── .env.example
├── ROADMAP.md              # phased delivery plan
└── README.md
```

## Roadmap

See [ROADMAP.md](./ROADMAP.md) for the phased delivery plan and current status.

---

## Target Users

- **Primary:** Product Designers — a centralized workspace to create, manage, iterate, and reuse design artifacts.
- **Secondary:** UI/UX Designers, startup founders, design students.

## Positioning

> AI Asset & Workflow Management Platform for Product Design Teams — not just screen generation.
