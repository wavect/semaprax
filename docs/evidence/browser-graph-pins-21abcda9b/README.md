# Hosted graph accounting after borrowed exit comparison

Status: captured hosted fixture evidence; refreshed browser and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 39005559079, job 117317575631](https://github.com/wavect/semaprax/actions/runs/39005559079/job/117317575631)
built `21abcda9bc82864834914dc5b89c91bc5a9c94a5` with Rust 1.88 on Linux
x86-64. Its fixture gate rejected stale graph digests. Complete baseline,
renamed and compiler provenance records pass strict base64, byte-length and raw
SHA-256 checks. Provenance binds the source head and compiler binary hash.
Both graph-domain digests were independently recomputed.

Both complete graphs differ from the retained 7d34f1269 records only in
`budget.used_builder_bytes` (2,696,104 to 2,691,397) and their self-digests.
Restoring those fields reproduces every prior byte. The net decrease is 4,707
cumulative builder bytes; it is not process RAM or CPU performance evidence.
Project/workspace revisions, graph structure and every other budget field
remain exact. Complete service admission remains separately unproven.

The live pins use the captured digests. The bridge requires the exact new bill,
reconstructs the complete prior graph and invokes the entire previous bridge.
`before_21abc.rs` retains that receiver verbatim except for its runner include,
which selects the archived runner. Fixed source hashes and an exact inverse
bind both originals. Every prior compiled receiver and rehashed non-builder
mutation control remains. Hosted browser execution and all current-head CI
requirements still need to pass.
