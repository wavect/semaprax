# Resumable effects continuation v1

Status: **local library contract, first slice.** Implemented for the already
admitted sequential Copy-scalar profile by
`src/resumable_effects/continuation.rs` and its `journal` submodule (Unix
only). It is not a CLI, service, hosted, or production runtime, and it adds no
language syntax.

Audience: compiler and runtime contributors, host integrators embedding the
library, and reviewers auditing issue #296.

Owner: kevin.riedl@wavect.io. Contract identity:
`semaprax.resumable-continuation.v1`; journal line schema:
`semaprax.resumable-journal.v1`. Any breaking change to either needs a new
identity. [Resumable Effects v1](RESUMABLE-EFFECTS-V1.md) still owns the source
profile, lowering, envelope, and backend-refusal rules this document builds on.

## 1. Profile

The contract admits exactly the functions that the v2 source checkpoint
envelope admits: an explicitly identified free function with **two to eight**
direct sequential top-level `yield` sites, Copy-scalar parameters, request,
answer, and result, and no ordinary effects. A one-site function is refused
with `UnsupportedProfile` because the sequential continuation carrier does not
exist for it. Admission, the budget check (`1..=MAX_STEPS_LIMIT` interpreter
steps per segment), and the pure start plan run **before** any file is
created, so a refused start leaves storage untouched.

Execution uses only the compiler-owned start and per-site resume plans through
`interpreter::resumable`. The driver adds no second evaluator: every resumed
segment replays the recorded prefix and checks each historical request bit for
bit, exactly as the in-memory lane does.

## 2. Identities

| Name | Definition |
| --- | --- |
| program digest | `SourceEffectSignature::plan_identity`, the sequential lowering identity of the exact checked program and selected function |
| invocation id | caller-chosen, 1..1024 bytes, no control characters |
| policy epoch | caller-chosen `u64` |
| site index | number of settled sites before this suspension (`0..yield_count`) |
| envelope | the v2 HMAC source checkpoint of the suspended continuation, scoped to `resumable-plan:sha256:<program digest>`, the invocation id and the policy epoch |
| envelope digest | SHA-256 of the exact envelope bytes |
| arguments digest | SHA-256 over the canonical scalar JSON of each argument, each followed by a newline |

## 3. Non-bearer request and answer

A `ContinuationRequest` is `{program digest, invocation id, site, envelope
digest, request scalar}`. A `ContinuationAnswer` carries the same four identity
fields and one scalar value. The request is a description; holding it, or the
envelope, or the journal, confers nothing.

`answer` accepts an answer only when all of the following hold, checked in this
order, and otherwise refuses with the named class and leaves the journal
byte-identical:

1. The invocation is awaiting an answer (`Dispatched` is the tail). A settled or
   terminal invocation refuses `AlreadySettled`; an undispatched site refuses
   `NotDispatched`, or `ReplayedAnswer` if the answer names an earlier site.
2. The host presents a `CapabilityPolicy` that allows the selected function's
   persistent id (`CapabilityDenied`).
3. The program digest matches (`ProgramMismatch`).
4. The invocation id matches (`InvocationMismatch`).
5. The site is the awaited one: an earlier site is `ReplayedAnswer`, any other
   is `SiteMismatch`.
6. The envelope digest is the awaited one (`StaleEnvelope`).
7. The value has the compiler-derived answer type (`AnswerTypeMismatch`).

Consequently an answer is single-use: once its `Answered` record is durable the
same answer can only ever be replayed, and a replay is refused both in the live
process and after recovery.

## 4. Host authority

The continuation carries no authority. The library never opens storage by
itself: the host opens one owner-private directory and passes it as a
`JournalDirectory`. The host supplies its HMAC key by reference and its
`CapabilityPolicy` at every `dispatch`, `answer`, and `abandon`. Decoding a
genuine envelope or journal dispatches nothing, answers nothing, and evaluates
nothing that is not already determined by durable records. The HMAC key only
authenticates bytes; it is never serialized and is zeroized on drop.

