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
- The shared OpenRouter key is **not** in git (gitignored `.env`) — from the team (~$9.57/$10 left). `*_MOCK=false` (ASSET/EMBED/LLM) flips on real image gen + embeddings + answer-synthesis (cheap; cached). Default `.env.example` keeps all mocks on (free dev/CI).

## What's built (Phases 0–5 + RAG, all mock-mode by default)
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
- **Smart search / dedup** — `ai/embeddings.rs` indexes assets on insert (generate/derive/**upload**/audio) into `visual_embeddings`. Board search box → `/assets/search?q` (semantic ranking); pre-generate nudge → `/assets/similar-check`; `/assets/:id/similar`; `/embeddings/backfill`. `EMBED_MOCK=true` (default) = free feature-hashed (lexical); `EMBED_MOCK=false` + key = **real semantic** embeddings (`openai/text-embedding-3-small` via OpenRouter `/embeddings`, `dimensions` param matched to columns, disk-cached). After flipping mock↔real, re-run `/embeddings/backfill` + `/context/backfill` (mock and real vectors aren't comparable). Visual embeddings use the caption; pixel-CLIP is a future swap behind `embed_text`.
- **Semantic context ("Ask this project")** — `semantic_embeddings` over brief / asset prompts / comments / canon; box atop the Canon tab → `/context?q` returns `{answer, sources}` + `/context/backfill`.
- **LLM answer-synthesis** — `ai/llm.rs` synthesizes a grounded answer from the retrieved snippets (`LLM_MOCK` default; real = `google/gemini-2.5-flash`, cheap). `ai/cache.rs` content-addressed disk cache under `AI_CACHE_DIR` (gitignored) so identical calls never re-spend. **The shared OpenRouter key also serves text** (it's a fraction of a cent/answer). Local `.env` currently has `LLM_MOCK=false` (real) — flip to `true` for free dev.
- **Smart versioning** — each canon version gets an auto-generated deterministic "what changed" note (`canon.change_note`, mig 0008); `GET /canon/history` + a version-history list in the Canon tab. No LLM.
- **Asset naming** — `assets.name` (mig 0009, editable in the inspector) with an auto-derived display label (`api.displayName`, role+prompt) used across board/lineage/review; the name drives the export filename.
- **Audio** — `POST /projects/:id/audio` generates `kind='audio'` assets via `ai/audio.rs`. `AUDIO_MOCK=true` (default) synthesizes a free placeholder WAV; `AUDIO_MOCK=false` + key calls **OpenRouter `google/lyria-3-clip-preview`** (music model, ~$0.04/clip): the prompt is framed toward a short loopable game cue, the streamed (`stream:true`) base64 MP3 is decoded and **trimmed to `AUDIO_CLIP_SECS`** (default 8s; Lyria's native clip is ~30s) on an MPEG frame boundary — no decoder dep. The board has an image/audio toggle; clips play inline in the grid + inspector.
- **Generation recipes** — reusable derivation templates (`generation_recipes`, mig 0011; `routes/recipes.rs`). The derive panel saves the current instruction as a recipe and applies saved ones via chips.
- **Exemplar loop (the moat)** — approved assets can be flagged style exemplars (inspector toggle, ★ board badge; `assets.exemplar`, mig 0010). From-scratch generation then conditions on the latest approved exemplar (reference img2img) so new assets inherit the approved style; provenance in `metadata.exemplar_id`. Closes PLAN §6's "only approved assets influence future derivations".
- **Verticals (adapter framework)** — a vertical is defined in **two registries**: `backend/src/verticals.rs` (`{key, label, render_hint}` — the prompt-framing rule + the validation authority) and `frontend/src/app/verticals.ts` (`VERTICALS`: derive presets + canon fields), keyed identically. Backend `compile_prompt` uses the vertical's `render_hint`; `generate`/`derive` read `project.vertical`; `projects.create` 400s on an unknown vertical. The project-create picker is generated from `VERTICALS` (single source). **To add a vertical: one row in each registry** — nothing else (the `engines` export hook defaults to empty). Verticals: `game_2d`, `manhwa`, `illustration`, `marketing`.
- **Activity feed** (Activity tab) — `GET /projects/:id/activity` merges recent asset creations + comments + canon versions (existing tables, no schema) into a time-sorted timeline; asset/comment rows open the inspector.
- **Export** — pre-export checks (`POST /export/check`: filename, format/dimensions/alpha, issues) + a zip pack (`POST /export`: `manifest.json` with `groups[]` by role/tag + `assets/<group>/<file>`, rejected/undecodable skipped). Triggered from a collection or board multi-select via the Export dialog.
- **Engine export adapters (Godot 4 + Unity)** — the export is vertical-neutral by default; a vertical can declare supported `engines` (the per-vertical export-adapter hook on `verticals::Vertical`, a `&[Engine]` list). `game_2d` → `[Godot, Unity]`. `POST /export` takes an optional `target` (`"godot"`/`"unity"`); when the vertical `supports()` it the pack gains engine scaffolding. **Godot**: a sibling `<file>.import` per texture (2D-sprite settings; the machine-local cache/uid are omitted so Godot regenerates them on first import while keeping our `[params]`) + a minimal drop-in `project.godot` + README. **Unity**: a sibling `<file>.meta` per texture (`textureType: 8` Sprite, alpha kept, a deterministic 32-hex GUID so refs survive) + README; copy `assets/` into a project's `Assets/`. Unknown target → 400; target the vertical doesn't support → 400. Packers: `backend/src/export/{godot,unity}.rs`; the Export dialog shows a Generic + one-button-per-engine toggle. **Caveat:** the Unity `.meta` is format-validated + curl-verified, *not* editor-import-tested (the editor is licensed); Godot likewise verified by pack structure, not a real engine import. Adding an engine = one `Engine` variant + a packer + listing it on the verticals that support it.

## Code map
- `backend/src/routes/` — `auth, workspaces, projects, canon, assets, audio, collections, folders, comments, lineage, export, search, context, recipes, activity, usage` · vertical registry in `src/verticals.rs` (incl. the per-vertical `engines` export hook) · engine packers in `src/export/` (`godot.rs`, `unity.rs`, shared `TextFile` in `mod.rs`).
- **Per-asset versioning (Pro pipeline A2)** — every asset is a stable identity whose bytes live in `asset_versions` (mig 0017; `version` seq, `s3_key`, `prompt`, `change_note`, `created_by`); `assets.current_version_id` is the head, and `assets.s3_key`/`mime_type` are kept as a denormalized cache of the head so the file route / export / mirror / embeddings work unchanged. `record_version()` (in `routes/assets.rs`) appends the next version and advances the head; generate/derive/upload/audio all record a v1 (async jobs thread the requester via the job payload). Endpoints: `GET /assets/:id/versions` (history + author email), `POST /assets/:id/versions/:vid/restore` (**non-destructive** — appends a copy of the target as the new head), `POST /assets/:id/regenerate` (new version conditioned on canon + exemplar; optional new prompt), `GET /assets/:id/file?version=:vid` (serve a specific version). `with_url` pins each asset's URL to its head version id, so the long-cache is immutable yet self-busts when the head moves (no stale thumbnails after a regenerate). UI: `assets/VersionHistory` in the inspector (version list, regenerate, restore, before/after slider diff).
- **Folders (Pro pipeline A1)** — per-project asset tree (`folders` table, mig 0016; self-FK parent, `assets.folder_id`). `routes/folders.rs`: `GET/POST /projects/:id/folders` (flat list with direct `asset_count`; client nests by `parent_id`), `PATCH /folders/:id` (rename/reparent — rejects self/descendant cycles via a recursive CTE), `DELETE /folders/:id` (subtree cascades; contained assets are **unfiled**, `folder_id → NULL`, never destroyed). Asset moves go through `PATCH /assets/:id { folder_id }` (a double-`Option` so JSON `null` = move to root vs. absent = unchanged). The board's left-rail `FolderTree` selects a folder to scope the list (`?folder=<id>|root`), supports inline create/rename/delete, and drag-a-tile-onto-a-folder moves. Folders **coexist** with collections (folder = canonical home; collection = cross-cutting set).
- **Production hardening** — `app()` adds baseline **security headers** (`X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`) and an env-driven **CORS** policy (permissive in dev; set `CORS_ALLOWED_ORIGINS` to a comma-separated allowlist to lock down + allow credentials). **Content moderation**: `moderation::check_prompt` (a keyword denylist) gates generation at job-enqueue and inside the shared `run_generate` core, rejecting disallowed prompts with 400 before any spend. **Cost guardrail**: `guardrail::check_can_spend` sits next to `check_prompt` on every spend path (generate / job-enqueue + worker / derive / audio) and refuses *before* spending — (a) a **credit floor** (`GUARDRAIL_MIN_CREDIT_USD`, default 0.50): 503 when the shared key's remaining credit drops below it (mock-credit is never blocked); (b) a **per-workspace daily cap** (`GUARDRAIL_DAILY_GEN_CAP`, default 100): 429 when a workspace has produced more than the cap of seeded/derived assets in a rolling 24h (count-aware, no new table). Both env vars are read at runtime — tune without a recompile. A real per-workspace $-ledger can replace the count cap later behind the same seam. (Dev-token password-reset/email-verify is deferred — its own auth feature + migration.)
- **Async generation queue** — `POST /projects/:id/jobs` enqueues a background generation (`jobs` table, mig 0012); the client polls `GET /jobs/:id` (or `GET /projects/:id/jobs`). An in-process worker (`src/jobs.rs`, spawned in `main`) claims queued jobs with `FOR UPDATE SKIP LOCKED` and runs them via `run_generate` — the **same canon/exemplar-aware core** the sync `POST /projects/:id/assets` route uses (factored out of `routes/assets.rs`). On the board, image generation enqueues a job, a `JobsBanner` shows in-flight work, and the board refreshes when the job finishes. Audio is still sync. Status is `queued|running|succeeded|failed` (TEXT+CHECK). Integration-tested end-to-end (`tests/api.rs`). **Scale-to-zero hosts:** set `JOBS_WORKER=false` (no in-process loop) and have a scheduler hit `POST /internal/jobs/drain` (guarded by `JOBS_DRAIN_SECRET`; drains ≤25 jobs/call) — both paths claim via `SKIP LOCKED`, so they're interchangeable/safe together. See **[DEPLOY.md](DEPLOY.md)** (Cloud Run + Neon + Cloud Scheduler).
- **Usage / credit** — `GET /usage` (auth-gated) surfaces the shared OpenRouter key's remaining credit via `ai/usage.rs` (fetches `GET /api/v1/auth/key` → `limit_remaining`/`usage`/`limit`, cached 60s; `USAGE_MOCK=true` or no key → mock; a failed fetch reuses the last value as `source:"stale"`). The Workspace Hub shows a `CreditChip`. First commercialization slice; the seam can later become per-workspace quotas.
- `backend/src/ai/images.rs` — generate + `derive_image` (img2img) + mock. `backend/src/ai/audio.rs` — audio generation (mock WAV synth) behind the same boundary.
- `backend/src/storage.rs` — S3/MinIO (+ inline fallback). `backend/src/models.rs` — all DTOs/rows.
- `backend/migrations/` — `0001` base, `0002` auth, `0003` canon+asset fields, `0004` drop dead UI tables, `0005` derivation (`asset_links`), `0006` collections, `0007` comments, `0008` canon change-note, `0009` asset name, `0010` exemplar flag, `0011` recipes, `0012` async jobs, `0013` keyset index, `0014` dual embeddings, `0015` accounts/collab/trash, `0016` folders, `0017` asset versions.
- `frontend/src/lib/api.ts` — typed API client (one place for all endpoints).
- `frontend/src/app/` — `WorkspaceHub`, `ProjectWorkspace` (left-rail nav: Board/Canon/Review/Lineage/Collections/Activity), `assets/AssetLibrary` + `AssetInspector` + `FolderTree` + `VersionHistory` + `ReviewQueue` + `CommentThread` + `LineageView` + `ActivityView`, `canon/CanonView` + `canon/ContextAsk`, `collections/CollectionsView`, `export/ExportDialog`.

## Persistence (where everything lands)
- **Metadata** (projects, assets, canon, comments, collections, recipes, jobs, lineage, prompts) → **Postgres** (local docker volume `db_data`).
- **Asset bytes** (generated / derived / uploaded images, audio) → **MinIO/S3** (local volume `minio_data`) when `S3_BUCKET` is set, else **inline data-URL in Postgres**.
- **AI text calls** (LLM answers, embeddings) → content-addressed disk cache `backend/.ai-cache/` (identical calls never re-pay).
- **Local mirror (optional):** set `ASSET_MIRROR_DIR` → every asset is *also* written to `<dir>/<project_id>/<asset_id>.<ext>` as a plain browsable file (`src/mirror.rs`, best-effort, gitignored). So creations live in the DB and on local disk simultaneously.

## Conventions
- **Branch per PR**, ~3 logical commits, merge with `--merge` (no squash unless asked). End commits with the Co-Authored-By trailer.
- Verify every change: `cargo build` + `cargo test` + a curl smoke test (backend), `tsc -b` + `npm run build` (frontend).
- **Tests + CI:** `cargo test` runs **26 DB-free unit tests** over the core pure logic (embeddings, canon diff, export slug, WAV, cache key, llm prompt, verticals, compile_prompt, godot/unity packers). A **DB-backed integration test** (`backend/tests/api.rs`, `#[ignore]`'d) drives the real router in-process via `tower::oneshot` through signup → workspace → project → generate → all three export packs + the 400 paths (mock AI, inline storage); run it with `cargo test -- --ignored` (needs Postgres). The crate is split bin (`main.rs`) + lib (`lib.rs`, exposes `app(state)`) so tests build the same app. GitHub Actions (`.github/workflows/ci.yml`): a DB-free `backend` job (build+test) + a frontend job (always), plus an `integration` job with a `pgvector/pgvector:pg16` service that runs the `--ignored` tests. No secrets (runtime sqlx + mock-default AI).
- `git pull` has a quirky upstream config on some branches — if it errors, use `git merge --ff-only @{u}`.

## Not in git (local-only planning docs)
`ATLAS_PLAN.md`, `PHASE1_PLAN.md`, `PHASE2_PLAN.md`, `PHASE3_PLAN.md` are intentionally untracked scratch/plan notes — ignore for handoff; the source of truth is `PLAN.md` + `ROADMAP.md`.

## Next up
Open candidates (see [ROADMAP.md](ROADMAP.md)): **engine export adapters** (Godot/Unity, consume the export `groups[]` — the registry can grow a per-vertical export hook); a **4th vertical** (marketing imagery — pure config); **animation** (frame sequences — own spike, real-model spend); swapping the mock embedder for a **real text/CLIP model** (shared-key spend); and the **commercialization track** (async gen queue, billing/quotas, deploy, tests, CORS lockdown, content moderation, password-reset/email-verify).

**Board pagination is done** — `GET /projects/:id/assets` is keyset-paginated
(`?limit&cursor&status&role&source&collection` → `{items, next_cursor}`, cursor =
`created_at_micros_id`) with `GET /projects/:id/assets/facets` feeding the
filter-rail counts and migration 0013's `(project_id, created_at DESC, id DESC)`
index; the board pages via a "Load more" button (search stays a bounded ranked
set). Follow-ups: paginate the Activity feed, and "select all matching" for
batch ops across pages (today batch select only sees loaded pages).

> Migrations note: the embedding stores (`semantic_embeddings` 1024-d, `visual_embeddings` 768-d) ship from `0001` with placeholder dims; the mock embedder matches them. Reconcile dims when a real model is chosen.
