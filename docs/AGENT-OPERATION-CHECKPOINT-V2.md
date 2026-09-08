# Agent operation checkpoint v2

Status: bounded live adapter with 12 focused local cases passed: four durable
execution cases and eight checkpoint codec cases. The two focused joined
Runtime v2 cases also pass locally, covering ordinary and durable execution.

Audience: runtime integrators and compiler contributors.

`CompiledTypedEffects::run_durable` executes the same retained iterative stages
and typed registry through the private IterativeDriver hooks. The ordinary
one-read lifecycle and non-durable typed registry paths remain unchanged.
The joined Runtime v2 wrapper supplies its privately bound execution revision
and ProgramRoot identities; the lower-level typed adapter treats root strings
as binding inputs and does not independently grant Project authority.

The caller supplies the existing CheckpointStore interface. Its commit replaces
one complete generation atomically. The caller must hold exclusive writer
authority for the invocation, and a resume snapshot must come from that
caller-authorized trusted store. Self-hashes bind bytes and ordering; they do
not authenticate arbitrary caller-fabricated host observations. Stored host
observations remain trusted input, not proof that an external effect occurred.
The adapter creates no filesystem, process, provider, network or store authority.

Checkpoint identity binds the exact execution revision, ProgramRoot, typed
registry and invocation. Invocation identity includes task bytes and budget,
every ordered proposal's exact bytes, all requested stage/effect ceilings and
the durable reserved-fuel ceiling. Recovery requires exact identity and exact
effective checkpoint limits before performing a stage or writing the store.

The canonical checkpoint wire is compact JSON with sorted object keys, ordered
entry arrays and one terminal newline. Every generation binds its predecessor
digest, complete identity/limits, usage and event. Empty generation zero has a
binding-derived digest. The wire is bounded to 2 MiB and 4096 entries. Closed
objects, exact canonical re-encoding and closed flat typed values reject
unknown keys, duplicate keys, malformed integers, reminted predecessors,
substituted identities/limits and byte-counter refunds.

Each effect has these ordered entries:

- Intent: exact turn, registered operation/effect, current authorization binding,
  checked State, canonical proposal and exact typed arguments. It charges one
  call and the exact canonical argument transport before dispatch.
- Observed: the same context and checked typed result, or a closed failure
  reason with bounded attempted-result bytes. It follows the physical host call
  and precedes any reducer execution. Failed or oversized host work remains
  charged; invalid results never reach reduce.
- Transition: the same context, exact checked Continue/Complete/Suspend/Fail
  selection and terminal/next-state carrier. Only the retained reducer selects
  this value. A stored failed observation cannot acquire a transition.

StageReservation entries interleave these effect phases. Before every retained
stage—including initialize, observe, authorize, replayed stages, reducers after
replayed observations, and all new-tail stages—the adapter durably charges that
stage's maximum fuel. Reservation fuel is cumulative and never refunded by
actual-use accounting, failed stages, cancellation, crashes or recovery.
This deliberately conservative ceiling prevents repeated crashes from obtaining
unmetered deterministic execution.

Intent commit must acknowledge before the host is called. Observed commit must
acknowledge before reduce. Transition commit must acknowledge before a new turn.
Any store error poisons the live journal and stops execution. The error retains
the latest local checkpoint candidate because a lost acknowledgement may have
stored it. Recovery uses the actual trusted-store snapshot, not an assumption
that the last acknowledged local generation is still current.

A snapshot ending in Intent is uncertain and is rejected before any stage,
store write or host call. It never automatically dispatches that effect again.
For retained Observed/Transition prefixes, recovery re-executes deterministic
stages under new persisted fuel reservations and obtains a fresh checked grant
on every turn. It compares the exact State/proposal/operation/argument/grant
context, reuses the recorded observation without calling the host, and compares
every retained transition against the newly checked reducer result. New host
work is reachable only after that exact prefix is consumed. Resuming a completed
three-turn run therefore makes zero additional host calls.

Failure to persist a final Complete/Suspend/Fail transition returns an explicit
DurableTypedFailure containing the already selected IterativeRun. Persistence
failure does not silently publish success or replace the selected terminal
meaning. A failed Continue transition stops before the next turn. Cancellation
is observed by the existing stage boundaries; an observed effect remains in
the journal for a later authorized resume.

DurableTypedRun exposes the typed run, checkpoint bytes/digest and cumulative
call/byte/reserved-fuel usage. DurableTypedFailure exposes diagnostics, optional
selected terminal run and the latest local checkpoint candidate. Their evidence
is immutable and carries no authority. This profile covers local retained
execution and the caller-injected store/handlers; distributed writers, automatic
reconciliation, checkpoint migration and hosted clients remain separate gates.

Focused selectors:

- `agent_lifecycle::iterative::effects::durable::tests`: actual three-turn
  execution/full replay, lost acknowledgements at Intent/Observed/Transition
  including the final terminal, stored typed failure, changed roots, and
  nonrefundable reducer/recovery fuel.
- `agent_runtime_v2::checkpoint::tests`: canonical codec replay, hostile field,
  identity, predecessor and limit changes, exact successful byte charges,
  retained failed-work charges and terminal restrictions.
- The joined Runtime v2 integration additionally binds these live results to
  actual retained Project/source/deployment roots and checks changed invocation
  rejection before store or host work.
