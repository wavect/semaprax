# Source Agent owned wait public v1 — fresh two-turn session

Status: **fresh and first-Prepared public entries passed local 2 MiB gates at `46d99ddfb`; continued-target success and Started-ACK uncertainty passed locally; remaining focused gates pending.**
Audience: SourceLive host, interpreter, journal, SDK, and runtime implementers and reviewers.

This document owns the public construction boundary for the SourceLive
v8 owned-Agent route. It is intentionally smaller than issue #330: it specifies
one fresh, interpreter-selected, cumulative run with two turns and a
`Complete` terminal, plus the exact first-turn Prepared process restart. It does not claim general restart, hosted support,
native/Wasm execution, external report delivery, or the whole issue.

## 1. Admission and scope

The public request is opaque. A caller cannot construct it from Agent IDs,
source text, a journal path, a checkpoint, a terminal carrier, an HMAC key, or
an evidence capsule. Its trusted host constructor receives exactly:

1. one retained `AgentRuntimeV2` and its authenticated Project revision;
2. the compiler-checked `(Agent ID, propose-operation ID, helper ID)` binding;
3. one model adapter whose existing `SourceModelBinding` matches that runtime;
4. the existing `SourceLivePolicy`, cancellation token, and clock; and
5. a caller-held fresh SourceLive-v8 store preparation plus its checkpoint key.

The constructor independently derives `CheckedTypedOwnedWaitExecutionV8`,
then checks the runtime, typed lifecycle, source revision, proposal schema,
deployment/instance model binding, task, effect ceilings, ordinary policy, and
the exact owned-wait binding. It accepts no caller-provided digest as a
substitute for one of those facts.

The public profile is selected only when the checked Context selects
`semaprax.source-agent-owned-wait.cumulative.v1`, has room for exactly two
turns, and is interpreter-selected. A non-empty store, recovered store,
foreign registration, source/policy drift, unbound adapter, cancellation,
expired deadline, missing target capability, unsupported terminal, or any
other phase outside this contract refuses before its next model or target
effect. Refusal does not fall back to the private source route.

## 2. Fresh store registration

Fresh registration has two stages because the final dev/inode identity and
generation do not exist before exclusive file creation:

1. The host obtains the existing explicit registration grant and calls the
   existing read-only preparation with a held directory FD and expected scope,
   execution, binding, limits, and directory identity. This checks the host
   process and directory before file creation.
2. The existing fresh opener creates and locks the empty v8 file, obtains its
   complete directory/file pins, and derives the sole registration generation.
   The host durably retains that full registration out of band. Only its
   explicit retained-registration acknowledgement authorizes the fresh lease
   to append.

The public journal wrapper owns the resulting registered lease and journal. It must
never expose the lease, `File`, registration grant, checkpoint key, append
witness, or an owner. The existing same-FD, PID, uid/mode/nlink, no-follow,
flock, scope, generation, prefix-byte, sequence, and MAC checks must remain in
force at every existing boundary.

Production export/import of registration data is inert descriptive data. It
does not become a grant. A recovered lease stays read-only unless a later
phase-specific restoration contract materializes the exact physical owner.

## 3. Success path

The run composes only existing consuming joins, in this order:

```text
Initialize -> Observe -> Start -> Prepared -> Model -> Resume -> Completed
-> ProposalAdmitted/Transfer -> Authorize -> Ready/Consumed -> Target
-> Decision cleanup -> Reduce -> Step(Continue)
-> next State/Observe -> Start -> Prepared -> Model -> Resume -> Completed
-> ProposalAdmitted/Transfer -> Authorize -> Ready/Consumed -> Target
-> Decision cleanup -> Reduce -> Step(Complete) -> terminal Report claim
```

The first Step must be the checked `Continue` case and the second must be the
checked `Complete` case. The driver invokes the already selected model adapter
only after its Intent acknowledgement and the caller-held `TargetHostHandler`
only after its target Intent acknowledgement. It invokes a canonical cleanup
observer only after the matching Started acknowledgement. The existing rows,
holds, consumption, cleanup vectors, and physical owner transfers are used
unchanged; no snapshot, decoded checkpoint, receipt, evidence, or terminal
carrier enters this chain as authority.

