#!/usr/bin/env bash
# scripts/version-tag.sh — Create a git tag from VERSION file.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

version=$(tr -d '[:space:]' < "$PROJECT_DIR/VERSION")
tag="v$version"

# Sync Cargo.toml in case VERSION was edited manually
sed -i "s/^version = \".*\"/version = \"$version\"/" "$PROJECT_DIR/Cargo.toml"

git -C "$PROJECT_DIR" add VERSION Cargo.toml
git -C "$PROJECT_DIR" commit -m "chore: release $tag" --allow-empty
git -C "$PROJECT_DIR" tag -a "$tag" -m "Release $tag"

echo "Tagged $tag"
