#!/usr/bin/env bash
# Bootstrap the Irongate OAuth server and register a test client.
# Run this once after starting the server with cargo lambda watch.

set -euo pipefail

SERVER="http://localhost:9000"

echo "==> Bootstrapping admin API key..."
BOOTSTRAP=$(curl -s -X POST "$SERVER/admin/bootstrap")
echo "$BOOTSTRAP"

API_KEY=$(echo "$BOOTSTRAP" | python3 -c "import sys,json; print(json.load(sys.stdin)['api_key'])" 2>/dev/null || true)

if [ -z "$API_KEY" ]; then
  echo "Bootstrap may have already been done. Enter your admin API key:"
  read -r API_KEY
fi

echo ""
echo "==> Registering test client..."
curl -s -X POST "$SERVER/admin/clients" \
  -H "X-Admin-API-Key: $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "client_id": "test-app",
    "client_type": "public",
    "redirect_uris": ["http://localhost:3000/"],
    "allowed_grant_types": ["authorization_code", "refresh_token"],
    "allowed_scopes": ["openid", "profile"],
    "pkce_required": true
  }' | python3 -m json.tool

echo ""
echo "==> Done! Open http://localhost:3000 in your browser."
