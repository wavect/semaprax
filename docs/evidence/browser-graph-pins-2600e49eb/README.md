# Hosted graph bill after replay allocation changes

[CI run 38824367721, job 116728654352](https://github.com/wavect/semaprax/actions/runs/38824367721/job/116728654352)
built `2600e49eb6d5853197fa53c972179f71df002bc0` using Rust 1.88 on Linux
x86-64. Complete logged graph and compiler provenance records were decoded
with strict base64, length and full-file SHA-256 checks. Both graph-domain
digests were independently recomputed.

Both complete graphs differ from the retained 17104e20c captures only in
`budget.used_builder_bytes` (3,930,276 to 3,296,148) and the graph self-digest.
Restoring those fields reproduces every previous byte. The 634,128-byte
reduction is the observed combined bill after the intervening replay allocation
changes; it is not attributed to a single helper.

Live browser pins now use the complete new graph digests. A new bridge requires
the exact observed bill, reconstructs the entire immediately preceding graph,
and invokes the complete previous bridge. That bridge remains compiled as
`before_2600.rs`; only its runner include points at the retained full old runner.
Its original four tests, all older graph bytes, whole-parent source custody,
rehash mutation refusal and original cursor helper remain unchanged. A new
source-custody test pins the entire old bridge and runner and proves the one
include substitution is its sole source change. New graph and mutation cases
exercise the complete composed bridge.

The actual composed bridge, complete previous module and parent cursor helper
passed all 10 focused Rust checks, including digest formatting controls, with a
sampled aggregate compiler peak of 182,736 KiB under a 200 MiB stop guard. This
component run does not execute the full compiler or Chromium consumers.

This capture proves graph bytes, measured bills and compiler provenance. The
refreshed browser run and full current-head CI still need hosted qualification.
