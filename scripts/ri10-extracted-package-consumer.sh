#!/usr/bin/env bash
set -euo pipefail

fail() {
    printf 'RI10 extracted-package gate: %s\n' "$*" >&2
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

scratch=$(mktemp -d "$temp_base/semaprax-ri10-extracted.XXXXXX")
scratch=$(CDPATH='' cd -- "$scratch" && pwd -P)
case "$scratch/" in
    "$repo_root/"*) fail "temporary package root must be outside the repository" ;;
esac

finished=0
cleanup() {
    local status=$?
    trap - EXIT
    if [[ $status -eq 0 && $finished -eq 1 ]]; then
        rm -r "$scratch"
    else
        printf 'RI10 extracted-package gate: retained temporary package at %s\n' "$scratch" >&2
    fi
    exit "$status"
}
trap cleanup EXIT

consumer="$scratch/source-consumer"
project="$scratch/calculator-project"
shim_dir="$scratch/shims"
mkdir -p "$consumer" "$project" "$shim_dir"
cp -R "$repo_root/examples/calculator-rust/build-script-consumer/." "$consumer/"
cp -R "$repo_root/examples/calculator-project/." "$project/"

builder_log="$scratch/builder-invocations.log"
nested_cargo_log="$scratch/nested-cargo-invocations.log"
: > "$builder_log"
: > "$nested_cargo_log"

cat > "$shim_dir/semaprax-builder" <<'SH'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$SEMAPRAX_RI10_BUILDER_LOG"
export CARGO="$SEMAPRAX_RI10_CARGO_GUARD"
exec "$SEMAPRAX_RI10_BUILDER_REAL" "$@"
SH

cat > "$shim_dir/cargo" <<'SH'
#!/bin/sh
printf '%s\n' "$*" >> "$SEMAPRAX_RI10_NESTED_CARGO_LOG"
printf 'nested Cargo invocation is forbidden by the RI10 extracted-package gate\n' >&2
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

assert_no_nested_cargo() {
    [[ ! -s "$nested_cargo_log" ]] || {
        cat "$nested_cargo_log" >&2
        fail "observed nested Cargo invocation"
    }
}

run_explicit() (
    cd "$consumer"
    local real_builder=$SEMAPRAX_RI10_BUILDER
    export PATH="$shim_dir:$PATH"
    export CARGO_TARGET_DIR="$scratch/source-target"
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

run_required() {
    local label=$1
    local log="$scratch/$label.log"
    if ! run_explicit > "$log" 2>&1; then
        cat "$log" >&2
        fail "$label cargo test failed"
    fi
    assert_no_nested_cargo
}

sdk_directory() {
    local count
    count=$(find "$scratch/source-target" -type d -path '*/out/semaprax_generated' | wc -l | tr -d '[:space:]')
    [[ "$count" == 1 ]] || fail "expected exactly one generated SDK directory, found $count"
    find "$scratch/source-target" -type d -path '*/out/semaprax_generated' -print -quit
}

capture_sdk() {
    local destination=$1
    local source
    case "$destination" in
        "$scratch/"*) ;;
        *) fail "generated SDK capture must remain inside the gate scratch directory" ;;
    esac
    source=$(sdk_directory)
    rm -rf "$destination"
    cp -R "$source" "$destination"
}

manifest_value() {
    local sdk=$1
    local field=$2
    python3 - "$sdk/semaprax.native-rust-sdk.json" "$field" <<'PY'
import json
import sys

with open(sys.argv[1], encoding="utf-8") as source:
    value = json.load(source)
for field in sys.argv[2].split("."):
    if not isinstance(value, dict):
        value = None
        break
    value = value.get(field)
if not isinstance(value, str) or not value:
    raise SystemExit(f"RI10 extracted-package gate: missing manifest string {sys.argv[2]}")
print(value)
PY
}

