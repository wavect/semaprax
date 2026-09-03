# Supported graph-operational product workflow v1

Status: the closed workflow passed its clean exact-subject local evidence gate
at commit `3c605fe3055539a9a5f2bf83e98c8c2a521ff741`, bundle
`8e18e9dea2050844c554a826e9485394ef44381c24915602ff52952448862cfa`.
This qualifies only the fixture, profiles, clients, provider, and transitions
frozen here; full Phase 1 and the graph-operational programme remain **Partial**.

Audience: agent-client authors, embedding hosts, release engineers, and
programme reviewers.

This contract defines one closed product workflow named
`function_signature_review_publish_v1`. It composes existing v5 operations
without changing their schemas, authority, or failure semantics. Canonical
`.spx` remains repository authority. Its exact-subject gate passed for the
captured subject above; every later subject requires fresh evidence. Static
discovery exposes the available composition and exact profile binding but does
not embed or infer executed support from the presence of constituent APIs.

## Closed fixture and intention

The workflow selects the retained source function `calculator.add` in the
calculator Project. Its typed `change_function_signature` intention is exactly:

```json
{
  "kind": "change_function_signature",
  "target": "calculator.add",
  "parameters": [
    {"from": "right", "name": "rhs"},
    {"from": "left", "name": "lhs"},
    {"name": "offset", "type": "i64", "argument": {"kind": "i64", "value": 0}}
  ]
}
```

The compiler must migrate every authenticated fixture caller, preserve the
function's stable identity, and admit the complete candidate. This scalar
fixture does not establish general signature evolution, ownership-sensitive
migration, external callers, ABI compatibility, or behavioral equivalence.

The selected publication target is one host-created local Unix bare SHA-256 Git
repository, one fixed branch ref, and one exact expected base. The agent cannot
select or modify those values. Every language-client execution uses an isolated
repository and Project fixture; no run shares approval or publication state.

## Review session

The first v5 session has semantic reads, candidate preparation, and the fixed
interpreter test policy. It has no Git publication host or approval. The client
must perform these transitions in order and retain the exact returned bytes:

1. `workspace/open` returns the exact image, Project, and workspace revisions.
2. `image/function-reference-export` exports a function-only compact reference
   for `calculator.add`. `image/function-reference-resolve` must resolve it
   against the same image and freshly return the matching summary.
3. `image/analysis-coverage` records the base image's retained facts and all
   eight explicit evidence areas.
4. `candidate/open` creates the immutable root candidate.
5. `candidate/apply-intent` submits only the typed intention above and returns
   the changed candidate handle.
6. `candidate/validate` independently replays that complete candidate.
7. `candidate/semantic-delta` records and independently verifies the exact
   function-bound semantic change against the intended candidate.
8. `candidate/test-plan` records the compiler-selected fixture tests, then
   `candidate/test` executes the complete declared interpreter test closure
   under the startup policy. A nonpassing test report stops the workflow.
9. `candidate/source-review` is read to its terminal chunk. The client verifies
   offsets, total bytes, candidate binding, complete report digest, each source
   digest, and the human-readable source diff.
10. `candidate/analysis-coverage` records candidate-bound retained facts and the
   same explicit blind-spot vocabulary. It is descriptive evidence, not a
   validation or runtime report.
11. `candidate/recovery-export` is read to its terminal chunk. The reconstructed
    canonical `semaprax.project-candidate-recovery.v1` capsule, exact candidate
    digest, compact function reference, intention bytes, validation/delta/test
    results, source-review digest, and both analysis-coverage digests form the
    bounded handoff. The capsule must fit the ordinary v5 restore request bound;
    this workflow defines no implicit filesystem or archive fallback.

The review session then finishes with its ordinary final source authentication.
Raw Project source and the Git ref must remain unchanged. A report, capsule,
reference, digest, or successful test carries no approval or publication
authority.

## Separate approval and publication session

Outside the protocol, the embedding host reviews the exact handoff and may
grant one approval for the candidate digest. The host then creates a new v5
session bound at startup to the same manifest bytes, the fixed Git target, the
expected old ref, and that exact approval. There is no in-session approval RPC.

The generated client must:

1. call `workspace/open` and require the same original image and Project;
2. resolve the compact function reference against that fresh exact image;
3. call `candidate/recovery-restore` with the exact capsule and independently
   replay the full history, requiring the same candidate and Project digests;
4. repeat `candidate/validate` and reconstruct `candidate/source-review`,
   requiring byte-identical review and handoff digests;
5. call `source-commit/status` before publication and require `available`, the
   expected pending approval, no receipt, and no prior uncertainty;
6. call `candidate/commit` once with the exact candidate and approval revisions;
7. on success, call `source-commit/status`, require terminal `published`, and
   read `candidate/commit-report` to its terminal chunk using the returned
   report revision; and
8. independently inspect the real bare repository's fixed ref, commit, tree,
   parents, and committed Project source objects against the authenticated
   candidate and receipt. This inspection is evidence, not another commit.

The publish session never applies a new intention, repairs a candidate, changes
the target, or refreshes to a different base. Successful publication is
terminal and cannot be reused for a second commit.

## Generated clients

