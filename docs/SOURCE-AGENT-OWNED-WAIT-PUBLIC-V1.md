# Source Agent owned wait public v1 — fresh two-turn session

Status: **normative design only; no public API or executable driver is exposed.**

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
