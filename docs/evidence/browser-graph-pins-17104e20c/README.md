# Hosted graph bill after paid-scratch changes

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

CI run 38793590554, job 116624982882, built
`17104e20c16db1d4ac6845266b78908f390b9896` using Rust 1.88 on Linux x86-64.
The complete logged graph and compiler provenance records were decoded with
strict base64, length and full-file SHA-256 checks. Both graph-domain digests
were independently recomputed.

Both complete graphs differ from the previous hosted 268cb0523 captures only
in `budget.used_builder_bytes` (4,393,980 to 3,930,276) and the graph self-digest.
Restoring those fields reproduces every previous byte. This is the observed
combined bill after the intervening paid-scratch changes; it does not attribute
the whole reduction to one individual cursor.

The live browser pins include the complete new bill. The Rust bridge requires
that exact bill and still reproduces the entire original historical graphs
before invoking the unchanged original cursor helper and known answers. A new
whole-wire gate additionally reproduces the immediately preceding captures.
The existing source-custody and rehashed non-builder-field mutation controls
remain intact.

This capture proves the graph bills and compiler provenance. The refreshed
browser run and full current-head CI still require hosted qualification.
