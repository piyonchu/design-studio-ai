#!/usr/bin/env bash
# Shared config for the deploy/demo scripts. Source this from the others.
set -euo pipefail

export PROJECT="${PROJECT:-asset-studio-500908}"
export REGION="${REGION:-asia-southeast1}"     # Singapore — near the Neon DB (ap-southeast-1)
export SERVICE="${SERVICE:-canonforge}"

gcloud config set project "$PROJECT" >/dev/null 2>&1 || true

svc_url() {
  gcloud run services describe "$SERVICE" --region "$REGION" \
    --format 'value(status.url)' 2>/dev/null
}
