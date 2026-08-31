# Agent Task Economics v1

Status: integrated observation and regression source authored; unrun and
unmeasured. No productivity, latency, model-token or completion claim.

Audience: agent evaluators, workflow integrators, and compiler contributors.

The integrated
[graph-operational Git workflow](PROJECT-GRAPH-OPERATIONAL-GIT-WORKFLOW-V1.md)
records a bounded `semaprax.agent-task-economics.v1` observation while exercising
the workflow's twelve requirement criteria. This is separate from
[Agent Context Economics v1](AGENT-ECONOMICS-V1.md), which measures one-file
context selection. The task observation describes exact compiler protocol and
review traffic for one authored workflow; it is not a model or human study.

The recorder wraps the already configured in-process v5 session used by the
regression. Every semantic request still passes through the ordinary bounded
`VNextSession::handle_frame` dispatcher. The wrapper measures the exact
response-bearing request and response bytes and computes their digests before
parsing the response as JSON for the test. It does not add a protocol method,
grant, side channel or runtime
authority, and it cannot change a request or response.

## Recorded observations

Each row follows invocation order and records its session, method, associated
requirement criteria, request and response byte counts, repository lexical
units, SHA-256 digests and success/error outcome. Criterion associations are
logical rather than chronological: one signature operation is associated with
both signature change and caller migration without becoming two protocol calls.
The report aggregates distinct semantic protocol calls and an exact method
histogram. This fixed workflow has zero notifications; the recorder makes no
claim to capture response-less frames. Request/response counts are compiler
protocol traffic, never model tool calls.

The workflow additionally records:

- bounded review material sizes for the candidate report, semantic delta,
  impact and human-readable source differences;
- one disjoint sibling-candidate reconciliation and the deliberately scripted conflict
  or authority-control rejections;
- zero stale recoveries, because this scenario does not perform live-source
  refresh or semantic rebase;
- explicit validation, recovery replay, semantic-delta verification,
  interpreter-test and target-admission observations;
- twelve criteria rows only after the owning test assertions establish the
  corresponding snapshot, selection, migration, invariant, evidence,
  conflict and publication facts.

Scripted `SPX-G235`, `SPX-G286` and `SPX-G287` responses exercise control paths.
They are not observed agent mistakes. The accepted sibling merge is conflict
reconciliation, not stale recovery. Native-C11 and Core-Wasm rows count
compiler emission or structural admission; native and Wasm execution remain
zero.

All byte counts use exact UTF-8 lengths. Lexical units reuse
`semaprax.lexical-token.v1` and retain `model_tokens: false`. Ratios or savings
are not inferred from either measure. The pre-publication review traffic is
source-deterministic. Commit, commit-report and source-commit-status routes bind
the canonical temporary manifest and bare-repository identities, so their exact
digests are invocation evidence and carry `host_route_bound: true`. Portable
recovery traffic remains separately labeled. The regression checks schema,
limits, relationships and counter self-consistency; it does not freeze
invocation-specific byte, lexical, digest or method-count values as goldens.

## Explicitly unobserved fields

The report keeps model identity, tokenizer identity, model input/output tokens,
model or external agent tool calls, wall and CPU time, peak memory, monetary
cost, and human review duration null with status `not_observed`. Validation
invocation counts are not validation cost or elapsed time. Review-material bytes
are not review effort. A deterministic scripted success is not an agent success
rate.

A comparative productivity claim requires a separately captured immutable
observation bundle for both semantic and source-first workflows. It must bind
exact prompts and contexts, model and tokenizer versions, ordered external tool
traffic, corpus/compiler/source revisions, cold/warm state, correctness rubric,
validation timing method and human-review protocol. None of those external
observations is synthesized from compiler protocol bytes.

## Bounds and evidence

The fixed test supplies two finite sessions and their response-bearing frames.
Each request remains under the protocol's 64 KiB request limit, the session
retains its existing 1 MiB response limit, and the regression rejects a final
report over 256 KiB. The compact sorted-key JSON retains digests and counters
rather than becoming a second source or candidate archive. The report has
`source_authority: false`, `execution_authority: false`, and no publication
authority; the separately attached Git host remains the sole authority for the
scripted commit step.

The integrated SHA-1 and SHA-256 regressions are authored but unrun. No report
from the current head has been produced, no benchmark value is claimed, and the
graph-operational programme remains Partial.
