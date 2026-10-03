# Rust API Index v1

Status: proposed private RI-03 preparation boundary.

`crates/semaprax-rust-api-index/fixtures/protocol-envelope-example.json` is a
synthetic protocol example. The separate
`crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json` is
an actual compiler-resolved extraction from the locked `regex` 1.13.1 package.
It is local evidence for this pinned target and feature set, not hosted or
cross-target support.

`semaprax-rust-api-index` admits a bounded, canonical
`semaprax.rust-api-index.v1` document. It is a metadata boundary only: it
does not run Cargo, Rust macros, rustdoc, rustc, or foreign code.

The document binds package name/version/source SHA-256, the chosen target and
feature digest, and the exact extractor executable SHA-256, Rust version and
rustdoc format label. Items are sorted by public path and carry kind, receiver,
signature, type depth, and either `supported` or a precise rejection reason.
The v1 reasons are private, sealed trait, opaque return, unsupported generic,
unsupported signature, and expansion limit. Missing facts do not become
supported items.

Two modes are explicit. `prepared` is for stable consumers and only replays an
already-prepared index. `nightly-rustdoc-json` identifies an externally
installed pinned extractor; it is never fetched, invoked, or inferred by this
crate. A caller without a prepared index or installed extractor receives a
setup requirement. The extractor is responsible for compiler-resolved cfg,
re-exports, macro expansion, visibility, associated items, and reachable type
closure. Its raw rustdoc/private-compiler schema is outside the stable replay
surface.

This first replay envelope is intentionally narrower than the full RI-03
index contract: it does not yet encode item spans, docs, explicit visibility,
separate type/lifetime/const parameter lists, or bound and associated-type
graphs. Those require an extractor that supplies compiler-resolved facts and
are acceptance gaps, not facts inferred from signatures or prose.

The dependency-free fixture sources are
`crates/semaprax-rust-api-index/fixtures/local_api_fixture.rs` and
`stable_signature_check.rs`. The real package fixture was produced from the
registry archive whose SHA-256 is
`f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d`, with
the exact `aarch64-apple-darwin` target and default Cargo feature enabled. The
feature digest is SHA-256 of the UTF-8 bytes `cargo-features-v1\ndefault\n`.
The source was acquired explicitly with `cargo fetch --locked`; rustdoc itself
was then run offline with the project-pinned `nightly-2026-10-02` toolchain
(`rustdoc 1.101.0-nightly (c36f14571 2026-10-01)`, JSON format 61):

```sh
CARGO_REGISTRY_SRC="${CARGO_HOME:-$HOME/.cargo}/registry/src/index.crates.io-1949cf8c6b5b557f"
rustup run nightly-2026-10-02 cargo fetch \
  --manifest-path "$CARGO_REGISTRY_SRC/regex-1.13.1/Cargo.toml" --locked
CARGO_TARGET_DIR="$PWD/target/ri03-rustdoc" \
  rustup run nightly-2026-10-02 cargo rustdoc \
  --manifest-path "$CARGO_REGISTRY_SRC/regex-1.13.1/Cargo.toml" \
  --locked --offline --lib --target aarch64-apple-darwin -- \
  -Z unstable-options --output-format json --document-private-items
```

The resulting `target/ri03-rustdoc/aarch64-apple-darwin/doc/regex.json` was
passed to the repository's stdlib-only converter. The converter does not
invoke tools or acquire dependencies; its SHA-256 is embedded in the envelope.
The full committed envelope can be reproduced with:

```sh
python3 crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py \
  --rustdoc-json target/ri03-rustdoc/aarch64-apple-darwin/doc/regex.json \
  --package-name regex --package-version 1.13.1 \
  --source-sha256 sha256:f020237b6c8eed93db2e2cb53c00c60a8e1bc73da7d073199a1180401450218d \
  --target aarch64-apple-darwin \
  --feature-digest sha256:dcacb5b38acb8b53818ae1c0cb2020947aefbea8ac5a9380e49aa0e0ec4db1aa \
  --rustdoc-version 'rustdoc 1.101.0-nightly (c36f14571 2026-10-01)' \
  --rustdoc-format-version 61 \
  --output /tmp/regex-selected-index.json
```

The broader fixture envelope contains 167 records, including rejected generic
and unsupported signatures; its total size is below the 1 MiB replay bound.
The stable test replays that exact envelope, checks its package/target/features,
selects four supported paths, and type-checks their Rust function-pointer
signatures against the locked `regex` dependency.

For the dependency-free local fixture, rustdoc JSON can also be produced with:

```sh
rustdoc --edition=2021 --crate-name local_api_fixture \
  crates/semaprax-rust-api-index/fixtures/local_api_fixture.rs \
  --cfg 'feature="fixture-selected"' -Z unstable-options \
  --output-format json -o target/ri03-rustdoc/local
```

The selected signatures can be independently checked with the selected
stable compiler:

```sh
rustc --edition=2021 --crate-type lib --crate-name stable_signature_check \
  crates/semaprax-rust-api-index/fixtures/stable_signature_check.rs \
  --cfg 'feature="fixture-selected"' --emit=metadata \
  -o /tmp/semaprax-ri03-stable-signatures.rmeta
```

The local fixture also has a deliberate negative signature source; the stable
test requires rustc to accept the positive source and reject the mismatch. It
checks a re-export, macro-generated method, trait method with associated
output, and enabled cfg item, while the fixture contains a disabled cfg item,
sealed trait, opaque output, and generic case for extractor classification.

Replay requires the exact canonical bytes, a terminal LF, no NUL, at most
1 MiB, at most 512 items, and type depth at most 32. It rejects duplicate or
out-of-order paths, unknown/missing fields, malformed digests, feature/target
or source identity drift, and truncation before an adapter can prepare a
foreign call. Digests use lowercase hexadecimal. Extractor-output admission
requires the `nightly-rustdoc-json` mode; a `prepared` index cannot be passed
off as extractor output. The document digest uses the length-framed domain
`semaprax.rust-api-index.digest.v1\0`.

An index provides discovery only. Rust remains authoritative for selected
signature and call acceptance on the stable target. The current regex fixture
is not a SEMAPRAX adapter acceptance test: its methods use reference and
`Result` signatures outside the current scalar adapter ABI. Adapter trust and
declared SEMAPRAX effects remain separate input; docs, package metadata and
index facts cannot assert purity or grant execution authority. Full RI-03
acceptance still requires richer structured metadata (visibility, spans, docs,
generic/bound and associated-type graphs), systematic reachable-type closure,
and compiler validation integrated into actual binding construction.
