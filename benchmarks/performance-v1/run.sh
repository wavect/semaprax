#!/usr/bin/env bash
set -euo pipefail

# Wrapper for benchmarks/performance-v1/run.py — no hyperfine needed.
# Usage: ./benchmarks/performance-v1/run.sh [--with-build] [--quick] [--dry-run] [--release]
#            [--compare BASELINE] [--output FILE] [--markdown FILE] [--only ID]
#            [--semaprax BINARY] [--scenarios FILE] [--root DIR]
#
# The suite directory is resolved from this script, so the wrapper runs from any
# working directory. Only --output and --compare follow the caller's cwd.

SUITE="$(cd "$(dirname "$0")" && pwd)"
OUTPUT="${OUTPUT:-$SUITE/results/local.json}"
FORWARD=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --with-build) FORWARD+=(--with-build); shift ;;
    --quick) FORWARD+=(--quick); shift ;;
    --dry-run) FORWARD+=(--dry-run); shift ;;
    --release) FORWARD+=(--release); shift ;;
    --compare|--markdown|--only|--semaprax|--scenarios|--root)
      if [[ $# -lt 2 ]]; then
        echo "error: $1 needs a value" >&2
        exit 2
      fi
      FORWARD+=("$1" "$2")
      shift 2
      ;;
    --output)
      if [[ $# -lt 2 ]]; then
        echo "error: --output needs a file path" >&2
        exit 2
      fi
      OUTPUT="$2"
      shift 2
      ;;
    *) echo "error: unknown argument: $1" >&2; exit 2 ;;
  esac
done

echo "Running macrobenchmarks..."
echo "  suite: $SUITE"
echo "  output: $OUTPUT"
echo "  forwarded: ${FORWARD[*]:-none}"

python3 "$SUITE/run.py" --output "$OUTPUT" ${FORWARD[@]+"${FORWARD[@]}"}

echo ""
echo "Done. Results:"
ls -lh "$OUTPUT"
python3 -m json.tool "$OUTPUT" | head -n 60
