# Hosted graph bill after HIR and replay storage changes

Status: private implementation evidence; refreshed Chromium and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 38874738766, job 116887554233](https://github.com/wavect/semaprax/actions/runs/38874738766/job/116887554233)
built `c11a6350960d4b186d9fa3b88faccf2526f6686f` with Rust 1.88 on Linux
x86-64. Its exact-pin fixture gate failed after compiler construction succeeded.
The complete graph and compiler-provenance records were decoded with strict
base64, byte-length and full-file SHA-256 checks. Both graph-domain digests
were independently recomputed.

Both complete graphs differ from the retained c451086ce records only in
`budget.used_builder_bytes` (2,971,194 to 2,967,742) and their self-digests.
Restoring those two fields reproduces every previous byte. The observed
3,452-byte decrease is the combined bill after the intervening storage changes;
it is not attributed to one helper and does not measure process RAM. Project
and workspace revisions remain identical.

Live browser pins use these complete captured graph digests. The new bridge
requires the exact observed bill, reconstructs the entire preceding graph,
and invokes the complete previous bridge. That bridge remains compiled as
`before_c11.rs`; only its runner include points at the complete retained runner.
Every older bridge, test, graph, cursor control and source-custody check remains
compiled. Fixed hashes bind both complete old files and the single permitted
include substitution. Rehashed changes outside the two permitted fields fail
the composed bridge.

These records establish fixture identity and accounting only. Refreshed browser
execution and all current-main CI gates still require hosted qualification.
