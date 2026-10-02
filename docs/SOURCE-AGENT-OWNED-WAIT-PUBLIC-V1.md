# Source Agent owned wait public v1 — fresh two-turn session

Status: **normative design only; no public API or executable driver is exposed.**
Audience: SourceLive host, interpreter, journal, SDK, and runtime implementers and reviewers.

This document owns the proposed public construction boundary for the SourceLive
v8 owned-Agent route. It is intentionally smaller than issue #330: it specifies
one future fresh, interpreter-selected, cumulative run with two turns and a
`Complete` terminal. It does not claim restart, recovery, hosted support,
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

The future session will own the resulting registered lease and journal. It must
never expose the lease, `File`, registration grant, checkpoint key, append
witness, or an owner. The existing same-FD, PID, uid/mode/nlink, no-follow,
flock, scope, generation, prefix-byte, sequence, and MAC checks must remain in
force at every existing boundary.

Production export/import of registration data is inert descriptive data. It
does not become a grant. A recovered lease stays read-only unless a later
phase-specific restoration contract materializes the exact physical owner.

## 3. Success path

The future session must compose only existing consuming joins, in this order:

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

Only after the terminal ACK may a future session copy the existing checked
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
has not reached a State cleanup boundary. The failed-target State tail now has
the same private acknowledged cleanup/receipt/Stop join and owner-bearing
quarantine. Several other post-effect tails still lack a terminal join.
Consequently no public session or executable `run` method is exposed. Recovery
and terminal/report delivery from reopened bytes remain unsupported. An executable
method is admitted only with the complete failure dispatch and its regression
matrix.

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
private failure path. This does not expose a public session, lease, report,
State, recovery route, or finalizer authority.

The public recovery matrix remains separate: process restart at every durable
phase, no redispatch, no uncharged work, phase-specific restoration permits,
hostile registrations/tails, and Report recovery/delivery all remain required
to close #330.

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
obligation remains pending. This is not the runtime-owned quarantine lifetime
required for a public session. Failed Observe and target paths currently stop
in this quarantine rather than selecting their separate cleanup/Stop drivers.

Terminal evidence uses the authenticated current stage count with the existing
omitted-detail representation. The driver accepts no host-supplied count or
checked-run evidence. Only the final authenticated terminal ACK and physical
Report claim can produce the copied projection.

The `owned_composed_second_turn_` regression family covers real terminal
success and store reopen, Model-settlement and terminal prewrite faults,
cancelled admission, and refusal of a three-iteration profile. These tests are
authored but unexecuted at this change. This private composition closes a
substantial driver join; the runtime composition in section 9 joins first-turn authorization/effect/Step orchestration. Public request construction,
complete runtime shutdown settlement, and general restart acceptance remain open.


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
shutdown, public request construction and general restart acceptance remain
required. Section 9 owns the subsequent first-turn bridge and second-turn
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
Complete or quarantine status without a new append, dispatch or cleanup. Failed
target and cleanup-observer paths remain quarantined and refuse close; this
composition does not pretend those failures reached a cleanup/Stop terminal.
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
Public request construction, complete runtime shutdown settlement, the broader
failure cleanup matrix and restart-to-terminal restoration remain required;
no public lifecycle support or issue #330 closure is claimed.
