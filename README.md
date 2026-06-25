# CanonForge — AI Game Asset Studio

> An AI asset studio where every generated asset belongs to a versioned **project canon**, is searchable and reusable, is checked for consistency, and exports production-ready. **First vertical: a 2D game asset workflow** — sprites, props, tiles, UI, SFX, and music loops, with Godot/Unity export.

> **Status:** 🔄 Direction reset. The project pivoted away from an earlier "AI design / UI-generation studio" (which drifted into Figma/v0's lane with no moat) back toward the original brief: an AI tool for a designer's *creative* work — multi-modal asset **generation + management** inside a real workflow. See **[PLAN.md](./PLAN.md)** for the full, current plan.

---

## The idea in one loop

```
Project brief → Project canon → Generate assets → Review consistency → Organize / reuse → Export
```

A game asset studio that **remembers your art direction**: define a style/audio canon once, generate sprites/UI/props/SFX/music conditioned on it, catch off-style and duplicate assets automatically, and export an engine-ready pack.

**Why it's defensible:** not the raw generation (that's a rented model), but the **project canon** that conditions everything, the **asset graph** with reuse + canon-change propagation, and **engine-ready export** — a niche the big generators (Midjourney/Suno) and the big suites (Adobe/Canva) don't serve.

The full rationale, architecture, data model, build sequence, and de-risk spike are in **[PLAN.md](./PLAN.md)**.

## Current state of the code

The repo already contains reusable platform plumbing from the earlier build, most of which carries over directly to this direction:

- **Backend** (Rust + Axum): auth + workspaces, project tenancy, rate limiting, Postgres + pgvector, S3/MinIO object storage with an authed file proxy, an AI provider boundary (image generation via OpenRouter) with mock mode.
- **Frontend** (React + Vite + TypeScript): app shell, auth, project workspace, asset library panel.

The UI-generation pieces (flow canvas, wireframe DSL renderer, design-system/hi-fi theming) belong to the abandoned direction and will be removed as the pivot lands. See [PLAN.md §10](./PLAN.md) for what carries over vs. what's net-new.

## Getting started

**Prerequisites:** Rust (stable), Node.js 20+, Docker + Docker Compose.

```bash
# 1. Configure environment
cp .env.example .env        # fill in API keys / S3 creds as needed

# 2. Start Postgres + pgvector and MinIO (object storage)
docker compose up -d

# 3. Backend (http://localhost:8080) — applies DB migrations on boot
cd backend && cargo run     # GET /health → {"status":"ok","db":"ok"}

# 4. Frontend (http://localhost:5173)
cd frontend && npm install && npm run dev
```

Generation runs in **mock mode by default** (`AI_MOCK=true`, `ASSET_MOCK=true`) so dev needs no API keys or spend. Real generation requires the relevant key in `.env`.

## De-risk first

Before the rewrite, run the **canon-consistency spike** ([`spikes/canon-consistency/`](spikes/canon-consistency/)) — ~8 images, ~$0.30 — to confirm that canon conditioning produces consistent, controllable game assets. Details in [PLAN.md §8](./PLAN.md).

## Repository layout

```
.
├── backend/            # Rust + Axum API (auth, projects, assets, storage, image gen)
├── frontend/           # React + Vite + TypeScript
├── infra/db/init/      # Postgres init (vector, uuid-ossp)
├── spikes/             # throwaway experiments (canon-consistency spike)
├── design-screens/     # CanonForge UI reference mockups (canonforge-01…06)
├── docker-compose.yml  # Postgres 16 + pgvector + MinIO
├── PLAN.md             # ← the current, canonical plan (source of truth)
└── README.md
```

## Design reference

Target UI mockups live in [`design-screens/`](design-screens/): asset board, derive slide-over, review & approve, canon studio, project hub, asset inspector. The screen-by-screen UX is in [PLAN.md](./PLAN.md).
