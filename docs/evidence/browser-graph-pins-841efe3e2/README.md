# Hosted browser graph pins

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

The complete current graphs came from CI run 38770620786, job 116546107383,
compiled at `841efe3e27ab510049891440d3033a0d82f96bb1` with Rust 1.88 on
Linux x86-64. The log's `SEMAPRAX_PROJECT_GRAPH_EVIDENCE_V1` record carries
base64 bytes, byte lengths and SHA-256 for both graphs and compiler provenance.
All three were checked before writing these files. The browser job reached its
exact graph-subject assertion and reported the captured baseline digest.

`current-hosted-*` and `prior-hosted-*` preserve the full recorded bytes and
compiler provenance. The older capture is from `7f9080f85316bf0e2e8f530cf4c5adbcda247c16`.
Every non-digest field is identical between the respective graphs except
`budget.used_builder_bytes`, which changes from 4,852,372 to 4,852,388.
`receipt.json` records the independent graph-domain digests and full-file hashes.

The `historical-*` files are exact reconstructions, not newly emitted historical
records: restoring the already-pinned 1,214,409-byte bill in each prior hosted
wire reproduces its complete previous known-answer graph digest. Restoring only
that bill and its self-digest in the current wire yields every historical byte.
The unchanged original cursor helper then reproduces both still-pinned original
cursor-control digests after its existing 3 × 257 × 24-byte subtraction.

The live JavaScript pins include the complete current graph and current bill.
The Rust bridge is test-only and requires the exact current bill, validates the
current graph digest, and compares the complete reconstructed historical wire
before invoking the complete unchanged original helper. Its hostile control
changes another budget field and recomputes a valid graph digest; the bridge
still refuses it. The full prior Rust contract source is retained and SHA-bound,
with only the declared module registration and call adapter inverted.

No production graph is normalized, no budget field is omitted, and no source,
revision, declaration, edge, or nonclaim is removed from either comparison.
Local checks cover the captured records and pure controls; the next hosted run
must qualify live graph creation, all generated packages, and Chromium behavior.