Only after the terminal ACK may the run copy the existing checked
`delivery_projection`. The projection must be bounded canonical data plus
terminal evidence. It must contain no Report owner and confer no store, model,
target, cleanup, append, or recovery authority. Returning that projection is
the entire initial delivery surface. Network, queue, filesystem, process,
signing, and other external delivery are separate caller-held capability
protocols.

## 4. Failure and quarantine boundary

The existing private continuation failures retain phase-specific physical
owners. A public driver may not return those owners and may not drop them as a
shortcut. Before exposing the executable session, the driver must map every
reachable failure through an existing acknowledged cleanup/Stop path or retain
it in a sealed runtime-held quarantine object that admits only its prescribed
cleanup. A quarantined object exposes no retry, extraction, raw journal, or
recovery authority.

The initial and continued failed-Observe paths now have a private driver that
performs the acknowledged State cleanup, receipt, and sticky Stop sequence and
seals its released State inside the runtime. At each incomplete boundary it
retires the journal and returns a private opaque error holding the reached
physical owner. Its caller must retain that error; a public runtime-held
quarantine is still unfinished. A first-turn model failure can now be consumed
into a private opaque quarantine that retires its journal and retains the
exact parked or Resume owner; it has no retry or extraction API because Model
has not reached a State cleanup boundary. The first-turn failed-target State
tail enters its private acknowledged cleanup/receipt/Stop join from runtime
custody, retaining incomplete owners in runtime quarantine. Several other
post-effect tails still lack a terminal join.
The fresh public entry now returns an opaque run handle that borrows the locked
journal and owns the private runtime. The handle exposes status and the checked
terminal projection, and `try_close` returns the same handle if any physical
obligation remains. Known failed Observe, failed target, and observer-failure
State tails enter their existing acknowledged settlement drivers. Other tails
stay in runtime quarantine; dropping that handle retires append authority, but
does not claim semantic cleanup. The admitted Prepared and transferred-owner recovery entries are specified in
sections 11, 13 and 16; Report delivery from reopened bytes remains unsupported.
Sections 14–17 close the required later-target failure, actual Refused and
eligible cancellation/abandonment joins for this bounded public profile. Other
unsupported phases remain quarantined without retry or reminted authority.

## 5. Required evidence before promotion

The owning regression must execute a real nontrivial two-turn Agent where
State changes through Initialize, Observe, Authorize, Reduce, first Step
Continue, second Step Complete, and terminal projection. It must also cover
fresh store registration/retained acknowledgement, pre-effect refusal for
wrong source binding and recovered/non-empty store, one model dispatch per
turn, one target dispatch per turn, canonical cleanup order, no owner escape,
and refusal before effects for unsupported phase selection.

The private terminal predecessor now has a consuming projection seam: after an
authenticated `Complete` terminal ACK and the existing physical Report claim,
it returns the checked canonical Report projection with the exact authenticated
terminal evidence bytes and consumes the Report owner. The evidence is kept as
a bounded UTF-8 string, including its original LF, rather than parsed and
reserialized. A failed projection check retains that same owner in the sealed
private failure path. This exposes no lease, report owner, State, recovery
route, or finalizer authority.

The required recovery matrix follows the admitted authority classifications in
[owned-wait v1](SOURCE-AGENT-OWNED-WAIT-V1.md), especially sections 7–9:
transferred-owner recovery, no duplicate dispatch or uncharged reconstruction,
one-use phase-specific permits, and hostile registration/tail refusal. Required
transferred-owner recovery now has the exact first TransferCompleted entry in
section 16. Other destination phases, the broader restoration packet in
section 5, unsupported effect phases in section 9, and deferred Report
restoration in sections 51–52 are not promoted by this public profile. Report
delivery from recovered bytes remains outside its admitted scope.

The following private joins are predecessors consumed by the fresh public
entry in section 12. Their historical gate notes do not extend that entry to
general restart or public shutdown.

