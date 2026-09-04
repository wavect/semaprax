#!/usr/bin/env bash
set -euo pipefail

# Wrapper for benchmark/run.py — no hyperfine needed.
# Usage: ./benchmark/run.sh [--with-build] [--quick] [--compare baseline.json]

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT="${OUTPUT:-benchmark/results/local.json}"
COMPARE=""
WITH_BUILD=""
QUICK=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-build) WITH_BUILD="--with-build"; shift ;;
    --quick) QUICK="--quick"; shift ;;
    --compare) COMPARE="--compare $2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

echo "Running macrobenchmarks..."
echo "  output: $OUTPUT"
echo "  with_build: ${WITH_BUILD:-no}"
echo "  quick: ${QUICK:-no}"

# shellcheck disable=SC2086
python3 "$ROOT/benchmark/run.py" --output "$OUTPUT" $WITH_BUILD $QUICK $COMPARE

echo ""
echo "Done. Results:"
ls -lh "$OUTPUT"
cat "$OUTPUT" | python3 -m json.tool | head -n 60
