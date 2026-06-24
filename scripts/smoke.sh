#!/usr/bin/env bash
# End-to-end smoke test for the Design Studio API (auth + CRUD lifecycle).
# Assumes the backend is running on localhost:8080 and the DB is up.
set -euo pipefail
BASE=${BASE:-http://localhost:8080}
JAR=$(mktemp)            # user A cookie jar
JAR_B=$(mktemp)          # user B cookie jar
trap 'rm -f "$JAR" "$JAR_B"' EXIT
j() { jq -r "$1"; }
EMAIL_A="a-$RANDOM@example.com"
EMAIL_B="b-$RANDOM@example.com"

echo "== health =="
curl -s "$BASE/health"; echo

echo "== unauthenticated call -> 401 =="
echo -n "  GET /workspaces (no cookie) -> "
curl -s -o /dev/null -w '%{http_code}\n' "$BASE/workspaces"

echo "== signup user A (creates default workspace + owner membership) =="
SIGNUP=$(curl -s -c "$JAR" -XPOST "$BASE/auth/signup" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL_A\",\"password\":\"supersecret\",\"workspace_name\":\"Acme Design\"}")
echo "$SIGNUP" | jq '{user: .user.email, workspace: .workspace.name}'
WID=$(echo "$SIGNUP" | j '.workspace.id')

echo "== GET /auth/me (authenticated) =="
curl -s -b "$JAR" "$BASE/auth/me" | jq '{email}'

echo "== create project =="
PID=$(curl -s -b "$JAR" -XPOST "$BASE/workspaces/$WID/projects" -H 'content-type: application/json' \
  -d '{"name":"Onboarding","brief":"Mobile signup flow"}' | j '.id')
echo "  project: $PID"

echo "== create artifact (idea) + v1 =="
ART=$(curl -s -b "$JAR" -XPOST "$BASE/projects/$PID/artifacts" -H 'content-type: application/json' \
  -d '{"kind":"idea","name":"Signup idea","content":{"text":"3-step signup"},"prompt":"initial"}')
AID=$(echo "$ART" | j '.id')
V1=$(echo "$ART" | j '.head_version.id')

echo "== add v2 (ai) =="
V2=$(curl -s -b "$JAR" -XPOST "$BASE/artifacts/$AID/versions" -H 'content-type: application/json' \
  -d '{"content":{"text":"2-step signup"},"change_source":"ai","change_summary":"reduced steps"}' | j '.id')

echo "== get artifact (head should be v2) =="
curl -s -b "$JAR" "$BASE/artifacts/$AID" | jq '{head_version_id, head_text: .head_version.content.text}'

echo "== version history (v2.parent==v1, v1 root) =="
curl -s -b "$JAR" "$BASE/artifacts/$AID/versions" | jq --arg v1 "$V1" --arg v2 "$V2" \
  '{count: length, v2_parent_ok: (.[0].id==$v2 and .[0].parent_id==$v1), v1_root: (.[1].parent_id==null)}'

echo "== second artifact + link =="
AID2=$(curl -s -b "$JAR" -XPOST "$BASE/projects/$PID/artifacts" -H 'content-type: application/json' \
  -d '{"kind":"user_flow","name":"Signup flow","content":{"nodes":[]}}' | j '.id')
curl -s -b "$JAR" -XPOST "$BASE/artifacts/$AID2/links" -H 'content-type: application/json' \
  -d "{\"to_artifact_id\":\"$AID\",\"relation\":\"derived_from\"}" >/dev/null
echo "  links from flow:"; curl -s -b "$JAR" "$BASE/artifacts/$AID2/links" | jq '{count: length, relation: .[0].relation}'

# Non-members get 404 (existence hidden); 403 is reserved for members whose
# role is too low for the action.
echo "== access control: user B cannot see user A's resources -> 404 =="
curl -s -c "$JAR_B" -XPOST "$BASE/auth/signup" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL_B\",\"password\":\"supersecret\"}" >/dev/null
echo -n "  user B GET A's artifact -> "
curl -s -b "$JAR_B" -o /dev/null -w '%{http_code}\n' "$BASE/artifacts/$AID"
echo -n "  user B GET A's workspace projects -> "
curl -s -b "$JAR_B" -o /dev/null -w '%{http_code}\n' "$BASE/workspaces/$WID/projects"

echo "== auth negatives =="
echo -n "  duplicate-email signup -> "
curl -s -o /dev/null -w '%{http_code}\n' -XPOST "$BASE/auth/signup" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL_A\",\"password\":\"supersecret\"}"
echo -n "  wrong password login -> "
curl -s -o /dev/null -w '%{http_code}\n' -XPOST "$BASE/auth/login" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL_A\",\"password\":\"wrongpass\"}"
echo -n "  correct login -> "
curl -s -o /dev/null -w '%{http_code}\n' -XPOST "$BASE/auth/login" -H 'content-type: application/json' \
  -d "{\"email\":\"$EMAIL_A\",\"password\":\"supersecret\"}"

echo "DONE"
