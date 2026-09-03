# Project Candidate Blind-Spot Declarations v1

Status: library implementation and regression sources authored, **unrun**.
Both attachments are descriptive caller declarations. They advance one analysis
area to `partial`; they do not establish observed, current, or conformant
external state.

Audience: agent authors and compiler contributors attaching bounded facts to
one exact admitted candidate.

## Shared boundary

Both APIs authenticate the expected candidate before accepting a declaration.
The declaration must be canonical JSON, fit its fixed byte bound, and match a
domain-separated length-delimited SHA-256 digest. Unknown or missing fields,
stale candidate identities, noncanonical bytes, invalid digests, duplicate
identities, and incomplete source/export joins fail closed.

Each result starts from the ordinary exact candidate analysis-coverage report.
It preserves all source, Project, Workspace, graph, manifest, authority, and
other area rows. Only the area owned by the declaration changes to `partial`.
The embedded declaration retains its canonical bytes and digest so another
reader can replay the exact caller statement.

These are library-only surfaces at this source head. They add no Project Agent
Transport v5 method, schema catalogue entry, generated client method, MCP tool,
parallel read, or editor command. The existing deployment-contract protocol is
a separate attachment and does not expose either declaration below.

## Generated-file provenance

`ProjectCandidate::analysis_generated_file_provenance_evidence` accepts
`semaprax.project-candidate-generated-file-provenance-declaration.v1`, bounded
to 65,536 bytes. Its closed root contains only `schema`,
`candidate_revision`, and `files`. The nonempty file inventory has at most 64
rows and is strictly ordered by artifact path.

Each row contains three closed identities:

- `artifact`: retained path, byte count, and SHA-256;
- `source`: the same retained path plus its exact source revision and digest;
- `generator`: one bounded opaque token and one canonical SHA-256 digest.

Artifact path, length, and digest must equal one retained candidate source.
Repeated generator tokens must carry the same digest. A generator token is not
a path, command, URL, package coordinate, process selector, or executable
capability. The attachment changes only `generated_file_provenance` and its
blind-spot row to `partial`.

The declaration does not prove generator inputs, execution, reproducibility,
freshness, ownership, unlisted outputs, the current filesystem, a generated
artifact, a deployment, or runtime behavior. It performs no scan, generation,
materialization, or publication.

## External API contract

`ProjectCandidate::analysis_external_api_contract_evidence` accepts
`semaprax.project-candidate-external-api-contract-declaration.v1`, bounded to
131,072 bytes. Its closed root contains only `schema`,
`candidate_revision`, `scope`, and `operations`. `scope` contains only `kind`:

- `manifest_exports` requires the complete ordered manifest export inventory;
- `explicit_stable_exports` accepts a nonempty, strictly ordered subset.

Every selected identity must be an explicit retained function in the exact
manifest export set. Each operation row contains only `export_id`, one
canonical `operation_digest`, and one canonical `schema_digest`. There is no
field for an endpoint, URL, provider, locator, secret, credential, version,
route, transport, or runtime value. The attachment changes only
`external_api_behavior` and the matching combined external/deployed-runtime
blind-spot row to `partial`; runtime observation remains absent.

The digests are caller-declared comparison facts. They are not provider
authentication, availability, compatibility, network behavior, remote side
effects, version selection, or runtime conformance evidence. The method opens
no network or process and grants no ambient, filesystem, publication, or
deployment authority.

## Bounds and diagnostics

Generated-file declarations use `SPX-G430` for invalid shape, `SPX-G431` for
capacity, and `SPX-G432` for exact-binding failures. External API declarations
use `SPX-G433`, `SPX-G434`, and `SPX-G435` for the same three classes. Both
evidence reports are capped at 2 MiB. Facts are rejected rather than truncated
or repaired to fit.

## Authored evidence

Library unit regressions cover exact success and the single-area `partial`
transition. Generated-file cases reject unknown generator fields, locator-like
generator identities, stale source bindings, and declaration digest changes.
External API cases cover complete manifest scope, an explicit stable subset,
incomplete manifest coverage, unknown exports, and an attempted URL field.

The regressions are authored and unrun. No test, target, provider, network,
runtime, filesystem, protocol, generated-client, MCP, editor, or quality-gate
evidence is claimed.