## 6. Private second-turn composition

The private `finish_second_turn_v8` driver now consumes the actual first
Continue Step and composes the whole second turn through Observe settlement,
Start, Model Intent/dispatch/Usage, Resume/Completed, authorization, target
Intent/dispatch, Decision cleanup, Reduce, Step cleanup/transfer, terminal ACK,
and the consuming Report projection. It obtains its journal directly from the
physical predecessor, so callers cannot substitute another causal store. Entry
requires the cumulative initialized profile, turn zero, a physical Continue,
and an authenticated two-iteration ceiling. Cancellation and unsupported
iteration profiles refuse before another append or external dispatch.

Every unsuccessful join retires that journal and retains the exact reached
owner-bearing failure inside a private opaque quarantine. Its only observation
is a phase label; it exposes no owner extraction, downcast, retry, journal,
cleanup or dispatch method. The caller must retain the quarantine while its
obligation remains pending. The runtime in section 7 owns that lifetime for
the fresh public route and selects the known failed Observe and target
cleanup/Stop drivers.

Terminal evidence uses the authenticated current stage count with the existing
omitted-detail representation. The driver accepts no host-supplied count or
checked-run evidence. Only the final authenticated terminal ACK and physical
Report claim can produce the copied projection.

The `owned_composed_second_turn_` regression family covers real terminal
success and store reopen, Model-settlement and terminal prewrite faults,
cancelled admission, and refusal of a three-iteration profile. These tests are
authored but unexecuted at this change. This private composition closes a
substantial driver join; the runtime composition in section 9 joins first-turn
authorization/effect/Step orchestration. Complete runtime shutdown settlement
and general restart acceptance remain open.


## 7. Private runtime custody across caller sessions

`live_upstream/runtime` now owns one typed lifecycle slot for an exact fresh
cumulative two-turn journal. Its short-lived session handle borrows that slot;
dropping the handle cannot destroy, extract, or retry the physical owner.
Initialize, Observe, Start and Model/Resume run through the original consuming
joins. A completed first Model remains in runtime custody for the next lifecycle
transfer. Reopening a handle returns the existing status without dispatch or
append. No new checkpoint or evidence format is introduced.

Failures retain distinct admission, Initialize, Observe, Start, Model, and
failed-Observe cleanup owner types. An acknowledged failed Observe stays in its
cleanup-capable phase; it is not erased or prematurely poisoned. The runtime
can consume that exact phase after the caller handle disappears, selecting the
existing fixed State CleanupStarted, physical cleanup, receipt, and sticky Stop
path. Only the acknowledged Stop permits ordinary runtime close. An append
failure or observer failure retains the actual reached phase and cannot retry
cleanup. No supplied row or copied diagnostic can request this transfer.

`try_close` returns the same runtime for every unresolved physical obligation.
Forced destruction of the private runtime retires the journal before releasing
process backing; this is not language cleanup, a successful Stop, or a public
shutdown protocol. A successful State or quarantined owner still needs its
phase-specific continuation or recovery/settlement route. Consequently this
runtime is private and is not exported as a public session constructor. The
caller-handle lifetime is now enforced structurally, while complete runtime
shutdown and general restart acceptance remain required. Section 9 owns the
subsequent first-turn bridge and second-turn
runtime join.

The `owned_runtime_` owning selectors exercise real first-turn Model completion,
handle disposal and reopening without redispatch, runtime close refusal with the
same original backing retained, successful failed-Observe cleanup after handle
disposal, failures at each of the three cleanup append boundaries, and observer
panic. They are authored but unexecuted in this source-only batch. No public
support or issue-closure claim is made from these authored tests.


## 8. Complete Report consumption retires the inherited Reduce hold

The consuming private Report projection now settles its inherited prospective
Reduce registry membership before normal owner destruction. This transition
requires the actual claimed Report, pointer equality with that Report's unique
hold, the current authenticated Complete TerminalSnapshot witness, the same
journal container, exact hold identity, Step phase, turn, charged funding,
sequence, acknowledged bytes and authentication tail. Copying a delivery view,
claiming a Report, or reopening terminal evidence does not retire the hold.

