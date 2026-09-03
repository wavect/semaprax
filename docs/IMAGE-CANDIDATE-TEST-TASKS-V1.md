# Candidate test tasks v1

- Status: implemented; focused project and compile evidence recorded locally
- Protocol: additive Semaprax image-agent v5 methods
- Audience: embedding hosts, MCP/editor clients, and compiler contributors

This contract turns the existing host-granted candidate reference-interpreter
test into one bounded asynchronous task per v5 session. It adds scheduling and
cooperative cancellation without adding source, process, network, target,
artifact, or publication authority. Canonical `.spx` source remains authoritative.

## Selected surface

The four methods exist only when the host selected `candidate_prepare` and a
fixed `CandidateTestPolicy` before session construction:

| Method | Effect |
| --- | --- |
| `candidate/test-task-start` | Authenticates held source, resolves one exact retained candidate, independently replays it in a worker, and returns a session-scoped task digest in `queued` state. |
| `candidate/test-task-status` | Authenticates held source, releases queued execution on first poll, observes `running`, `completed`, `cancelled`, or `failed`, and never embeds the report. |
| `candidate/test-task-cancel` | Authenticates held source, sets one monotonic cancellation flag, releases a queued worker, and returns the sticky terminal race winner. |
| `candidate/test-task-result` | Authenticates held source and returns a UTF-8-safe bounded chunk of the unchanged canonical candidate-test report only after `completed`. |

The task digest binds the starting image and candidate. A session admits only
one task, including after it becomes terminal. A successful refresh clears the
handle; a new task requires the refreshed or a new session. Requests cannot
change fuel, execution bytes, report bytes, worker count, or cancellation mode.

## Execution and cancellation

`ProjectCandidate::execute_tests_cancellable` shares exact candidate replay,
test-plan construction, report construction, and final legacy `ProjectExecution`
rendering with `execute_tests`. The cancellable path uses the prepared evaluator
on its fixed-stack thread with zero retained trace events. An uncancelled run must produce the
same execution envelope, report bytes, and report digest as the synchronous path.

Cancellation is observed only at evaluator step boundaries. Immediate cancel is
deterministic: start leaves the worker behind a one-shot queue gate, cancel sets
the token before releasing it, and the evaluator returns `before_step: 1` with
zero steps. A task already running can complete before observing cancellation;
the first terminal outcome is sticky. Cancellation emits no test report and
cannot make a candidate pass.

This contract claims no wall-time preemption, deadline, fairness, resource
isolation, native/Wasm execution, debugger control, or operating-system process
termination.

## Source invalidation and lifetime

Start, status, cancel, and result release all use the ordinary held-source
authentication boundary. While a task handle exists, drift observed by any
non-refresh request cancels and joins the worker, invalidates the task, and makes
the session terminal. Late worker completion cannot release a report. Successful
refresh, stream finish, and session drop also cancel and join. Refresh preview
does not adopt state; a later task operation still authenticates the original
held subject and fails closed after drift.

Every success payload binds image, project, candidate, and task revisions and
contains `source_authority: false`, a closed all-false authority object, and the
six still-uninspected runtime/deployment/generated/external blind spots. A
completed reference report may update only its existing bounded runtime evidence;
it does not inspect those blind spots.

## MCP and editor mapping

The selected v5 methods automatically become the ordinary MCP tools
`candidate__test-task-start`, `candidate__test-task-status`,
`candidate__test-task-cancel`, and `candidate__test-task-result`. This is a real
Semaprax task lifecycle over the pinned MCP stdio adapter. It is not the optional
MCP Tasks facility and does not implement `notifications/cancelled`.

The VS Code adapter consumes the same startup-selected tools through cancellable
progress. Editor cancellation invokes the compiler task cancel method; dirty
buffers, file/config changes, stop, and epoch changes reject late UI adoption.
The editor cannot enable the test grant and continues to exclude build and source
publication.

## Bounds and diagnostics

- one retained task per session and at most eight active tasks process-wide;
- each active task owns one scheduling thread and one fixed-stack evaluator
  thread while executing; the process-wide task cap bounds both populations;
- report chunks are 4 KiB through 512 KiB and the report retains its existing
  2 MiB host-policy ceiling;
- `SPX-G365` identifies stale/unknown handles, duplicate start, and invalid task
  lifecycle access;
- `SPX-G366` identifies worker spawn, panic, or disconnect failure;
- `SPX-G367` identifies result offset/capacity inconsistency.

The existing JSON-RPC and MCP frame bounds still apply. There is no retry,
deduplication, durable recovery, TTL, cross-session handle, or exactly-once claim.

## Focused evidence

The project-layer regressions pin uncancelled byte/digest/execution parity and
the zero-step immediate cancellation boundary. The v5 harness authors direct and
MCP start/cancel cases, sticky status, wrong lifecycle rejection, source
immutability, and drift withholding. The editor Node harness covers validated
status/result assembly, cancellation, and epoch invalidation. Focused compilation
is recorded for the v5 integration harness; broad quality, real Extension Host,
hosted/cross-platform, MCP conformance, and timing/economic measurements remain
open. The completion-matrix row therefore remains Partial.
