# RI-08 selected trait fixture

`index-envelope.json` is an actual compact extraction of `callback_fixture.rs`,
not a handwritten trait signature. Capture used pinned nightly
`nightly-2026-10-02-aarch64-apple-darwin`, rustdoc JSON format from that tool,
and the repository's `rustdoc_json_to_index.py`. The envelope retains the exact
source SHA-256, extractor hash, nightly version and selected stable Rust 1.98.0
version. Empty features use SHA-256 of `[]`.

Capture command shape (absolute paths and version strings supplied explicitly):

```sh
rustdoc --edition=2021 --crate-name callback_fixture -Z unstable-options \
  --output-format json callback_fixture.rs -o CAPTURE_DIR
python3 crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py \
  --rustdoc-json CAPTURE_DIR/callback_fixture.json \
  --package-name callback_fixture --package-version 0.0.1 \
  --source-sha256 EXACT_SOURCE_DIGEST --target aarch64-apple-darwin \
  --feature-digest EXACT_EMPTY_FEATURE_DIGEST \
  --stable-rustc-version EXACT_STABLE_VERSION --source-root CAPTURE_DIR \
  --rustdoc-version EXACT_NIGHTLY_VERSION \
  --rustdoc-format-version EXACT_JSON_FORMAT \
  --select callback_fixture::Accumulator::advance \
  --select callback_fixture::Accumulator::Error --output index-envelope.json
```

The owning `indexed_trait_callback_` tests replay this envelope, verify the
committed source hash and compile the unchanged source as a real separate Rust
crate. Deliberately false/partial index controls must still fail the safe Rust
impl compiler proof. Tests never invoke rustdoc, fetch a toolchain, or use the
network. The index is metadata evidence, not a compiler or trust certificate.
