# Semantic Image Facets v1

Status: authored, unrun. This additive read-only slice does not complete the
full graph-operational programme.

An immutable `ProjectSemanticImage` can return a compact function summary and
expand revision-bound facets without reading source paths, parsing source
text, adding graph-only meaning, or granting source/publication authority.
Image v1 serialized bytes, its digest, and existing Graph/Context/Impact
schemas remain unchanged. These queries inspect the already validated
per-module HIR retained by the image's `ProjectRevision`.

## API and selection

`function_summary(expected_image_digest, stable_id)` returns compact JSON with
schema `semaprax.image-function-summary.v1`. It describes one declared resolved
function: display name, relative source path/module, source revision and span,
parameter count, return type identity, effects, contract counts, and seven
`{facet, handle}` references. Compiler-owned declarations, missing IDs, and
non-function declarations are not selectable.

`expand_facet(expected_image_digest, stable_id, facet, handle, cursor, options)`
returns schema `semaprax.image-facet.v1`. The closed `ImageFacet` enum admits
`signature`, `contracts`, `callers`, `ownership`, `loans`, `cleanup`, and
`relationships`; `ImageFacet::parse` rejects other names. Every response binds
image and Project revisions, target ID, facet, handle, relative source path,
source revision, item offset/count, and an optional next cursor.

`ImageFacetOptions::new(page_size, max_bytes)` admits 1–128 items per page and
1,024–1,048,576 output bytes. Defaults are 32 items and 65,536 bytes. Targets are
at most 4,096 bytes, handles at most 71 bytes, and cursors at most 100 bytes.
Each inventory admits at most 65,536 items. Existing compiler contract/plan
renderers run under a 16 MiB intermediate rendering budget. These limits bound
wire output and admitted intermediate renderers; they are not a total heap
allocation claim. Oversized individual pages fail instead of returning
partial JSON or silently omitting facts. Consumers may reduce the page size
or increase the output limit within the fixed bounds.

API strings omit terminal LF. JSON objects use canonical lexical key order,
while arrays preserve their specified order. Cursor offsets must be canonical
page boundaries and remain inside the complete inventory. Empty inventories
return one empty first page with no cursor.

## Facet meaning and evidence

| Facet | Facts and their limits |
| --- | --- |
| Signature | Parameter ID, name, canonical type identity, ownership, source span; result identity/type; declared effects and module permits. A declared permit does not grant this query capability authority. |
| Contracts | Requires/ensures in source order, expression identity/type/span, and the existing compiler Graph contract expression projection, including resolved calls and operators. No evaluator runs and no predicate is newly proven. Existing unsupported contract-projection diagnostics remain fail-closed. |
| Callers | All declared functions and function templates in every retained module are traversed through the compiler's existing exhaustive HIR call visitor. Entries group direct calls by caller and requires/body/ensures region, with occurrence counts and local/cross-file classification. This includes local edges absent from Workspace Analysis's six cross-file edge families. No dynamic or external consumers, artifact imports, or duplicated generic-instance execution counts are inferred. |
| Ownership | Authenticated parameter ownership, structural cleanup inventory slots/types, flags count, and entry-state owned-parameter facts. Structural discovery order is not runtime liveness or destruction order. |
| Loans | Complete existing LoanPlan header, loans, endpoints, and edges through the established Graph loan renderer. End-edge and path-sensitive liveness data remain unchanged proof data, not runtime references or new authority. |
| Cleanup | Complete existing CleanupPlan header, slots, status sources, blocks, edges, regions, and exits through the established Graph cleanup renderer. Transition/finalizer vectors retain their canonical execution order without sorting or repair. |
| Relationships | Existing Project profile admission, declared entry/test module membership, entry/test linked-closure membership, and selected Web export IDs. Native/Wasm target checking and artifact emission are explicitly not performed; test closure membership is not test coverage or execution evidence. |

Proof plans flatten into section-tagged items: header first, then sections in
the table's order, then each section's exact vector order with its original
index. Concatenating pages reconstructs the emitted section values without
changing their order. Contract and plan JSON is compiler-owned projection
input; no user-supplied HIR or proof plan is deserialized.

Every summary and page labels its evidence as
`descriptive_projection_of_validated_hir`. Existing admission and independently
replayed HIR attachments supply the facts; facet output itself is not an
independent verification receipt, execution proof, authorization, or a claim
of stronger analysis.

## Reference binding and diagnostics

A handle is SHA-256 over domain `semaprax.image-facet-handle.v1` followed by NUL,
then image digest, target ID, and facet name. Each field has a preceding u64
little-endian byte length. A cursor is `<offset>:<digest>`; its digest uses
domain `semaprax.image-facet-cursor.v1` followed by NUL and the same length
framing around handle, canonical decimal offset, and canonical decimal page
size. References are deterministic opaque selectors, not secrets or bearer
permissions. Changing the image, target, facet, page size, or offset rejects
an old reference. Maximum output bytes may change between pages.

| Code | Meaning |
| --- | --- |
| `SPX-G227` | Unknown facet, invalid target, missing/non-function target, or invalid compiler-owned projection. |
| `SPX-G228` | Input, page, inventory, or intermediate/output rendering limit. |
| `SPX-G229` | Stale, unknown, noncanonical, or mismatched handle/cursor. |

Expected image digest validation retains Image v1's `SPX-G219`–`SPX-G221`
codes. Ordinary existing contract projection diagnostics remain unchanged.
An old retained image remains queryable after disk changes; these APIs do not
promise current filesystem freshness. A live host must independently retain
its normal source-authentication/recheck boundary.

## Evidence and remaining work

[Authored integration evidence](../tests/image_facets_v1.rs) covers cross-root
determinism, unchanged Image v1 bytes, real contract expressions, local and
cross-file callers across body/contract regions, paginated completeness,
reference rejection across revisions/targets/facets/page sizes, and owned-byte
loan/cleanup facts. Tests were not run at the user's request; no green-gate or
hosted-completion claim is made.

Incremental indexes, persisted typed HIR, expression-level source mutations,
arbitrary declaration kinds, dynamic/external consumers, target-admission
execution, inferred test coverage, and all publication authority remain outside
this query contract.
