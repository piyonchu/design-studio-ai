#!/usr/bin/env bash
# Take the demo OFFLINE when you're done. The service keeps its built image (so
# start-demo is instant) but stops serving the public — no traffic, no AI spend.
# Cloud Run min-instances=0 means idle cost is already ~$0; this also closes the
# public door. Neon scales to zero on its own.
#   ./deploy/stop-demo.sh
cd "$(dirname "$0")/.."
source deploy/config.sh

# Remove public access (anonymous requests get 403) and pin to zero instances.
gcloud run services update "$SERVICE" --region "$REGION" \
  --no-allow-unauthenticated --min-instances 0 >/dev/null
echo "⏹  demo stopped — $SERVICE is no longer public (image kept; ~\$0 idle)."
echo "   Run ./deploy/start-demo.sh to bring it back. (Hard teardown:"
echo "   gcloud run services delete $SERVICE --region $REGION)"
