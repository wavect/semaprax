# Project Candidate Ownership Delta v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors and agents reviewing ownership-sensitive changes.

This additive whole-candidate report compares checked parameter ownership,
structural cleanup inventories, Shared Loan Plans and CleanupPlans between a
candidate's original base and final admitted Project revision. It makes changed
ownership evidence discoverable without querying every function or generating
target artifacts. It does not introduce a new ownership analysis or alter the
compiler's plan builders, validators or execution backends.

## API and binding

```rust
pub fn ownership_delta(&self, expected_candidate: &str)
    -> Result<String, Vec<Diagnostic>>;
pub fn verify_ownership_delta(&self, expected_candidate: &str, bytes: &[u8])
    -> Result<String, Vec<Diagnostic>>;
```

The report uses `semaprax.project-candidate-ownership-delta.v1`; verification
uses `semaprax.project-candidate-ownership-delta-verification.v1`.
Both operations require the exact candidate digest. Reports bind both Project
and Workspace revisions and their source inventories. Source callables are
identified by persistent declaration ID. Equal display names in different
modules do not identify the same function.

Verification independently replays the entire candidate history and canonical
candidate evidence before recomputing and comparing the exact report bytes.
Submitted JSON, even with recomputed public digests, is never trusted as a plan,
HIR attachment or source update. Evidence does not grant publication authority.

## Checked facts and comparisons

The report retains actual resolved parameter ownership and type identities.
Result facts retain the checked result identity/type; return ownership is not
guessed from a source spelling. Structural inventory facts preserve storage
origins, ordered shapes, flags and entry-state facts. Structural discovery order
is not runtime initialization or destruction order.

Loan and cleanup plans are rendered by their existing compiler Graph renderers.
Every vector retains its original order: no sorting, deduplication, plan repair
or reconciliation occurs. This includes loan identities, parents, endpoint and
edge vectors, cleanup transitions, atomic call-argument commit groups, failure
status selection, guarded finalization order and result publication.

Functions with changed facts expose complete before/after values. Per-facet
comparisons separate signature, structural inventory, loans, cleanup and source
changes. An absent function differs from an existing function with empty owned
storage or an empty loan plan. Unavailable checked facts are explicit rather
than treated as empty successful plans.

Each `functions` row contains the persistent `id`, `change`, `comparison`,
`base` and `candidate`. A present side contains name/source provenance, its
source-declaration digest, `hir_availability`, `signature`, `cleanup_inventory`,
`loan_plan`, `cleanup_plan` and `instances`. The comparison carries exact
before/after fact digests, `exact_equal`, per-facet equality, `instances_equal`,
`source_equal` and explicit reasons. Per-facet equality is null if a present
side lacks that checked fact. Inventory counts include unchanged functions;
their full payloads are omitted.

Source-level generic templates retain their checked signatures where available;
they do not receive invented monomorphic plans. Retained concrete instances are
separately identified by their actual compiler instance ID, template and type
arguments. Instance identities and plan-local expression, loan, storage, block
and edge IDs remain revision-bound. They are not new persistent source IDs.

Plan comparisons are exact compiler-projection comparisons, including those
revision-local IDs. Source edits can therefore produce plan differences without
changing runtime behavior. The report neither normalizes such differences into
a claimed equivalence nor infers that a changed plan is unsafe.

Ordinary complete Project admission and HIR replay continue to own type,
ownership, loan and cleanup validity. A plan is proof data, not permission to
transfer a physical owner, invoke a finalizer, settle a native call or publish a
result. Existing restrictions on borrowed values, resource types, generic
instances and module/profile combinations remain in force.

## V5 discovery and authority

`candidate/ownership-delta` is available only when the startup host grants
candidate preparation. Required parameters are `image_revision` and
`candidate_revision`; there is no target selector. Optional `offset` is
0–8 MiB and must be a UTF-8 boundary within the report. `chunk_bytes` is
1,024–65,536, defaulting to 16,384.

The closed `semaprax.image-ownership-delta-chunk.v1` response binds the image,
candidate, report schema, current/next offset, total bytes and exact chunk.
Discovery and generated clients describe that envelope; the heterogeneous
compiler report remains explicitly unbundled. V1–v4 and existing delta report
schemas remain unchanged.

The session authenticates live source before and after preparation. This read
does not mutate candidates, join parallel image-only workers, emit target
artifacts, run interpreters/tests/targets, or acquire source, Git, filesystem,
cache, approval or publication authority.

## Bounds and evidence

Reports use canonical compact UTF-8 JSON, lexical object keys, preserved arrays,
and one terminal LF. The output limit is 8 MiB. Logical fact/render work is
bounded to 32 MiB, structural inventories to 65,536 items, type/shape/JSON
traversal to 1,048,576 visits, and recursive depth to 256. The budget is shared
across base, candidate and comparison construction. Compiler plan rendering
and compiler-generated JSON parsing retain their own bounds as well.
Existing source, retained-HIR,
plan-builder and candidate limits remain mandatory. Overflow rejects the whole
report instead of silently dropping an owner, instance, plan section or action.
Output and structural bounds do not establish peak memory or latency guarantees.

Digests use the domains `semaprax.candidate-ownership-delta.fact.v1`,
`semaprax.candidate-ownership-delta.source.v1` and
`semaprax.candidate-ownership-delta.report.v1`, each followed by NUL, then the
little-endian u64 byte length and exact bytes. Canonical JSON facts/reports
include their LF; source-declaration digests bind the exact authenticated
function span bytes.

`SPX-G328` reports inconsistent ownership-delta facts, `SPX-G329` capacity
overflow, and `SPX-G330` exact replay mismatch. Existing stale candidate, source,
compiler and plan projection diagnostics may propagate unchanged.

`tests/project_candidate_ownership_delta_v1.rs` owns focused library evidence;
`tests/image_ownership_delta_transport_v5.rs` covers the v5 surface. Cases are
authored and unrun. No tests, compiler checks, interpreter, target executable or
long local quality gate was run for this batch, and no completion row is promoted.

General ownership/lifetime reasoning, reverse field-to-obligation queries,
physical settlement, runtime equivalence, target execution, broader resource
admission and full graph-operational completion remain outstanding.