prepared_inputs() {
    local sdk=$1
    local extracted_consumer=$2
    local archive
    if [[ $(uname -s) == MINGW* || $(uname -s) == MSYS* || $(uname -s) == CYGWIN* ]]; then
        archive=semaprax_native_rust_sdk.lib
    else
        archive=libsemaprax_native_rust_sdk.a
    fi
    printf '%s;%s;%s;%s;%s;%s' \
        "$extracted_consumer/Cargo.lock" \
        "$sdk/src/lib.rs" \
        "$sdk/src/semaprax_native_rust_interop.rs" \
        "$sdk/src/semaprax_native_rust_interop_ffi.rs" \
        "$sdk/semaprax.native-rust-sdk.json" \
        "$sdk/native/$archive"
}

assert_shipped_text_is_relocatable() {
    local sdk=$1
    local extracted_consumer=$2
    local text_files=(
        "$extracted_consumer/Cargo.toml"
        "$extracted_consumer/Cargo.lock"
        "$extracted_consumer/build.rs"
        "$extracted_consumer/src/main.rs"
        "$sdk/src/lib.rs"
        "$sdk/src/semaprax_native_rust_interop.rs"
        "$sdk/src/semaprax_native_rust_interop_ffi.rs"
        "$sdk/semaprax.native-rust-sdk.json"
    )
    local file
    for file in "${text_files[@]}"; do
        [[ -f "$file" ]] || fail "shipped package text file is missing: $file"
        if grep -Fq -- "$repo_root" "$file"; then
            fail "shipped package text leaks the developer repository path: $file"
        fi
        if grep -Eq '"/(Users|home)/' "$file"; then
            fail "shipped package text leaks an absolute developer home path: $file"
        fi
        if [[ "$file" == "$extracted_consumer/Cargo.toml" ]]; then
            if grep -Eq '(^|[[:space:]])path[[:space:]]*=' "$file"; then
                fail "extracted consumer has a path dependency: $file"
            fi
            if grep -Eq '(^|[[:space:]])git[[:space:]]*=' "$file"; then
                fail "extracted consumer has a Git dependency: $file"
            fi
        fi
    done
}

run_prepared() (
    local label=$1
    local extracted_consumer=$2
    local sdk=$3
    local descriptor_digest=$4
    local target_dir=$5
    cd "$extracted_consumer"
    unset SEMAPRAX_RI10_BUILDER SEMAPRAX_RI10_PROJECT_MANIFEST
    export CARGO_TARGET_DIR="$target_dir"
    export SEMAPRAX_RI10_PREPARED_SDK="$sdk"
    export SEMAPRAX_RI10_INPUTS="$(prepared_inputs "$sdk" "$extracted_consumer")"
    export SEMAPRAX_RI10_SDK_VERSION="$(manifest_value "$sdk" crate.version)"
    export SEMAPRAX_RI10_DESCRIPTOR_DIGEST="$descriptor_digest"
    export SEMAPRAX_RI10_BUNDLE_DIGEST="$(manifest_value "$sdk" inner.bundle_digest)"
    "$SEMAPRAX_RI10_CARGO" test --locked --offline \
        --manifest-path "$extracted_consumer/Cargo.toml" -- --test-threads=1
)

expect_prepared_refusal() {
    local label=$1
    local extracted_consumer=$2
    local sdk=$3
    local descriptor_digest=$4
    local expected=$5
    local log="$scratch/$label.log"
    if run_prepared "$label" "$extracted_consumer" "$sdk" "$descriptor_digest" "$scratch/prepared-target" > "$log" 2>&1; then
        cat "$log" >&2
        fail "$label unexpectedly accepted"
    fi
    grep -Fq -- "$expected" "$log" || {
        cat "$log" >&2
        fail "$label did not report the expected refusal: $expected"
    }
}

run_required initial-source
[[ $(builder_count) == 1 ]] || fail "initial source build should invoke the builder once"
capture_sdk "$scratch/stale-sdk"

# A source change must rerun the explicitly authorized builder. Its original
# expectation fails against the new result, so an old generated package cannot
# silently satisfy the source-derived consumer.
python3 - "$project/src/core.spx" <<'PY'
from pathlib import Path
import sys

