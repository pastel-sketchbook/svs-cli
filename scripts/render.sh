#!/usr/bin/env bash
# scripts/render.sh — Production render wrapper for svs CLI.
#
# Usage:
#   ./scripts/render.sh <input> [options]
#
# Examples:
#   ./scripts/render.sh slides.pdf
#   ./scripts/render.sh slides.pdf --voice kore --transition fade
#   ./scripts/render.sh ./slides/ --output talk.mp4 --notes-model gemini-2.5-flash
#
# All flags are forwarded directly to `svs render`. Run `svs render --help`
# for the full list.
#
# Environment:
#   GEMINI_API_KEY  — required (or pass --api-key)
#   SVS_BIN        — path to svs binary (default: cargo run --release)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <input> [svs render flags...]" >&2
  echo "" >&2
  echo "Voices:      zephyr, puck, charon, kore, fenrir" >&2
  echo "Transitions: none, fade, slide, wipe, zoom" >&2
  echo "" >&2
  echo "Examples:" >&2
  echo "  $0 deck.pdf" >&2
  echo "  $0 deck.pdf --voice kore --transition fade" >&2
  echo "  $0 ./slides/ --output out.mp4" >&2
  exit 1
fi

if [[ -n "${SVS_BIN:-}" ]]; then
  exec "$SVS_BIN" render "$@"
else
  exec cargo run --release --quiet --manifest-path "$PROJECT_DIR/Cargo.toml" -- render "$@"
fi
