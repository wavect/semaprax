# Rust calculator consumer

This standalone example has two phases. The setup package calls the public
SEMAPRAX Native Rust SDK builder for `../calculator.spx`. The consumer package
then depends only on the generated `semaprax-generated-native-rust-sdk` package at
`../generated-sdk`; it has no source or workspace dependency on the SEMAPRAX
compiler.

The hosted public-SDK gate copies this directory to a fresh temporary
directory, builds `generated-sdk` there, and runs the consumer with Cargo in
locked offline mode. The repository intentionally does not contain generated
SDK artifacts.

The same gate builds `callback-sdk` from `callback.spx` and runs the separate
callback consumer. Its host implements only the stable-ID-derived callback
method and returns the generated closed import-result type.

The additive `project` setup mode accepts an authenticated Project Manifest v1
path and emits `generated-project-sdk`. The separate `project-consumer` uses
all six stable-ID exports selected by
`../calculator-project/semaprax.toml`. The Project integration gate builds and
runs this consumer before and after a daemon-applied display rename, with the
daemon shut down before the second build. The two packages intentionally bind
different Project and source revisions; the evidence compares their public
stable-ID behavior, not whole-package bytes.

On macOS, one local setup invocation is:

```sh
RUSTC=/opt/homebrew/bin/rustc \
CLANG=/usr/bin/clang \
SEMAPRAX_ARCHIVER=/usr/bin/libtool \
cargo run --locked --offline --manifest-path examples/calculator-rust/Cargo.toml -- \
  project "$(pwd)/examples/calculator-project/semaprax.toml" \
  "$(pwd)/examples/calculator-rust/generated-project-sdk"
```

Darwin deliberately admits `/usr/bin/libtool -static -D`; `ar` and `llvm-ar`
are not substitutes for that frozen archive plan. The generated directory is
local evidence and must not be committed.
