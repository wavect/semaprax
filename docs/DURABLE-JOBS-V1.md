# Durable Jobs v1

Audience: language users, tool authors, and compiler contributors.

Status: bounded durable host-runtime slice of issue #192's durable-job profile.
This tranche ships the dialect-agnostic, pure, effect-free decision procedures that govern
a job's lifecycle, lease legality, retry/backoff, idempotent enqueue,
scheduling, cancellation, and — the issue's central word — **delivery
uncertainty**, plus a Rust-only in-memory fixture and an explicit bounded
checkpoint adapter that compose those procedures into host-driven recovery.
It does **not** ship a message broker, a durable
queue transport, a background scheduler thread, or any new host operation.
See [Non-claims](#non-claims-and-remaining-work) for the remaining physical
provider, clock, concurrency, and scheduler limits.

## Objective

[Database Access v1](DATABASE-ACCESS-V1.md) established the pattern this
profile follows: define pure, checked decisions before introducing host
authority, run them on every backend, and use a separate Rust-only fixture to
show how the model could compose into an engine. A durable job queue must
distinguish success, retry, permanent refusal and uncertainty. In the uncertain
case, the worker lost contact before learning whether its last attempt took
effect.

`std.jobs` models six things as closed, checked, deterministic computations:

1. **Lifecycle state.** Ten closed `usize` codes and the predicates over them.
2. **Leases.** An affine grant bound to a worker's `(lease_generation,
   job_generation, deadline_tick)` tuple; legality for claim, begin-execution,
   heartbeat, and completion is one pure function each.
3. **Retry and backoff.** A bounded attempt ceiling, a closed failure-kind
   classification (success, retryable, permanent, uncertain), and a bounded
   exponential backoff that can never grow past an explicit cap.
4. **Idempotent enqueue.** A three-way outcome (fresh, duplicate, conflict)
   over an idempotency key and a payload descriptor.
5. **Scheduling.** Due/missed-window/catch-up/recurrence-exhaustion over an
   explicit deterministic tick clock (not a wall clock; see
   [Non-claims](#non-claims-and-remaining-work)).
6. **Delivery uncertainty.** An explicit `UNCERTAIN` resting state and its
   reconciliation, bounded by whether the handler is declared idempotent.

These operations use scalar arithmetic, boolean logic and byte-slice comparison.
They open no sockets, start no threads and read no clocks or files, so they
need no effect, `permit` or provider. The profile is `useful-data.v1`, as used
by `std.db`.

## Job states

| Code | State | Meaning |
| ---: | --- | --- |
| 0 | `PENDING` | enqueued, no schedule, available to claim |
| 1 | `SCHEDULED` | enqueued with a future `next_run_tick` |
| 2 | `LEASED` | a worker holds a current lease, has not yet started running |
| 3 | `RUNNING` | the worker has started executing the handler |
| 4 | `SUCCEEDED` | terminal: the handler reported success |
| 5 | `RETRYABLE_FAILURE` | a failed attempt remains under the attempt ceiling |
| 6 | `PERMANENT_FAILURE` | terminal: classified non-retryable, or an illegal transition landed here as the sticky failure sink |
| 7 | `CANCELLED` | terminal: an explicit cancel reached a cancellable state |
| 8 | `UNCERTAIN` | the last attempt's outcome cannot be observed (see below); not terminal, awaits reconciliation |
| 9 | `DEAD_LETTERED` | terminal: the attempt ceiling was reached without success |

`std.jobs.state.is_valid(state: usize) -> bool` is `state <= 9usize`.
`std.jobs.state.is_terminal(state: usize) -> bool` is exactly `SUCCEEDED`,
`PERMANENT_FAILURE`, `CANCELLED`, or `DEAD_LETTERED` — `UNCERTAIN` is
deliberately excluded: it is an inspectable resting state a caller must
explicitly reconcile, never a state that quietly resolves itself.
`std.jobs.state.holds_lease` is `LEASED` or `RUNNING`.
`std.jobs.state.awaits_worker` is `PENDING` or `SCHEDULED`.

## Leases

A lease is an affine grant, not a state by itself: `(lease_generation,
job_generation, deadline_tick)`. `std.jobs.lease.is_current(lease_generation,
job_generation, now_tick, deadline_tick) -> bool` is `lease_generation ==
job_generation && now_tick < deadline_tick` — a lease from a superseded
generation (the job was reclaimed after this worker's lease expired) or past
its deadline is never current, matching the required test "expired/stale
leases cannot complete jobs" exactly.
`std.jobs.lease.is_expired(now_tick, deadline_tick) -> bool` is `now_tick >=
deadline_tick`.

Four legality gates compose `state` with `lease.is_current` (or, for `claim`,
with `schedule.is_due`) rather than folding the lease check into the state
machine itself, because unlike `std.db`'s single-connection transaction, a
job is observed by many racing workers: a losing claim, heartbeat, or
completion attempt must be refused without corrupting the job for whichever
attempt is actually authoritative, so an illegal attempt here is a plain
`false`, not a forced state transition.

- `std.jobs.claim.is_legal(state, is_due) -> bool`: `state == PENDING`, or
  `state == SCHEDULED && is_due`.
- `std.jobs.begin_execution.is_legal(state, lease_generation, job_generation,
  now_tick, deadline_tick) -> bool`: `state == LEASED` and the lease is
  current.
- `std.jobs.heartbeat.is_legal(...) -> bool`: the state holds a lease (
  `LEASED` or `RUNNING`) and the lease is current.
- `std.jobs.completion.is_legal(...) -> bool`: `state == RUNNING` and the
  lease is current.

## Retry, backoff, and dead-lettering

An outcome `kind` is one of four closed codes: `0` success, `1` retryable
failure, `2` permanent failure, `3` uncertain.
`std.jobs.outcome.kind_is_valid(kind: usize) -> bool` is `kind <= 3usize`.
`std.jobs.attempt.is_within_ceiling(attempt: u8, max_attempts: u8) -> bool` is
`attempt <= max_attempts`; `std.jobs.retry.should_dead_letter` is `attempt >=
max_attempts`.

`std.jobs.retry.next_state_after_outcome(kind, attempt, max_attempts) ->
usize` is the single decision procedure a runner consults after every
attempt: success goes to `SUCCEEDED`; permanent failure (or an invalid `kind`)
goes to `PERMANENT_FAILURE`, the same sticky sink an illegal transition uses
elsewhere in this profile; uncertain goes to `UNCERTAIN` unconditionally —
never silently retried, never silently dead-lettered; retryable goes to
`DEAD_LETTERED` once the ceiling is reached, otherwise `RETRYABLE_FAILURE`.

`std.jobs.retry.backoff_ticks(attempt: u8, base_ticks: usize, max_ticks:
usize) -> usize` doubles `base_ticks` once per attempt already made, capped at
`max_ticks`; `ensures result <= max_ticks` holds by construction, so a runaway
attempt counter can never produce an unbounded wait, matching the "no
unbounded retries" requirement.

## Idempotent enqueue

`std.jobs.idempotency.enqueue_outcome(key_exists: bool, existing_descriptor:
borrow Slice<u8>, candidate_descriptor: borrow Slice<u8>) -> usize` answers
the issue's open question ("duplicate enqueue returns existing job or a
closed conflict") with one three-way, ordered-tag-byte-slice comparison
(reusing `std.bytes.equals`, the same closed-domain descriptor-as-byte-slice
idiom `std.db.descriptor` and this profile's payload schema both use, since
the bounded reference interpreter does not admit record-field projection —
see [HTTP Application Routing v1](HTTP-APPLICATION-ROUTING-V1.md#route-and-status-identity)):

| Code | Meaning |
| ---: | --- |
| 0 | fresh — no job holds this idempotency key; enqueue creates one |
| 1 | duplicate — the existing job's descriptor matches; enqueue returns the existing job, an idempotent no-op |
| 2 | conflict — the existing job's descriptor differs; enqueue is a closed refusal, never a silent merge |

`std.jobs.payload.schema_is_compatible(expected_descriptor, actual_descriptor)
-> bool` is the same byte-slice equality, used to refuse a queued job whose
stored payload descriptor no longer matches the handler's current schema
rather than decoding it speculatively.

`std.jobs.revision.is_known(job_bound_revision: u8, current_revision: u8) ->
bool` is `job_bound_revision >= 1u8 && job_bound_revision <=
current_revision`: a job is bound to the handler/schema revision current at
enqueue time, and a revision ledger (like `std.db.migration`'s) only grows, so
a bound revision is known exactly when it lies in the closed range
`1..=current_revision`. `std.jobs.revision.requires_refusal` is the negation —
the queued-job "migration-safe or fail closed" requirement.

## Scheduling

This profile models a schedule over an explicit `usize` tick clock the host
supplies, not a wall clock, calendar, or time zone — see
[Non-claims](#non-claims-and-remaining-work) for exactly why and what a real
adapter still owes.

- `std.jobs.schedule.is_due(now_tick, next_run_tick) -> bool`: `now_tick >=
  next_run_tick`.
- `std.jobs.schedule.missed_windows(now_tick, next_run_tick, interval_tick) ->
  usize` (`requires interval_tick > 0usize`): how many full recurrence
  intervals have elapsed since the job was due.
- `std.jobs.schedule.catch_up_next_run(now_tick, next_run_tick, interval_tick,
  max_catch_up) -> usize`: this profile's catch-up policy is **skip-missed**
  — a job due several windows ago fires once, for the most recent window, not
  once per missed window — and `max_catch_up` bounds how far one step can
  jump `next_run_tick`, so a long-dead clock can never produce an unbounded
  catch-up burst.
- `std.jobs.schedule.recurrence_is_exhausted(occurrences_run, max_occurrences)
  -> bool`: `occurrences_run >= max_occurrences` — a recurring job's total
  occurrence count is bounded, matching "no unbounded schedules".

## Cancellation and compensation

`std.jobs.cancel.is_legal(state) -> bool` admits `PENDING`, `SCHEDULED`,
`LEASED`, and `RETRYABLE_FAILURE` — a job that is actively `RUNNING` cannot be
forced to stop from outside; only a cooperative check inside the handler
could do that, which is outside this pure layer (see
[Non-claims](#non-claims-and-remaining-work)).
`std.jobs.cancel.next_state(state) -> usize` returns `CANCELLED` when legal,
the unchanged state when the attempt is simply too late to matter (a normal
race, not corruption), or the sticky `PERMANENT_FAILURE` sink for a state code
that was never valid to begin with.

`std.jobs.compensation.is_required(ever_ran: bool, final_state: usize) ->
bool` is true exactly when a job that actually started running (`ever_ran`)
ends at `PERMANENT_FAILURE` or `CANCELLED` — a partial external effect may be
outstanding. `true` means "a compensation hook must run"; this pure predicate
never runs one itself.

## Delivery uncertainty

This is the issue's central word, and the profile's most important refusal.
`UNCERTAIN` (state `8`) is reached from `retry_next_state_after_outcome` when
`kind == 3` (uncertain): the runner observed that it could no longer confirm
whether the last attempt's externally visible effect took place — a
post-publication I/O failure, in the vocabulary of issue #228 — and refuses to
guess `SUCCEEDED` or silently retry.

`std.jobs.uncertain.retry_is_permitted(is_idempotent_handler: bool, attempt,
max_attempts) -> bool` is `is_idempotent_handler &&
!retry_should_dead_letter(attempt, max_attempts)`: an automatic retry of an
uncertain outcome is admitted only when the handler is declared idempotent
*and* the attempt ceiling has not been reached. This is
issue #192's "Explicitly out of scope: … Automatic retry of uncertain
non-idempotent operations" encoded as a refusal, not a comment.

`std.jobs.uncertain.reconcile(decision: usize, is_idempotent_handler, attempt,
max_attempts) -> usize` is how an `UNCERTAIN` job leaves that state: `decision
== 0` (confirmed succeeded) goes to `SUCCEEDED`; `decision == 1` (confirmed
failed) goes to `PERMANENT_FAILURE`; `decision == 2` (retry) goes to
`RETRYABLE_FAILURE` only when `uncertain_retry_is_permitted` holds, and
**stays at `UNCERTAIN` otherwise** — a non-idempotent handler's uncertain job
is never silently retried and never silently dead-lettered; it waits for an
explicit `0` or `1` decision. `decision` is deliberately opaque to this
profile: whether it comes from an operator, a reconciliation job that queries
the external system, or another checked SEMAPRAX computation is a runner
concern, not this pure layer's.

### Checked publication outcomes and recovery uncertainty

[Host Operation Outcome v1](HOST-OPERATION-OUTCOME-V1.md) closes issue #228
for the additive checked atomic-write route: a checked handler can receive
`Published`, `NotPublished`, or `Uncertain` as an ordinary `std.fs` value.
Only that provider-observed third result maps to `kind == 3`; an ordinary
provider failure must remain a retryable or permanent handler outcome and may
not be relabelled as uncertainty.

The bounded host adapter in `src/job_runtime.rs` validates a job payload with
an exact `CompiledInteractionSchema` before dispatching an explicit host
handler seam. It checkpoints the existing `JobStore` lifecycle as bounded,
versioned bytes through caller-supplied CAS persistence, invokes no source text
or ambient path/network API, and uses `complete_durable` for the ledger state.
It records an unconfirmed durable completion as `UNCERTAIN`. On recovery, a
checkpointed `RUNNING` attempt also becomes `UNCERTAIN` before any new claim;
only a `LEASED` attempt that never began execution may expire back to
`PENDING`. Reconciliation remains explicit, and `JobStore` still refuses an
uncertain non-idempotent retry.

## Local evidence: `src/job_fixture.rs`

A separate, Rust-only fixture (`src/job_fixture.rs`) composes `std.jobs`'s
decision procedures (duplicated at the Rust layer, the same choice
`database_fixture.rs` made for `std.db`, and for the same reason: proving the
model holds independent of the interpreter, not that one calls the other)
with `crate::database_fixture::DatabaseFixture` as the durable ledger an
enqueue and an application-state change commit through together, into an
in-memory job store with real lease claims, heartbeats, expiry, worker-crash
recovery, concurrent-claim races (simulated single-threaded, the same
technique `database_fixture.rs`'s own
`concurrent_runner_duplicate_attempt_is_never_applied_twice` uses — this is
explicitly a deterministic simulation, not real concurrent threads), retry
with bounded backoff, dead-lettering, schedule catch-up, and cancellation. It
is explicitly **not** a message broker, **not** a durable transport, opens no
socket, spawns no thread, and grants no authority; it is local evidence that
the decision procedures above compose into something a real job runner could
implement, nothing more.

**Lease fencing across reclaim.** Until this tranche, `JobStore::claim`
derived a job's next `lease_generation` from the *currently outstanding*
lease (`self.lease.map_or(0, |lease| lease.lease_generation) + 1`), which
becomes `None` after every completion and every expiry-driven reclaim — so
the very next claim's generation restarted from `1` every time. Two
different lease grants for the same job could therefore be handed the
identical `lease_generation`, and a worker whose call arrived late (after
its lease had been reassigned) would satisfy `lease.is_current` against the
new holder's still-current lease by coincidence — exactly the "lease expiry
... can produce concurrent execution" failure case this issue names.
`JobRecord::lease_epoch` closes this: a counter on the record itself,
incremented once per `claim` and never reset by a completion or a reclaim,
so no two lease grants a job ever makes can share a generation. The new
regression test proving this,
`job_fixture::tests::a_reclaimed_workers_stale_completion_can_never_land_over_the_new_holders_run`,
drives a second worker's claim all the way to `Running`, inside its own
still-current deadline, before the first (reclaimed) worker's stale
completion call arrives, so the refusal cannot be explained by the job
simply not having reached `Running` yet the way the pre-existing reclaim
test's ordering allowed.

**Completion durability.** Until this tranche, `complete` mutated only the
in-memory job record: the ledger's `state` column, once written by
`enqueue` as a placeholder, was never rewritten by anything, so nothing
distinguished "the handler ran and the outcome is durably recorded" from
"the handler ran and the worker crashed before that record committed" — one
of the two concurrent/crash cases this issue's write-up names as usually
faked. `DatabaseFixture::update_column` (an additive primitive alongside its
existing `insert`/`select_eq`) and `JobStore::complete_durable` close that
gap: the outcome's resulting state commits into the ledger row *before* the
in-memory record advances, and if that commit cannot be confirmed (a
stuck-open prior transaction stands in for the crash in
`complete_durable_forces_uncertain_rather_than_the_handlers_outcome_when_the_ledger_commit_is_never_confirmed`),
the job rests at `Uncertain` rather than presenting the handler's reported
outcome as if it had been durably recorded. `complete` itself is unchanged
and still exists for callers that do not need the ledger write; `job_evidence`
records `DurableCompleted` separately so recovery can distinguish a confirmed
ledger outcome from one that must remain uncertain.

## Host checkpoint runtime v1

`src/job_runtime.rs` composes the existing lifecycle and evidence owners with
an explicit `JobCheckpointStore` and `HostJobHandler`. Payload bytes must decode
against the supplied compiler-derived interaction schema. The handler trait
itself does not authenticate a compiled callable or deployment policy.

`semaprax.job-runtime-checkpoint.v1` uses little-endian integers in this order:
`SPXJOB01` magic; u16-length UTF-8 schema digest; u8 bound revision;
u16-length idempotency key and payload descriptor; u32-length payload;
u8 maximum attempts; canonical u8 idempotency boolean; canonical u8 schedule
presence (if present, four u64 values: next tick, interval, occurrence bound,
catch-up bound); u64 base and maximum backoff; u16 entry count; then each
u8-length canonical evidence entry followed by its u64 tick, u32 worker ID,
and u64 lease duration; finally u8 claimed state. Trailing bytes, malformed
booleans, unknown entries and invalid replay transitions are refused.
The complete document is capped at 16 KiB, with 64 entries, a 128-byte digest,
256-byte key/descriptor and 4096-byte payload. The native file wrapper prefixes
a u64 CAS generation, increased on every replacement.

The native file store uses one caller-selected existing directory, fixed file
names, an OS advisory lock released on process exit, a synced staged file and
same-directory rename. It does not promise power-loss durability, hostile-root
confinement or distributed locking. The checkpoint is the persistence boundary;
`DatabaseFixture` remains an in-memory decision mirror, not a physical database
transaction with application state.

The generation-backed job-store seam leaves a crash-left staging file in place:
it never promotes, reads, or deletes that uncommitted file during `open`.
Recovery scans its bounded owned staging namespace and allocates later names;
an excessive or exhausted namespace refuses open. This avoids reusing an
ambiguous stage while writer exclusion remains a separate store concern.

`GenerationJobStore` supports one writable owner per canonical root. `open`
holds an OS-backed advisory lock through the handle lifetime and returns a
closed busy refusal to a second live writer, including relative and canonical
path aliases. The fixed lock filename is only a rendezvous point: process exit
releases the OS lock, so its retained name never blocks a later recovery.

Execution checkpoints a claim and then `Running` before invoking the handler.
Recovery replays the original attempt times, expires an unstarted lease at its
recorded deadline, and turns a retained `Running` attempt into `Uncertain`.
An explicit runtime cancellation uses the same `JobStore::cancel` reducer and
checkpoints `Cancelled`. A successful recurring occurrence advances through
the existing bounded skip-missed reducer before its drive returns; evidence
records the exact next due tick and occurrence count, so recovery restores the
same future due time. Overflow in that recurrence arithmetic is refused before
the handler is dispatched.
Checkpoint failure poisons that runtime instance: further mutation refuses
until the caller recovers from storage. Evidence grants no authority; recovery
requires explicit storage and the current schema. The revision byte follows
the fixture's known-revision range, not a cryptographic handler identity.
`JobRuntime::compensation_is_required` exposes the fixture's own pure
predicate (true exactly when the job actually started running and ended
`PermanentFailure` or `Cancelled`) to a host driving this runtime directly;
it selects and runs no compensating action itself.
This runtime currently drives one job; physical database integration
remains follow-on work.

**Heartbeat and current-lease query.** `drive_once` is one atomic host call:
nothing else in this process can observe or renew the lease while a handler
is running, so a handler whose own work may run long asks for more time from
inside its own `execute` instead. `HostJobHandler::execute` now also receives
a [`JobHeartbeat`] handle scoped to exactly that call: `current_deadline`
reports the tick the held lease currently expires at, and `extend_lease`
re-arms the same lease through the existing `JobStore::heartbeat` reducer to
`now_tick + extend_ticks` (relative to the tick `drive_once` was called with,
not the current deadline) and checkpoints the extension immediately. A
refused extension (tick overflow, a stale worker/lease, or an
already-poisoned runtime) does not block the handler's own outcome from
completing normally. A confirmed heartbeat adds no evidence entry — it is a
liveness renewal of the already-recorded claim, not a new lifecycle fact — but
it does update this runtime's own crash-recovery replay bookkeeping, so a
process that crashes right after a confirmed heartbeat and then recovers
replays the extended window rather than the shorter one originally claimed.
Because `FileJobCheckpointStore` writes through a real file, a heartbeat's
extended deadline is genuinely visible to any other process reading that same
checkpoint at that moment; no test here exercises *this specific heartbeat
scenario* concurrently from a second reader. The store's underlying CAS write
path is, however, now exercised under genuine concurrent access: see
`real_os_thread_cas_race_lets_exactly_one_writer_win_the_shared_checkpoint_file`
in `src/job_runtime/tests.rs`, and the "Model true concurrency" entry below.

### Retained source-handler binding v1

`src/job_runtime/source_handler.rs` provides the one checked route for a
source job: it derives `SourceJobHandlerBinding` from an immutable retained
`ProjectRevision`, an exact program-root digest, retained source revision,
explicit non-`main` callable ID, exact nominal payload type, and bounded
interpreter fuel. The binding rechecks the retained source, derives its
payload schema and typed carrier graph, and admits the callable through the
existing effect-free retained-call interpreter. Its domain-separated identity
also commits the `i64-status-v1` result profile: `0` succeeds, `1` is a
retryable failure, and `2` is permanent; every other source result or
interpreter failure is a terminal permanent failure. This source seam never
manufactures `UNCERTAIN` from an effect-free evaluation.

`SourceJobHandlerBinding::bind_submission` writes that exact identity into the
already persisted, bounded opaque `payload_descriptor`. Before
`drive_checked_source_job` can claim a lease, it requires byte-for-byte
descriptor equality, the caller-retained binding identity, and the exact
runtime payload-schema digest. A same-schema job cannot therefore resume with
a different handler identity. This uses the existing checkpoint v1 submission
field without changing its wire format; callers must retain the expected
binding digest and use the checked drive route rather than implementing
`HostJobHandler` directly.

## Job evidence: `src/job_evidence.rs`

The one acceptance criterion the first tranche of this issue recorded as
**not met** was "job evidence is replayable but grants no execution
authority" — no checkpoint/evidence-root format existed for jobs
specifically. `src/job_evidence.rs` closes that gap with a format scoped to
jobs alone, not copied from `src/agent_lifecycle/durable`'s checkpoint
machinery (a resumable run's settled read observation and program counter is
a different, larger claim than replaying one job's already-finished
lifecycle).

`JobEvidenceLog` is an append-only, domain-separated SHA-256 hash chain over
typed entries mirroring every `JobStore` transition (`Enqueued`, `Claimed`,
`BegunExecution`, `Completed`, `DurableCompleted`, `ConnectionUncertain`,
`ReconciledUncertain`, `Cancelled`, `LeaseExpired`); the chained digest after
the last entry is the evidence root.
`JobEvidenceLog::replay` **independently recomputes** the job's final
lifecycle state from the recorded entries alone, reusing
`job_fixture::decisions` (the same Rust mirror of `std.jobs` `job_fixture.rs`
already uses) rather than a second, parallel state machine, and returns
`FinalStateMismatch` when a recorded input's recomputed outcome disagrees
with the log's own claimed final state — the tamper case, where a recorded
field changed without the claim changing to match. A terminal state
(`SUCCEEDED`, `PERMANENT_FAILURE`, `CANCELLED`, `DEAD_LETTERED`) can never be
reopened by a later entry, mirroring this repository's sticky-failure-
selection invariant one level up from `cleanup_plan`.

Appending an entry, computing a root, and replaying are all inert data
operations: no socket, no thread, no lease claim, no execution. This is
exactly "a settlement or concurrency model is proof data, not permission to
perform a physical finalizer, spawn runtime work, or publish an artifact"
(`AGENTS.md`), applied to a job's own recorded history instead of a single
checkpoint's program counter.

**What replay does not check.** `Claimed`'s `is_due` and `Completed`'s
`outcome_kind` are asserted facts, not independently verified against a
clock or an external system this format does not carry: replay only checks
that the *asserted* facts admit a legal `std.jobs` transition and that they
recompute to the log's own claimed final state, exactly the boundary an
agent checkpoint's single registered read observation already accepts. A
runner that asserts a false `is_due` produces a log that still replays
internally consistently; catching that requires the runner's own clock
input to be part of the trust boundary, which is out of scope for this
tranche.

## Non-claims and remaining work

This tranche adds no new checked host operation, effect name, or ABI, and
touches no file under `src/hir`, `src/wasm`, `src/codegen`,
`src/interpreter*`, `src/cleanup*`, or `src/cli`. Concretely, it does **not**:

- **Run autonomously.** There is no scheduler thread, worker pool, timer, or
  ambient source evaluation. A host explicitly drives each claim with a worker
  ID, tick, lease duration, checked schema, handler, and explicit checkpoint
  store. The retained source-handler binding is an in-process, effect-free
  callable evaluator only; arbitrary `HostJobHandler` implementations remain
  unchecked and no handler gains filesystem, process, network, clock, or
  scheduler authority.
- **Claim physical database durability.** `DatabaseFixture` remains an
  in-memory transaction model. The physical checkpoint stores runtime/evidence
  bytes only; it is not a database driver or an atomic application-data join.
- **Model a real wall clock, time zone, or DST.** `schedule.*` takes an
  explicit `usize` tick supplied by the caller; there is no numeric cast in
  this language and no calendar library in this repository, so a real
  wall-clock adapter must convert its own calendar arithmetic to ticks before
  calling in — this package validates the *catch-up and recurrence decision
  procedure*, not a specific calendar system, exactly as `std.db.migration`
  validates its decision procedure over an opaque one-byte checksum tag
  rather than a real cryptographic digest.
- **Cross-import `std.db` from `std.jobs`'s own `.spx` source.** `std.db` is
  not yet in the compiler's closed bundled-dependency registry
  (`src/project/standard_dependencies.rs`) that a `[dependencies]` entry
  resolves against, so `std.jobs` cannot declare `std.db = "=0.1.0"` and
  `use function … from std.db` today. This package instead depends on the
  already-bundled `std.bytes` for slice equality and restates the two small,
  jobs-specific bounds (`revision.is_known`, `payload.schema_is_compatible`)
  it needs locally rather than the whole `std.db.descriptor`/`std.db.migration`
  apparatus; `src/job_fixture.rs` composes with the real
  `crate::database_fixture::DatabaseFixture` type directly instead, at the
  Rust layer where that type already lives. Registering `std.db` as a bundled
  dependency is a shared-registry change outside this tranche's file lease,
  not attempted here.
- **Provide a message broker or durable transport.** No SQS/Kafka/Postgres
  LISTEN/NOTIFY-shaped adapter, no wire protocol, no network authority. A real
  adapter needs either a from-scratch protocol implementation or a Cargo
  dependency this repository's own `Cargo.toml` cannot add without a
  maintainer decision, exactly as [Database Access
  v1](DATABASE-ACCESS-V1.md#non-claims-and-remaining-work) already records
  for a real database driver; [Project Dependencies
  v1](PROJECT-DEPENDENCIES-V1.md#rust-crate-inputs) is the same extension
  point a real broker adapter would use.
- **Model true multi-process concurrency.** `src/job_fixture.rs`'s original
  concurrent-claim test is a deterministic single-threaded simulation of two
  racing workers, exactly the technique `database_fixture.rs` already uses for
  its own concurrent migration test; that remains local evidence the decision
  procedure is race-safe on paper, not proof against a real runner. Two
  further tests now close the *real OS thread* half of that gap specifically:
  `src/job_fixture.rs`'s
  `real_os_thread_concurrent_claim_race_grants_the_lease_to_exactly_one_worker`
  races 16 genuine OS threads (synchronized on a `Barrier`, so the OS
  scheduler — not the test — decides call order) against a shared
  `Mutex<JobStore>` and asserts exactly one ever wins, repeated across 25
  independent jobs; `src/job_runtime/tests.rs`'s
  `real_os_thread_cas_race_lets_exactly_one_writer_win_the_shared_checkpoint_file`
  does the same against `FileJobCheckpointStore`'s real advisory-locked CAS
  file write, with independent racing threads each opening their own store
  handle at the same directory. Neither test claims to prove true
  multi-*process* concurrency (separate address spaces, no shared `Arc`, real
  process scheduling and signal delivery) — that remains open, and is the
  larger claim a physical database driver behind `DatabaseFixture` would still
  need its own dedicated test to establish.

## Local evidence

```sh
cargo test --locked -p semaprax --lib job_fixture::
cargo test --locked -p semaprax --lib job_evidence::
cargo test --locked -p semaprax --lib job_runtime::
cargo test --locked -p semaprax --test project -- standard_library::
cargo test --locked -p semaprax --test documentation
```

The first command covers the Rust-only in-memory fixture described above.
The second covers the replayable job evidence log immediately above it,
including its cross-checks against the live `JobStore`. The third covers the
bounded checkpoint codec, physical-store CAS, hostile decode refusals, and
evidence replay recovery. The fourth covers `std/jobs`'s canonical formatting, stable identities,
examples, and conformance module once it is registered in
`std/packages.json` and `std/catalog.json` (that registration, and the
regenerated `docs/STANDARD-LIBRARY-CATALOG.md`, land in their own commit —
see the accompanying change's report — so this document's local-evidence
list can be checked incrementally, exactly as
[Database Access v1](DATABASE-ACCESS-V1.md#local-evidence) records for the
same reason).

The checked source binding resolves the selected retained source module. Imported
callable closures that require linked Project resolution are refused; the
current route does not claim general imported-handler execution.
