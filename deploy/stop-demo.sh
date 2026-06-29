#!/usr/bin/env bash
# Take the demo OFFLINE when you're done. The service keeps its built image (so
# start-demo is instant) but stops serving the public — no traffic, no AI spend.
# Cloud Run min-instances=0 means idle cost is already ~$0; this also closes the
# public door. Neon scales to zero on its own.
#   ./deploy/stop-demo.sh
cd "$(dirname "$0")/.."
source deploy/config.sh

# Remove public access via IAM (anonymous requests get 403). The service + its
# built image stay; min-instances is already 0 so idle cost is ~$0.
gcloud run services remove-iam-policy-binding "$SERVICE" --region "$REGION" \
  --member=allUsers --role=roles/run.invoker >/dev/null 2>&1 || true
echo "⏹  demo stopped — $SERVICE is no longer public (image kept; ~\$0 idle)."
echo "   Run ./deploy/start-demo.sh to bring it back. (Hard teardown:"
echo "   gcloud run services delete $SERVICE --region $REGION)"
