# Hosted graph bill after compact observations and borrowed status sets

Status: private implementation evidence; refreshed Chromium and full CI qualification pending.

Audience: compiler maintainers and reviewers.

[CI run 38846289813, job 116797898127](https://github.com/wavect/semaprax/actions/runs/38846289813/job/116797898127)
built `c451086ce24484261d072ec5e5c7207988b0d93f` using Rust 1.88 on Linux
x86-64. Complete logged graph and compiler provenance records were decoded
with strict base64, length and full-file SHA-256 checks. Both graph-domain
digests were independently recomputed.

Both complete graphs differ from the retained 2600e49eb captures only in
`budget.used_builder_bytes` (3,296,148 to 2,971,194) and the graph self-digest.
Restoring those fields reproduces every previous byte. The 324,954-byte
reduction is the observed combined bill after the intervening allocation
changes; it is not attributed to a single helper. Project and workspace
revisions are unchanged.

Live browser pins use the complete captured graph digests. The new bridge
requires the exact observed bill, reconstructs the entire preceding graph,
and invokes the complete previous bridge. That bridge remains compiled as
`before_c451.rs`; only its runner include points at the retained complete old
runner. Every prior bridge, test, graph, cursor control and source-custody
check remains compiled. The new source-custody test pins the complete old
bridge and runner and proves that one include substitution is the only change
to the retained module. Rehashed changes outside the two permitted fields
still fail the composed bridge.

All 13 focused Rust checks passed with a sampled aggregate compiler peak of
173,552 KiB under a 200 MiB stop guard. This executes the actual composed
bridge and original cursor helper; it does not execute the compiler or the
Chromium consumers. The refreshed browser and full current-head CI still
require hosted qualification.