## 5. Settlement

Terminal outcomes are sticky: `Completed(result)` or `Failed(class)` with
classes `language_failure`, `fuel_exhausted`, `call_depth_exceeded`,
`evaluation_rejected`, `handler_failed`, `answer_type_mismatch` and
`host_abandoned`. After the terminal record, terminal cleanup runs through the
host's `CleanupHandler` at most once. A cleanup failure is recorded as its own
settlement (`failed`) and never replaces the outcome. The outcome is
**published** (`ContinuationStatus::Settled`) only after the cleanup settlement
is durable; before that the status is `CleanupPending` or `CleanupInDoubt`,
which carry at most the failure class and never a completed result.

## 6. Journal protocol

One journal file per invocation, named by a domain-separated SHA-256 of the
invocation id plus `.journal`, so no caller text becomes a path component.

Storage rules:

- The directory must be a directory owned by the effective user with no group
  or other permission bits; it is opened with `O_DIRECTORY | O_NOFOLLOW`.
- Journals are opened relative to that descriptor with `openat` and
  `O_NOFOLLOW`; a journal is created with `O_CREAT | O_EXCL` and mode `0600`,
  and the directory is flushed after creation (`F_FULLFSYNC` on Apple
  platforms, falling back to `fsync`). A second start of the same invocation
  refuses `AlreadyStarted`.
- Each open journal holds an exclusive non-blocking `flock` for the lifetime of
  its descriptor, taken before any byte is read. A second live instance, in
  this or another process and including a poisoned instance not yet dropped,
  refuses `JournalBusy`; this is the single-writer rule that keeps concurrent
  recoveries from dispatching or cleaning up twice.
- A journal must be a regular file owned by the effective user, mode without
  group or other bits, with exactly one link.
- Each record is one line passed to a single `write_all` on an `O_APPEND`
  descriptor (which may issue several `write` calls), then `sync_all`ed
  (`F_FULLFSYNC` on Apple platforms). The append is **acknowledged** only when
  both succeed; any later step may rely only on acknowledged records. Any
  error after an acknowledged append, from storage or from preparing the next
  record, poisons the in-memory invocation (`Poisoned`); only recovery may
  continue it.
- If `sync_all` fails after the write, the record may or may not be durable.
  The invocation is poisoned and recovery acts on whatever is observed. For
  `Dispatched` and `CleanupStarted` this is exactly the in-doubt case: the
  handler has not run, recovery sees either the previous tail (the step is
  retried, never having happened) or the new record (in doubt, never
  repeated).

Each line is canonical JSON
`{"mac","prev","record","schema","seq"}`, where `seq` counts from zero, `prev`
is `sha256:` of the previous line's bytes (all zero for the first), and `mac`
is `hmac-sha256:` over the domain `semaprax.resumable-journal-record.v1\0` and
the canonical JSON of the other four fields. A journal holds at most 28 records
and 512 KiB. `Started` also binds the per-segment step budget (`max_steps`),
so recovery under a different budget refuses `BudgetMismatch` instead of
reaching a different terminal outcome.

Records, in the only admitted order:

```text
Started{contract, function, program_digest, invocation_id, policy_epoch,
        arguments_digest, yield_count, max_steps}
( Yielded{k, envelope, envelope_digest}
  Dispatched{k, envelope_digest}
  Answered{k, envelope_digest, answer, answer_digest} )*    k = 0, 1, ... in order
then exactly one of:
  Completed{result} | Failed{language_failure | fuel_exhausted |
                             call_depth_exceeded | evaluation_rejected}
      -- after Started or Answered (a plan step)
  Yielded{k} Dispatched{k} Failed{handler_failed | answer_type_mismatch |
                                  host_abandoned}
      -- a dispatched site settled as a host failure
CleanupStarted
CleanupSettled{completed | failed | host_confirmed}
```

Any prefix of this sequence is a valid observed journal.

Ordering obligations of the writer:

- `Yielded` is durable before a request can be dispatched.
- `Dispatched` is durable before the request is returned to the host or passed
  to its handler.