The callback-free retirement clears only the matched registry record and marks
that unique hold completed. It does not refund funding, construct another hold,
append a row or grant another turn. Normal Drop then releases the physical
Report and inherited lineage without poisoning a completed journal. Any
validation or projection failure leaves the hold's quarantine Drop active and
returns the actual Report owner. All earlier failure phases preserve their
existing quarantine behavior.

The composed two-turn regression now checks the settled registry, usable held
store after projection, refusal of fresh reinitialization, and actual store
close/reopen. Its Model-settlement and terminal-prewrite refusal cases check
that registry membership survives quarantine. The later terminal Report tests
also check retirement on success and retained membership on projection refusal.
These strengthened regressions are authored but unexecuted in this source-only
change.


## 9. Private runtime-owned full two-turn composition

The runtime session can now consume its retained first `ModelCompleted` owner
through `finish_two_turn_run`. The `runtime/continue_run` child composes the
original Authorize, Ready/Consumed, target Intent/dispatch/settlement, Decision
cleanup/receipt, Outcome, original Reduce, Step cleanup/receipt/transfer and
Transition joins. The actual first Continue then enters the existing second-turn
driver. Each session acquisition, append, witness advance and shape refusal
retains its exact reached physical owner in the enclosing runtime; no owner,
raw journal, retry method or downcast is returned through the session.

Success consumes the actual terminal Report and retains only its checked
canonical delivery projection. The runtime exposes that inert data by shared
borrow and permits normal close after Complete. A new handle observes the same
Complete or quarantine status without a new append, dispatch or cleanup. A
first-turn failed target can now enter its separate checked cleanup/Stop driver;
cleanup-observer and other failure paths remain quarantined and refuse close.
The existing failed-Observe settlement entry remains separately available.

The `owned_runtime_two_turn_` regression family enters through genuine
Initialize and first Model, exercises both model and target calls, observes four
canonical physical cleanup callbacks, consumes the Report, closes the runtime,
and reopens authenticated terminal evidence. It covers every one of the 19
first-turn bridge prewrite boundaries, the second Model settlement and terminal
prewrite, cancellation, denied capability policy, target failure and observer
panic. Each failure also drops and reopens a session handle, refuses runtime
close, and checks that no dispatch, cleanup or append is retried. These tests
are authored and unexecuted in this source-only batch.

This closes the private first-turn orchestration and second-turn custody gaps.
The fresh public construction in section 12 consumes this runtime. Complete
runtime shutdown settlement, the broader failure cleanup matrix and general
restart acceptance remain required; issue #330 closure is not claimed.

## 10. Private Prepared restart into runtime custody

The runtime now has a consuming `restart_first_prepared` entry for the exact
cumulative two-turn profile. It accepts the existing authenticated recovered
journal and separate one-use restoration and continuation host grants. The
existing recovery validator admits only the current first-turn Prepared tail
under its read-only registered lease. The runtime takes the restored physical
owner before the existing continuation crosses into original Model execution;
no restored State or completed Model owner escapes to the caller.

Successful Model completion occupies the same runtime slot as fresh execution
and enters the existing `finish_two_turn_run` chain. Reservations, consumption,
source association and journal identity remain those authenticated from the
original run. Initialize, Observe and Start are not run again. The original
Model Intent is appended once; a current Intent, answered or terminal history
cannot be admitted by searching backward for a historical Prepared row.

A continuation failure retains its exact Parked or Resume owner inside the
runtime, reports `restart-model` quarantine, and refuses normal close. Reopening
the caller handle does not retry dispatch, append or cleanup. Forced runtime
destruction still only retires authority and releases process backing; it
does not record semantic cleanup or a successful shutdown.

The `owned_runtime_restart_prepared_` gates are authored and unexecuted. They
cover five original Model append prewrite boundaries, physical backing custody,
fresh/non-cumulative refusal, and a preparer process that exits before a
separate process independently rebuilds the checked runtime and completes both
turns. The process gate also checks cancellation before restoration, terminal
history refusal in a third process, hostile trailing bytes, original cumulative
funding, two Model dispatches, two target dispatches and four cleanup callbacks.
Complete failure settlement, post-Intent restoration,
Report restoration and the other durable phase classifications remain open.

