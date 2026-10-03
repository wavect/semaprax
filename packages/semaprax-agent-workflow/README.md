# `@semaprax/agent-workflow`

This package orchestrates the one bounded SEMAPRAX
`function_signature_review_publish_v1` workflow. It accepts an already generated
v5 codec and a host transport. It does not open files, run processes, use the
network, inspect Git, hold secrets, create approvals, or enlarge the codec's
capabilities.

```js
import { connectMcpWorkflowTransport, ToolPayloadObserver, runReview, runPublish } from '@semaprax/agent-workflow';

const observer = new ToolPayloadObserver({
  sessionId: reviewMcpWire.sessionId,
  measureText: { tokenizer: 'local-tokenizer', fingerprint: 'sha256:...', measureText },
});
const reviewTransport = await connectMcpWorkflowTransport(reviewMcpWire, observer);
const publishTransport = await connectMcpWorkflowTransport(publishMcpWire);

const review = await runReview(codec, reviewTransport, {
  target: 'calculator.add',
  parameters: [
    { from: 'right', name: 'rhs' },
    { from: 'left', name: 'lhs' },
    { name: 'offset', type: 'i64', argument: { kind: 'i64', value: 0 } },
  ],
  classifyFailure,
});
if (review.status !== 'ready') throw new Error(review.failure.kind);

const inspectPublication = Object.assign(
  async ({ receipt, reportRevision }) => hostChecksFixedRefAndPreparedCommit(receipt, reportRevision),
  { classifyFailure },
);
const published = await runPublish(codec, publishTransport, review.handoff, inspectPublication);
```

`connectMcpWorkflowTransport` initializes the pinned MCP 2025-11-25 lifecycle,
sends the initialized notification, maps each generated v5 method to its exact
Semaprax tool name, validates the one-text-item result, and restores the inner
codec correlation ID. Its caller-owned wire supplies `exchange` for requests
and `notify` for the response-free initialized notification. The adapter does
not list, infer, or enlarge tools; an unavailable host-selected tool fails at
the real MCP boundary.

## Optional tool-payload observation

`ToolPayloadObserver` is an opt-in, host-neutral collector for
`semaprax.token-observation.v1` metadata. For MCP it observes exactly the
decoded `content[0].text` before the SDK restores the inner correlation ID.
`observeDirectWorkflowTransport` provides the corresponding wrapper for an
already connected direct v5 transport. These are separate `boundary` values;
outer MCP framing and normalized direct-v5 responses must never be combined.

The host supplies an opaque session ID and may supply a local `measureText`
adapter with its tokenizer identity and fingerprint. Without one, events still
carry exact UTF-8 byte counts and SHA-256 digests, while `tokens` is null. The
package retains no payload text, does no I/O, does not invoke a model, and does
not start a tokenizer. `await observer.drain()` before exporting immutable
events; `summary()` exposes dropped-event state so a capped session is visibly
partial. A callback may nominate a same-selected-JSON reference or explicitly
attested caller-context count. Missing references remain `baseline_unavailable`.

These numbers are **tool-payload tokens**. A paired group describes a payload
reduction versus the named reference for the selected tokenizer and boundary.
They are not evidence that a provider read the payload, provider billing,
prompt replay frequency, cached pricing, output-token reduction, task
equivalence, or money saved.

The opt-in selected-law Project v7 MCP profile uses this existing transport
adapter and observer with `law/status` and `law/check` (the MCP tool names are
`law__status` and `law__check`). Its tool text carries the complete candidate-
bound law result, including strict verdict, required count, and proof attempt.
The observer's `success` means a well-formed tool payload was delivered; a
failed law still has `validity.accepted: false` and `view.accepted: false` and
is never a success reward. The selected-law v2 result adds local process/query
reservation counts and explicitly unavailable provider cost. Tool-payload
tokens remain the observer's separate, existing measurement; neither field
asserts model consumption or provider billing.
`runReview` and `runPublish` remain scoped to the separate image v5 workflow.
`node scripts/test-law12-selected-mcp.mjs` checks the v7 method mapping and
one observation per delivered tool text without starting a second telemetry
stream; the installed Z3 Workspace gate exercises the physical MCP server.

The review and publish transports must have different nonempty `sessionId`
values. `runReview`
uses exactly the thirteen methods frozen by the workflow, reconstructs bounded
UTF-8 chunks, requires a passing interpreter test report, and returns a
SHA-256-bound handoff with explicitly empty `compilerRepairOptions`. The caller
supplies a bounded stable target and a closed ordered mapping of existing and
new scalar parameters. This package does not claim that the compiler admits a
repair for a rejected signature change. Only a semantic review rejection
returns the non-executing workflow guidance
`transitionRepairOptions: ['start_new_review_with_different_intention']`.
That guidance starts a separate review; it is not a compiler repair, candidate
mutation, automatic retry, or authority grant. Every other transition has an
empty workflow guidance array.

`runPublish` replays the subject, reference, recovery capsule, validation, and
source review in a separate session. It obtains the exact approval revision
only from `source-commit/status`, invokes `candidate/commit` at most once, then
requires published status, the complete receipt, and the host's independent
inspection callback. Lost or malformed results after the commit invocation are
reported as `publication_uncertain`; callers must never blindly retry.

The handoff binds the review codec contract and review workflow profile
separately. Publication validates both retained bindings and records the
separate publish codec contract and publish workflow profile in the host
inspection input and successful result. Review and publication profiles are not
required or expected to have the same contract digest.

The required `classifyFailure` callback may classify only a structurally typed
application error into the workflow's closed events. Transport failures and
malformed responses bypass it and remain uncertainty. Failures expose one
closed `failure` union: typed application diagnostics, a typed workflow
transition message, or a transport/response variant whose `opaqueCause` is the
only untyped interior.

Every ready, published, or failed result also carries an immutable `transcript`.
Each attempted step binds its phase, workflow index, request ID, method, decoded
or failed outcome, and a validated copy of that step's selected-profile
`responseContract`. The contract carries the exact grants, effect, authority
flags, blind-spot ledger, and only permitted runtime-evidence update. It is
accountability metadata, not authority and not evidence that an uninspected
area was observed.

A successful package result is evidence about these protocol transitions only;
it does not establish
deployment configuration, generated artifact provenance, external API behavior,
external consumer compatibility, or general signature evolution.
