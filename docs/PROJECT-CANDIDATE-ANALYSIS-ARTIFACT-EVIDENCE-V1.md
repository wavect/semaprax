# Project Candidate Analysis Artifact Evidence v1

Status: additive library implementation and regression sources authored,
**unrun**. No completion-matrix promotion is claimed.

Audience: agents and compiler contributors reviewing one admitted candidate and
one selected pathless carrier before publication.

This report composes the exact [candidate analysis coverage](PROJECT-CANDIDATE-ANALYSIS-COVERAGE-V1.md)
inventory with a freshly and independently replayed [candidate artifact delta](PROJECT-CANDIDATE-ARTIFACT-DELTA-V1.md).
It changes only the `generated_artifacts` boundary from `not_inspected` to
`partial`. It does not turn pathless carrier evidence into generated-file
provenance, materialization, deployment, execution or consumer evidence.

## Library API and binding

```rust
pub fn ProjectCandidate::analysis_artifact_evidence(
    &self,
    expected_candidate: &str,
    kind: ImageArtifactKind,
) -> Result<String, Vec<Diagnostic>>;
```

`kind` is `Web`, `Npm`, `OpenApi` or `C`. The closed report schema is
`semaprax.project-candidate-analysis-artifact-evidence.v1`; output is bounded to
10 MiB. The method authenticates the exact candidate selector first, invokes
the existing candidate coverage and artifact-delta owners itself, and accepts
no serialized report supplied by a caller. Existing carrier/profile admission
and diagnostics propagate unchanged.

The 20 root fields are the exact 19 candidate-coverage fields, with `schema`
and `evidence_class` changed, plus `artifact_delta`. The composed evidence class
is `retained_source_and_verified_pathless_candidate_artifact_evidence`. The
nested value is the complete unchanged
`semaprax.project-candidate-artifact-delta.v1` report, including its base and
candidate projections, source bindings, files, exports, comparisons,
inventories and nonclaims.

Before composition the method checks candidate/base/project/image/graph/kind
bindings, false authority and execution flags, both projection evidence
classes, and exact path/source-revision/source-digest joins between coverage and
the selected candidate projection. A report cannot associate carrier hashes
from one candidate or kind with coverage from another.

## Boundary change

The fixed eight-area order remains canonical. Seven rows are byte-for-byte JSON
equal to candidate analysis coverage:

- `declared_source_inputs`
- `declared_external_contracts`
- `deployment_configuration`
- `generated_file_provenance`
- `external_api_behavior`
- `runtime_environment`
- `external_consumers`

Only `generated_artifacts` becomes `partial`, with basis
`independently_replayed_selected_pathless_candidate_artifact`. This means that
one selected Web, npm, OpenAPI or C carrier was rebuilt pathlessly for the exact
base and candidate revisions and compared using its owning report. Exact file
paths, lengths and SHA256 values, carrier-envelope bindings, selected exports
and authenticated source joins are evidence inside that projection.

The partial row expressly records that only the selected kind was inspected,
encoded file bodies are omitted from this composite, no filesystem
materialization/install/deployment/runtime execution occurred, and facts
outside the projection are not absence evidence. Zero selected files is not
evidence that another artifact kind or deployed artifact is absent. Closing the
boundary requires separately authorized materialization and deployment binding,
plus runtime and external-consumer conformance for the selected artifact.

`generated_file_provenance` stays `not_inspected`: a generated-looking path or
listed `.spx` source is checked source, not a generator receipt. Deployment
configuration stays `not_inspected`; unlisted deployment files are neither read
nor inferred. External API behavior, runtime environment and consumers stay
uninspected. The report makes no compatibility, package-install, publication,
test, compiler-execution or behavioral-equivalence claim.

## Determinism, authority and diagnostics

Composition retains no image or candidate and mutates no source. The same
candidate and kind produce the same compact no-terminal-LF JSON. All inherited
flags remain false: `source_authority`, `external_io`, `execution`,
`candidate_retained`, `publication_authority`, nested
`artifact_materialization` and nested `target_execution`.

Malformed and stale candidate selectors retain `SPX-G222` and `SPX-G224`.
Coverage, artifact, profile and carrier diagnostics propagate from their owning
operations. `SPX-G352` reports an internally inconsistent nested shape or
binding; `SPX-G353` reports final composite capacity overflow. Failure does not
substitute empty artifact evidence.

Authored, unrun regressions in
`tests/project_candidate_analysis_artifact_evidence_v1.rs` compare the complete
report with independently invoked coverage and delta owners, exercise changed
and unchanged Web evidence and admitted npm/OpenAPI/C carriers, check exact
source/hash/export joins, preserve the other seven boundaries, reject stale and
sibling selectors and unsupported carrier admission, ignore an unlisted
deployment file, and preserve source and candidate bytes. No test, compiler,
target, package manager or application executable was run for this tranche.
