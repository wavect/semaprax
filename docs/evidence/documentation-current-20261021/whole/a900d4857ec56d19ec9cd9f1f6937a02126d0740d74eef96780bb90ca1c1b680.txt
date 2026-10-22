# OPT843 held closure receiving candidate

Status: historical source evidence; qualification is limited to the recorded revisions and results.

Audience: compiler contributors and reviewers.

Source-only implementation against `7800b014151b6ce0709c351ccdfd52ae25832e07`.
No Rust compilation, tests, Clippy, runtime allocation observations or performance
measurements were executed for this candidate.

The ordinary factored caller acquires one `ClosureReceiving` before its two
weighted roots. Cold closure production returns unit and publishes directly to
that held success/refusal owner. An independent ready flag distinguishes normal
arithmetic `None` from refusal. The success path reads the refusal discriminant
before extracting a Diagnostic, so it constructs neither the old inner Result
nor a successful Option<Diagnostic> transport. Every original nested budget,
ordered-map, alias and memo-insertion Result remains real; their existing charges
remain. The outer closure Result, scratch Result and copied root fee remain.

Acquisition pays its actual owner initializer/receiver, acquisition Result,
Option initializers, held Diagnostic receiving, and positive-bootstrap Result.
Each miss still pays the borrowed destination and three small success transports.
The source names these layouts with `size_of`, without asserting a machine size
or inferring savings from compiler liveness. Additional acquisition cost can
outweigh fewer cold misses; application incidence and any net gain are unmeasured.
The test-only actual-miss counter increments at the unit producer boundary, not
at each memo insertion. A single producer can publish several memo entries.

The complete 7800 canonical body and complete factored caller are compiled as
independent old controls. Only their lexical imports/visibility differ, and the
full bodies are asserted against the immutable before sources. Existing older
receivers, assertions, fees, profiles, caps and protected runtime JSONs are kept.
The cfg(test) `id`, `successors` and `closure` compatibility entries acquire a
real single-request owner; they are not the production two-root receiving-cost
measurement. The production-slot controls call `id_with_receiving` directly.

Ten authored literal controls cover every observed semantic-work frontier,
six-field Diagnostic equality, duplicates/overflow/cycles, two-root reuse, actual
miss counts, cache one-work/no-allocation behavior, cold exact/short payment,
separate real old/new physical frontiers and complete Drop, field-preserving
failure moves, sticky foreign/nested/expired ownership, arithmetic None reuse,
and the complete caller. These controls remain unexecuted. The known 7800
unrelated old-test import/privacy compilation repairs belong to the parent's
9d435807 successor, not this isolated feature commit.

`runtime-projections.json` is a protected runtime input: complete candidate bytes
project to immutable 7800 before the existing held/MAIN historical hops. Preserve
it byte-exact. `source-inverses.json` records all 11 Rust owners: six complete
existing predecessors and five additions. All five additions enter all three
existing source inventory unions. The original Source36 a/b and its full
assertions are unchanged. A merged #842/#843/current-source successor must
explicitly authenticate merged bytes before entering each isolated predecessor;
neither feature manifest claims authority over combined source.

Static preparation and the pending exact selectors are external under
`/Volumes/Semaprax/provenance/opt843-held-closure-receiver-20261015/`.
