# Project Candidate External API Contract Delta v1

Status: library implementation authored and unrun. Project Agent Transport v5,
generated-client and MCP exposure are authored at this source head where noted
below. The delta compares caller-declared digest inventories and deliberately
does not assess compatibility.

Audience: reviewers and agent clients comparing the declared external API
surface of one retained base Project with one exact candidate.

## Exact declarations

`ProjectCandidate::external_api_contract_delta` accepts an expected candidate
revision plus canonical base and candidate declaration bytes and their
domain-separated digests. The base declaration uses
`semaprax.project-candidate-external-api-contract-base-declaration.v1` and is
bound to the retained base Project revision. The candidate declaration reuses
`semaprax.project-candidate-external-api-contract-declaration.v1` and is bound
to the candidate revision.

Both declarations retain the existing closed, digest-only operation rows:
`export_id`, `operation_digest` and `schema_digest`. Each scope is either the
complete exact manifest export inventory or a nonempty, strictly ordered set of
explicit stable exports. Every identity must resolve to an explicit function
in the corresponding retained base or candidate manifest. Unknown fields,
locator-like fields, malformed digests, incomplete complete-manifest scopes and
stale Project/candidate selectors fail closed.

## Descriptive delta

The result schema is
`semaprax.project-candidate-external-api-contract-delta.v1`. It binds the base
and candidate Project, Workspace and semantic-graph digests, retains both exact
canonical declarations and their digests, and emits the canonical union of
declared export identities. Each row is `added`, `removed`, `changed` or
`unchanged`; changed rows name only the `operation_digest` and/or
`schema_digest` facets that differ. Inventory counts summarize those rows.

Added and removed describe the two caller-declared inventories. They do not
prove that a provider added or removed a deployed API. The comparison can also
use different explicit subsets, so the result must be reviewed with both
retained scope declarations.

`compatibility` is always `not_assessed`. The report supplies no endpoint,
provider, network, runtime, version, conformance, consumer or migration
observation. All source, filesystem, process, network, ambient, publication and
deployment authority fields are false. The method performs no external I/O.

## Bounds, transport and diagnostics

Each declaration keeps the 131,072-byte external-API declaration bound. The
complete delta is capped at 2 MiB and contains at most the bounded manifest
export union. `SPX-G446`, `SPX-G447` and `SPX-G448` identify invalid shape,
capacity and exact-binding failures respectively. Rows are rejected rather
than repaired or truncated.

The typed v5 route requires the ordinary candidate-preparation grant and
returns bounded UTF-8 chunks of the immutable report. Its request carries both
canonical declaration strings and digests beside the exact image/candidate
selectors. The ordinary 64 KiB JSON-RPC frame bound is tighter than the two
independent library declaration maxima, so transport callers must fit the
complete combined request within that existing limit. Discovery and generated
TypeScript/Python/Rust clients share the same closed request and response
schemas. The MCP catalogue exposes the same closed input schema and forwards
the v5 JSON-RPC response as opaque text; it does not claim an MCP output schema.
The route adds no parallel-read admission or editor command.

## Authored evidence

Library regressions cover unchanged and changed digest inventories, added and
removed declared identities, canonical row order, stale base bindings and
rejection of an attempted URL field. Transport and generated-consumer
regressions cover discovery, schema closure and chunk continuation where
present. They are authored and unrun; no provider, network, runtime, deployment,
consumer, compatibility or current-head quality-gate execution is claimed.
