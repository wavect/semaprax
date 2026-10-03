# RI-10 prepared Cargo consumer

This is an ordinary standalone Cargo consumer for a previously generated calculator SDK.
`build.rs` does not invoke Cargo, rustc, the SEMAPRAX compiler, or the network.
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
