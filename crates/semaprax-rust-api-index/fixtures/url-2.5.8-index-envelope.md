# Url 2.5.8 selected index capture

This envelope was extracted from the real registry package with the explicitly installed `nightly-2026-10-02` rustdoc (format 61), offline, for `aarch64-apple-darwin`. Only `Url::parse` and `Url::as_str` are selected. The extractor bytes and versions are recorded in the envelope. Stable execution uses Rust 1.98.0.

The archive SHA-256 is `ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed`. The captured Cargo closure is `crates/semaprax-toolchain/src/fixtures/ri06-url-2.5.8.Cargo.lock`.

Resolved package features are `default` and `std`; the feature digest is SHA-256 of the exact UTF-8 bytes `["default","std"]\n`, `af269ab39e76ec749dfa30da5ce5878b153b3b47b25fc5bd1a61084a929b17a1`.

`parse` returns `Result<Self, url::ParseError>` and remains rejected by general discovery as `unsupported_signature`. The explicit closed Url owner bridge admits that exact signature after checking resource identity and the two complete type records. `as_str` is supported metadata; execution separately authenticates its receiver-bound view relation. Neither index record authorizes tool execution or package publication.

Capture commands (run under a caller-owned temporary package named `ri06-url-owner` with `url_alias={package="url",version="=2.5.8"}`):

```sh
cargo +nightly-2026-10-02 rustdoc --offline -p url -- -Z unstable-options --output-format json
python3 crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py --rustdoc-json "$CAPTURE/target/doc/url.json" --package-name url --package-version 2.5.8 --source-sha256 sha256:ff67a8a4397373c3ef660812acab3268222035010ab8680ec4215f38ba3d0eed --renamed-from url_alias --target aarch64-apple-darwin --feature-digest sha256:af269ab39e76ec749dfa30da5ce5878b153b3b47b25fc5bd1a61084a929b17a1 --stable-rustc-version "$(rustc --version)" --source-root "$URL_SOURCE" --rustdoc-version "$(rustdoc +nightly-2026-10-02 --version)" --rustdoc-format-version 61 --select url::Url::parse --select url::Url::as_str --output "$OUTPUT"
```