The selected discovery bundle must generate the closed request helpers and
response aliases used above for TypeScript, Python, and Rust. Each language has
an independently executed harness over its own two sessions and isolated real
Git fixture. The harness runs each generated codec in a bounded subprocess and
feeds its framed requests directly to the in-process v5 session; this is not an
MCP transport execution. Generated code performs no I/O or capability
selection itself.

Each harness must submit the typed intention through its public generated
request type, decode every closed response wrapper, preserve opaque chunk
interiors as exact UTF-8 bytes, and reject malformed or mismatched revisions.
Static compilation, type checking, or deterministic source equality alone is
not a workflow pass. One language's real publication cannot be transferred to
another language's result.

The generated clients also expose the closed workflow metadata and per-step
response contracts defined by [Supported workflow response accountability
v1](IMAGE-SUPPORTED-WORKFLOW-RESPONSE-ACCOUNTABILITY-V1.md). V5 application
failures preserve optional structured diagnostics through the generic
`decodeTyped`/`decode_typed` API; the existing method-specific typed success
decoders still use the generic error surface. Generic transport and grammar
failures remain explicitly unstructured. The earlier archived workflow run did
not exercise this later structured-error path. These types do not choose a
transition, apply a repair, perform I/O, or grant authority.

## Error transition policy

The evidence report records one of these closed terminal outcomes for each
language. It never infers success from process exit alone.

| Condition | Required transition |
| --- | --- |
| Malformed response, transport timeout, or lost response before publication | Retire the client session. The workflow is `transport_uncertain_no_publish_claim`; do not retry a mutation in that session. |
| Source/image/reference drift or failed final authentication | Invalidate every retained selector and handoff. The workflow is `stale_subject`; no approval or publication follows. |
| Intention, validation, test, review, coverage, or recovery failure | The workflow is `review_rejected`. No approval is requested. A different intention starts a new review workflow. |
| Restore, repeated validation, review-digest, status, candidate, base, or approval mismatch in the publication session | The workflow is `publish_precondition_rejected`. Do not invoke `candidate/commit`. |
| Definite pre-pivot commit failure | Approval is consumed. Record `publish_failed_pre_pivot`; a retry requires a newly configured and approved session. |
| `SPX-G267` or any lost/contradictory result after the real ref update may have occurred | The host and workflow become terminal `publication_uncertain`. Inspect the fixed ref and prepared commit; never retry blindly or report rollback. |
| Successful commit plus complete authenticated receipt and independent Git inspection | The workflow is `published`. No later receipt-query failure can authorize or repeat publication. |

Cancellation is not a transition in v1. The current MCP/stdio adapters are
synchronous and do not interrupt a running tool. Closing a UI chooser or
discarding a late editor result is not proof that the server operation stopped.

## Blind spots and nonclaims

Both analysis-coverage reports must retain every required evidence area.
`deployment_configuration`, `generated_file_provenance`, `generated_artifacts`,
`external_api_behavior`, `runtime_environment`, and `external_consumers` remain
`not_inspected` unless a separately authenticated evidence owner changes that
one area. Declared interface imports are at most partial declaration evidence.
Zero imports or graph edges never proves that no external service or caller
exists. Test success is not runtime, path, deployment, or provider coverage.

The workflow-level blind-spot ledger may describe `runtime_environment` as
`partial` only by citing the separately bound successful reference-interpreter
test report. That composite status means bounded interpreter execution only;
the base and candidate analysis-coverage payloads both continue to classify
the runtime environment as `not_inspected`. The workflow ledger also keeps
`analysis_completeness: partial`; retained compiler facts and a passing fixture
do not prove complete impact, behavior, or environment coverage.

The qualifying exact-subject local run did not establish:

- general signature changes, owned/resource migrations, dynamic or external
  callers, behavioral compatibility, or every operation class;
- deployment configuration, generated provenance, provider implementation,
  external API behavior, runtime environment, or installed consumer validity;
- native or Wasm runtime equivalence, filesystem publication, checkout
  visibility, remote Git hosting, physical crash/power-loss durability, or
  multi-writer atomicity;
- cancellation, request deduplication, automatic retry, exactly-once delivery,
  session durability, or recovery of approval/authority from the handoff;
- a packaged SDK, editor UI, MCP certification, network isolation, hosted or
  cross-platform support, an exact release tag, comparative economics, full
  quality, completion-matrix promotion, or programme completion.

## Executable evidence gate and record

The owning evidence tranche must bind one clean exact commit and tree; exact
`Cargo.toml` and `Cargo.lock` bytes; compiler and client-generator identity; the
selected host policies; all three generated artifacts and runtime tools; every
request/response transcript; handoff artifact; approval binding; real Git
object format, ref and object digests; committed source objects; receipt; raw
source before/after digests; and the closed per-language terminal outcome.

The gate executes three isolated successful workflows plus hostile cases for
stale references, source drift, failed tests, tampered recovery, wrong approval,
definite pre-pivot failure, injected result loss after a real ref update, and
malformed generated-client responses. All nonclaims above remain
machine-readable.

[Phase 1 product workflow execution evidence
v1](GRAPH-OPERATIONAL-PHASE1-PRODUCT-WORKFLOW-EXECUTION-EVIDENCE-V1.md)
records the clean exact local gate: all three isolated successful workflows and
all ten hostile transitions passed. Its tracked archive is evidence for the
captured subject only; it does not transfer the result to this later report
commit or weaken any nonclaim above.
