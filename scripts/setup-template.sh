#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C
export LANG=C

root_dir="${IRONGATE_TEMPLATE_ROOT:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
project_input="${1:-$(basename "$root_dir")}"

project_slug="$(
  printf '%s' "$project_input" |
    tr '[:upper:]_' '[:lower:]-' |
    perl -pe 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/-{2,}/-/g'
)"

if [[ -z "$project_slug" ]]; then
  echo "Project name must contain at least one letter or number." >&2
  exit 1
fi

display_name="${PROJECT_DISPLAY_NAME:-$(
  printf '%s' "$project_slug" |
    perl -pe 's/-/ /g; s/\b([a-z])/\U$1/g'
)}"

template_files=(
  "README.md"
  "package.json"
  "package-lock.json"
  "sst.config.ts"
  "packages/functions/package.json"
  "packages/functions/auth/Cargo.toml"
  "packages/functions/auth/Cargo.lock"
  "packages/functions/auth/src/config.rs"
  "packages/functions/auth/src/error.rs"
  "packages/functions/auth/src/lib.rs"
  "packages/functions/auth/src/main.rs"
  "packages/functions/auth/src/oauth/authorize.rs"
  "packages/functions/auth/src/provider/oauth2.rs"
  "packages/functions/auth/src/routes.rs"
  "packages/functions/auth/src/storage/mod.rs"
)

changed_files=()

replace_in_file() {
  local relative_path="$1"
  local file_path="$root_dir/$relative_path"

  [[ -f "$file_path" ]] || return 0

  local before
  before="$(cksum "$file_path")"

  PROJECT_SLUG="$project_slug" DISPLAY_NAME="$display_name" perl -0pi -e '
    s/irongate_session/$ENV{PROJECT_SLUG}_session/g;
    s/\birongate\b/$ENV{PROJECT_SLUG}/g;
    s/\bIrongate\b/$ENV{DISPLAY_NAME}/g;
  ' "$file_path"

  local after
  after="$(cksum "$file_path")"

  if [[ "$before" != "$after" ]]; then
    changed_files+=("$relative_path")
  fi
}

for file in "${template_files[@]}"; do
  replace_in_file "$file"
done

echo "Project name: $display_name"
echo "Project slug: $project_slug"
echo "Default AWS dev profile: $project_slug-dev"
echo "Default AWS production profile: $project_slug-prod"

if [[ "${#changed_files[@]}" -gt 0 ]]; then
  echo
  echo "Updated files:"
  printf '  - %s\n' "${changed_files[@]}"
else
  echo
  echo "No template placeholders were changed."
fi
