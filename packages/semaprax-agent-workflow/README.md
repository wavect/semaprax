# `@semaprax/agent-workflow`

This package orchestrates the one bounded SEMAPRAX
`function_signature_review_publish_v1` workflow. It accepts an already generated
v5 codec and a host transport. It does not open files, run processes, use the
network, inspect Git, hold secrets, create approvals, or enlarge the codec's
capabilities.

```js
import { runReview, runPublish } from '@semaprax/agent-workflow';

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

The two transports must have different nonempty `sessionId` values. `runReview`
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
