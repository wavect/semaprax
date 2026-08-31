# Frame payload Rust consumer

This package depends only on the generated safe owned-data SDK plus Serde for
reading the committed corpus. Build the Project-v8 Rust target, then run the
consumer:

```sh
cargo run --locked -p semaprax-toolchain --bin semaprax-full -- build --manifest-path examples/frame-payload-project/semaprax.toml --target rust -o examples/frame-payload-generated-sdk
cp examples/frame-payload-project/corpus.json examples/frame-payload-rust/corpus.json
cargo run --locked --offline --manifest-path examples/frame-payload-rust/Cargo.toml
```

The generated SDK is local evidence and must not be committed. Its public
`spx_frame_dot_payload_hyphen_result` method is derived from the stable ID,
not the SEMAPRAX display name.

Rust package generation requires the unpublished full-toolchain host above;
the standalone `semaprax` binary deliberately rejects this target. The
integration fixture runs Cargo with the external consumer as its working
directory, so repository-local Cargo configuration is not inherited by
configuration discovery.

The standalone consumer lock pins the existing repository Serde dependency
versions and checksums, including derive feature edges. Its registry sources
must already be cached on the host; offline execution must fail if they are
absent. No dependency installation is part of the consumer acceptance gate.
Both baseline and display-renamed integration fixtures copy this lock before
execution and assert its bytes remain unchanged afterward.

The explicit `msrv_handoff::provisioned_frame_consumer_handoff_binds_both_revisions`
gate uses the current Linux AArch64 compiler to authenticate both real Project
revisions and then retains exactly two seven-file SDKs plus four unchanged
four-file consumers. A separate compiler-free Linux AArch64 environment ran all
four consumers with Rust/Cargo 1.85.1, `--locked --offline`, a read-only handoff
and registry, no checkout mount or network, and a fresh target. It observed four
exact success markers, empty stderr and unchanged bytes for all 30 input files.

This is local generated-package and consumer evidence for Rust 1.85.1. It is
not a Rust 1.85 build of the compiler, exact Rust 1.85.0, Windows, hosted,
published-SDK or minimum-supported-toolchain evidence.