core = Path(sys.argv[1])
source = core.read_text(encoding="utf-8")
needle = "    left + right\n"
if source.count(needle) != 1:
    raise SystemExit("RI10 extracted-package gate: expected exactly one calculator.add body")
core.write_text(source.replace(needle, "    left + right + 1\n", 1), encoding="utf-8")
PY

drift_log="$scratch/source-drift.log"
if run_explicit > "$drift_log" 2>&1; then
    cat "$drift_log" >&2
    fail "changed Project source did not change the consumer result"
fi
grep -Fq 'Ok(43)' "$drift_log" || fail "changed Project source did not expose its fresh result"
grep -Fq 'Ok(42)' "$drift_log" || fail "source-drift control did not compare against the old result"
[[ $(builder_count) == 2 ]] || fail "changed Project source did not rerun the builder"
assert_no_nested_cargo

python3 - "$consumer/src/main.rs" <<'PY'
from pathlib import Path
import sys

consumer = Path(sys.argv[1])
source = consumer.read_text(encoding="utf-8")
if "Ok(42)" not in source:
    raise SystemExit("RI10 extracted-package gate: consumer no longer contains its original result")
consumer.write_text(
    source.replace("Ok(42)", "Ok(43)").replace('println!("42")', 'println!("43")'),
    encoding="utf-8",
)
PY
run_required accepted-source-drift
[[ $(builder_count) == 2 ]] || fail "consumer-only change reran the Project builder"

extracted="$scratch/extracted-package"
extracted_consumer="$extracted/consumer"
extracted_sdk="$extracted/semaprax-sdk"
mkdir -p "$extracted"
cp -R "$consumer" "$extracted_consumer"
capture_sdk "$extracted_sdk"
assert_shipped_text_is_relocatable "$extracted_sdk" "$extracted_consumer"

fresh_descriptor=$(manifest_value "$extracted_sdk" inner.descriptor_digest)
run_prepared relocated "$extracted_consumer" "$extracted_sdk" "$fresh_descriptor" "$scratch/prepared-target"
[[ $(builder_count) == 2 ]] || fail "prepared-only extracted consumer invoked the builder"
assert_no_nested_cargo

# The old output has a different descriptor identity after the source change.
# Pairing it with the fresh identity must refuse before it can link or execute.
expect_prepared_refusal stale-output "$extracted_consumer" "$scratch/stale-sdk" "$fresh_descriptor" \
    'RI10-E005 prepared SDK descriptor_digest does not match its explicit binding'
[[ $(builder_count) == 2 ]] || fail "stale prepared output invoked the builder"

wrong_target_manifest="$scratch/wrong-target-sdk.json"
cp "$extracted_sdk/semaprax.native-rust-sdk.json" "$wrong_target_manifest"
python3 - "$extracted_sdk/semaprax.native-rust-sdk.json" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as source:
    value = json.load(source)
value["crate"]["target"] = "not-the-generated-target"
with open(path, "w", encoding="utf-8") as output:
    json.dump(value, output, separators=(",", ":"))
    output.write("\n")
PY
expect_prepared_refusal wrong-target "$extracted_consumer" "$extracted_sdk" "$fresh_descriptor" \
    'RI10-E005 prepared SDK target does not match its explicit binding'
cp "$wrong_target_manifest" "$extracted_sdk/semaprax.native-rust-sdk.json"

expect_prepared_refusal wrong-api-digest "$extracted_consumer" "$extracted_sdk" \
    'sha256:0000000000000000000000000000000000000000000000000000000000000000' \
    'RI10-E005 prepared SDK descriptor_digest does not match its explicit binding'
[[ $(builder_count) == 2 ]] || fail "prepared negative controls invoked the builder"
assert_no_nested_cargo

printf 'RI10 extracted-package gate passed: locked offline prepared-only consumer relocated without path dependencies; stale output, target, and API digest refused; builder calls=%s; nested Cargo calls=0\n' \
    "$(builder_count)"
finished=1
