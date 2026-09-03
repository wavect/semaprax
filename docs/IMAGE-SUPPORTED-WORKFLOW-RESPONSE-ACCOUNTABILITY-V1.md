# Supported workflow response accountability v1

Status: implemented with focused local discovery and generated-client evidence;
the broader Phase 1 product surface remains **Partial**.

Audience: agent-client authors, embedding hosts, workflow reviewers, and
evidence-runner authors.

## Purpose

The supported `function_signature_review_publish_v1` workflow has one closed
ordered review phase and one separately authorized publication phase. Every
ordered step now carries a `response_contract` in the selected capabilities
document. The contract makes the response's required grant, effect, authority
boundary, and analysis-boundary treatment explicit before an agent submits the
request.

This is selected-profile accountability metadata. It does not add fields to
the frozen `semaprax.image-agent-result.v5` success envelope and must not be
described as self-description embedded in each wire response.

## Closed response contract

Each step binds:

- the exact method and payload schema;
- the required host-selected grant set;
- one effect: `read_only`, `candidate_overlay_mutation`,
  `bounded_test_execution`, `source_publication`, or `receipt_read`;
- whether the step can mutate the candidate overlay, execute the selected
  bounded test policy, or invoke the separately attached publication host;
- `request_capability_changes: false` and
  `evidence_or_handoff_grants_authority: false`;
- the workflow blind-spot ledger by closed reference; and
- the only permitted evidence update: a bound passing `candidate/test` report
  may describe `runtime_environment` as partial reference-interpreter evidence.

Every other step has a null permitted runtime update. No response contract may
change deployment, generated-file, generated-artifact, external-API, external-
consumer, native, Wasm, or hosted evidence.

The workflow also binds a domain-separated SHA-256 revision over the exact
selected protocol, complete method set, complete grant set, and host test
policy. This digest identifies the selected profile metadata; it is not a
capability, an execution result, a release identity, or authority.

## Generated clients

Generated TypeScript, Python, and Rust clients expose closed workflow, phase,
step, response-contract, authority, blind-spot, transition, outcome, and repair
types. Client initialization accepts only the immutable producer-embedded
catalogue and its separately emitted expected profile revision; client callers
do not supply workflow metadata. The producer independently reconstructs every
step contract and recomputes the domain-separated profile revision before it
emits any client. A helper then resolves an exact phase/index/method tuple and
rejects a mismatch. The helper does not execute the method or authenticate its
eventual response bytes; the ordinary method-bound decoder still owns that
check.

Application failures may carry the typed data owned by [Image agent
application error data v1](IMAGE-AGENT-APPLICATION-ERROR-DATA-V1.md). The host
still selects the workflow event from its known request and publication state,
then resolves the corresponding closed transition. A diagnostic code alone
cannot safely distinguish a definite pre-pivot failure from an uncertain
post-pivot outcome.

## Authority and blind spots

Read and review evidence never confers source or publication authority. The
recovery capsule and the response-accountability metadata transfer no approval.
Only `candidate/commit` in the separately configured publish session can invoke
the fixed publication host, and only with its startup-selected exact approval.

The accountability catalogue prevents a client from silently forgetting which
blind spots and authority restrictions belong to a step. It does not inspect a
deployment, generated files, external services, installed consumers, or a
runtime. Those areas remain governed by the workflow's exact blind-spot ledger
and independently authenticated evidence.

## Nonclaims

This profile is not a packaged SDK or a complete workflow driver. It provides
typed metadata and codecs without transport or filesystem I/O. It does not add
automatic orchestration, diagnostic-repair catalogue selection, cancellation,
parallel mutation, request deduplication, retries, session durability, MCP
conformance, editor UX, hosted/cross-platform evidence, or programme
completion.
