# Project Candidate Blind-Spot Declarations v1

Status: library, Project Agent Transport v5, generated-client, MCP-catalogue,
and regression sources authored, **unrun**. The individual generated-file and
external-API attachments are descriptive caller declarations. A separate
bundle composes them with the existing deployment-contract declaration. These
surfaces do not establish observed, current, or conformant external state.

Audience: agent authors and compiler contributors attaching bounded facts to
one exact admitted candidate.

## Shared boundary

Each API authenticates the expected candidate before accepting a declaration.
The declaration must be canonical JSON, fit its fixed byte bound, and match a
domain-separated length-delimited SHA-256 digest. Unknown or missing fields,
stale candidate identities, noncanonical bytes, invalid digests, duplicate
identities, and incomplete source/export joins fail closed.

Each result starts from the ordinary exact candidate analysis-coverage report.
It preserves all source, Project, Workspace, graph, manifest, authority, and
other area rows. Only the area owned by the declaration changes to `partial`.
The embedded declaration retains its canonical bytes and digest so another
reader can replay the exact caller statement.

Project Agent Transport v5 exposes both individual attachments and
`candidate/analysis-boundary-bundle` as `candidate_prepare` queries with exact
image/candidate selectors, canonical declaration or bundle text, and its
digest. Responses regenerate the report and return bounded UTF-8 chunks with a
stable whole-report digest. Their closed chunk schemas feed the existing
TypeScript, Python and Rust client generator and MCP catalogue. The methods are
not admitted to parallel reads and add no editor command. These protocol and
client surfaces are authored/unrun at this source head.

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

## Combined analysis-boundary bundle

`ProjectCandidate::analysis_boundary_bundle` accepts
`semaprax.project-candidate-analysis-boundary-bundle.v1`, bounded to 24,576
bytes. Its closed root contains only `schema`, `candidate_revision`, and the
three children `deployment_contract`, `generated_file_provenance`, and
`external_api_contract`. Each child contains only its canonical `declaration`
text and `declaration_digest`.

The bundle has its own domain-separated digest, then independently regenerates
all three child reports through their owning candidate attachment methods. It
replays their exact candidate, source, manifest, export, declaration, and
digest bindings before composition. The output preserves every base coverage
fact and advances exactly `deployment_configuration`,
`generated_file_provenance`, and `external_api_behavior` to `partial`. It
retains the canonical bundle and all child digests in
`semaprax.project-candidate-analysis-boundary-bundle-report.v1`; it does not
promote any area to `known`.

The bundle performs no filesystem scan, generator execution, artifact
materialization, network or provider observation, runtime observation, or
conformance check. It grants no source, ambient, publication, deployment, or
other execution authority. The v5 method returns only bounded report chunks;
it does not make the three declarations observation or deployment evidence.

## Bounds and diagnostics

Generated-file declarations use `SPX-G430` for invalid shape, `SPX-G431` for
capacity, and `SPX-G432` for exact-binding failures. External API declarations
use `SPX-G433`, `SPX-G434`, and `SPX-G435` for the same three classes. Both
evidence reports are capped at 2 MiB. Bundle validation uses `SPX-G440`,
`SPX-G441`, and `SPX-G442` for invalid shape, capacity, and binding failures;
its report is also capped at 2 MiB. The v5 bundle chunk route rejects invalid
UTF-8 offsets or chunk bounds with `SPX-G444` and report-capacity or
non-progress cases with `SPX-G445`. Facts are rejected rather than truncated
or repaired to fit.

## Authored evidence

Library unit regressions cover exact success and the single-area `partial`
transition. Generated-file cases reject unknown generator fields, locator-like
generator identities, stale source bindings, and declaration digest changes.
External API cases cover complete manifest scope, an explicit stable subset,
incomplete manifest coverage, unknown exports, and an attempted URL field.
Bundle cases cover exact three-child composition, independent child digest
replay, stale candidates, child tampering, missing or extra fields, area and
blind-spot preservation, closed v5 schemas, generated clients, MCP discovery,
and candidate-only admission.

The regressions are authored and unrun. No test, generated-client, MCP, target,
provider, network, runtime, filesystem, editor, or quality-gate execution
evidence is claimed.
