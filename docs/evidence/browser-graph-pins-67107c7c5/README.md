# Hosted graph bill after paid replay setup

Status: captured hosted fixture evidence; refreshed browser and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 38990109757, job 117270798564](https://github.com/wavect/semaprax/actions/runs/38990109757/job/117270798564)
built `67107c7c5d98b6e4e991aa3da1e18af91ae73621` with Rust 1.88 on Linux
x86-64. Its exact fixture gate rejected stale graph digests. Complete baseline,
renamed and compiler provenance records pass strict base64, byte-length and raw
SHA-256 checks. Provenance records the source head and compiler binary hash.
Both graph-domain digests were independently recomputed.

Both complete graphs differ from the retained 524a80bc9 records only in
`budget.used_builder_bytes` (2,917,432 to 2,927,728) and their self-digests.
Restoring those fields reproduces every prior byte. This is an observed net
10,296-byte cumulative builder increase across the intervening accounting and
storage changes, including paid replay setup. It is not a process RAM
measurement or attribution to one helper. Project and workspace revisions,
all graph structure and every other budget field remain exact.

Live fixture pins now use those complete captured digests. The bridge requires
the exact new bill, reconstructs the complete prior graph and invokes the entire
previous bridge. `before_671.rs` retains that receiver verbatim except for its
runner include, which selects the archived runner. Fixed hashes and an exact
inverse bind both original sources. Every prior compiled receiver and rehashed
non-builder mutation control remains. These captures do not qualify browser
execution or current-head full CI.

The focused static-standard-library probe executed all 28 current and historical
wire, source, digest and tamper controls successfully. Source inventory (2),
documentation (3), release contracts (13), JavaScript syntax, Rust formatting
and diff checks passed. The sampled compile peak was 172,752 KiB under the
unchanged 200 MiB process cap. Full compiler and browser execution still require
hosted qualification.
