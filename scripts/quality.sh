#!/usr/bin/env sh
set -eu

cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo test --locked --doc
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
cargo build --locked --release
cargo package --locked --allow-dirty
cargo run --locked -- check examples/meaning.spx
cargo run --locked -- check examples/ownership.spx
cargo run --locked -- check examples/control_flow.spx
cargo run --locked -- check examples/records.spx
cargo run --locked -- fmt examples/meaning.spx --check
cargo run --locked -- fmt examples/effects.spx --check
cargo run --locked -- fmt examples/ownership.spx --check
cargo run --locked -- fmt examples/control_flow.spx --check
cargo run --locked -- fmt examples/records.spx --check
