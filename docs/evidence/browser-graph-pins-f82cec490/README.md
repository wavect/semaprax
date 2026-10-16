# Hosted graph bill after final path receivers and nominal context

Status: private accounting evidence; refreshed Chromium and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 38888509010, job 116932619854](https://github.com/wavect/semaprax/actions/runs/38888509010/job/116932619854)
built `f82cec490db31898d9876645876ae98549611baf` with Rust 1.88 on Linux
x86-64. The exact-pin browser gate then refused the changed graph digest.
Complete graph and compiler-provenance records were decoded with strict base64,
byte-length and raw-file SHA-256 checks. Both graph-domain digests were
independently recomputed. The compiler image SHA-256 is retained in provenance.

Both entire graphs differ from the c11a63509 records only in
`budget.used_builder_bytes` (2,967,742 to 2,958,908) and their self-digests.
Restoring those two fields reproduces every prior byte. The observed net
8,834-byte reduction includes the intervening final path/prefix storage and
funded nominal metadata context changes. It is not attributed to one helper
and does not measure process RAM. Project and workspace revisions are unchanged.

Live browser pins use the complete captured digests. The bridge checks the exact
observed bill, reconstructs the complete prior graph and invokes the complete
previous bridge. That original bridge remains compiled as `before_f82.rs`;
only its runner include is redirected to the complete retained runner. Fixed
hashes bind both original files and that single substitution. All older
bridges, source controls and rehashed non-builder mutation controls remain.

These records establish fixture identity and accounting only. The refreshed
browser execution and every current-main CI gate still require hosted execution.
