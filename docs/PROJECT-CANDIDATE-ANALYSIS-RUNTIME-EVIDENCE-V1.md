# Project Candidate Analysis Runtime Evidence v1

Status: additive implementation and regression sources authored, **unrun**.
This report is reference-interpreter evidence for one exact candidate test
attempt. It is not native, Wasm, deployment, environment, path-coverage,
external-service, consumer, or compatibility evidence.

Audience: agent authors, embedding hosts, and compiler contributors reviewing
an immutable Project candidate before publication.

The compiler can identify a declaration and still lack the runtime contract
that surrounds it. Candidate Analysis Coverage therefore keeps runtime as an
explicit blind spot. This additive report composes that exact coverage
inventory with one freshly produced `CandidateTestReport`. It changes
only the `runtime_environment` area from `not_inspected` to `partial`; every
other area and every nested input remains byte-for-byte attributable to its
existing evidence owner.

## Library API and selection

```rust
pub fn ProjectCandidate::analysis_runtime_evidence(
    &self,
    expected_candidate: &str,
    policy: &CandidateTestPolicy,
) -> Result<String, Vec<Diagnostic>>;
```

The method requires the exact candidate digest, obtains the existing
`analysis_coverage` report, then freshly calls `execute_tests` with the supplied
immutable host policy. The test owner independently replays the complete
candidate before execution. No serialized report, graph, HIR, execution
envelope, or caller-provided evidence is accepted as an input. Interpreter
failure remains evidence of an observed failed attempt rather than becoming a
successful gate.

The schema is
`semaprax.project-candidate-analysis-runtime-evidence.v1`. The final canonical
JSON is bounded by
`MAX_PROJECT_CANDIDATE_ANALYSIS_RUNTIME_EVIDENCE_BYTES`, exactly 4 MiB. The
method retains no candidate, test report, image, approval, or host authority.

## Exact composition

The report starts from the exact 19-field candidate coverage object. It retains
the candidate/base/image/Project/Workspace/graph bindings, manifest, sources,
inventory, external contracts, source authority, external I/O, candidate
retention, publication authority, and nonclaims without adding a nested or
alternative coverage projection. It then applies only these changes:

| Field | Meaning |
| --- | --- |
| `schema` | Replaced by the runtime-evidence schema above. |
| `evidence_class` | Replaced by `retained_source_and_bounded_reference_interpreter_evidence`. |
| `areas` | The same eight rows in the same order, with only `runtime_environment` replaced as specified below. |
| `execution` | `true`: this composed evidence contains one completed interpreter attempt. |
| `reference_interpreter_execution` | `true`. |
| `target_execution` | `false`: neither native nor Wasm target code ran. |
| `candidate_test_report_digest` | Added from the fresh `CandidateTestReport::report_digest()`, binding its exact canonical bytes including terminal LF. |
| `candidate_test_report` | Added as the complete parsed JSON value from the fresh `CandidateTestReport::to_json()`. |
| `nonclaims` | Retains every coverage nonclaim and appends the five runtime-specific nonclaims below. |

The derived `runtime_environment` row is:

```json
{
  "area": "runtime_environment",
  "status": "partial",
  "basis": "exact_candidate_replay_and_bounded_reference_interpreter_test_closure_attempt",
  "limitations": [
    "reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime",
    "one_manifest_declared_test_closure_is_not_dynamic_path_coverage",
    "no_trace_liveness_environment_configuration_or_drift_observation",
    "pass_is_not_full_quality_compatibility_external_api_or_deployment_proof",
    "nonpassing_outcome_is_one_bounded_attempt_not_complete_failure_classification"
  ],
  "required_evidence": [
    "native_and_wasm_runtime_conformance_bound_to_this_candidate",
    "authenticated_deployment_environment_and_external_provider_evidence",
    "dynamic_coverage_and_full_quality_profile_evidence"
  ]
}
```

No execution details are summarized into a second looser shape: the exact test
report already owns outcome, origin, policy, source inventory, diffs, execution
envelope, digests, fuel, and pass/fail semantics. `passed:false`, including a
returned failure or fuel exhaustion, is retained unchanged. It still makes the
runtime area partial because an attempt was observed; it never makes a gate
pass.

All seven non-runtime `areas` rows and every other retained coverage field,
apart from the explicitly appended nonclaims, must equal the separately
produced candidate coverage report exactly. In particular,
interpreter execution does not authenticate
deployment configuration, generator provenance, generated artifacts, external
API behavior, or external consumers. Declared external contracts retain only
their earlier declaration evidence.

## Diagnostics, bounds, and nonclaims

Unexpected compiler-owned nested JSON/schema, evidence class, canonical area
inventory, candidate/base/Project/Workspace/graph binding, source join, test
origin, policy, execution, or test-report shape uses `SPX-G361`. Final
construction or canonical output above 4 MiB uses `SPX-G362`; it fails rather
than truncating a nested report or an area row. Malformed or stale caller
candidate selectors retain `SPX-G222`/`SPX-G224`; coverage and test execution
diagnostics likewise retain their existing owners.

The root retains every coverage nonclaim and appends exactly:

```text
reference_interpreter_only_not_native_wasm_generated_or_deployed_runtime
no_dynamic_path_coverage_trace_liveness_or_environment_drift_observation
one_declared_test_closure_is_not_full_quality_or_external_contract_proof
no_current_filesystem_deployment_external_provider_or_consumer_authentication
no_source_publication_target_process_network_or_external_io_authority
```

`execution:true` states only that the nested report owns a completed reference
interpreter attempt. It does not imply external I/O; the existing candidate
test policy grants none. A report digest is integrity evidence, not a secret,
capability, approval, or replay permission.

Focused regressions in
`tests/project_candidate_analysis_runtime_evidence_v1.rs` are authored but
unrun. They pin exact nested reports, the single-row coverage change, passing
and failing attempts, deterministic composition, selector mismatch rejection,
immutability, the exported 4-MiB cap with successful evidence below it, and
strict authority/nonclaim fields. No test,
compiler, interpreter, application, target, or external service ran while this
slice was authored.
