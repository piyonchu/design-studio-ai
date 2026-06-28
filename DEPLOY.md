# Deploy — cheapest scale-to-zero (Cloud Run + Neon)

Target: a demo that costs **~$0 idle** and scales to zero, paying only OpenRouter
per generation. Backend on **Cloud Run** (`min-instances=0`), Postgres+pgvector
on **Neon** (serverless, also scales to zero), object storage skipped (inline
fallback), async jobs driven by **Cloud Scheduler**.

```
            ┌─ Cloud Scheduler (every 1 min) ─ POST /internal/jobs/drain ─┐
 browser ─▶ Cloud Run (Rust/Axum, scale-to-zero) ─▶ Neon (Postgres+pgvector)
                         └─ OpenRouter (pay-per-generation) ─┘
```

## Why this shape
- **Cloud Run** scales to zero and cold-starts a small Rust binary in ~1s. The
  app honors `$PORT` (Cloud Run default 8080).
- The in-process job worker can't run between requests on a scale-to-zero host,
  so we **disable it** (`JOBS_WORKER=false`) and have **Cloud Scheduler** hit
  `POST /internal/jobs/drain` (secret-guarded) once a minute to drain the queue.
- **Neon** gives managed Postgres **with pgvector** that itself scales to zero —
  unlike Cloud SQL (no scale-to-zero, ~$9+/mo). Migration `0001` runs
  `CREATE EXTENSION vector` / `uuid-ossp`, both supported on Neon.
- **No object store:** leave `S3_BUCKET` empty → assets are stored inline as data
  URLs in the DB. Fine for a demo; swap in R2/GCS later.

## 1. Database (Neon)
1. Create a Neon project (Postgres 16). Copy the pooled connection string.
2. That's it — the backend runs migrations on boot (extensions included).

## 2. Backend (Cloud Run)
From the repo root (the `Dockerfile` builds the backend):

```bash
gcloud run deploy canonforge \
  --source . \
  --region <REGION> \
  --allow-unauthenticated \
  --min-instances 0 --max-instances 2 \
  --set-env-vars "DATABASE_URL=<NEON_URL>,JOBS_WORKER=false,ASSET_MOCK=false,EMBED_MOCK=false,LLM_MOCK=false,COOKIE_SECURE=true,CORS_ALLOWED_ORIGINS=https://<YOUR_FRONTEND_ORIGIN>" \
  --set-secrets "OPENROUTER_API_KEY=openrouter-key:latest,JOBS_DRAIN_SECRET=jobs-drain-secret:latest"
```

Put `OPENROUTER_API_KEY` and a random `JOBS_DRAIN_SECRET` in **Secret Manager**
first (`gcloud secrets create ...`). For a free, no-spend demo set the `*_MOCK`
vars to `true` and skip the OpenRouter secret.

## 3. Async jobs (Cloud Scheduler)
Drive the queue every minute (the endpoint is inert unless `JOBS_DRAIN_SECRET` is set):

```bash
gcloud scheduler jobs create http canonforge-drain \
  --schedule "* * * * *" \
  --uri "https://<CLOUD_RUN_URL>/internal/jobs/drain" \
  --http-method POST \
  --headers "x-drain-secret=<SAME_SECRET>" \
  --location <REGION>
```

`POST /internal/jobs/drain` processes up to 25 queued jobs per call and returns
`{"processed": N}`. Wrong/missing secret → 401; secret unset → 404 (disabled).

## 4. Frontend (single origin = cookies just work)
Auth is an httpOnly session cookie, so keep the frontend **same-origin** as the
API. Easiest on GCP: **Firebase Hosting** with a rewrite to the Cloud Run
service — same origin, no CORS, no `SameSite=None` dance.

```jsonc
// firebase.json
{ "hosting": {
    "public": "frontend/dist",
    "rewrites": [{ "source": "/(auth|projects|workspaces|jobs|usage|health|internal)/**",
                   "run": { "serviceId": "canonforge", "region": "<REGION>" } },
                 { "source": "**", "destination": "/index.html" }]
}}
```

Build with `VITE_API_BASE_URL=""` so the client calls same-origin paths.
(Alternative: serve `frontend/dist` straight from the backend — a small
`ServeDir` route — if you'd rather not run Firebase Hosting.)

## Notes / trade-offs
- **Cold start:** first request after idle wakes the machine (~1–2s).
- **Worker vs. scheduler:** locally keep `JOBS_WORKER` unset (in-process worker,
  no scheduler needed). In Cloud Run set `JOBS_WORKER=false` + the scheduler.
  Both claim via `FOR UPDATE SKIP LOCKED`, so running both is safe.
- **Even simpler, not scale-to-zero:** a $4/mo VPS + `docker compose up` runs
  everything (incl. the in-process worker) predictably.
- **AWS:** workable but fights this goal (no scale-to-zero on App Runner/RDS;
  Lambda has no background loop; free tier expires) — prefer Cloud Run here.