## 11. First-turn failed-target runtime settlement

The private first-turn bridge now returns the actual `Failed` outcome as a
typed State owner to the same runtime slot. A later runtime call consumes that
owner through the existing checked failed-target State CleanupStarted,
physical cleanup, receipt, and sticky Stop driver. The caller session cannot
extract or retry the owner. Only an acknowledged Stop permits normal runtime
close; a failure at any cleanup append boundary retains the reached owner in
runtime quarantine and refuses close. This applies to the first turn only.

The `owned_runtime_two_turn_failed_target_checked_stop_and_faults` regression
is authored but unexecuted. It exercises the real failed target, one State
cleanup callback, no redispatch, normal close after Stop, and each of three
prewrite faults without retry. Other first-turn failure phases, the later
failed-target path, and general shutdown/recovery remain
open.

## 12. Fresh public entry and custody

`SourceOwnedAgentJournalV1::create_fresh` derives the owned-wait binding from
the selected source in the retained `AgentRuntimeV2` Project revision. It binds
the caller's already checked model adapter and SourceLive policy to that
runtime, requires the exact two-iteration profile, and validates the caller's
explicit cancellation and clock before creating a store. The host supplies an
open directory descriptor, protected-history assertion, checkpoint key and
registration-retention callback.
The callback receives complete inert registration facts after the exclusive
fresh file has been created, and must durably retain those facts before
acknowledging. Refusal leaves the empty file without append authority.

`run` derives the initialized Task input from the same checked source and
runtime and composes both private consuming turns. It accepts only a matching
unused adapter, current clock, caller-held capability policy and target host.
It returns `SourceOwnedAgentRunV1`, which owns the reached physical custody
while borrowing the opaque journal. A terminal `Complete` exposes a bounded
checked Report projection; an acknowledged Stop may close without a projection.
An unresolved phase remains in the handle and refuses `try_close`. Dropping a
handle with unresolved ownership retires journal authority before releasing
process backing; it is not semantic cleanup or a durable recovery permit.

The `public_owned_agent_fresh_entry_runs_two_real_turns_and_projects_report`
regression exercises the public constructor and run with a source-retained
nontrivial Agent, wrong-source preflight, full retention facts, two model calls,
two target calls, four cleanup observations, terminal projection and replay
refusal. At `46d99ddfb`, this regression passed 1/1 locally on macOS in 395.90s on an
explicit 2 MiB worker after the terminal driver and ACK handoffs were heap
staged. Its real first-target failure also reaches acknowledged State cleanup
and Stop. Broader failure, shutdown and required recovery evidence remain
separate; this entry does not close issue #330.

## 13. Public first Prepared process recovery

`SourceOwnedAgentJournalV1::recover` imports the exact complete inert registration
projection retained by the host at fresh creation. It recompiles the selected
Agent from the retained Project, checks the model adapter, SourceLive policy,
two-turn ceiling, source revision, epoch, directory identity, execution and
binding, then passes the registration to the existing physical recovery route.
That route checks the registered generation and directory/file pins before
opening the locked read-only lease. JSON import is strict: omitted, extra or
altered facts refuse. The registration remains evidence, not a host grant.

`restart_first_prepared` consumes separate one-use trusted-host recovery and
continuation grants. The existing authenticated first-Prepared validator must
reconstruct the precise physical parked owner before the original Model path
can dispatch. The same runtime custody then finishes the first and second
turns. Fresh, answered, terminal and hostile tails cannot request this entry;
failed continuation retains its owner in runtime quarantine.

`public_owned_agent_prepared_relaunch_completes_and_hostile_tail_refuses`
starts a preparer process, retains its registration outside that process, and
exits before a separate process rebuilds the checked runtime and resumes the
original first Model. It checks two model calls, two targets, four cleanup
observations, terminal Report projection, forged generation refusal, fresh
history refusal and hostile tail refusal before dispatch. At `46d99ddfb`, this
regression passed 1/1 locally on macOS in 445.90s with
`RUST_MIN_STACK=2097152`, inherited by the independent child processes. This
is exact first-Prepared recovery evidence, not general durable-phase recovery.

