# RI-13 M1: one selected Regex and Url Project

`project/src/app.spx` declares both indexed registry APIs and two checked
exports. `prepare` acquires one held Project subject covering all four imports
and both exports, then stages separate generated Regex and Url crates whose
binding plans carry the identical subject digest. The two carriers use distinct
C ABI symbols, so one Rust consumer can execute both source bodies. Their
package locks are pinned independently; the consumer has its own committed
offline lock for the combined dependency closure.

The local physical gate runs `prepare` with locked offline Cargo, then runs
`consumer` with an explicit absolute `CLANG` path. Its build script compiles
the two generated C files and links both objects under the committed offline
lock.

From the repository root:

```sh
CARGO_TARGET_DIR=target/ri13-m1 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 /opt/homebrew/bin/cargo run --locked --offline --manifest-path examples/ri13-m1-regex-url/prepare/Cargo.toml
CLANG=/usr/bin/clang CARGO_TARGET_DIR=target/ri13-m1 CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 /opt/homebrew/bin/cargo run --locked --offline --manifest-path examples/ri13-m1-regex-url/consumer/Cargo.toml
```

The recorded local run used Cargo/rustc 1.98.0 (Homebrew) and Apple clang
21.0.0 on macOS arm64. `prepare` printed subject
`sha256:f61293d7c7d825f0ce880656507688a31dc57c197c50a1848f91fefb1081f71a`;
`consumer` printed `ri13-m1-regex-url-ok`. Its committed consumer lock is the
combined dependency closure; generated package locks are still replayed
separately by the builder. This evidence is local and has no Linux rerun.
The consumer checks results, zero-copy Regex input counters, Url owner-tied
view, and cleanup. The two C compilations are build steps, not per-function
handwritten adapters.

This is a bounded local mixed-project route. It does not claim arbitrary
foreign package combinations, a merged owner carrier, or publication through
the general Project SDK CLI.

## Matched batch investigation

The local five-sample Darwin arm64 fixed-workload result at `93c9e2599` is
recorded in [measurements/](measurements/). It verifies each 4,096-operation
batch's exact borrow, copy, and cleanup ledger before computing medians. The
context-allocation fix leaves one extra carrier allocation per batch, but the
generated/direct throughput ratios are 0.8910 for Regex scan and 0.6378 for
Url parse/view. Both remain below the 0.90 investigation trigger, so the
record is adverse local evidence rather than a threshold result.
