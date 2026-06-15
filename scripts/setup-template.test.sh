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

output="$(IRONGATE_TEMPLATE_ROOT="$test_root" bash "$repo_root/scripts/setup-template.sh" "My Cool_App")"

grep -Fq "Project slug: my-cool-app" <<<"$output"
grep -Fq "Default AWS dev profile: my-cool-app-dev" <<<"$output"
grep -Fq "Default AWS production profile: my-cool-app-prod" <<<"$output"

assert_contains "README.md" "# My Cool App"
assert_contains "README.md" "My Cool App deploys my-cool-app."
assert_contains "package.json" '"name": "my-cool-app"'
assert_contains "package-lock.json" '"name": "my-cool-app"'
assert_contains "package-lock.json" 'node_modules/@my-cool-app/functions'
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
assert_not_contains "package-lock.json" "@irongate/functions"
