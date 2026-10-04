#!/bin/sh
# Run the HR-07 interpreter pilot against one already-built, current-head CLI.
set -eu

if [ "$(uname -s)" != "Darwin" ]; then
    echo "macOS pilot requires Darwin" >&2
    exit 2
fi

semaprax=
output=/tmp/hot-reload-benchmark.json
while [ "$#" -gt 0 ]; do
    case "$1" in
        --semaprax) semaprax=${2-}; shift 2 ;;
        --output) output=${2-}; shift 2 ;;
        *) echo "usage: $0 --semaprax ABSOLUTE_EXECUTABLE [--output ABSOLUTE_JSON]" >&2; exit 2 ;;
    esac
done

if [ -z "$semaprax" ] || [ ! -x "$semaprax" ]; then
    echo "--semaprax must name an executable" >&2
    exit 2
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
commit=$(git -C "$root" rev-parse HEAD)
exec python3 "$root/benchmarks/hot-reload-v1/run.py" \
    --semaprax "$semaprax" \
    --samples 11 \
    --warmups 3 \
    --expected-commit "$commit" \
    --output "$output"
