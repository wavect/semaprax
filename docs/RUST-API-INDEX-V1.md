# Rust API Index v1

Status: proposed private RI-03 preparation boundary.

`crates/semaprax-rust-api-index/fixtures/protocol-envelope-example.json` is a
synthetic protocol example. Its package label and digests are illustrative; it
is not extracted from regex or evidence of an end-to-end compiler-resolved
index. The real pinned-package extractor fixture remains an RI-03 acceptance
gap.

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

Replay requires the exact canonical bytes, a terminal LF, no NUL, at most
1 MiB, at most 512 items, and type depth at most 32. It rejects duplicate or
out-of-order paths, unknown/missing fields, malformed digests, feature/target
or source identity drift, and truncation before an adapter can prepare a
foreign call. Digests use lowercase hexadecimal. Extractor-output admission
requires the `nightly-rustdoc-json` mode; a `prepared` index cannot be passed
off as extractor output. The document digest uses the length-framed domain
`semaprax.rust-api-index.digest.v1\0`.

An index provides discovery only. Rust remains authoritative for selected
signature and call acceptance on the stable target. Adapter trust and declared
SEMAPRAX effects remain separate input; docs, package metadata and index facts
cannot assert purity or grant execution authority.
