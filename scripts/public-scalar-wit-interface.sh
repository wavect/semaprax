#!/bin/sh
set -eu

readonly_script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd -P)
readonly_root=$(CDPATH= cd -- "$readonly_script_dir/.." && pwd -P)
readonly_consumer="$readonly_root/platform-tests/public-scalar-wit-interface"
readonly_manifest="$readonly_consumer/Cargo.toml"

test -f "$readonly_manifest"

# Dependency acquisition is explicit. Every compilation and execution below
# is independently locked and offline.
cargo fetch --locked --manifest-path "$readonly_root/Cargo.toml"
cargo fetch --locked --manifest-path "$readonly_manifest"
export CARGO_NET_OFFLINE=true

cargo fmt --manifest-path "$readonly_manifest" --package semaprax-public-scalar-wit-interface-consumer -- --check
cargo clippy --locked --offline --manifest-path "$readonly_manifest" --all-targets --all-features -- -D warnings
cargo test --locked --offline --manifest-path "$readonly_manifest" --all-targets --all-features
cargo run --locked --offline --manifest-path "$readonly_manifest"
