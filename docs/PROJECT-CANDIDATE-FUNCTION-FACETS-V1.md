# Project Candidate Function Facets v1

Status: additive implementation and regression sources authored, **unrun**.

Audience: agents and embedding hosts navigating exact function facts in a
retained final candidate.

Image [Function Facets v1](SEMANTIC-IMAGE-FACETS-V1.md) exposes compact declared
function summaries and nine paged HIR facets. This candidate projection derives
an invocation-local semantic image from one exact fully admitted candidate and
reuses those existing facts and item order. It includes changed and newly added
functions without treating the original base image as candidate evidence.

## Library API

```rust
pub fn ProjectCandidate::function_summary(
    &self,
    expected_candidate: &str,
    target: &str,
) -> Result<String, Vec<Diagnostic>>;

pub fn ProjectCandidate::expand_function_facet(
    &self,
    expected_candidate: &str,
    target: &str,
    facet: ImageFacet,
    handle: &str,
    cursor: Option<&str>,
    options: ImageFacetOptions,
) -> Result<String, Vec<Diagnostic>>;
```

The summary schema is `semaprax.project-candidate-function-summary.v1`; the
page schema is `semaprax.project-candidate-function-facet.v1`. Summary output is
bounded to 64 KiB. Page options retain the existing 1–128 item and
1,024–1,048,576 byte bounds, with defaults 32 and 65,536. Candidate selection
is authenticated before deriving the temporary image. Neither query retains
that image or a new candidate.

The summary binds exact candidate and base Project revisions, candidate image
and final Project revisions, target identity and source provenance. Its nine
facets remain in the canonical existing order: `signature`, `contracts`,
`callers`, `ownership`, `loans`, `cleanup`, `relationships`, `data-access`, and
`unsafe-boundaries`. Each handle is candidate-bound. The page repeats those
bindings and preserves the selected existing image-facet item interiors and
their order without sorting, repairing or reinterpreting compiler plans. Each
heterogeneous item is carried as
`{schema:"semaprax.project-candidate-function-facet-item.v1",value:<exact image item>}`
so generated clients can validate the envelope without pretending to own every
compiler-specific interior.

Only declared resolved functions in the final candidate are selectable. A
declaration present only in an earlier source, a non-function stable ID, a
compiler-owned function or a missing target remains unavailable. This report
does not turn a removed declaration into retained final-candidate meaning. The
current typed intent catalogue has no declaration-removal producer, so authored
evidence exercises a missing ID and an actual added record through the same
final-HIR unavailable path rather than fabricating a removal case.

## Handles, cursors and paging

A handle binds the candidate digest, derived image digest, target and facet.
It is invalid for the base candidate, a sibling candidate, another target or
another facet. A cursor binds that handle, the canonical offset and page size.
Changing page size rejects it; `max_bytes` may vary between pages as in the
existing image contract. Handles and cursors are opaque selectors rather than
secrets, capabilities or retained server objects.

`SPX-G358` owns invalid wrapper shape or binding, `SPX-G359` wrapper capacity,
and `SPX-G360` stale or mismatched candidate references. Candidate selector,
underlying target/facet, option and compiler projection diagnostics propagate
from their existing owners. Pagination never silently drops an oversized item
or claims a truncated inventory is complete.

## V5 transport and authority

With `candidate_prepare`, v5 selects `candidate/function-summary` and
`candidate/function-facet`. Both require exact `image_revision`,
`candidate_revision` and `target`; facet expansion additionally requires one of
the nine names and the exact handle, with optional cursor, page size and byte
limit. The closed reports are generated in TypeScript, Python and Rust clients
and exposed through MCP as `candidate__function-summary` and
`candidate__function-facet`.

These are pure reads over a detached retained candidate and are eligible for
the authenticated parallel/read-batch path. Sequential, detached and batch
responses use the same handler and exact bytes. Live source authentication
still surrounds host calls; source drift releases no response and does not
mutate the candidate registry.

`source_authority`, `target_execution`, `candidate_retained`, `execution` and
`publication_authority` are false. Facets remain descriptive projections of
validated HIR. They do not prove runtime liveness, contract truth, test
coverage, target admission, external/dynamic callers, compatibility, source
freshness outside the host boundary, or authority to edit, execute or publish.

Authored, unrun evidence in `tests/image_v5/candidate_function_facets.rs`
covers changed and newly added functions, all nine facets and exact item order,
candidate/base/sibling/target reference isolation, cursor-option binding,
unavailable and non-function targets, selected schemas and generated clients,
MCP, detached parallel/read-batch parity, source drift, registry immutability
and false authority. No tests, compiler executable, target or application was
run while authoring this tranche.
