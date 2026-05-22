#!/usr/bin/env bash
# scripts/render-fast.sh — Quick render: no transitions, lower DPI.
#
# Usage:
#   ./scripts/render-fast.sh <input> [extra svs flags...]
#
# Defaults: voice=zephyr, transition=none, dpi=150, concurrency=2.
# Override any default by passing the flag explicitly.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

if [[ $# -lt 1 ]]; then
  echo "Usage: $0 <input> [extra flags...]" >&2
  exit 1
fi

INPUT="$1"; shift

exec "$SCRIPT_DIR/render.sh" "$INPUT" \
  --transition none \
  --pdf-dpi 150 \
  --gemini-concurrency 2 \
  "$@"
