# OPT877 ordinary enumerated HIR source candidate

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Base: 36c6deb98720e243b0daaf81e813fbe3ec3b0a2d. No Cargo, compiler, test,
benchmark, tokenizer, provider or full graph build was run in this lane.
Standalone rustfmt parsed the changed/new Rust definitions; this does not
establish name/type correctness or executable qualification.

`before.rs` independently compiles the complete unchanged sequence and root
comparison definitions from frozen 36. Six complete old source files are kept
verbatim. `source-inverse.json` binds their complete current/predecessor bytes
and ordered reverse edits. The source-custody lib owner reconstructs all six
and refuses whole-current drift; later integration needs an explicit finite
source acquisition rather than weakening these guards.

Focused new owners: existing lib module
`cleanup_plan::replay::skeleton_work::enumerated_prefix::{tests,source_custody}`.
All original #856/#858/#831, HIR/PLAN, physical and application assertions stay
required. Current 452's canonical memo work mismatch and factored contract
physical mismatch remain actual unresolved evidence; this candidate does not
claim to fix them. The original 64 MiB codec refusal is likewise unchanged
current-head evidence until this new source actually executes.

The existing layout is reused to avoid silently changing historical observation
carrier sizes. Seq/Cat construction and append are a separate nonmerging route
with ordinary free-work charges and the existing prepaid nonrecursive release
frontier. Exact receiver-census review, full independent old/new runtime,
byte/work cuts, sticky/floor/allocator failures, complete Drop, actual paid and
physical tuples, appropriate production Clippy and actual moved-pin consumers
remain prerequisites. The route grants no public support or application claim.
