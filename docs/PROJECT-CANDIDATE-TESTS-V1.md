# Candidate Test Selection and Execution v1

Audience: agent builders, compiler contributors, embedding-host authors, and reviewers.

Status: implementation and regressions authored, unrun. The user explicitly
requested no local tests, interpreter execution, compiler runs, or long quality
gates. This specification does not claim an observed passing execution or a
completed graph-operational programme requirement.

This additive library route distinguishes a static affected-test plan from an
explicit request to execute the complete manifest-declared test closure. It
uses the existing [Project execution](PROJECT-MANIFEST-V1.md) interpreter,
with no new filesystem, process, target, network, trace, or publication
authority. Existing candidate construction, image serialization, and legacy
Project execution bytes are unchanged.

## Library API

```rust
CandidateTestPolicy::new(max_steps, max_execution_bytes, max_report_bytes)
    -> Result<CandidateTestPolicy, Vec<Diagnostic>>
ProjectCandidate::test_plan(expected_candidate_digest)
    -> Result<String, Vec<Diagnostic>>
ProjectCandidate::execute_tests(expected_candidate_digest, &host_policy)
    -> Result<CandidateTestReport, Vec<Diagnostic>>
```

Policy fields are private. The embedding host constructs the policy once;
requests select only an already retained candidate and cannot override the
host's options. Getters disclose `max_steps`, `max_execution_bytes`, and
`max_report_bytes`. There is no policy default that silently grants execution.

| Limit | Admitted range |
| --- | --- |
| Interpreter fuel | 1–1,000,000 charged steps |
| Existing execution envelope | 1,024–65,536 bytes |
| Candidate execution report | 16,384–2,097,152 bytes |
| Static test plan | Fixed 65,536-byte ceiling |
| Callable/call/closure inventory | At most 65,536 each |
| Call depth | Existing fixed interpreter ceiling, 256 |
| Trace events/bytes | Zero; trace is disabled and not produced |

These are independent limits. Fuel bounds interpreter evaluation, not the
preceding source replay or artifact admission work. Existing source, candidate,
and Project bounds still apply. Report size is not a bound on total HIR memory.

`CandidateTestReport::to_json()` returns canonical report bytes with one
terminal LF. `report_digest()` binds every exact report byte. `execution()`
exposes the existing immutable `ProjectExecution`; `passed()` is true only
when the declared test root returned zero. Nonzero returns, normalized language
failures, fuel exhaustion, and call-depth exhaustion never become passing
reports. Interpreter admission/guard/output diagnostics produce no test report.

## Static relevance, not coverage

The plan schema is `semaprax.project-candidate-test-plan.v1`. Planning checks
the candidate digest and walks actual stable-ID calls from both the original
and candidate `test_program()` HIR. Each graph includes body calls and
pre/postcondition calls; traversal begins at the authenticated declared test
entry. Thus local helpers and cross-file providers are followed transitively,
while unused functions in an otherwise reachable module are not automatically
classified as relevant. Both revisions matter when a change removes an old
dependency. Conditional branches are statically included without pretending
they were exercised.

Callable edits contribute their target identities. Function creation contributes
the new identity instead of its unchanged placement anchor; extraction
contributes both the edited function and introduced helper. Declaration moves
conservatively select the test closure because origins and module bindings
change. Record-field changes conservatively select it because callable-only
dependency traversal does not model every structural type fact. Unclassified
future changes and opaque reachable call dependencies also conservatively
select the closure. An unchanged candidate selects nothing.

The plan records `changed_targets`, base/candidate reachable changed targets,
reachable callable counts, conservative reasons, and one authenticated
`test_origin`. The origin binds module, stable entry ID, path, source revision,
and source digest. `selected_tests` contains that sole manifest-declared root
when selected; otherwise it is empty. `execution` is always `not_run` in the
plan. Selection does not discover filesystem tests, external consumers,
runtime coverage, or prove unselected code safe.

