# CanonForge — reference-driven visual asset studio

> Bring a reference → derive consistent variants bound to a versioned **project canon** → review → organize → version → **export engine-ready**. A studio that *remembers your art direction* instead of generating one-off images. First deep vertical: **2D game assets** (Godot/Unity export); also manhwa, illustration, marketing.

**Status:** built and working — Phases 1–5 + RAG/LLM + audio + multi-vertical framework + async job queue + production hardening, all **mock-first** (free dev/CI; real AI behind flags). Sources of truth: **[PLAN.md](./PLAN.md)** (vision), **[ROADMAP.md](./ROADMAP.md)** (status), **[HANDOFF.md](./HANDOFF.md)** (setup + architecture), **[DEPLOY.md](./DEPLOY.md)** (deploy).

## The loop

```
reference → project canon → derive variants → review → organize/reuse → export pack
```

## Why it's defensible (the moat)

Not the generation — that's a rented model. The defensibility is **data + workflow lock-in that compounds inside a project**:

- **Exemplar / canon loop (the real moat).** Approved assets feed back as img2img conditioning, bound to a *versioned canon*, so a team's approved house-style corpus grows and gets harder to reproduce elsewhere — a switching cost that increases with use.
- **Consistency at scale, not one-off gen.** Lineage (`derived_from` + `canon_version_id`), canon-drift detection, and a review gate make a *coherent set* — the actual hard part.
- **Export wedge.** Godot/Unity import-ready packs drop assets straight into the user's build pipeline.

## What's built

- **Canon** — versioned style rules + exemplars (JSONB); auto "what-changed" diff notes + history; feeds every generate/derive prompt.
- **Generate / derive** — text→image and reference-conditioned img2img (presets per vertical), each derivative recording provenance; review gate (approve/reject/needs-review).
- **Exemplar loop** — approved assets condition future generation.
- **Smart board** — filters, semantic search, status visual language, batch ops, generation recipes, batch "derive all", style-fit score.
- **RAG** — embeddings (mock feature-hash *or* real `text-embedding-3-small`, cached) power search/dedup/find-similar + "Ask this project" with LLM answer-synthesis over retrieved snippets.
- **Collaboration** — per-asset comment threads, lineage graph, activity feed.
- **Verticals** — a vertical = one row in two registries (backend prompt rules + validation; frontend presets/canon fields). game_2d, manhwa, illustration, marketing.
- **Export** — pre-export checks + grouped zip (`manifest.json` + `groups[]`), and **engine adapters**: Godot 4 (`.import` + `project.godot`) and Unity (`.meta` Sprite + GUID), behind a per-vertical `engines` hook.
- **Audio** — mock WAV synth behind the same provider boundary.
- **Commercialization** — async generation queue (DB-backed jobs + worker / scale-to-zero drain endpoint), OpenRouter credit visibility, and hardening (CORS allowlist, security headers, prompt content denylist).

## Stack

- **Backend** — Rust + Axum, sqlx (runtime queries — no DB needed to build), Postgres 16 + pgvector, argon2 session-cookie auth, S3/MinIO (with inline fallback), OpenRouter (images/embeddings/LLM). Split bin + lib (`lib.rs` exposes `app()`).
- **Frontend** — React 19 + Vite + TypeScript + Tailwind v4.
- **Tests/CI** — 31 DB-free unit tests + DB-backed integration tests (`tower::oneshot`); GitHub Actions runs both (a `pgvector` service for the integration job).

## Getting started

**Prerequisites:** Rust (stable), Node 20+, Docker + Docker Compose.

```bash
cp .env.example .env          # all AI defaults to mock — no keys needed
docker compose up -d          # Postgres 16 + pgvector + MinIO
cd backend && cargo run       # :8080 — applies migrations on boot
cd frontend && npm install && npm run dev   # :5173
```

Open http://localhost:5173 → sign up → create a project. Generation runs in
**mock mode by default** (`ASSET_MOCK`/`EMBED_MOCK`/`LLM_MOCK`/`AUDIO_MOCK`) — no
keys, no spend. Set them `false` + `OPENROUTER_API_KEY` for real AI (cached).

## Deploy

Cheapest scale-to-zero (≈$0 idle): **Cloud Run + Neon**, async jobs via Cloud
Scheduler hitting `POST /internal/jobs/drain`. Full runbook in **[DEPLOY.md](./DEPLOY.md)**.
A `Dockerfile` builds the backend; the app honors `$PORT`.

## Repository layout

```
backend/     # Rust + Axum API (lib + bin); migrations; tests/api.rs (integration)
frontend/    # React + Vite + TypeScript
infra/db/    # Postgres init (vector, uuid-ossp)
Dockerfile   # backend image (Cloud Run / any container host)
PLAN.md ROADMAP.md HANDOFF.md DEPLOY.md
```
