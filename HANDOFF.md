# Handoff — CanonForge (design-studio-ai)

Reference-driven visual asset studio: bring a base asset → derive consistent
variants bound to a project **canon** → review (approve/reject) → organize into
**collections**. Full vision in [PLAN.md](PLAN.md); status in [ROADMAP.md](ROADMAP.md).

## Stack
- **Backend** — Rust + Axum, sqlx, `backend/`. Listens on `:8080`.
- **Frontend** — React 19 + Vite + TS + Tailwind v4, `frontend/`. Dev on `:5173`.
- **DB** — Postgres 16 + pgvector (Docker). **Storage** — MinIO (S3-compatible, Docker).
- **AI** — OpenRouter `google/gemini-2.5-flash-image` for generate + derive.

## Run it (verified working)
Prereqs: Docker Desktop, Rust (rustup), Node ≥ 20.19.

```bash
cp .env.example .env          # if .env missing
docker compose up -d          # db (:5432) + MinIO (:9000, console :9001)
# backend (terminal 1)
cd backend && cargo run       # :8080
# frontend (terminal 2)
cd frontend && npm install && npm run dev   # :5173
```
Open http://localhost:5173 → sign up → open a project.

### AI modes (in `.env`)
- `ASSET_MOCK=true` → **free** placeholder SVGs, no API calls. Default for dev.
- `ASSET_MOCK=false` + `OPENROUTER_API_KEY=...` → real images (≈$0.04/image).
- The shared OpenRouter key is **not** in git (`.env` is gitignored) — get it from the team. (~$8.78 left at handoff.)

## What's built (Phase 0–3 complete)
- **Auth** — email+password, httpOnly session cookie, workspace roles.
- **Projects** (`vertical='game_2d'`) + **Canon** — versioned style rules (Canon tab). Canon feeds both generate + derive prompts.
- **Assets** — generate (text→image), upload base, **derive** (img2img, base sent as reference) with presets (walk/action/variant/matching). Each derivative records `derived_from` + canon version.
- **Review** — approve/reject/needs_review on tiles.
- **Inspector** (slide-over) — preview, edit role/tags, lineage strip (base ↔ derivatives), delete, add-to-collection.
- **Collections** — packs (Collections tab): create, open, remove, delete; add via inspector.
- **Smart board** — filter rail (status / role / source / collection), search, status visual language, multi-select batch (approve / reject / add-to-collection). All client-side over existing endpoints.
- **Review queue** (Review tab) — candidate + needs-review backlog as a worklist; focused preview + approve/needs-review/reject with the discussion side-by-side; a decision advances to the next.
- **Comments** — per-asset discussion thread (in the inspector and the queue): author + relative time, post, delete-own (project Owner can moderate).
- **Lineage** (Lineage tab) — roots → derivatives tree; canon-drift detection: assets predating the current canon are flagged stale, with per-node Keep (reconcile) / Regenerate and a "Keep all" action.
- **Smart search / dedup** — `ai/embeddings.rs` (mock feature-hashed embedder, `EMBED_MOCK` default) indexes assets on insert (generate/derive/**upload**/audio) into `visual_embeddings`. Board search box → `/assets/search?q` (semantic ranking); pre-generate nudge → `/assets/similar-check`; `/assets/:id/similar`; `/embeddings/backfill` for imports/old assets. Real text/CLIP model is a localized swap.
- **Semantic context ("Ask this project")** — `semantic_embeddings` over brief / asset prompts / comments / canon; box atop the Canon tab → `/context?q` (ranked snippets) + `/context/backfill`. Retrieval-only (no LLM synthesis yet).
- **Audio** — `POST /projects/:id/audio` generates `kind='audio'` assets via `ai/audio.rs` (mock WAV synth; `AUDIO_MOCK=true` default, no hosted provider yet). The board has an image/audio toggle; clips play inline in the grid + inspector.
- **Export** — pre-export checks (`POST /export/check`: filename, format/dimensions/alpha, issues) + a grouped zip pack (`POST /export`: `manifest.json` with `groups[]` by role/tag + `assets/<group>/<file>`, rejected/undecodable skipped). Triggered from a collection via the Export dialog. Vertical-neutral; engine-specific packers (Godot/Unity) are deferred per PLAN (rule of three) and will consume the grouped manifest.

## Code map
- `backend/src/routes/` — `auth, workspaces, projects, canon, assets, audio, collections, comments, lineage, export, search, context`.
- `backend/src/ai/images.rs` — generate + `derive_image` (img2img) + mock. `backend/src/ai/audio.rs` — audio generation (mock WAV synth) behind the same boundary.
- `backend/src/storage.rs` — S3/MinIO (+ inline fallback). `backend/src/models.rs` — all DTOs/rows.
- `backend/migrations/` — `0001` base, `0002` auth, `0003` canon+asset fields, `0004` drop dead UI tables, `0005` derivation (`asset_links`), `0006` collections, `0007` comments (`asset_comments`).
- `frontend/src/lib/api.ts` — typed API client (one place for all endpoints).
- `frontend/src/app/` — `WorkspaceHub`, `ProjectWorkspace` (Canon/Assets/Review/Lineage/Collections tabs), `assets/AssetLibrary` + `AssetInspector` + `ReviewQueue` + `CommentThread` + `LineageView`, `canon/CanonView`, `collections/CollectionsView`, `export/ExportDialog`.

## Conventions
- **Branch per PR**, ~3 logical commits, merge with `--merge` (no squash unless asked). End commits with the Co-Authored-By trailer.
- Verify every change: `cargo build` + a curl smoke test (backend), `tsc -b` + `npm run build` (frontend).
- `git pull` has a quirky upstream config on some branches — if it errors, use `git merge --ff-only @{u}`.

## Not in git (local-only planning docs)
`ATLAS_PLAN.md`, `PHASE1_PLAN.md`, `PHASE2_PLAN.md`, `PHASE3_PLAN.md` are intentionally untracked scratch/plan notes — ignore for handoff; the source of truth is `PLAN.md` + `ROADMAP.md`.

## Next up
Semantic-context RAG ("why was this created / what's it for") over briefs / prompts / comments / canon, using the same mock embedder into `semantic_embeddings` — mock-able, no spend. Then nav shell (left rail), and engine adapters (deferred until a 2nd vertical). Swapping the mock embedder for a real text/CLIP model (true semantic "feel") needs a spend go-ahead. See [ROADMAP.md](ROADMAP.md).
