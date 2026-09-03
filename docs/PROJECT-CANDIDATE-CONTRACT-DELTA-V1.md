# Project Candidate Contract Delta v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors and agents reviewing candidate contract changes.

This additive report compares contracts across an immutable candidate's original
base and final admitted Project revision. It identifies changed predicate
projections and changes to the callable dependencies used by those predicates.
Agents need not query every function or generate target artifacts to discover
these changes. Existing Semantic Delta v1 and Interface Delta v1 remain separate
and keep their report schemas.

## API and source binding

```rust
pub fn contract_delta(&self, expected_candidate: &str)
    -> Result<String, Vec<Diagnostic>>;
pub fn verify_contract_delta(&self, expected_candidate: &str, bytes: &[u8])
    -> Result<String, Vec<Diagnostic>>;
```

The report schema is `semaprax.project-candidate-contract-delta.v1`.
Verification uses `semaprax.project-candidate-contract-delta-verification.v1`.
Both operations require the exact retained candidate digest. The report binds
the original and candidate Project/Workspace revisions and their ordered source
provenance. Functions are selected by persistent declaration identity, never
by coincident display names or relocated source spans.

Contract slots use the owning function, phase (`requires` or `ensures`), and
ordinal within that phase. A slot is revision-bound, not a newly assigned
persistent contract identity. Arrays preserve predicate order. No predicate is
matched across revisions solely by an expression ID, and duplicate predicates
must not be deduplicated.

Verification independently replays the complete candidate history and existing
canonical candidate evidence, recomputes the report, and compares all submitted
bytes. Replacing digests in a modified report cannot authenticate its facts.
Submitted JSON is never loaded as HIR, source, or approval authority.

## Predicate and dependency comparisons

Predicate expressions come from the compiler's checked Graph contract renderer.
They retain actual operators, literals, resolved callable identities, argument
order and checked types. Source locations and expression identities remain
available as provenance. Each affected `functions` row contains `id`, `change`,
`comparison`, `base`, and `candidate`. Each side retains its name, provenance and
ordered `predicates`. Predicate rows include phase/index, source fragment and
digest, checked expression projection and digest, root expression/type identity,
HIR availability, provenance and dependencies.

`comparison` separates exact equality, `predicate_projection_equal`,
`dependency_equal` and `source_equal`, with before/after fact digests and explicit
reasons. Predicate comparison retains phase, ordinal and checked expression.
Dependency comparison retains phase/ordinal and dependency identity, fact digest
and availability. Source comparison retains function name and ordered predicate
source-fragment digests. Source spans/revisions and root expression IDs remain in
exact facts but not these comparison views. Equality is not an arbitrary
recursive deletion of keys or a claim of semantic equivalence.

Affected functions retain complete ordered contract inventories on both sides,
including unchanged sibling predicates. An absent function is distinct from a
present function with no predicates. Functions without predicates on both sides
are counted without invented contract rows. Counts summarize unchanged inventory
rather than silently treating omitted rows as missing declarations.

Static dependencies begin at calls inside predicates and follow retained HIR
direct calls through callable bodies and contracts. Available dependency facts
bind the actual stable identity and authenticated source; unavailable facts are
explicit. This detects a changed helper behind an unchanged predicate expression.
It does not assert that a static call executes or that a changed helper changes
the truth value of a predicate. Recursive call relationships remain bounded.

Comparison is descriptive compiler/source projection equality. It does not
classify contracts as stronger or weaker, prove implication or satisfaction,
establish behavioral equivalence, or infer dynamic/external dependencies.
Existing Project admission remains responsible for contract purity, effects,
ownership and supported source/profile rules. This report admits no new source
syntax and weakens no candidate invariant.

Checked templates use their retained template predicates rather than duplicated
instantiation counts. Source-only predicates explicitly report unavailable HIR;
their source fragments must not be presented as checked expression projections.
If either side contains a source-only predicate, predicate and dependency
equality are null with explicit unavailable reasons, rather than true for empty
inventories. Empty dependencies in that case are not evidence of no calls.
Shared callable fingerprints retain source declaration/body digests, signatures,
direct-call identities and available normalized plan facts. They can change on
source-level edits without establishing a runtime behavior change.

## Protocol and authority

V5 exposes `candidate/contract-delta` only when the startup host grants candidate
preparation. Required parameters are `image_revision` and `candidate_revision`;
optional `offset` and `chunk_bytes` select bounded UTF-8 report chunks. The
response schema is `semaprax.image-contract-delta-chunk.v1`. Chunking preserves
the exact immutable report bytes. V1–v4 method sets remain unchanged.
Offsets are 0–8 MiB and must be valid UTF-8 boundaries within the actual report;
chunks are 1,024–65,536 bytes, defaulting to 16,384. Discovery and generated
clients describe the closed chunk envelope. The heterogeneous compiler report
is explicitly listed as unbundled, not claimed to have a complete client schema.

The route authenticates the session's source before and after the read. It does
not mutate a candidate, run tests or targets,
or gain source, Git, cache, build, or publication authority. The report and its
verification receipt cannot authorize a commit.

The [parallel retained-read extension](IMAGE-PARALLEL-CANDIDATE-READS-V1.md)
uses this same pure handler with only the selected immutable candidate.

## Bounds and evidence

Reports are compact canonical UTF-8 JSON with lexical object keys, ordered
arrays, and one terminal LF. The output limit is 8 MiB. Capacity failure rejects
the report rather than truncating predicate or dependency inventories. Item
inventories are bounded to 65,536; predicate expression/pattern visits and
dependency walks to 1,048,576 each; traversal depth to 256; retained/cloned fact
work to 32 MiB. The shared callable inventory retains its separate existing
32 MiB work bound per revision and its existing traversal limits. These are
structural/output bounds, not allocator, peak-memory or latency guarantees.
Existing source, HIR, candidate and Graph projection limits remain in force.

Digests use the domains `semaprax.candidate-contract-delta.fact.v1`,
`semaprax.candidate-contract-delta.source.v1`,
`semaprax.candidate-contract-delta.predicate.v1` and
`semaprax.candidate-contract-delta.report.v1`, each followed by NUL, then a
little-endian u64 byte length and exact bytes. Canonical JSON includes its LF;
source fragments use their exact authenticated span bytes. Dependency fact
digests retain the shared Interface Delta fact domain and normalization.

`SPX-G325` reports inconsistent source-backed contract facts, `SPX-G326` reports
capacity overflow, and `SPX-G327` reports exact replay mismatch. Existing stale
candidate, source and compiler projection diagnostics can propagate unchanged.
The unchanged shared callable collector can also propagate its existing
Interface Delta inventory/capacity diagnostics.

`tests/project_candidate/contract_delta.rs` owns focused library regression
evidence. `tests/image_transport_v5/contract_delta.rs` covers host gating,
exact chunk reassembly, stale inputs and physical source drift; the discovery
module also checks generated method helpers.
These cases are authored and unrun: no tests, compiler checks, interpreter,
target executable or long local quality gate was run for this change.

General invariant dependency graphs, logical contract reasoning, complete
ownership/cleanup deltas, runtime coverage, executed protocol compatibility and
the full graph-operational programme remain outstanding.
