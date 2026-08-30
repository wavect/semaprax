# Project Candidate Semantic Delta v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler maintainers and agents reviewing immutable Project candidates.

Semantic deltas compare actual compiler projections over an immutable
candidate's original base and final revision. They are descriptive, independently
recomputable facts, not runtime equivalence proofs or publication authority.

## API and binding

The additive `ProjectCandidate` methods are:

```rust
pub fn semantic_delta(&self, expected_candidate: &str, target: &str)
    -> Result<String, Vec<Diagnostic>>;
pub fn semantic_delta_catalog(&self, expected_candidate: &str)
    -> Result<String, Vec<Diagnostic>>;
pub fn verify_semantic_delta(&self, expected_candidate: &str,
    target: &str, bytes: &[u8]) -> Result<String, Vec<Diagnostic>>;
```

The selected report schema is `semaprax.project-candidate-semantic-delta.v1`;
the catalogue schema is `semaprax.project-candidate-semantic-delta-catalog.v1`.
Verification returns `semaprax.project-candidate-semantic-delta-verification.v1`.
Each request checks the caller's exact candidate digest. Selected reports bind
both Project revisions, Workspace revisions, image digests, and exact source
path/schema/revision/digest facts. A target absent from both revisions rejects.
Added declarations have an explicit absent base; moved identities retain both
source origins. Types, records, fields, cases, imports, protocols, and other
authored declarations are not discarded by a function-only filter.

Verification first independently replays the complete candidate history and
canonical candidate evidence. It then recomputes the selected report and compares
every exact submitted byte. It does not load submitted JSON as HIR or authority.
No interpreter, target executable, test, source write, or publication is invoked.

## Projections and comparison

Every available declaration retains its existing typed identity/index facts and
its authenticated canonical authored declaration fragment. The fragment binds
body and declaration changes that cannot be inferred from a signature alone.
Records/classes/variants additionally expose checked field identities, concrete
type identities, and ordering; selected fields retain their actual owner ID.
Variant cases retain their own field inventory. When a declaration lacks a
specialized typed projection, its authenticated identity and source fragment
remain available rather than being reported as a function.

Retained resolved functions reuse the existing image facet implementation for
signatures, contracts, callers, ownership inventory, loans, cleanup, data access,
unsafe boundaries, and Project relationships. Non-function targets explicitly
report function facets as not applicable. Generic templates retain authored
facts and participate in the direct-call relationship scan; this version does
not invent monomorphic function facets for an uninstantiated template.

Each facet carries exact before/after digests and byte lengths. Equal payloads
are omitted. Changed payloads appear in full, preserving canonical array order.
A separate `provenance_only` classification removes only these named keys
recursively before comparing:

```text
image_revision, project_revision, source_revision, source_digest,
span, source_span, expression_id, initializer_expression_id,
base_expression_id, container_expression_id
```

This limited normalization is projection equality, not semantic equivalence.
Stable declaration/value IDs, field indices, operations, literals, effects,
contracts, plan structure, and ordered vectors remain in the comparison. Exact
raw digests still distinguish provenance-only changes. No whole unchanged graph
is embedded in a report.

The compact catalogue compares every authored declaration's identity, name,
source path/module, and canonical fragment. It lists added, removed, moved, or
modified roots without copying their fragments. It is a root inventory, not a
complete dynamic impact closure.

## Fields, callers, tests, and targets

A bounded retained-HIR scan indexes selected field/type sites: constructors,
field initialization, update-result fields, projections, explicit places,
borrows, in-place field assignment, and recursive record/variant patterns.
Rows bind actual persistent field/type IDs, function IDs, phases, source paths,
and expression IDs. `read_or_move` deliberately does not infer runtime ownership
liveness. Whole aggregate reads are not expanded into every leaf.

Reverse closure follows actual local and imported direct calls, including
contracts and generic template bodies. It reports the direct field-user
functions and whether the declared test root reaches the selected target or
one of those users. It does not model external/dynamic callers, path feasibility,
or execution coverage. The existing candidate test plan is included separately,
including its conservative fallback reasons; no tests run.

Native C11 and structurally validated Core Wasm facts are freshly rederived for
the complete admitted entry and test closures. Their byte counts, content
digests, admission failures, and validation class are actual compiler projection
facts. They are **whole-closure** artifacts, not per-symbol code generation or
attribution, and do not establish runtime equivalence, portability beyond the
admitted lanes, or target execution.

## Canonical bytes, bounds, and evidence

Reports use compact UTF-8 JSON, recursively lexical object keys, preserved array
order, and exactly one terminal LF. Fact digests use the domain
`semaprax.candidate-semantic-delta.fact.v1\0`; verification uses
`semaprax.candidate-semantic-delta.report.v1\0` for the whole report. Authored
fragments use `semaprax.candidate-semantic-delta.authored.v1\0`. Each digest is
SHA-256 over domain, little-endian u64 byte length, and exact bytes. JSON fact
and report bytes include their LF; fragment bytes are the exact authenticated
source span.

Selected reports are capped at 8 MiB; catalogues at 1 MiB. Declaration/site/call
inventories and pattern traversal are bounded to 65,536 items, the HIR scan to
1,048,576 expression visits, and traversal depth to 256. Authored fragment
storage is capped at 32 MiB. Existing image and facet bounds also apply. Output
bounds are not total heap bounds. Overflow fails rather than silently truncating
facts. Transport may chunk the exact returned bytes without changing their
meaning.

New diagnostics are `SPX-G252` for invalid delta selection/facts, `SPX-G253` for
capacity, and `SPX-G254` for failed exact recomputation. Existing stale candidate,
image/facet, Project, and target-projection diagnostics may propagate unchanged.

`tests/project_candidate_semantic_delta_v1.rs` contains authored, unrun cases for
signature changes, omission of equal contract payloads, exact replay/tampering,
stale selection, source preservation, added-function absent-base contracts,
non-function field/record deltas, real read/write/pattern sites, static test
callers, and moved source origins. No local tests, compiler checks, executables,
or long gates were run, as requested. General semantic equivalence, complete
resource/liveness reasoning, runtime coverage, per-symbol target attribution,
and full graph-operational completion remain open.
