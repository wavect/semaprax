# Hosted graph bill after single-successor vector reuse

Status: private accounting evidence; refreshed Chromium and current-main CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 38902117832, job 116977210355](https://github.com/wavect/semaprax/actions/runs/38902117832/job/116977210355)
built `75b314f7001f42c48ec5569f1182171b47ad526d` with Rust 1.88 on Linux
x86-64. Its exact fixture gate refused the changed graph digest. Three complete
captured records pass strict base64, byte-length and raw SHA-256 validation;
compiler provenance binds that exact source head and binary image hash. Both
graph-domain digests were independently recomputed.

The complete baseline and renamed graphs differ from the retained f82cec490
records only in `budget.used_builder_bytes` (2,958,908 to 2,954,152) and their
self-digests. Restoring those fields reproduces every prior byte. This is an
observed net 4,756-byte cumulative builder reduction across intervening changes,
including vector reuse. It is not a process RAM measurement or attribution to
one helper. Project and workspace revisions remain byte-exact.

Live fixture pins use those complete captured digests. The bridge checks the
exact observed bill, restores the entire prior graph, then calls the complete
old bridge. `before_75b.rs` retains that old receiver with only its runner
include redirected to the archived runner. Fixed source hashes and a single
literal inverse bind both original files. Every older compiled receiver and
rehashed non-builder mutation control remains.

These are prior-head fixture and accounting records. The later leaf-clone change
and test compilation fix require current-main execution. The refreshed browser
job and every full quality gate remain required.
