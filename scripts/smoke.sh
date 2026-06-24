#!/usr/bin/env bash
# End-to-end smoke test for the Design Studio API.
# Assumes the backend is running on localhost:8080 and the DB is up.
set -euo pipefail
BASE=${BASE:-http://localhost:8080}
j() { jq -r "$1"; }

echo "== health =="
curl -s "$BASE/health"; echo

echo "== create workspace =="
WID=$(curl -s -XPOST "$BASE/workspaces" -H 'content-type: application/json' \
  -d '{"name":"Acme Design"}' | tee /dev/stderr | j '.id'); echo

echo "== create project =="
PID=$(curl -s -XPOST "$BASE/workspaces/$WID/projects" -H 'content-type: application/json' \
  -d '{"name":"Onboarding","brief":"Mobile signup flow"}' | tee /dev/stderr | j '.id'); echo

echo "== create artifact (idea) + v1 =="
ART=$(curl -s -XPOST "$BASE/projects/$PID/artifacts" -H 'content-type: application/json' \
  -d '{"kind":"idea","name":"Signup idea","content":{"text":"3-step signup"},"prompt":"initial"}')
echo "$ART"; echo
AID=$(echo "$ART" | j '.id')
V1=$(echo "$ART" | j '.head_version.id')

echo "== add v2 =="
V2=$(curl -s -XPOST "$BASE/artifacts/$AID/versions" -H 'content-type: application/json' \
  -d '{"content":{"text":"2-step signup"},"change_source":"ai","change_summary":"reduced steps","prompt":"make it shorter"}' \
  | tee /dev/stderr | j '.id'); echo

echo "== get artifact (head should be v2) =="
curl -s "$BASE/artifacts/$AID" | jq '{head_version_id, head_text: .head_version.content.text}'

echo "== version history (newest first; v2.parent_id == v1) =="
curl -s "$BASE/artifacts/$AID/versions" | jq --arg v1 "$V1" --arg v2 "$V2" \
  '{count: length, v2_parent_ok: (.[0].id==$v2 and .[0].parent_id==$v1), v1_root: (.[1].parent_id==null)}'

echo "== second artifact + link =="
AID2=$(curl -s -XPOST "$BASE/projects/$PID/artifacts" -H 'content-type: application/json' \
  -d '{"kind":"user_flow","name":"Signup flow","content":{"nodes":[]}}' | j '.id')
curl -s -XPOST "$BASE/artifacts/$AID2/links" -H 'content-type: application/json' \
  -d "{\"to_artifact_id\":\"$AID\",\"relation\":\"derived_from\"}" >/dev/null
echo "links from flow:"; curl -s "$BASE/artifacts/$AID2/links" | jq '{count: length, relation: .[0].relation, to: .[0].to_artifact_id}'

echo "== error cases =="
echo -n "missing artifact -> "; curl -s -o /dev/null -w '%{http_code}\n' "$BASE/artifacts/00000000-0000-0000-0000-000000000000"
echo -n "project under missing workspace -> "; curl -s -o /dev/null -w '%{http_code}\n' \
  -XPOST "$BASE/workspaces/00000000-0000-0000-0000-000000000000/projects" \
  -H 'content-type: application/json' -d '{"name":"x"}'
echo "DONE"
