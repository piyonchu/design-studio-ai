#!/usr/bin/env bash
# Bring the demo back online (instant — reuses the already-built image). Use
# after stop-demo.sh when you want to demo / continue. If the service was
# deleted, run ./deploy/deploy.sh instead (rebuilds).
#   ./deploy/start-demo.sh
cd "$(dirname "$0")/.."
source deploy/config.sh

if ! svc_url >/dev/null; then
  echo "✖ service '$SERVICE' not found — run ./deploy/deploy.sh first."
  exit 1
fi
gcloud run services update "$SERVICE" --region "$REGION" --allow-unauthenticated >/dev/null
echo "▶  demo live: $(svc_url)"
