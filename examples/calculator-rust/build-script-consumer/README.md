# RI-10 Cargo consumer

This is an ordinary standalone Cargo consumer for a calculator SDK.
The default prepared-only `build.rs` mode does not invoke Cargo, rustc, the
SEMAPRAX compiler, or the network.
It accepts an explicit absolute prepared-SDK directory and an explicit semicolon-separated
list of absolute `.spx`, manifest, index, lock, and tool-identity files to track.
It copies the safe generated facade and private raw adapter into `OUT_DIR`, checks the
generated SDK target against Cargo's `TARGET`, and links the staged native archive.

Run the explicit setup phase first, outside the consumer workspace, to create the SDK.

```sh
SEMAPRAX_RI10_PREPARED_SDK=/absolute/generated-sdk \
SEMAPRAX_RI10_SDK_VERSION=0.1.0 \
SEMAPRAX_RI10_DESCRIPTOR_DIGEST=sha256:... \
SEMAPRAX_RI10_BUNDLE_DIGEST=sha256:... \
SEMAPRAX_RI10_INPUTS='/absolute/calculator.spx;/absolute/semaprax.toml;/absolute/Cargo.lock;/absolute/tool-identity.txt' \
cargo test --locked --offline
```

Changing any tracked input makes Cargo rerun the build script. It does not regenerate the
SDK, so a changed input requires a new explicit prebuild. This prepared-only M1 route refuses
missing, relative, unsafe-path, target-mismatched, version-mismatched, descriptor-mismatched,
or bundle-mismatched inputs before linking. Its stable failure prefixes are `RI10-E001` through
`RI10-E006`.

An explicitly authorized local build can instead set
`SEMAPRAX_RI10_BUILDER` to an absolute, already-built
`semaprax-native-rust-sdk` executable and `SEMAPRAX_RI10_PROJECT_MANIFEST` to
the absolute Project manifest. Include that manifest and every declared `.spx`
source in `SEMAPRAX_RI10_INPUTS`. The build script calls the executable's
`project` route directly into its own `OUT_DIR`, then stages its generated
facade and archive. It does not run nested Cargo. Source, manifest, tool, or
tracked lock/index changes rerun the script; the previous generated child is
removed before rebuilding. The target in the generated SDK must still equal
Cargo's `TARGET`. `RI10-E007` reports a missing configured builder, untracked
manifest, invalid generated child, or failed explicit build.

```sh
SEMAPRAX_RI10_BUILDER=/absolute/bin/semaprax-native-rust-sdk \
SEMAPRAX_RI10_PROJECT_MANIFEST=/absolute/project/semaprax.toml \
SEMAPRAX_RI10_INPUTS='/absolute/project/semaprax.toml;/absolute/project/src/app.spx;/absolute/project/Cargo.lock' \
RUSTC=/absolute/bin/rustc CLANG=/absolute/bin/clang \
SEMAPRAX_ARCHIVER=/absolute/bin/libtool \
cargo test --locked --offline
```

The `libtool` path is the macOS example; other supported hosts must use their
admitted archiver. The caller explicitly chooses the compiler, native tools,
Project sources, and build-script authority. The SDK still remains unpublished
and this example does not admit rich owned resources. The consumer has one real
Rust-to-SEMAPRAX `cargo test` case; it invokes the generated facade rather than
merely compiling it.
The crate denies handwritten unsafe code while the generated private FFI module
contains its own narrowly scoped unsafe allowance.

## Extracted-package gate

`scripts/ri10-extracted-package-consumer.sh` creates a fresh SDK through the
explicit route, changes the copied Semaprax source to establish that stale
output cannot remain usable, then copies only this consumer and the generated
SDK to a second directory outside the repository. It runs that copy with
`cargo test --locked --offline` in prepared-only mode, without
`SEMAPRAX_RI10_BUILDER` or a Project manifest.

The gate rejects stale output paired with the fresh descriptor identity, a
wrong generated target, and a mismatched API digest. It scans only the shipped
consumer and SDK text inventory for the developer repository path, absolute
home paths, Cargo path dependencies, and Git dependencies; configured tools
and generator-only inputs are intentionally outside that scan.

The calculator route crosses scalar ABI values only. It exports no direct Rust
package type, so this evidence makes an explicit opaque-boundary claim instead
of claiming that separately linked Rust crates share an opaque type identity.
Rich direct Rust type crossings remain closed until their exact
package/source identity has a dedicated consumer gate.
