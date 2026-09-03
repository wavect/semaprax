# Project Candidate Environment Review v1

Status: library implementation authored and unrun. Project Agent Transport v5,
generated-client and MCP exposure are authored at this source head where noted
below. This review is descriptive evidence and carries no approval or
publication authority.

Audience: reviewers and agent clients that need one complete source review
beside an explicit account of deployment, generated-file and external-API blind
spots.

## Composition boundary

`ProjectCandidate::environment_aware_review` accepts one exact candidate
revision plus the canonical analysis-boundary bundle and its digest. It does
not trust nested caller reports. The compiler independently regenerates the
complete [candidate source review](PROJECT-CANDIDATE-SOURCE-REVIEW-V1.md) and
the three-declaration [analysis-boundary bundle](PROJECT-CANDIDATE-BLIND-SPOT-DECLARATIONS-V1.md),
parses both as exact canonical compiler JSON, and joins them back to the
retained candidate.

The join verifies the candidate and base Project revisions, Workspace revision,
semantic graph digest, bundle digest, the complete candidate source inventory,
and every changed file's base and candidate text and digest. Missing,
duplicated, stale, reordered or substituted identities fail closed. The source
review still owns the canonical source pairs and diff; the boundary bundle
still owns only the three bounded caller declarations.

The result schema is
`semaprax.project-candidate-environment-aware-review.v1`. It nests both complete
reports and binds their exact byte streams with SHA-256 values. It also retains
the source-review report revision, bundle digest, candidate/base/Project/
Workspace/graph identities, and a domain-separated revision for the complete
combined report. Nested reports do not acquire authority by composition.

## Meaning and nonclaims

The report places reviewable source changes beside the external contracts the
compiler cannot observe from source alone. It therefore exposes known blind
spots rather than treating absence of compiler graph edges as proof that no
external dependency exists.

All source, approval, publication, filesystem, environment, generator,
network, provider, runtime, conformance and deployment authority or observation
fields are false. `semantic_compatibility` is `not_assessed`. The report does
not claim source approval, current filesystem or deployment state, generator
execution, provider behavior, runtime conformance, external-consumer success,
or semantic, behavioral, API, ABI or migration compatibility.

## Bounds and diagnostics

The input bundle keeps its 24,576-byte bound. The complete result is bounded to
the 16 MiB source-review maximum plus the 2 MiB boundary-report maximum and a
64 KiB wrapper allowance. Reports fail instead of truncating a nested review.
`SPX-G452`, `SPX-G453` and `SPX-G454` identify invalid nested reports, capacity
failures and exact-binding failures respectively.

The typed v5 route requires the ordinary candidate-preparation grant and
returns bounded UTF-8 chunks of the immutable combined report. Discovery and
generated TypeScript/Python/Rust clients reuse the same closed request and
response schemas. The MCP catalogue exposes the same closed input schema and
forwards the v5 JSON-RPC response as opaque text; it does not claim an MCP
output schema. The route does not add an editor command, parallel-read
admission or ambient I/O.

## Authored evidence

Library regressions cover complete source and declaration composition, exact
identity joins, authority/nonclaim preservation, stale selectors and malformed
or oversized nested material. Transport and generated-consumer regressions
cover discovery, chunk continuation and closed schemas where present. They are
authored and unrun; no current-head runtime, provider, deployment, generated
consumer or quality-gate execution is claimed.

## Next composition: attached package consumers

The [Project Candidate Environment Consumer Review
v1](PROJECT-CANDIDATE-ENVIRONMENT-CONSUMER-REVIEW-V1.md) composes this complete,
unchanged environment review with independently regenerated summary and
consumer reports from one host-attached authenticated package graph. It
requires an exact candidate-era provider-source and target join, then publishes
a separate operational coverage schema in which only `external_consumers`
advances to `partial`. The attached graph remains input evidence with no Project,
filesystem, registry, runtime, compatibility or publication authority. Its v5
route requires both candidate preparation and host graph attachment, returns
bounded chunks, and stays outside parallel reads.
