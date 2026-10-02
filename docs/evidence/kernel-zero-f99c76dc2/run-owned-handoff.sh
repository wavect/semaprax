#!/bin/sh
set -eu
repo=/Users/kevin/Documents/ChatGPT/AI-Lang-v070
target=$repo/target/issue337-29065a007
evidence=/Users/kevin/Documents/ChatGPT/semaprax-evidence/issue328/f99c76dc2
container run --rm --memory 8G --cpus 4 --network none \
  --mount type=bind,source=$repo,target=$repo,readonly \
  --mount type=bind,source=$repo/target,target=$repo/target \
  --mount type=bind,source=/Users/kevin/Documents/ChatGPT/AI-Lang/.git,target=/Users/kevin/Documents/ChatGPT/AI-Lang/.git,readonly \
  --mount type=bind,source=$evidence,target=/evidence \
  --workdir $repo \
  --env CARGO_HOME=$target/cargo-home \
  --env CARGO_TARGET_DIR=$target \
  --env RUSTUP_HOME=$target/rustup-home \
  --env RUSTUP_TOOLCHAIN=1.97.1 \
  --env HOME=/tmp \
  --env GIT_CONFIG_COUNT=1 \
  --env GIT_CONFIG_KEY_0=safe.directory \
  --env GIT_CONFIG_VALUE_0=$repo \
  --env CARGO_NET_OFFLINE=true \
  --env CARGO_BUILD_JOBS=1 \
  --env CARGO_INCREMENTAL=0 \
  --env CARGO_PROFILE_DEV_DEBUG=0 \
  --env CARGO_PROFILE_TEST_DEBUG=0 \
  --env RUST_TEST_THREADS=2 \
  --env SEMAPRAX_REQUIRE_KERNEL_ZERO_RUNG_TWO_TARGETS=1 \
  --env SEMAPRAX_REQUIRE_KERNEL_ZERO_CROSS_BACKEND=1 \
  --env RUST_MIN_STACK=16777216 \
  --env CC=/usr/bin/clang \
  --env CXX=/usr/bin/clang++ \
  --env AR=/usr/bin/ar \
  --env LD=/usr/bin/ld \
  --env SEMAPRAX_TEST_NATIVE_STAGE_CLANG=/usr/bin/clang \
  semaprax-issue327-quality:curl-v2 \
  bash -c 'test "$(git rev-parse HEAD)" = f99c76dc2d26dd57c81f4fdd5f26fe91d50118e4 && test -z "$(git status --porcelain)" && { rustc --version; cargo --version; clang --version | head -n 1; node --version; git rev-parse HEAD; } > /evidence/tool-versions.log && cargo test --locked --offline -p semaprax --lib owned_handoff -- --test-threads=2 > /evidence/owned-handoff.log 2>&1'
