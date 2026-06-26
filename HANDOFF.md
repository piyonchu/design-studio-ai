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

## "Backend won't run" — checklist (these actually bit us)
1. **Docker not running** → start Docker Desktop, confirm `docker info` works, then `docker compose up -d`.
2. **`cargo` not found** → the terminal predates the rustup install. Open a *fresh* terminal (or restart VS Code), or use the full path: `& "$env:USERPROFILE\.cargo\bin\cargo.exe" run`.
3. **Port 8080 busy / `Access is denied` on build** → a stale backend holds the lock: `Get-Process design-studio-backend | Stop-Process -Force` (PowerShell), then rerun.
4. **`migration N was previously applied but has been modified`** → CRLF line-ending on a `.sql`. `.gitattributes` pins `*.sql` to LF; if it recurs, reset the dev DB (no real data): `docker compose down -v && docker compose up -d`.
5. **Added a migration but it doesn't apply** → `sqlx::migrate!` embeds at compile time; force a rebuild: `touch backend/src/db.rs` then `cargo run`.
6. **Node too old** → vite 8 needs Node ≥ 20.19; `node -v`, upgrade if below.

## What's built (Phase 0–3 PR2)
- **Auth** — email+password, httpOnly session cookie, workspace roles.
- **Projects** (`vertical='game_2d'`) + **Canon** — versioned style rules (Canon tab). Canon feeds both generate + derive prompts.
- **Assets** — generate (text→image), upload base, **derive** (img2img, base sent as reference) with presets (walk/action/variant/matching). Each derivative records `derived_from` + canon version.
- **Review** — approve/reject/needs_review on tiles.
- **Inspector** (slide-over) — preview, edit role/tags, lineage strip (base ↔ derivatives), delete, add-to-collection.
- **Collections** — packs (Collections tab): create, open, remove, delete; add via inspector.

## Code map
- `backend/src/routes/` — `auth, workspaces, projects, canon, assets, collections`.
- `backend/src/ai/images.rs` — generate + `derive_image` (img2img) + mock.
- `backend/src/storage.rs` — S3/MinIO (+ inline fallback). `backend/src/models.rs` — all DTOs/rows.
- `backend/migrations/` — `0001` base, `0002` auth, `0003` canon+asset fields, `0004` drop dead UI tables, `0005` derivation (`asset_links`), `0006` collections.
- `frontend/src/lib/api.ts` — typed API client (one place for all endpoints).
- `frontend/src/app/` — `WorkspaceHub`, `ProjectWorkspace` (Canon/Assets/Collections tabs), `assets/AssetLibrary` + `AssetInspector`, `canon/CanonView`, `collections/CollectionsView`.

## Conventions
- **Branch per PR**, ~3 logical commits, merge with `--merge` (no squash unless asked). End commits with the Co-Authored-By trailer.
- Verify every change: `cargo build` + a curl smoke test (backend), `tsc -b` + `npm run build` (frontend).
- `git pull` has a quirky upstream config on some branches — if it errors, use `git merge --ff-only @{u}`.

## Not in git (local-only planning docs)
`ATLAS_PLAN.md`, `PHASE1_PLAN.md`, `PHASE2_PLAN.md`, `PHASE3_PLAN.md` are intentionally untracked scratch/plan notes — ignore for handoff; the source of truth is `PLAN.md` + `ROADMAP.md`.

## Next up
Phase 3 PR3 — smart asset board (filters: role/status/source/collection + search; status visual language; multi-select → batch approve / add-to-collection). See [ROADMAP.md](ROADMAP.md).
