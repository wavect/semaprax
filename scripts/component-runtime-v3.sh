#!/bin/sh
set -eu

readonly_script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
readonly_root=$(CDPATH= cd -- "$readonly_script_dir/.." && pwd -P)
readonly_runner="$readonly_root/platform-tests/component-runtime"
readonly_manifest="$readonly_runner/Cargo.toml"
readonly_toolchain_lock="$readonly_runner/toolchain.lock"

test -f "$readonly_manifest"
test -f "$readonly_toolchain_lock"
cd "$readonly_runner"
readonly_rustc=$(rustc -vV)
readonly_cargo=$(cargo -Vv)
test "$(grep -c '^rustc.release=1.97.1$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^rustc.commit=8bab26f4f68e0e26f0bb7960be334d5b520ea452$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^rustc.commit-date=2026-07-14$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^rustc.llvm=22.1.6$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^cargo.release=1.97.1$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^cargo.commit=c980f4866141969fab6254a680546a277789d6f0$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^cargo.commit-date=2026-06-30$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^ci.host=x86_64-unknown-linux-gnu$' "$readonly_toolchain_lock")" -eq 1
test "$(grep -c '^wasmtime.version=47.0.3$' "$readonly_toolchain_lock")" -eq 1
printf '%s\n' "$readonly_rustc" | grep -Fx 'release: 1.97.1'
printf '%s\n' "$readonly_rustc" | grep -Fx 'commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452'
printf '%s\n' "$readonly_rustc" | grep -Fx 'commit-date: 2026-07-14'
printf '%s\n' "$readonly_rustc" | grep -Fx 'host: x86_64-unknown-linux-gnu'
printf '%s\n' "$readonly_rustc" | grep -Fx 'LLVM version: 22.1.6'
printf '%s\n' "$readonly_cargo" | grep -Fx 'release: 1.97.1'
printf '%s\n' "$readonly_cargo" | grep -Fx 'commit-hash: c980f4866141969fab6254a680546a277789d6f0'
printf '%s\n' "$readonly_cargo" | grep -Fx 'commit-date: 2026-06-30'
printf '%s\n' "$readonly_cargo" | grep -Fx 'host: x86_64-unknown-linux-gnu'

# These are the explicit root and isolated-runner dependency acquisition
# phases. Every subsequent Cargo command is independently locked and forced
# offline.
cargo fetch --locked --manifest-path "$readonly_root/Cargo.toml"
cargo fetch --locked --manifest-path "$readonly_manifest"
export CARGO_NET_OFFLINE=true

cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --features unstable-wit-component-harness --lib wit_component::result_v3::tests::
cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --features unstable-wit-component-harness --lib wit_component::source_result_v4::tests::
cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --features unstable-wit-component-harness --lib wasm::scalar_algebra_component_v5::tests::
cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --features unstable-wit-component-harness --lib wit_component::scalar_algebra_v5::tests::
cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --features unstable-wit-component-harness --lib wasm::nested_record_component_v6::tests::
cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --features unstable-wit-component-harness --lib wit_component::nested_record_v6::tests::
cargo test --locked --offline --manifest-path "$readonly_root/Cargo.toml" --test component_runtime_ci_contract
cargo fmt --manifest-path "$readonly_manifest" --all -- --check
cargo clippy --locked --offline --manifest-path "$readonly_manifest" --all-targets --all-features -- -D warnings
cargo test --locked --offline --manifest-path "$readonly_manifest" --all-targets --all-features
cargo run --locked --offline --manifest-path "$readonly_manifest"
