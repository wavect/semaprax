# Durable Jobs v1

Audience: language users, tool authors, and compiler contributors.

Status: first bounded slice of issue #192's durable-job profile. This tranche
ships the dialect-agnostic, pure, effect-free decision procedures that govern
a job's lifecycle, lease legality, retry/backoff, idempotent enqueue,
scheduling, cancellation, and — the issue's central word — **delivery
uncertainty**, plus a Rust-only in-memory fixture that proves those decision
procedures compose into a real job runner (crash/recovery, concurrent leases,
retry, schedule catch-up). It does **not** ship a message broker, a durable
queue transport, a background scheduler thread, or any new host operation.
See [Non-claims](#non-claims-and-remaining-work), and in particular
[The #228 boundary](#the-228-boundary-what-blocks-a-checked-semaprax-caller)
for exactly what a checked SEMAPRAX program can and cannot observe about an
uncertain outcome today.

## Objective

[Database Access v1](DATABASE-ACCESS-V1.md) established the pattern this
tranche follows: specify the pure, checked decision procedures a domain needs
before any host authority is involved, execute them on every backend, and let
a separate Rust-only fixture prove the model composes into something a real
engine could implement. A durable job queue is exactly where "at-least-once
delivery" stops being a slogan and starts being a set of exact states a caller
must be able to distinguish: a job that ran and succeeded, a job that will run
again, a job that is refused forever, and — the case competing systems most
often paper over — a job whose last attempt may or may not have taken effect
before the worker lost contact with it.

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

Everything above is scalar arithmetic, boolean logic, and byte-slice
comparison. None of it opens a socket, starts a thread, reads a clock, or
reads a file, so none of it needs an effect, a `permit`, or a provider; the
profile is `useful-data.v1`, the same one `std.db` uses.

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

### The #228 boundary: what blocks a checked SEMAPRAX caller

Everything above is a decision procedure over *already-known* outcome codes:
given that a `kind` of `3` (uncertain) was observed, `std.jobs` tells a runner
exactly what state that produces and exactly when a retry may proceed. That
half is fully checked, deterministic, and executed on the interpreter, native
C11, and Core Wasm lanes today, exactly like every other function in this
package.

What this tranche cannot do — and does not attempt to work around — is let a
**checked SEMAPRAX-authored job handler observe its own uncertain outcome**.
Issue #228 records the exact gap: every fallible host operation in this
repository ([Bounded Language Network I/O
v1](BOUNDED-LANGUAGE-NETWORK-IO-V1.md), file writes, and every other member of
the closed host-operation table) **aborts the enclosing invocation on
failure** rather than returning an inspectable value — "a nonzero status
aborts the enclosing function exactly like any other fallible host-command
operation" is the exact rule, unconditionally, for `net_connect`, `net_send`,
`net_recv`, and the rest. A handler written in checked SEMAPRAX that performs
a real effect (say, a `net_send` that may have reached its destination before
the connection dropped) has exactly one bit of information available after a
failure: the whole invocation stopped. It cannot distinguish "definitely
failed before anything left the process" from "possibly succeeded, but I lost
the connection before I could tell" — the three-way outcome issue #228 names
("validated and published", "validation failed, nothing published",
"published then I/O failed, outcome uncertain") cannot be produced by checked
SEMAPRAX source today, for any handler, in any profile, not just this one.

Consequently: **the `kind == 3` (uncertain) input to
`retry_next_state_after_outcome` and `reconcile` can only be supplied by a
host-side runner today**, exactly the way `std.db.transaction.next_on_connection_lost`
is "the one transition driven by an external signal rather than a requested
operation" and is only ever invoked from Rust in `database_fixture.rs`, never
from checked SEMAPRAX source. `src/job_fixture.rs` (below) demonstrates this:
its `JobStore::record_connection_uncertain` plays the same host-side role
`DatabaseFixture::connection_lost` already plays for `std.db` — observing a
failure the *Rust* layer cannot attribute to success or failure, and forcing
the sticky, honest `UNCERTAIN` outcome rather than guessing. A checked
SEMAPRAX-authored *handler function* gets no equivalent capability in this
tranche, could not get one without exactly the new value-typed host operation
issue #228 asks for, and this tranche deliberately does not invent a sentinel
value or route around the verifier to fake one — issue #228 gates that
behind its own independent review checkpoint (SPX-AI-019..025), not a bounded
worker's self-approval.

**The exact probe** a follow-on tranche needs once #228 lands: add a
value-returning variant of one fallible host operation (network or
filesystem) whose closed failure taxonomy includes a distinct "sent, outcome
unknown" case (not merely "failed"/"succeeded"), thread that outcome kind
through to `std.jobs.retry.next_state_after_outcome`'s `kind == 3` input from
*checked SEMAPRAX source* instead of only from a Rust-side runner, and add a
hostile-input regression proving a handler cannot forge `kind == 3` for an
operation that never actually left the process (the taxonomy must distinguish
"never attempted" from "attempted, outcome unknown"). Nothing in this
package's public surface would need to change; only its caller would gain
the ability checked SEMAPRAX handlers still lack.

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

## Non-claims and remaining work

This tranche adds no host operation, no new effect name, no new ABI, and
touches no file under `src/hir`, `src/wasm`, `src/codegen`,
`src/interpreter*`, `src/cleanup*`, or `src/cli`. Concretely, it does **not**:

- **Run anything.** There is no background scheduler thread, no worker pool,
  and no process that claims jobs on a timer. "A settlement or concurrency
  model is proof data, not permission to perform a physical finalizer, spawn
  runtime work, or publish an artifact" (`AGENTS.md`) applies exactly here:
  this package's state machine is proof data a runner must obey, not a
  runner.
- **Surface delivery uncertainty to a checked SEMAPRAX handler.** See
  [The #228 boundary](#the-228-boundary-what-blocks-a-checked-semaprax-caller)
  above — this is the half of #192 this tranche cannot deliver, precisely
  scoped, with the exact follow-on probe recorded.
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
- **Model true concurrency.** `src/job_fixture.rs`'s concurrent-claim test is
  a deterministic single-threaded simulation of two racing workers, exactly
  the technique `database_fixture.rs` already uses for its own concurrent
  migration test; it is local evidence the decision procedure is race-safe on
  paper, not a proof about a real multi-threaded or multi-process runner.

## Local evidence

```sh
cargo test --locked -p semaprax --lib job_fixture::
cargo test --locked -p semaprax --test project -- standard_library::
cargo test --locked -p semaprax --test documentation
```

The first command covers the Rust-only in-memory fixture described above.
The second covers `std/jobs`'s canonical formatting, stable identities,
examples, and conformance module once it is registered in
`std/packages.json` and `std/catalog.json` (that registration, and the
regenerated `docs/STANDARD-LIBRARY-CATALOG.md`, land in their own commit —
see the accompanying change's report — so this document's local-evidence
list can be checked incrementally, exactly as
[Database Access v1](DATABASE-ACCESS-V1.md#local-evidence) records for the
same reason).
