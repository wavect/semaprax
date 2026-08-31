# Semantic Image Cleanup Dependencies v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors, agent authors and embedding hosts.

This query connects source type, case and field identities to actual retained
cleanup and loan facts. It answers which compiler plan facts depend on a
selected member without making an agent inspect every function's complete
plans. Canonical `.spx` source remains authoritative; the query introduces no
language, runtime, backend or publication authority.

## Image API and source binding

`ProjectSemanticImage::cleanup_dependencies(expected_image, target)` returns
`semaprax.image-cleanup-dependencies.v1`. The exact image digest and a source
declaration ID select the report. Source facts bind the Project and Workspace
revisions, module/path, source revision and source digest. Compiler-owned or
unavailable targets are not substituted by name.

The report contains the selected declaration and typed descriptor, the selected
owner/member ID inventory, `obligations`, `unavailable_templates`, limits and
index-work counters. Each obligation identifies its source function, optional
concrete instance, source provenance, facet, plan coordinate, original fact,
matched declaration IDs and selection reason. Expression, storage, loan, block,
edge and instance IDs remain revision-scoped; they are not persistent source
identities or runtime handles.

An immutable image lazily retains the cleanup dependency index under its
existing declaration dependency index. Concurrent reads share initialization,
including a failed result. The index is neither serialized into Image v1 nor
restored from caller-provided JSON. Refresh produces a new image and a new
index. Existing image and dependency-report payloads are unchanged.

## Facts and interpretation

The collector reads retained checked cleanup inventories, CleanupPlans and
Shared Loan Plans. It does not reconstruct control flow from source, scan a
second expression-reference graph or infer dynamic callers. Ordinary complete
HIR validation and its independent inventory/plan replay remain the validity
owners described in [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) and
[Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md).

Selected facts include structural storage and flags; owned-entry facts;
cleanup slots, transitions, control edges, regions, finalization actions and
exit continuations; and loans, endpoints and path edges. Whole-place
associations follow actual retained storage identities and field-liveness
shapes. Projected places retain their exact field/case paths; a sibling field
does not become the target merely because it shares a record root.

Structural membership is distinct from a destruction obligation. Copy fields
do not acquire sibling owned-field finalizers. A loan endpoint's union summary
is not substituted for its path-exact edge facts. Conditional variant facts
retain their actual case guards and payload identities.

Compiler Graph renderers own the loan and cleanup fact projections. Every
embedded vector retains its original order, including atomic call arguments,
failure selection and guarded finalization. Query row selection never sorts,
repairs or rewrites those plan vectors. A plan coordinate identifies the
original fact, not a new execution sequence.

Generic source templates without concrete plans are explicitly unavailable;
they do not receive empty successful plans. Retained concrete instances have
their actual template and instance identities. The report does not synthesize
uninstantiated programs or claim runtime coverage for retained instances.

`verify_cleanup_dependencies(expected_image, target, bytes)` recomputes the
index freshly from the retained checked revision and compares exact report
bytes, bypassing the cached facet. Its
`semaprax.image-cleanup-dependencies-verification.v1` receipt describes exact
recomputation. This is not a fresh disk read, cold-source rebuild, runtime
liveness proof or permission to invoke a finalizer.

## Candidate-aware review

`ProjectCandidate::cleanup_dependencies(expected_candidate, target)` returns
`semaprax.project-candidate-cleanup-dependencies.v1`. It delegates both the
original base and final candidate revision to this same image collector. It
adds no second cleanup analysis.

Each `base` or `candidate` side contains its image digest, exact report digest
and nested image report. An absent declaration is null, distinct from a
present declaration with zero selected facts. `presence` is `both`, `added` or
`removed`; a target absent from both revisions rejects. Exact comparison fields
cover selected obligations, unavailable templates and the typed declaration.
Those comparisons retain source provenance and revision-local IDs and are null
when a side is absent. Equality is not behavioral equivalence; an unrelated
source edit can change provenance without changing runtime behavior.

`verify_cleanup_dependencies(expected_candidate, target, bytes)` independently
replays the complete source intention history and existing candidate evidence
before recomputing the exact paired report. Its receipt uses
`semaprax.project-candidate-cleanup-dependencies-verification.v1`. Submitted
report bytes cannot populate an index, alter HIR or confer publication authority.

## Protocol and authority

V5 provides `image/cleanup-dependencies` to ordinary read-only sessions. It
takes `image_revision`, `target`, optional `offset` and optional `chunk_bytes`.
The embedding-host immutable read batch admits this method under its existing
before/after held-source authentication and join-before-return boundary.

`candidate/cleanup-dependencies` additionally requires `candidate_revision`
and the host's existing candidate-preparation grant. It is excluded from
immutable read batches. Neither method can obtain test, build, filesystem,
network or source-commit permission through a request.

Chunks use `semaprax.image-cleanup-dependencies-chunk.v1` and
`semaprax.image-candidate-cleanup-dependencies-chunk.v1`. They bind the target,
image, report schema, exact byte range and next offset; the candidate wrapper
also binds its candidate digest. Offsets must be UTF-8 boundaries within the
complete report. `chunk_bytes` is 1,024–65,536, default 16,384. Follow
`next_offset` while keeping all other selectors fixed.

Discovery, version-matched instructions and generated TypeScript/Python/Rust
clients derive from the selected method registry. The chunk envelopes have
closed schemas; heterogeneous compiler report interiors remain explicitly
unbundled. Earlier protocol profiles and existing methods are unchanged.

## Bounds, diagnostics and evidence

The image report is bounded to 8 MiB. The retained cleanup index permits at most
65,536 items, 1,048,576 traversal visits, depth 256 and 32 MiB of accounted
fact/index bytes; capacity failure returns no partial inventory. Existing
declaration dependency index bounds remain in
force. These are logical work and wire limits, not measured process RSS.

The candidate wrapper caps both the combined image-report input bytes and its
final serialized output at 8 MiB. Its metadata can make a wrapper exceed the
limit even when the two input reports fit; this rejects rather than truncating
either side. No query or verification runs a native compiler, interpreter,
target program or test suite.

Image-specific diagnostics are `SPX-G334` for invalid source/type/proof binding,
`SPX-G335` for capacity and `SPX-G336` for exact report mismatch.
Candidate-specific diagnostics are `SPX-G337` for an invalid target/report,
`SPX-G338` for capacity and `SPX-G339` for exact-history replay mismatch.
Existing stale candidate/image and underlying collector diagnostics propagate.
Transport stale-image, invalid UTF-8 range and output-bound failures retain
their ordinary diagnostics.

Authored cases in [image evidence](../tests/image_cleanup_dependencies_v1.rs)
and [transport evidence](../tests/image_cleanup_dependencies_transport_v5.rs)
cover real plan relationships, unchanged older reports, source bindings,
candidate review and hostile selectors. These checks remain unrun, so no
completion-matrix row is promoted. General lifetime/alias analysis, external
consumers, physical settlement, runtime observations, broader ownership
admission and measured agent productivity remain outstanding.
