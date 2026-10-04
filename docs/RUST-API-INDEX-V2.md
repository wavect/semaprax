# Rust API Index v2

Audience: maintainers and contributors.

Status: proposed private RI-03 preparation boundary. This metadata format is
not a claim of generally supported Rust interop.

`crates/semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json`
is an actual local rustdoc JSON conversion produced with the explicitly
installed `nightly-2026-10-02` extractor. It records a selected stable compiler
identity (`rustc 1.98.0 (88d9e12ae 2026-08-18) (Homebrew)`) and a selected
`aarch64-apple-darwin` target plus fixture feature digest. The separate
`local-api-fixture-prepared-v2.json` is a stable prepared projection for replay
tests. `regex-1.13.1-index-envelope.json` is also a genuine v2 extraction from
the locked registry source, using the `regex_alias` Cargo name, default feature,
`aarch64-apple-darwin`, the pinned nightly rustdoc JSON format 61, and the
selected stable rustc identity. It contains five selected methods and six
reachable type records: `is_match` and `is_match_at` have complete public
closures; `captures`, `find`, and `new` are rejected with
`incomplete_type_closure` because their selected types reach unresolved public
API dependencies. This is local evidence for that exact package build, not
general crate or cross-target support.

The Rust crate accepts a bounded canonical `semaprax.rust-api-index.v2`
document. It never invokes Cargo, rustc, rustdoc, macro expansion, or foreign
code. A prepared index is replayed on stable and bound to package name/version,
source digest, Cargo alias, target, feature digest, and exact selected stable
compiler identity.

Items and reachable type records retain public paths, kind, receiver,
visibility, docs, normalized relative source spans, generic parameter kinds,
bounds, defaults and where predicates, associated-type bounds/defaults, and
reachable type references. The converter expands only the selected paths and
their bounded reachable type closure. It fails on malformed or over-limit
compiler output rather than emitting a truncated supported record. Private,
sealed, opaque, generic, unsupported-signature, and incomplete-closure cases
remain rejected with explicit reasons. External or otherwise unresolved types
make the closure incomplete.

The extractor mode is explicit. `prepared` is stable replay data.
`nightly-rustdoc-json` identifies an externally installed pinned extractor;
it is never fetched or invoked by this crate. A caller without an index or
extractor receives a setup requirement. A nightly index is discovery metadata
only: the selected stable compiler must accept the generated signatures and
calls before an adapter can execute them.

The local fixture covers a re-export, selected cfg item, macro-generated
inherent method, trait method and associated type, private sealed trait,
generic output, and opaque output. Its generated signatures have a stable
rustc positive and negative control in `stable_signature_check.rs` and
`stable_signature_mismatch.rs`. The fixture is local evidence, not upstream
package or cross-target support.

RI-07's additive demand layer accepts canonical concrete `usize` const arguments
and public, non-generic associated-type projections for an explicit concrete
implementor. It deduplicates identical requests before enforcing the fixed
64-entry expansion bound. A generated local `rustc` fixture executes a real
const-generic call and an associated-type projection; a `Clone` bound failure
remains an actual compiler refusal. The resolver carries no trait solver or
coherence authority: private/sealed items refuse from index facts, while trait
satisfaction remains a generated-wrapper compiler check.

Canonical replay requires sorted paths, exact fields and canonical JSON bytes,
a terminal LF, lowercase digests, no NUL, at most 1 MiB, at most 512 API items
and 512 type records, at most 32 type depth, at most 256 references per item,
and bounded per-item and total documentation/generic metadata. Type closure
cycles are handled by path identity; missing references, stale closure
summaries, truncation, and identity drift fail closed. The converter's focused boundary suite exercises cycle handling, exact and
over-limit transitive depth, per-item demand expansion, and selected-type union
size with controlled type graphs, and private method rejection. Run it with:

```sh
python3 crates/semaprax-rust-api-index/tools/test_rustdoc_json_to_index.py
```

The Rust replay suite independently tests canonical cycle replay and exact/over
index closure bounds.

To generate local rustdoc JSON without Cargo, use the explicitly installed
pinned rustdoc:

```sh
rustup run nightly-2026-10-02 rustdoc --edition=2021 \
  --crate-name local_api_fixture \
  crates/semaprax-rust-api-index/fixtures/local_api_fixture.rs \
  --cfg 'feature="fixture-selected"' -Z unstable-options \
  --output-format json -o target/ri03-rustdoc/local
```

Then run the stdlib-only converter with the recorded identities:

```sh
python3 crates/semaprax-rust-api-index/tools/rustdoc_json_to_index.py \
  --rustdoc-json target/ri03-rustdoc/local/local_api_fixture.json \
  --package-name local_api_fixture --package-version 0.0.0 \
  --source-sha256 sha256:dbc31a9272b4e500ca6363d633d8c7b5dac727ce7279cc65391cff5f377010dd \
  --target aarch64-apple-darwin \
  --feature-digest sha256:d3e066ea11e87bd8665f8fdca51534758ab735c2346340a1f64895fdd81c5a69 \
  --stable-rustc-version 'rustc 1.98.0 (88d9e12ae 2026-08-18) (Homebrew)' \
  --source-root crates/semaprax-rust-api-index/fixtures \
  --rustdoc-version 'rustdoc 1.101.0-nightly (c36f14571 2026-10-01)' \
  --rustdoc-format-version 61 \
  --output target/ri03-rustdoc/local/index-envelope.json
```

The converter does not acquire packages or tools. Cargo package extraction
requires an explicit prior source acquisition; no build-time network access is
allowed.

An index provides discovery only. Rust remains authoritative for the selected
signature and call on the stable target. The RI-04 indexed scalar integration
selector `implementation::tests::indexed_scalar::indexed_scalar_adapter_executes_and_rejects_flipped_rust_result`
passed on 2026-10-03 (1 passed, 0 failed, 0 ignored, 146 filtered). It starts
from a v2 extractor envelope, calls the public indexed builder, then compiles
and runs the generated adapter with stable rustc. The test checks exact
equality between actual `rustc --version` and indexed compiler identity, a
successful typed provider, a wrong return type rejected by rustc with no
executable produced, and a same-signature flipped provider rejected before
publication. Stale compiler, target, and Cargo alias identities also fail
closed without output.

The builder returns adapter source; this replay crate never launches a
compiler. Physical compile/run validation is an owning-builder integration
gate, rather than an operation performed by replay itself. The current scalar
adapter admits only its narrow receiver-free scalar ABI; regex methods such as
`&self` and `&str` remain outside that ABI. The v2 regex envelope is
compiler-resolved index evidence only, not adapter support.