## 14. Continued-target failed State settlement

After the second target records an actual supported `EffectFailed` outcome and
acknowledges successful Decision cleanup, the runtime retains the original
`PendingOwnedEffectReceiptV8` State owner. It does not attempt to mint a success
Outcome or enter Reduce. A continued-turn failure lineage binds that owner to
the exact same journal, proposal, policy, cancellation token, current turn,
settlement/recorded/cleanup rows and inherited funding registry.

The existing fixed failed-State append adapter acknowledges State
CleanupStarted before physical release, then acknowledges the observed receipt
and sticky `EffectFailed` Stop. The fold already admits these existing v8 rows
for checked cumulative turns; no new wire shape or restored owner is introduced.
Only an acknowledged successful receipt and Stop produce `FailedEffectStopped`
and permit normal public `try_close`. Cancellation before a fresh boundary,
failed append ACK, observer failure or source/pin/registry mismatch retains the
reached physical owner in `continued-failed-effect-cleanup` quarantine. Incurred
release/receipt guards still exclude fresh clock/cancellation checks; Stop
restores them. Drop only releases backing after authority retirement.

The `public_owned_agent_second_target_failure_` family uses the genuine public
constructor and run on explicit 2 MiB workers. It authors success, before/after
persistence faults at State Started/receipt/Stop, State observer panic and
cancellation after State release. Every scenario checks two Model calls, two
target calls, exact physical cleanup counts, no Report projection and no retry
or further append. The public success selector passed 1/1 in 316.81s and
Started-ACK before/after-write faults passed 1/1 (two scenarios) in 615.61s on
local macOS. Receipt-ACK faults passed 1/1 (two scenarios) in 629.13s and
Stop-ACK faults passed 1/1 (two scenarios) in 632.35s from the same emitted
2 MiB binary. State observer panic passed 1/1 in 317.10s and cancelled Stop
passed 1/1 in 315.94s. All six selectors are evidence for the e97bd0828 core
packet on local macOS, not an unexecuted later source revision.
This bounded settlement does not restore a post-Intent, transferred or Report
owner after a process restart, or settle unrelated quarantined phases.


## 15. Actual first-turn Refused State cleanup

The post-Authorize driver branches on the actual checked source Refused
Decision. The admitted Refused case owns only its scalar code; both the actual
root and compiler disposal proof establish an empty Decision vector. No empty
Decision cleanup receipt is invented. The runtime acknowledges the ordinary
`AuthorizationRefused` row and exact State `OwnedCleanupStarted` before it
consumes the original State. The observed State receipt precedes sticky
`Rejected` / `StageRefused` Stop and `AuthorizationRefusedStopped` status.

Authenticated inventory admits this narrow existing-wire route only for the
current first-turn Refused snapshot, original transfer basis and wait identity,
exact source Decision terminal facts, and canonical compiler State operations.
The private physical permit can be constructed only by the live Started ACK
path. Descriptive replay does not reconstruct State or authorize disposal.
Observer panic still records the actual failed receipt and retains quarantine;
append uncertainty keeps the actual reached holder without a cleanup retry.

The public 2 MiB success regression passed 1/1 locally on macOS in 33.85s.
The runtime Started-before/after-write retention selector passed 1/1 (two
scenarios) in 54.97s. The public observer-failure selector passed 1/1 in 33.62s:
it validates the authenticated failed receipt, rejects a candidate completed
Stop, and checks unchanged persisted bytes after refused retry and forced
teardown. All three ran from the same emitted binary as this Refused/recovery
packet. General cancellation and explicit shutdown of other eligible owners
remain separate work.

## 16. Exact first TransferCompleted recovery