## Explicit replay and execution

`execute_tests` first checks the exact candidate digest and host policy. It then
calls ordinary `ProjectCandidate::replay` from the independently admitted,
retained original source revision and ordered intention history, requiring the
entire reconstructed candidate report to match exactly. It never treats
serialized HIR, a semantic image, or a prior test report as source authority.
The retained API does not assert current disk freshness; a filesystem-bound
host remains responsible for its usual held-input checks.

Only after successful replay does the API call the existing
`ProjectRevision::execute_test` with the fixed host options. The execution
origin must match the exact manifest test module and linked test entry ID.
Explicit execution always evaluates the complete declared test closure, even
when its static relevance plan is empty. This avoids presenting an empty
selection as a successful test run. No native/Wasm executable, external test
runner, dynamic test discovery, or prepared-interpreter trace is involved.

The report schema is `semaprax.project-candidate-test-report.v1`. It binds:

- candidate digest, original/current Project revisions, current Workspace
  revision, compiler package version, and explicit interpreter compatibility;
- complete base/candidate source digest inventories and their canonical digest;
- each changed source's existing exact source-diff digest and byte count, plus
  a digest of that ordered inventory;
- exact test origin, static plan digest, static selection flag, and explicit
  full-closure execution scope;
- every policy option, the fixed call-depth/disabled-trace settings, and the
  canonical options digest;
- the unmodified existing execution envelope as a JSON string, its exact byte
  digest, the canonical outcome digest, fuel accounting, and pass/fail status.

The report's `candidate_replay` field is
`exact_source_and_evidence_replay_before_execution`. `execution_scope` is
`complete_manifest_declared_test_closure`. `passed` concerns only this one
reference-interpreter execution. Reports explicitly disclaim native/Wasm
execution, full quality-gate success, dynamic coverage, trace production, and
source publication authority. Compiler package/compatibility facts are not a
compiler-binary identity claim.

Digest strings use `sha256:` plus lowercase SHA-256. Each hashes its literal
domain (including terminal NUL), little-endian `u64` byte length, and exact
payload bytes. Candidate-test domains are
`semaprax.candidate-test.<part>.v1\0`, where `<part>` is `report`, `plan`,
`sources`, `diffs`, `options`, `execution-envelope`, or `outcome`. Report, plan,
source/diff inventory, options, and outcome payloads are recursively key-sorted
compact JSON with terminal LF. The execution-envelope digest instead hashes
the exact existing envelope bytes as returned, without reformatting. Individual
source diffs retain `semaprax.candidate.source-diff.v1\0` and the candidate's
existing exact diff bytes. The final report digest is returned by
`report_digest()` rather than embedded recursively in the report itself.

Oversized provenance is rejected before interpreter fuel is spent. The final
execution envelope may still exhaust the host's total report budget; that
returns a diagnostic with no truncated report. Such an error is not proof that
no in-memory evaluation occurred. It never changes source or grants success.
`SPX-G239` covers policy/retained-fact grammar and origin errors; `SPX-G240`
covers test-plan/report/inventory capacity. Stale candidate selectors keep
`SPX-G224`. Ordinary replay and interpreter diagnostics remain unchanged.

## Evidence and remaining boundaries

`src/project/candidate/testing.rs` owns selection and explicit execution.
Five authored, unrun regressions in `tests/project_candidate/testing.rs`
cover transitive local/imported calls, unused-function exclusion, move fallback,
full execution despite empty selection, independent replay and exact report
digests, deterministic outcomes, immutable source files, nonzero failure, fuel
exhaustion, policy bounds, and stale selectors. The parent protocol's separate
opt-in test profile owns transport authorization and request-level bounds.

General test suites, dynamic coverage, backend runtime equivalence, test-result
reuse, execution caching, trace evidence, cancellation/deadlines, and full
quality-gate success remain outside this slice. A test report is descriptive
evidence, not publication authority or a completed product gate.
