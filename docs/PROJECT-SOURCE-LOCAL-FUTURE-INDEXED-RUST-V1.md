# Project source-local Future indexed Rust v1

Status: private RI-13 admission profile; local execution evidence is pending.

`source-local-future-indexed-rust.v1` is a closed additive Package Manifest
profile for the one linked RI-13 application. It admits exactly:

- `regex = ["=1.13.1"]` and `url = ["=2.5.8"]`, without features;
- `web = ["regex.run", "url.run"]`; and
- one valid `rust_async` export, which retains the ordinary source-local Future
  signature.

The checked Project still has no Web, npm, or ordinary native emitter. Its
generated linked consumer is prepared only through the existing indexed Rust
package builders and source-local Future renderer. Those builders authenticate
the selected Rust API index, Cargo lock, source imports, and exact current
native target. A mismatched index target remains `SPX-B112`; this profile does
not authorize retargeting or cross compilation.

Any different dependency table, feature list, M1 export set, missing Future
selection, ordinary `source-local-future.v1` manifest with Rust dependencies,
or unselected indexed input remains refused. The profile adds no public ABI,
artifact publication, or execution claim.