`restart_first_transferred_state` is a separate trusted-host entry for an
otherwise read-only recovered journal. It admits only the authenticated current
first-turn `OwnedStateTransferCompleted` tail, with the adjacent admitted
proposal and transfer reservation, exact State and proposal digests, source
binding, registration, generation, wait identity and full-prefix pins. An
explicit protected-history assertion is required; journal data alone grants no
restoration authority. Earlier snapshots and later tails cannot select it.

A non-cloneable permit is consumed by reconstruction of that one State. A
post-materialization guard failure retains the owner in an opaque quarantined
run. Every recovered Authorize failure likewise retains its actual transferred
State or staged State/Decision; closing or retrying cannot release it or append
new work. Forced host destruction retires authority and frees process backing
without invoking a semantic finalizer or manufacturing a cleanup receipt.

Successful restoration crosses the exact pinned read-only lease boundary once,
then acknowledges the ordinary Authorize fuel reservation before invoking the
same checked source evaluator as the live route. It does not rerun Initialize,
Observe, the first Model call or the helper. The existing two-turn continuation
owns all later target work, cleanup and terminal Report projection. This entry
does not restore a staged Decision, post-Intent effect, or Report from bytes.

The `public_transferred_relaunch_` gates use separate preparer and recovery
processes and explicit 2 MiB workers. Success checks one remaining Model call,
two target calls and terminal close. Prewrite Authorize-reservation and
postwrite staged-Decision faults check zero downstream Model/target/cleanup,
retained physical backing, refused close/retry and unchanged durable bytes on
forced teardown. The separate-process success selector passed 1/1 in 366.52s (33.76s
preparation and 332.74s recovery) on local macOS. Authorize fault custody passed 1/1 in 131.30s (two separate-process
scenarios); settled-without-transfer refusal passed 1/1 in 65.97s. These are
local macOS results for this bounded recovery packet, not broader destination
phase restoration. The final wrapper also applies the existing known-failure
settlement used by fresh and first-Prepared public runs.

## 17. Completed-State cancellation and explicit shutdown

`prepare_first_model` retains the actual completed first-model State in the
opaque public run before Authorize. `finish` continues that same owner through
the existing checked path. `shutdown` explicitly abandons this eligible
Completed custody through existing v8 rows. Cancellation observed by `finish`
at this boundary selects the same shutdown path before any Authorize reservation
or target dispatch. Closing a still-completed run refuses and returns its owner.

Shutdown authenticates the current first-turn Completed tail, original wait
argument digest, exact actual State, checked proposal and completed result
digest. It acknowledges `OwnedWaitFailed` with `host_abandoned`, no reservation
and zero consumed fuel, then acknowledges canonical State CleanupStarted using
the Completed basis. Only this live ACK creates the private physical permit.
The interpreter releases the actual State once in compiler cleanup order,
records the observed receipt and acknowledges `Cancelled` Stop. No evaluator,
transfer, target dispatch, new funding or restored owner enters this route.

The already selected cancellation does not invalidate the incurred cleanup:
each physical action and append still checks the exact held source, registration,
generation and acknowledged prefix. Observer panic records failure and keeps
quarantine. Append uncertainty preserves the reached owner; repeated `finish`,
`shutdown` and fresh-entry attempts cannot release, append or dispatch again.
Only successful receipt and Stop yield `ShutdownStopped` and permit close.

This public shutdown entry is intentionally restricted to the current actual
first-model Completed owner. Unknown or quarantined phases retain their status
and custody; ordinary Drop remains process-backing teardown and does not invoke
semantic cleanup. It does not recover Report or post-Intent effect ownership.

The local macOS 2 MiB public success selector passed 1/1 (explicit and cancelled
scenarios) in 64.84s. Failure-selection/Started prewrite and postwrite faults
passed 1/1 (four scenarios) in 129.71s; receipt/Stop faults passed 1/1 (four
scenarios) in 129.87s. Observer panic plus authenticated failed-receipt Stop
refusal passed 1/1 in 32.75s. The existing cancellation/denied-policy preservation
selector passed 1/1 (two scenarios) in 52.96s. These five focused selectors used
the same emitted binary for this shutdown packet; no full profile was run.
