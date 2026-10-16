# Hosted paid-cursor graph bills

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

CI run 38778002471, job 116571426249, built
`268cb05237d3cc0806e67f66696989cf9469bcdf` with Rust 1.88 on Linux x86-64.
Its complete graph/provenance log record was decoded with strict base64,
length and full-file SHA-256 checks. Both graph-domain digests were independently
recomputed before updating any pin.

Each complete graph differs from the prior hosted 841efe3e2 graph only in
`budget.used_builder_bytes` (4,852,388 to 4,393,980) and its self-digest. Restoring
those two fields reproduces every prior byte. Restoring the independently
retained 1,214,409-byte historical bill and its self-digest likewise reproduces
the entire historical fixtures under `browser-graph-pins-841efe3e2`.

The live pins include the full new bill. The test-only historical bridge now
requires that exact bill and uses these new captures; its full historical
fixtures, complete original cursor helper, original known-answer hashes,
source-custody test and hostile rehashed-field control remain unchanged.
This proves the hosted graph bill reduction, not full library/application
admission or a green Chromium run after the pin update; those require new CI.
