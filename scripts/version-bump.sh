#!/usr/bin/env bash
# scripts/version-bump.sh — Bump VERSION file and sync to Cargo.toml.
# Usage: ./scripts/version-bump.sh <patch|minor|major>

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION_FILE="$PROJECT_DIR/VERSION"

current=$(tr -d '[:space:]' < "$VERSION_FILE")
IFS='.' read -r major minor patch <<< "$current"

case "${1:-}" in
  patch) patch=$((patch + 1)) ;;
  minor) minor=$((minor + 1)); patch=0 ;;
  major) major=$((major + 1)); minor=0; patch=0 ;;
  *) echo "Usage: $0 <patch|minor|major>" >&2; exit 1 ;;
esac

new="$major.$minor.$patch"
echo "$new" > "$VERSION_FILE"

# Sync to Cargo.toml
sed -i "s/^version = \".*\"/version = \"$new\"/" "$PROJECT_DIR/Cargo.toml"

echo "$new"
