#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf 'RI10 gate: %s\n' "$*" >&2
    exit 1
}

require_tool() {
    local name=$1
    local path=$2
    case "$path" in
        /*) ;;
        *) fail "$name must be an absolute path" ;;
    esac
    [[ "$path" != *$'\n'* && "$path" != *$'\r'* && "$path" != *';'* ]] || \
        fail "$name contains a line break or the tracked-input delimiter"
    [[ -f "$path" && -x "$path" ]] || fail "$name is not an executable file: $path"
}

: "${SEMAPRAX_RI10_CARGO:?set SEMAPRAX_RI10_CARGO to an absolute Cargo executable}"
: "${SEMAPRAX_RI10_BUILDER:?set SEMAPRAX_RI10_BUILDER to an already-built SDK executable}"
: "${SEMAPRAX_RI10_RUSTC:?set SEMAPRAX_RI10_RUSTC to an absolute rustc executable}"
: "${SEMAPRAX_RI10_CLANG:?set SEMAPRAX_RI10_CLANG to an absolute clang executable}"
: "${SEMAPRAX_RI10_ARCHIVER:?set SEMAPRAX_RI10_ARCHIVER to an absolute archiver executable}"
require_tool SEMAPRAX_RI10_CARGO "$SEMAPRAX_RI10_CARGO"
require_tool SEMAPRAX_RI10_BUILDER "$SEMAPRAX_RI10_BUILDER"
require_tool SEMAPRAX_RI10_RUSTC "$SEMAPRAX_RI10_RUSTC"
require_tool SEMAPRAX_RI10_CLANG "$SEMAPRAX_RI10_CLANG"
require_tool SEMAPRAX_RI10_ARCHIVER "$SEMAPRAX_RI10_ARCHIVER"

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd -P)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd -P)
[[ "$repo_root" != *$'\n'* && "$repo_root" != *$'\r'* && "$repo_root" != *';'* ]] || \
    fail "repository path contains a line break or the tracked-input delimiter"
temp_base=${TMPDIR:-/tmp}
case "$temp_base" in
    /*) ;;
    *) fail "TMPDIR must be absolute when set" ;;
esac
[[ "$temp_base" != *$'\n'* && "$temp_base" != *$'\r'* && "$temp_base" != *';'* ]] || \
    fail "TMPDIR contains a line break or the tracked-input delimiter"

scratch=$(mktemp -d "$temp_base/semaprax-ri10-consumer.XXXXXX")
scratch=$(CDPATH='' cd -- "$scratch" && pwd -P)
case "$scratch/" in
    "$repo_root/"*) fail "temporary consumer must be outside the repository" ;;
esac

finished=0
cleanup() {
    local status=$?
    trap - EXIT
    if [[ $status -eq 0 && $finished -eq 1 ]]; then
        rm -r "$scratch"
    else
        printf 'RI10 gate: retained temporary consumer at %s\n' "$scratch" >&2
    fi
    exit "$status"
}
trap cleanup EXIT

consumer="$scratch/build-script-consumer"
project="$scratch/calculator-project"
shim_dir="$scratch/shims"
mkdir -p "$consumer" "$project" "$shim_dir"
cp -R "$repo_root/examples/calculator-rust/build-script-consumer/." "$consumer/"
cp -R "$repo_root/examples/calculator-project/." "$project/"

builder_log="$scratch/builder-invocations.log"
nested_cargo_log="$scratch/nested-cargo-invocations.log"
: > "$builder_log"
: > "$nested_cargo_log"

# Observe direct builder calls while dispatching to the supplied prebuilt binary.
cat > "$shim_dir/semaprax-builder" <<'SH'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$SEMAPRAX_RI10_BUILDER_LOG"
export CARGO="$SEMAPRAX_RI10_CARGO_GUARD"
exec "$SEMAPRAX_RI10_BUILDER_REAL" "$@"
SH

# Any Cargo command resolved through PATH from a build script or builder fails
# immediately and leaves a trace for the assertion below.
cat > "$shim_dir/cargo" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$SEMAPRAX_RI10_NESTED_CARGO_LOG"
printf 'nested Cargo invocation is forbidden by the RI10 gate\n' >&2
exit 97
SH
chmod +x "$shim_dir/semaprax-builder" "$shim_dir/cargo"

project_manifest="$project/semaprax.toml"
tool_identity="$scratch/tool-identity.txt"
cat > "$tool_identity" <<EOF
builder=$SEMAPRAX_RI10_BUILDER
cargo=$SEMAPRAX_RI10_CARGO
rustc=$SEMAPRAX_RI10_RUSTC
clang=$SEMAPRAX_RI10_CLANG
archiver=$SEMAPRAX_RI10_ARCHIVER
EOF
tracked_inputs="$project_manifest;$project/src/app.spx;$project/src/core.spx;$project/src/tests.spx;$consumer/Cargo.lock;$tool_identity"

builder_count() {
    wc -l < "$builder_log" | tr -d '[:space:]'
}

run_cargo_test() (
        cd "$consumer"
        real_builder="$SEMAPRAX_RI10_BUILDER"
        export PATH="$shim_dir:$PATH"
        export CARGO_TARGET_DIR="$scratch/target"
        export SEMAPRAX_RI10_BUILDER="$shim_dir/semaprax-builder"
        export SEMAPRAX_RI10_BUILDER_REAL="$real_builder"
        export SEMAPRAX_RI10_BUILDER_LOG="$builder_log"
        export SEMAPRAX_RI10_CARGO_GUARD="$shim_dir/cargo"
        export SEMAPRAX_RI10_NESTED_CARGO_LOG="$nested_cargo_log"
        export SEMAPRAX_RI10_PROJECT_MANIFEST="$project_manifest"
        export SEMAPRAX_RI10_INPUTS="$tracked_inputs"
        export RUSTC="$SEMAPRAX_RI10_RUSTC"
        export CLANG="$SEMAPRAX_RI10_CLANG"
        export SEMAPRAX_ARCHIVER="$SEMAPRAX_RI10_ARCHIVER"
        "$SEMAPRAX_RI10_CARGO" test --locked --offline \
            --manifest-path "$consumer/Cargo.toml" -- --test-threads=1
    )

run_consumer_test() {
    local label=$1
    local log="$scratch/$label.log"
    if ! run_cargo_test > "$log" 2>&1; then
        cat "$log" >&2
        fail "$label cargo test failed"
    fi
    if [[ -s "$nested_cargo_log" ]]; then
        cat "$nested_cargo_log" >&2
        fail "$label observed nested Cargo invocation"
    fi
}

run_consumer_test initial
[[ $(builder_count) == 1 ]] || fail "initial test should invoke the prebuilt builder once"

# Change only the copied Project function. The consumer's original expectation
# must now fail with the new semantic result, proving Cargo reran the builder.
python3 - "$project/src/core.spx" <<'PY'
from pathlib import Path
import sys

core = Path(sys.argv[1])
source = core.read_text(encoding="utf-8")
needle = "    left + right\n"
if source.count(needle) != 1:
    raise SystemExit("RI10 gate: expected exactly one calculator.add body")
core.write_text(source.replace(needle, "    left + right + 1\n", 1), encoding="utf-8")
PY

drift_log="$scratch/changed-source.log"
if run_cargo_test > "$drift_log" 2>&1; then
    cat "$drift_log" >&2
    fail "changed Project function did not change the consumer result"
fi
grep -Fq 'Ok(43)' "$drift_log" || {
    cat "$drift_log" >&2
    fail "changed Project function did not produce the expected new result"
}
grep -Fq 'Ok(42)' "$drift_log" || {
    cat "$drift_log" >&2
    fail "consumer drift control did not compare against its original result"
}
[[ $(builder_count) == 2 ]] || fail "changed Project source did not rerun the prebuilt builder"
[[ ! -s "$nested_cargo_log" ]] || fail "nested Cargo invocation was observed"

# Accept the observed drift in the copied consumer and require the fresh build
# to pass. The consumer source is not a Project input, so this must not rebuild
# or reinvoke the SDK builder a third time.
python3 - "$consumer/src/main.rs" <<'PY'
from pathlib import Path
import sys

consumer = Path(sys.argv[1])
source = consumer.read_text(encoding="utf-8")
if "Ok(42)" not in source:
    raise SystemExit("RI10 gate: consumer no longer contains its original result")
consumer.write_text(
    source.replace("Ok(42)", "Ok(43)").replace('println!("42")', 'println!("43")'),
    encoding="utf-8",
)
PY

run_consumer_test accepted-drift
[[ $(builder_count) == 2 ]] || fail "consumer-only expectation change reran the Project builder"
[[ ! -s "$nested_cargo_log" ]] || fail "nested Cargo invocation was observed"

printf 'RI10 gate passed: locked offline consumer rebuilt from changed Project source; builder calls=%s; nested Cargo calls=0\n' \
    "$(builder_count)"
finished=1
