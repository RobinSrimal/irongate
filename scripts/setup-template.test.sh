#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
test_root="$(mktemp -d)"
trap 'rm -rf "$test_root"' EXIT

write_file() {
  local path="$1"
  mkdir -p "$(dirname "$test_root/$path")"
  cat > "$test_root/$path"
}

assert_contains() {
  local path="$1"
  local expected="$2"

  if ! grep -Fq "$expected" "$test_root/$path"; then
    echo "Expected $path to contain: $expected" >&2
    echo "--- $path ---" >&2
    cat "$test_root/$path" >&2
    exit 1
  fi
}

assert_not_contains() {
  local path="$1"
  local unexpected="$2"

  if grep -Fq "$unexpected" "$test_root/$path"; then
    echo "Expected $path not to contain: $unexpected" >&2
    echo "--- $path ---" >&2
    cat "$test_root/$path" >&2
    exit 1
  fi
}

write_file "README.md" <<'EOF'
# Irongate

Irongate deploys irongate.
EOF

write_file "package.json" <<'EOF'
{
  "name": "irongate"
}
EOF

write_file "package-lock.json" <<'EOF'
{
  "name": "irongate",
  "packages": {
    "": { "name": "irongate" },
    "node_modules/@irongate/functions": { "link": true },
    "packages/functions": { "name": "@irongate/functions" }
  }
}
EOF

write_file "sst.config.ts" <<'EOF'
const appName = "irongate";
const defaultDevProfile = `${appName}-dev`;
const defaultProdProfile = `${appName}-prod`;
EOF

write_file ".example.env" <<'EOF'
# Keep Irongate runtime secrets in SST secrets.
# SST_DEV_AWS_PROFILE=irongate-dev
# SST_PROD_AWS_PROFILE=irongate-prod
EOF

write_file "auth.clients.toml" <<'EOF'
[[clients]]
client_id = "web"
allowed_origins = ["http://localhost:3000"]
EOF

write_file "docs/setup/01-template-setup.md" <<'EOF'
# Template Setup

Run the Irongate setup script.
EOF

write_file "design/README.md" <<'EOF'
# Design

Irongate design docs.
EOF

write_file "infra/shared/stage-config.ts" <<'EOF'
const stageConfigs = {
  dev: {
    email: {
      brandName: "Irongate Dev",
    },
  },
};
EOF

write_file "packages/functions/package.json" <<'EOF'
{
  "name": "@irongate/functions"
}
EOF

write_file "packages/functions/auth/Cargo.toml" <<'EOF'
[package]
name = "irongate"
EOF

write_file "packages/functions/auth/Cargo.lock" <<'EOF'
[[package]]
name = "irongate"
EOF

write_file "packages/functions/admin/Cargo.toml" <<'EOF'
[package]
name = "irongate-admin"

[dependencies]
auth = { package = "irongate", path = "../auth" }
EOF

write_file "packages/functions/admin/Cargo.lock" <<'EOF'
[[package]]
name = "irongate-admin"

[[package]]
name = "irongate"
EOF

write_file "packages/functions/auth/src/routes.rs" <<'EOF'
cookie.strip_prefix("irongate_session=");
EOF

write_file "packages/functions/auth/src/error.rs" <<'EOF'
pub enum IrongateError {}
//! Error types for Irongate.
EOF

write_file "packages/examples/web/package.json" <<'EOF'
{
  "name": "@irongate/example-web"
}
EOF

write_file "packages/examples/web/src/session.ts" <<'EOF'
export const cookieName = "irongate_session";
EOF

write_file "packages/examples/app/package.json" <<'EOF'
{
  "name": "@irongate/example-app"
}
EOF

write_file "packages/examples/app/src-tauri/tauri.conf.json" <<'EOF'
{
  "productName": "Irongate"
}
EOF

output="$(IRONGATE_TEMPLATE_ROOT="$test_root" bash "$repo_root/scripts/setup-template.sh" "My Cool_App")"

grep -Fq "Project slug: my-cool-app" <<<"$output"
grep -Fq "Default AWS dev profile: my-cool-app-dev" <<<"$output"
grep -Fq "Default AWS production profile: my-cool-app-prod" <<<"$output"

assert_contains "README.md" "# My Cool App"
assert_contains "README.md" "My Cool App deploys my-cool-app."
assert_contains "package.json" '"name": "my-cool-app"'
assert_contains "package-lock.json" '"name": "my-cool-app"'
assert_contains "package-lock.json" 'node_modules/@my-cool-app/functions'
assert_contains ".example.env" "Keep My Cool App runtime secrets in SST secrets."
assert_contains ".example.env" "SST_DEV_AWS_PROFILE=my-cool-app-dev"
assert_contains ".example.env" "SST_PROD_AWS_PROFILE=my-cool-app-prod"
assert_contains "docs/setup/01-template-setup.md" "Run the My Cool App setup script."
assert_contains "design/README.md" "My Cool App design docs."
assert_contains "infra/shared/stage-config.ts" 'brandName: "My Cool App Dev"'
assert_contains "packages/functions/package.json" '"name": "@my-cool-app/functions"'
assert_contains "packages/functions/auth/Cargo.toml" 'name = "my-cool-app"'
assert_contains "packages/functions/auth/Cargo.lock" 'name = "my-cool-app"'
assert_contains "packages/functions/admin/Cargo.toml" 'name = "my-cool-app-admin"'
assert_contains "packages/functions/admin/Cargo.toml" 'auth = { package = "my-cool-app", path = "../auth" }'
assert_contains "packages/functions/admin/Cargo.lock" 'name = "my-cool-app-admin"'
assert_contains "packages/functions/admin/Cargo.lock" 'name = "my-cool-app"'
assert_contains "packages/functions/auth/src/routes.rs" 'cookie.strip_prefix("my-cool-app_session=");'
assert_contains "packages/functions/auth/src/error.rs" "pub enum IrongateError {}"
assert_contains "packages/functions/auth/src/error.rs" "//! Error types for My Cool App."
assert_contains "packages/examples/web/package.json" '"name": "@my-cool-app/example-web"'
assert_contains "packages/examples/web/src/session.ts" 'export const cookieName = "my-cool-app_session";'
assert_contains "packages/examples/app/package.json" '"name": "@my-cool-app/example-app"'
assert_contains "packages/examples/app/src-tauri/tauri.conf.json" '"productName": "My Cool App"'
assert_not_contains "package-lock.json" "@irongate/functions"
