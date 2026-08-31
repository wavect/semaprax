# Project Candidate Analysis Coverage v1

Status: additive implementation and regression sources authored, **unrun**.
No completeness percentage, external-input ingestion, target execution, or
completion-matrix promotion is claimed.

Audience: agent authors, embedding hosts, and compiler contributors reviewing
an immutable Project candidate before publication.

The image-level [Analysis Coverage v1](SEMANTIC-IMAGE-ANALYSIS-COVERAGE-V1.md)
describes retained source facts and the boundaries it does not inspect. This
candidate projection applies that same collector to the exact fully admitted
final revision of one `ProjectCandidate`. It lets review observe a proposed
source and inventory change without treating the current base image as evidence
about candidate source.

This is a final-candidate inventory, not a before/after coverage delta. A source
change cannot turn an uninspected deployment, generator, provider, runtime or
consumer into verified evidence merely because the corresponding report row is
unchanged.

## Library and transport selection

The library API is:

```rust
pub fn ProjectCandidate::analysis_coverage(
    &self,
    expected_candidate: &str,
) -> Result<String, Vec<Diagnostic>>;
```

It returns schema `semaprax.project-candidate-analysis-coverage.v1`, bounded to
1,048,576 bytes. The exact candidate digest is authenticated before image
derivation. The implementation derives an invocation-local semantic image from
the candidate's already admitted retained `ProjectRevision`, invokes the
existing image coverage collector, and translates only the schema and
candidate wrapper fields. It retains no new candidate or image.

The selected v5 `candidate/analysis-coverage` route takes exact
`image_revision` and `candidate_revision`. It uses the existing
`candidate_prepare` grant and is an authenticated pure candidate read, including
the detached parallel-read path. Default read-only sessions do not gain the
method. The route performs no build, test, artifact, refresh, source write,
commit or publication action. Live source authentication remains the host
boundary; the standalone library report continues to describe immutable
retained source if the checkout later drifts.

## Exact report meaning

The 19-field closed report preserves these image-coverage fields:

```text
schema, image_revision, project_revision, workspace_revision,
project_graph_digest, manifest, sources, inventory, external_contracts,
areas, source_authority, external_io, execution, evidence_class, nonclaims
```

Only `schema` changes from the image schema. The remaining image fields are
the exact JSON values from the independently derived candidate image.
The wrapper adds:

| Field | Meaning |
| --- | --- |
| `candidate_revision` | Exact selected candidate digest. |
| `base_project_revision` | Original base Project revision; ancestry only. |
| `candidate_retained` | Always `false`; the query creates no new retained candidate. |
| `publication_authority` | Always `false`. |

`project_revision`, `workspace_revision`, `image_revision`, graph digest and
per-source bindings describe the final candidate revision. Candidate operations
preserve manifest source membership and rebuild a complete admitted Project
revision; this report does not claim manifest mutation or discover unlisted
files. Agents needing a comparison must retain a separately bound base image
report and compare the two as descriptive inventories.

The fixed eight `areas` rows keep the image contract unchanged:

| Area | Candidate interpretation |
| --- | --- |
| `declared_source_inputs` | Exact retained manifest-listed candidate sources are `known` within Project admission. |
| `declared_external_contracts` | Declared interface imports are `partial`; no imports remains `not_inspected`, never an absence proof. |
| `deployment_configuration` | `not_inspected`; source capabilities and deployment-looking names are not deployed state. |
| `generated_file_provenance` | `not_inspected`; a listed `generated.spx` is checked source, not authenticated generator output. |
| `generated_artifacts` | `not_inspected`; no artifact is generated or replayed by this query. |
| `external_api_behavior` | `not_inspected`; declarations and source-local lookalikes do not verify a provider. |
| `runtime_environment` | `not_inspected`; no path, liveness, environment or execution observation occurs. |
| `external_consumers` | `not_inspected`; exports and graph edges do not enumerate installed clients. |

Current Graph admission rejects Native Rust interface imports with `SPX-G218`
before a candidate coverage report can exist. This query does not reinterpret
that rejected source as partial evidence. An admitted non-native interface
import remains declaration evidence only; a source-local function with a
matching name or signature is not joined to it as provider implementation
evidence.

The separate [Candidate Analysis Evidence
v1](PROJECT-CANDIDATE-ANALYSIS-EVIDENCE-V1.md) can attach one explicit,
independently replayed candidate-era package-consumer corpus. It changes only
`external_consumers` to `partial` for that bounded corpus; the other seven area
rows and this report's no-external-input contract remain unchanged.

The separate [Candidate Analysis Artifact Evidence
v1](PROJECT-CANDIDATE-ANALYSIS-ARTIFACT-EVIDENCE-V1.md) can instead attach one
independently replayed pathless Web, npm, OpenAPI or C delta. It changes only
`generated_artifacts` to `partial` for that selected carrier; generator
provenance, deployment, runtime and consumer rows remain unchanged.

## Bounds, diagnostics and authority

The image collector retains its 16-source, 65,536-fact, conservative
construction and 1 MiB report bounds. Candidate wrapper rendering uses the same
1 MiB final limit and fails instead of dropping rows. `SPX-G222` and `SPX-G224`
own malformed and stale candidate selectors. Image derivation and collector
diagnostics propagate unchanged; invalid internal coverage shape uses
`SPX-G219`, and final wrapper capacity uses `SPX-G220`.

`source_authority`, `external_io`, `execution`, `candidate_retained` and
`publication_authority` are false. The inherited nonclaims continue to reject a
completeness percentage, current-filesystem or deployment authentication,
absence proofs for undeclared systems, generator provenance, external provider
conformance and new host authority. A report is descriptive evidence bound to
one immutable candidate, not permission to fetch missing evidence.

Authored, unrun library regressions in
`tests/project_candidate_analysis_coverage_v1.rs` compare the complete wrapper
with an independently derived candidate image, observe a changed generated-
named source and introduced function, preserve all eight blind-spot statuses,
keep non-native imports partial and provider-like functions separate, preserve
the Native Rust `SPX-G218` boundary, reject sibling/stale selectors, ignore an
unlisted deployment input, and prove source/candidate immutability. Transport
coverage separately owns selected grants, closed schemas, live authentication
and parallel-read parity. No tests, compiler, target, external service or
application were run while authoring this tranche.
