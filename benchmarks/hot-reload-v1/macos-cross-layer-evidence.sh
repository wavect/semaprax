#!/bin/sh
# Build the current checkout into a private target and capture HR-07 evidence.
set -eu
if [ "$(uname -s)" != "Darwin" ]; then
    echo "macOS evidence requires Darwin" >&2
    exit 2
fi
root=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
target_dir=$root/target/hr07-macos-evidence
output=/tmp/hot-reload-macos-cross-layer-evidence.json
while [ "$#" -gt 0 ]; do
    case "$1" in
        --target-dir) target_dir=${2-}; shift 2 ;;
        --output) output=${2-}; shift 2 ;;
        *) echo "usage: $0 [--target-dir ABS_PRIVATE_TARGET] [--output ABS_JSON]" >&2; exit 2 ;;
    esac
done
case "$target_dir" in "$root"/target/*) ;; *) echo "--target-dir must be below $root/target" >&2; exit 2 ;; esac
exec python3 "$root/benchmarks/hot-reload-v1/macos_cross_layer_evidence.py" \
    --target-dir "$target_dir" --output "$output"
