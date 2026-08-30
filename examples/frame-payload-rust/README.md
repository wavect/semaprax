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

Status: lock and acceptance corrections are authored, unrun. No hosted,
minimum-toolchain, or published SDK claim follows from committing this lock.
