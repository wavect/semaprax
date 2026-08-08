#!/usr/bin/env sh
set -eu

cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --workspace --all-features --no-deps
cargo build --locked --workspace --release
cargo package --locked --allow-dirty -p semaprax
cargo run --locked -p semaprax -- check examples/meaning.spx
cargo run --locked -p semaprax -- check examples/ownership.spx
cargo run --locked -p semaprax -- check examples/lifecycle.spx
cargo run --locked -p semaprax -- check examples/control_flow.spx
cargo run --locked -p semaprax -- check examples/records.spx
cargo run --locked -p semaprax -- fmt examples/meaning.spx --check
cargo run --locked -p semaprax -- fmt examples/effects.spx --check
cargo run --locked -p semaprax -- fmt examples/ownership.spx --check
cargo run --locked -p semaprax -- fmt examples/lifecycle.spx --check
cargo run --locked -p semaprax -- fmt examples/control_flow.spx --check
cargo run --locked -p semaprax -- fmt examples/records.spx --check