- `Answered` is durable before the resume plan runs.
- The terminal record is durable before cleanup starts; `CleanupStarted` is
  durable before the cleanup handler runs.

## 7. Recovery

`recover` requires the same checked program, function, arguments, invocation
id, policy epoch, key and budget, reads the whole observed journal, and:

1. Verifies every complete line: exact field set, schema, sequence, chain,
   authentication, and byte-for-byte canonical form. Any failure is
   `TamperedJournal` (or `SchemaMismatch`); nothing is repaired or skipped.
2. Only after every complete line verifies, considers a final fragment with no
   trailing newline. Such bytes were never acknowledged. Under
   `TornTailPolicy::Refuse` recovery refuses `TornTail` and changes nothing;
   under `TornTailPolicy::TruncateUnacknowledged` it truncates exactly the
   bytes after the last newline and `fsync`s. A newline-terminated line is
   never truncated, whatever its content. A torn write that persisted later
   pages but not earlier ones can leave a newline-terminated but corrupt line:
   that fails closed as `TamperedJournal`, and v1 has no recovery path for it.
3. Compares `Started` with the caller's facts: `InvocationMismatch`,
   `FunctionMismatch`, `ProgramMismatch` (digest or yield count),
   `PolicyEpochMismatch`, `ArgumentsMismatch`, `BudgetMismatch`.
4. Replays records through the order above; out-of-order, wrong-site or
   wrong-digest records are `TamperedJournal`. Each `Yielded` envelope is
   decoded through the v2 envelope decoder under the recovered scope.
5. Continues from the observed tail:

| Tail | Recovery action |
| --- | --- |
| none (empty after torn-tail truncation) | acknowledge `Started` from the caller's facts, run the start plan |
| `Started` | run the pure start plan, append its step |
| `Yielded` | `AwaitingDispatch`: the site may be dispatched (it never was) |
| `Dispatched` | `AwaitingAnswer{in_doubt: true}`: never dispatched again; only an explicit bound answer or `abandon` settles it |
| `Answered` | run the pure resume plan with the journaled answer, append its step |
| terminal | `CleanupPending`: cleanup may run (it never started) |
| `CleanupStarted` | `CleanupInDoubt`: cleanup is never run again; only `confirm_cleanup` settles it (`host_confirmed`) |
| `CleanupSettled` | `Settled`: nothing further happens |

These rules give the two guarantees of this contract: **a settled yield is
never dispatched again** and **cleanup never runs twice**, across any crash
that preserves acknowledged records.

## 8. Threat model and limits

Covered: crashes at append boundaries (the tests simulate a crash immediately
before and after each append), torn final appends, tampered, reordered, or
dropped middle records, answers replayed or rebound across program, invocation,
site, or envelope, journals from another invocation or key, symlinked or
foreign directories and journals, and hosts without the capability.

Not covered in v1: an adversary who can write the owner-private directory and
**removes whole acknowledged trailing records** (rollback) is not detected;
that needs an external monotonic counter. `flock` is advisory: a process that
writes the file without taking the lock is outside the model. Ownership and
mode bits are checked, but macOS ACLs are not detected. Non-Unix hosts do not
compile the module.

## 9. Remaining work (issue #296)

- Owned and live frames, control-dependent suspension, borrows, resources, and
  effectful prefixes across a yield.
- Ordinary native and Wasm emission of `yields` functions (`SPX-B116` and
  `SPX-W126` still refuse) and a durable driver for those engines.
- Migration of an Agent lifecycle example onto this mechanism.
- One-site functions, rollback detection, a CLI or service surface, and
  hosted evidence.

## 10. Gate

```sh
cargo test --locked -p semaprax --lib resumable_effects::continuation::
```

This runs the positive multi-yield run, the explicit request/answer exchange,
sticky host failure, the crash before and after each of the thirteen records of
a three-site run, and the hostile answer, fact, journal, torn-tail, directory,
and admission cases. These are local, offline results, not hosted evidence.
