# Hosted graph accounting after replay route fixes

Status: captured hosted fixture evidence; refreshed browser and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 39000267955, job 117301876733](https://github.com/wavect/semaprax/actions/runs/39000267955/job/117301876733)
built `7d34f126956c7e6894c85a1cfbd81157159b475c` with Rust 1.88 on Linux
x86-64. Its exact fixture gate rejected stale graph digests. Complete baseline,
renamed and compiler provenance records pass strict base64, byte-length and raw
SHA-256 checks. Provenance records the source head and compiler binary hash.
Both graph-domain digests were independently recomputed.

Both complete graphs differ from the retained 67107c7c5 records only in
`budget.used_builder_bytes` (2,927,728 to 2,696,104) and their self-digests.
Restoring those fields reproduces every prior byte. The net decrease is
231,624 cumulative builder bytes across the intervening replay changes. It is
not a process RAM measurement or attribution to one helper. Project and
workspace revisions, graph structure and every other budget field remain exact.

Live fixture pins now use these complete captured digests. The bridge requires
the exact new bill, reconstructs the complete prior graph and invokes the entire
previous bridge. `before_7d34.rs` retains that receiver verbatim except for its
runner include, which selects the archived runner. Fixed hashes and an exact
inverse bind both original sources. Every prior compiled receiver and rehashed
non-builder mutation control remains. These captures do not qualify browser
execution, other application budgets or current-head full CI.
