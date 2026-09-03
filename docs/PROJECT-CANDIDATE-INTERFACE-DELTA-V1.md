# Project Candidate Interface Delta v1

Status: Partial; implementation and focused regression evidence authored, unrun.

Audience: compiler contributors and agents reviewing immutable Project candidates.

This additive report compares source-backed static protocol conformance across
an entire candidate's original base and final admitted Project revision. It
exposes changed required signatures, implementation bindings, and the functions
behind those bindings without requiring an agent to guess which individual
stable IDs to query. It leaves the selected Semantic Delta v1 and image/runtime
graph schemas unchanged.

## API and exact binding

```rust
pub fn interface_delta(&self, expected_candidate: &str)
    -> Result<String, Vec<Diagnostic>>;
pub fn verify_interface_delta(&self, expected_candidate: &str, bytes: &[u8])
    -> Result<String, Vec<Diagnostic>>;
```

The report schema is `semaprax.project-candidate-interface-delta.v1`; verification
uses `semaprax.project-candidate-interface-delta-verification.v1`. Reports bind
the exact candidate digest, both Project and Workspace revisions, and complete
ordered source bindings on both sides. Each source binding contains its path,
source revision, and source digest. Equal display names in different modules
never substitute for persistent declaration identity or source provenance.

Verification independently replays the complete candidate history and canonical
candidate evidence, regenerates this report, and compares all submitted bytes.
Recomputing public fact digests around a modified payload cannot authenticate
that payload. Submitted JSON is never deserialized as trusted HIR or treated as
permission to publish. A stale selector, different candidate, changed report,
or noncanonical encoding fails closed.

## Whole-candidate comparison

`protocols` contains affected protocol identities and their `comparison`.
`implementations` contains affected implementation identities, their
`comparison`, and the union of before/after required member identities. Every
member of an affected implementation remains present, including siblings whose
facts did not change. This distinguishes an unchanged requirement from a
missing implementation member.

A comparison contains its `change` classification, exact and separately
provenance-insensitive equality flags, before/after fact digests, and complete
`base` and `candidate` values. Absence is explicit null. Classification is
`added`, `removed`, `modified`, `provenance_only`, or `unchanged`; it is compiler
projection comparison, not proof of equivalent runtime behavior.

Provenance-insensitive comparison recursively removes only `provenance`,
`span`, `source_span`, `source_revision`, `source_digest`, `expression_id`, and
`exact_digest`. Declaration/body digests, parameter names, resolved type IDs,
ownership modes, requirements, contracts, and plan projection digests remain.
Dependency fact digests use this normalization so unchanged helper source
does not appear semantically changed solely because its source provenance moved.

Member facts bind the actual required signature, selected function identity,
source and typed function facts, and static reachable callable dependencies.
Consequently a function body, display name, permitted postcondition, or reachable
helper edit can affect the report even when the method-to-function mapping is
unchanged. Dependencies retain stable IDs, available fact digests, source
provenance, an explicit availability reason, and the evidence owner/class; static
reachability does not establish path feasibility or execution coverage.
`fact_availability` distinguishes `retained_source_callable` from
`external_or_unretained_callable`; unavailable body facts and provenance are
explicit null. Function facts retain sorted `direct_calls` identities, so equal
reachable sets cannot conceal different direct-call bindings.

The compact `inventory` gives before/after protocol and implementation counts
and counts of unchanged protocols and implementations. Entire unchanged
implementations are omitted from the detailed inventory, while complete source
bindings remain available. Ordinary targets with no static conformance do not
acquire invented protocol nodes or runtime dispatch edges.

## Admission and authority

Facts derive only from canonical sources of already admitted immutable Project
revisions. `implement_interface` still requires a complete local member table
with distinct compatible local functions. This report does not add cross-module
implementation bindings, dynamic dispatch, runtime witness tables, interface
editing syntax, or new candidate operations. Independent source modules can
carry independent local conformance, and callable dependency provenance may
cross module boundaries through ordinary authenticated imports.

A function display rename preserves persistent bindings. Permitted body or
postcondition changes remain subject to ordinary source verification and full
candidate admission. Incompatible signatures or stronger preconditions reject
before a new candidate/report is admitted; the old candidate and original
source remain unchanged. This report does not make unsupported signature
transformations available or weaken their ownership rules.

Reports and verification do not execute tests, interpreters, or emitted targets.
They do not prove behavioral contracts, runtime equivalence, dynamic reachability,
or backend-specific dispatch. They grant no filesystem, source publication,
Git, cache, or commit authority. Separate host-selected publication mechanisms
retain their existing independent checks.

## Bounds and evidence

V5 exposes `candidate/interface-delta` only when the host enables candidate
preparation. Required parameters are `image_revision` and `candidate_revision`;
optional `offset` is 0–8 MiB and `chunk_bytes` is 1,024–65,536 (default 16,384).
Offsets must lie on a UTF-8 boundary in the report. The
`semaprax.image-interface-delta-chunk.v1` envelope binds both request revisions,
the report schema, offset, total bytes, chunk, nullable next offset and
`source_authority:false`. Candidate immutability fixes all chunk contents.
The session authenticates live source before and after preparation; this route
does not mutate candidates. The
[parallel retained-read extension](IMAGE-PARALLEL-CANDIDATE-READS-V1.md) shares
the same pure handler with only the selected candidate. V1–v4
method sets remain unchanged. Transport/discovery evidence is authored in
`tests/image_transport_v5/workspace.rs` and the v5 discovery module.

Reports are deterministic compact JSON with recursively lexical object keys,
canonical array order, and one terminal LF. The report is bounded to 8 MiB;
inventories to 65,536 items; collected direct-call occurrences and dependency
closure walks to 1,048,576 each; HIR expression visits to 1,048,576 and depth
to 256; retained/cloned fact-work accounting to 32 MiB.
Existing source, retained-HIR, Project, and candidate
bounds remain in force. Overflow rejects instead of silently dropping members.
These are structural/output bounds, not peak heap or latency guarantees.

Fact, source-fragment, and report digests use the domains
`semaprax.candidate-interface-delta.fact.v1`,
`semaprax.candidate-interface-delta.source.v1`, and
`semaprax.candidate-interface-delta.report.v1`, respectively, each followed by
NUL. The preimage is that domain, the little-endian u64 byte length, and exact
bytes. Canonical JSON facts/reports include their final LF; source fragments
use their exact authenticated span bytes. Cleanup and loan facts retain
separate exact and provenance-normalized projection digests.

`SPX-G310` rejects inconsistent source-backed inventories, `SPX-G311` rejects
capacity overflow, and `SPX-G312` rejects exact replay mismatch. Existing stale
candidate, source, conformance, and Project diagnostics propagate unchanged.

`tests/project_candidate/interface_delta.rs` authors addition with complete
member tables, independent modules with identical display names, exact source
provenance, changed bound-function facts with unchanged sibling retention,
imported-helper edits affecting a binding whose own source is unchanged,
normal source rejection of incompatible requirements, replay tampering and
stale selection, deterministic output, and source preservation.

All evidence is authored and unrun. No tests, compiler, interpreter, application,
or long local quality gate was executed for this change. Hosted evidence and
the broader graph-operational programme remain outstanding.
