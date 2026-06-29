#!/usr/bin/env bash
# Build + deploy (or redeploy) the backend to Cloud Run in REAL AI mode.
# Secrets `neon-url` and `openrouter-key` must already exist in Secret Manager
# (see DEPLOY.md). Run from the repo root: ./deploy/deploy.sh
cd "$(dirname "$0")/.."
source deploy/config.sh

gcloud run deploy "$SERVICE" \
  --source . --region "$REGION" --allow-unauthenticated \
  --min-instances 0 --max-instances 2 --memory 1Gi \
  --set-env-vars "COOKIE_SECURE=true,JOBS_WORKER=true,ASSET_MOCK=false,EMBED_MOCK=false,LLM_MOCK=false,AUDIO_MOCK=false,USAGE_MOCK=false" \
  --set-secrets "DATABASE_URL=neon-url:latest,OPENROUTER_API_KEY=openrouter-key:latest"

echo "✅ deployed: $(svc_url)"
echo "   Set BACKEND_URL to that in the Cloudflare Pages project, then redeploy the frontend."
