# Prepared Project Revision Replacement v1

Status: additive implementation and regression evidence authored;
unrun and unpromoted.

Audience: compiler contributors, editor and agent integrators, and reviewers.

## Purpose and boundary

This additive library operation replaces the immutable Project subject used
by one existing [prepared interpreter](PROJECT-PREPARED-INTERPRETER-V1.md).
It reuses the worker thread, fixed stack, process-wide worker permit, and
original trace ceilings. It does not create a new worker or mutate either
Project revision.

```rust,ignore
prepared.replace_revision(expected_project_revision, candidate_revision)?;
```

The method takes `&self`, an expected current Project revision token as `&str`,
and an `Arc<ProjectRevision>`, and returns `Result<(), Vec<Diagnostic>>`.
The caller supplies an already admitted, immutable revision. This operation
does not discover, read, parse, or publish source files. Constructing a new
revision and authenticating live source inputs remain the caller's existing
Project responsibilities.

This is same-worker replacement, not incremental parsing, linking, or
per-function compilation. Candidate interpreter closure and origin indexes
are prepared again. No performance or latency improvement is claimed without
measurement; the structural guarantee is that replacement creates no worker.

## Transaction and stale-base rule

Execution and replacement share the same fail-fast operation admission.
Exactly one operation can be outstanding; there is no queued second request.
The admitted caller holds that guard until it receives a result or a terminal
worker error.

The expected token must be exactly `sha256:` followed by 64 lowercase
hexadecimal digits. Its size and spelling are checked before copying it into
the request. The worker compares it with its current subject's exact Project
revision token before preparing the candidate. A well-formed but stale token
rejects even if the supplied candidate is otherwise valid. Sequential
serialization alone is insufficient: this comparison prevents a delayed
editor request from silently replacing a newer subject.

The token identifies content, not a worker generation. After an intentional
`A → B → A` sequence, the expected token for A matches again. This operation
does not claim monotonic edit epochs or ABA detection.

On a matching base, the worker:

1. prepares and independently validates both candidate entry and test
   closures through the unchanged interpreter admission;
2. constructs and checks their combined source-origin index, including all
   existing node, byte, source-span, and minimum-report-fit bounds;
3. constructs one complete candidate state containing the revision and both
   closure indexes plus origin facts;
4. replaces the current state as one operation only after preparation
   succeeds; and
5. releases the old state and acknowledges the completed handoff.

A malformed token, stale base, or ordinary candidate-preparation rejection
leaves the old state unchanged. A subsequent old-state execution must produce
the same trace bytes for the same options and observed cancellation boundary.
There is no partially replaced entry/test pair or index from another subject.
Even an identical candidate is independently prepared; pointer or digest
equality is not a bypass for the candidate checks.

If preparation or handoff panics, or acknowledgement cannot be delivered, the
worker becomes terminal. A terminal error is not a rollback receipt: the
caller must not infer that the old state survived a possibly completed but
unacknowledged swap. No later execution or replacement can use that worker.
This terminates the local worker, not the embedding process.

## Evaluation and trace preservation

After successful replacement, both entry and test evaluation use only the
new state. Existing `semaprax.project-source-trace.v1` bytes bind the selected
Project/Workspace revisions, graph digest, source facts, and expression
origins exactly as before. No new trace schema, status, event, digest domain,
or replacement receipt is introduced.

Old traces remain replayable against the old immutable revision. They do not
become valid against a different revision merely because the worker was
reused; new traces likewise reject against the old revision. The ordinary
Project and interpreter routes remain unchanged. Replacement does not opt
into internal String interpretation or widen the prepared admission profile.

Evaluation fuel, depth refusal, cooperative cancellation, trace truncation,
and original worker ceilings are unchanged. Replacement does not reset or
consume a cancellation handle, and it does not carry one of its own.

## Bounds and errors

Each accepted candidate is subject to the existing combined entry/test bounds
of 262,144 origin nodes and 16 MiB of identity/source-index content. At the
handoff, the worker may retain the old state and one fully admitted candidate.
During construction, however, the unchanged preparation routine first builds
the entry and test indexes under their individual bounds, then checks the
combined indexes and source-origin facts. Its intermediate workspace is
additional to the retained old state; replacement does not impose a new
32 MiB construction or peak-heap ceiling. The accepted-content bounds are not
allocator-specific byte accounting. No cache of previous revisions is
introduced. Retention by other callers' `Arc` values is outside the worker.

The worker still owns its original 64 MiB stack and one of the eight
process-wide worker permits. Replacement does not acquire another permit.
Preparation is bounded by structural admission, not evaluation fuel; it has
no cancellation points or hard wall-clock deadline. Request rendezvous and
worker shutdown retain their existing synchronous behavior.

| Rejection | Diagnostic |
| --- | --- |
| Malformed or stale expected revision | `SPX-F108` |
| Candidate closure/origin preparation rejected | `SPX-F107` |
| Concurrent operation, closed worker, panic, or lost response | `SPX-F109` |

Existing evaluation and trace-rendering diagnostics remain unchanged.

## Evidence and nonclaims

Authored evidence covers distinct revisions with changed entry and
test behavior; exact legacy/prepared outcome and fuel agreement; same-worker
identity and permit retention; stale and malformed tokens; rejected candidate
followed by byte-identical old execution; source/trace cross-pair rejection;
repeated and same-candidate replacement; preserved cancellation, truncation,
and worker ceilings; shared concurrent-operation rejection; and terminal
panic/disconnect during handoff.

Behavioral fixtures live in
`src/project/prepared_interpreter/tests/replacement.rs`; worker-identity,
concurrency, and injected terminal-fault fixtures live in
`src/project/prepared_interpreter/worker/tests.rs`. All physical prepared-worker
unit fixtures share the existing process-wide test serialization guard.

```sh
cargo test --locked -p semaprax --lib project::prepared_interpreter
cargo test --locked -p semaprax --test interpreter_v1
```

These are required gates, not executed results. The implementation must not
claim hosted support, production readiness, dynamic-path replay, debugger
support, general incremental compilation, or a memory sandbox from static
review or authored tests.

This operation grants no filesystem refresh, network, clock, backend,
publication, persistent cache, transport, or mutation authority. A caller using
a live Project snapshot must continue to obey its existing held-input
authentication protocol. Worker replacement cannot extend that authority.
