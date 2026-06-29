# Deploy — Cloudflare Pages (frontend) + Cloud Run (backend) + Neon (DB)

A real, shareable deploy for testing collaboration. **~$0 idle**, scales to zero,
pay only OpenRouter per generation.

```
 browser ─▶ Cloudflare Pages  (static app + /api/* proxy Function)
                   │  same-origin → session cookie stays first-party
                   ▼
            Cloud Run (Rust/Axum, scale-to-zero, $PORT)
                   ├─▶ Neon (Postgres + pgvector, scale-to-zero)
                   └─▶ OpenRouter (pay-per-generation)
   Cloud Scheduler ─ every 1 min ─▶ POST /api/internal/jobs/drain
```

**Why a proxy and not two origins?** Auth is an httpOnly cookie. If the browser
talked to Pages *and* Cloud Run directly (two origins) the cookie would need
`SameSite=None` + cross-site CORS — fiddly and leak-prone. Instead, a tiny
**Pages Function** ([`frontend/functions/api/[[path]].js`](frontend/functions/api/%5B%5Bpath%5D%5D.js))
proxies `/api/*` to Cloud Run, so the browser only ever sees the Pages origin
and the cookie is first-party. No backend change.

---

## Accounts you'll need
| Service | Sign up | Cost | Used for |
|---|---|---|---|
| **Neon** | neon.tech (GitHub login) | Free tier | Postgres + pgvector |
| **Google Cloud** | you have it | Cloud Run free tier | backend container |
| **Cloudflare** | dash.cloudflare.com (free) | Pages free | frontend + API proxy |

One-time GCP prep (in your project):
```bash
gcloud auth login
gcloud config set project <YOUR_PROJECT_ID>
gcloud services enable run.googleapis.com cloudbuild.googleapis.com \
                       secretmanager.googleapis.com cloudscheduler.googleapis.com
# billing must be enabled on the project (Cloud Run's free tier still needs it linked)
```

## 1. Database — Neon
1. neon.tech → **New Project** (Postgres 16, pick a region near your Cloud Run region).
2. Copy the **pooled** connection string (`postgresql://…?sslmode=require`).
   Migrations (incl. `CREATE EXTENSION vector`/`uuid-ossp`) run automatically on boot.

## 2. Backend — Cloud Run
Store secrets first, then deploy from the repo root (the `Dockerfile` builds it):
```bash
printf '%s' '<YOUR_OPENROUTER_KEY>' | gcloud secrets create openrouter-key --data-file=-
printf '%s' "$(openssl rand -hex 16)"  | gcloud secrets create jobs-drain-secret --data-file=-

gcloud run deploy canonforge \
  --source . --region us-central1 --allow-unauthenticated \
  --min-instances 0 --max-instances 2 \
  --set-env-vars "DATABASE_URL=<NEON_URL>,JOBS_WORKER=false,COOKIE_SECURE=true,\
ASSET_MOCK=false,EMBED_MOCK=false,LLM_MOCK=false,AUDIO_MOCK=false,USAGE_MOCK=false" \
  --set-secrets "OPENROUTER_API_KEY=openrouter-key:latest,JOBS_DRAIN_SECRET=jobs-drain-secret:latest"
```
Note the printed **service URL** (e.g. `https://canonforge-xxxx-uc.a.run.app`).
- **Free, no-spend variant:** set the four `*_MOCK` vars to `true` and drop the
  `--set-secrets` line (mock AI needs no key). Collaboration/trash/profile work
  fully in mock mode — only image/audio are placeholders.
- `COOKIE_SECURE=true` is required (cookie travels over HTTPS).

## 3. Frontend — Cloudflare Pages
```bash
cd frontend && npm run build        # → frontend/dist  (+ functions/ ships as-is)
```
Then either the dashboard (Pages → Create → Connect to Git, or Direct Upload) or Wrangler:
```bash
npx wrangler pages deploy dist --project-name canonforge
```
In the Pages project **Settings → Environment variables**, set:
```
BACKEND_URL = https://canonforge-xxxx-uc.a.run.app   # your Cloud Run URL
```
That's what the `/api/*` proxy Function forwards to. Build settings if using Git:
**build command** `npm run build`, **output dir** `dist`, **root** `frontend`.

## 4. Async jobs — Cloud Scheduler
```bash
gcloud scheduler jobs create http canonforge-drain \
  --schedule "* * * * *" --location us-central1 \
  --uri "https://canonforge-xxxx-uc.a.run.app/internal/jobs/drain" \
  --http-method POST --headers "x-drain-secret=<THE_SECRET_YOU_GENERATED>"
```
`drain` processes ≤25 queued jobs/call. (Skip this if you set `JOBS_WORKER=true`
and `--min-instances 1` instead — simpler, but no longer scale-to-zero.)

## 5. Test collaboration
1. Open the Pages URL, **sign up** (you're Owner of a new workspace).
2. In another browser/incognito, **sign up** a second account.
3. As the owner: **Team → invite** the second account's email (Editor).
4. Log in as the second user → you both see the shared workspace; comments,
   review, and roles are live. Set a display name in the account menu.

## Notes / trade-offs
- **Cold start:** first request after idle wakes Cloud Run (~1–2s).
- **Storage:** `S3_BUCKET` unset → assets stored inline (data URLs) in Postgres —
  fine for a demo. For volume, add Cloudflare R2 / GCS later.
- **Even simpler, not scale-to-zero:** a $4/mo VPS + `docker compose up` runs
  everything (incl. the in-process worker) predictably.
- **AWS:** fights this goal (no scale-to-zero on App Runner/RDS; Lambda has no
  background loop; free tier expires) — prefer Cloud Run here.
