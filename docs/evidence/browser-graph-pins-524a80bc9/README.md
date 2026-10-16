# Hosted graph bill after clone cursor savings

Status: captured hosted fixture evidence; refreshed browser and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 38972207637, job 117213668436](https://github.com/wavect/semaprax/actions/runs/38972207637/job/117213668436)
built `524a80bc9827fdac6de02bbe575f934d0e5cc58d` with Rust 1.88 on Linux
x86-64. Its exact fixture gate rejected stale graph digests. Complete baseline,
renamed and compiler provenance records pass strict base64, byte-length and raw
SHA-256 checks. Provenance records the source head and compiler binary hash.
Both graph-domain digests were independently recomputed.

The complete baseline and renamed graphs differ from the retained 75b314f70
records only in `budget.used_builder_bytes` (2,954,152 to 2,917,432) and their
self-digests. Restoring those fields reproduces every prior byte. This is an
observed net 36,720-byte cumulative builder reduction across intervening changes,
including clone cursor savings. It is not a process RAM measurement or attribution
to one helper. Project and workspace revisions remain exact.

Live fixture pins use the captured digests. The bridge requires the exact new
bill, restores the entire prior graph, and invokes the complete previous bridge.
`before_524.rs` retains that receiver verbatim except for its runner include,
which names the archived runner. Fixed hashes and an exact inverse bind both
original source files. Every earlier compiled receiver and rehashed non-builder
mutation control remains. These captures do not establish browser execution,
current-head full CI success, or the pending low-stack result.

The focused local static-std probe executed all 25 current and historical wire,
source, digest and tamper tests successfully. The source inventory (2 tests),
documentation (3), release contracts (13), JavaScript syntax, Rust formatting
and diff checks passed. The sampled compile peak was 191,408 KiB under the
unchanged 200 MiB owned-process cap. Full compiler and browser execution remain
hosted qualifications.
