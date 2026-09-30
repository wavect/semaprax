# Direct Agent Runtime v2

Audience: runtime integrators and compiler contributors.

Direct Runtime v2 binds checked Project code, deployment, typed effects, and
one invocation before execution. The resulting roots describe that execution;
they do not grant host or publication authority.

Status: **HOSTED GREEN** under the [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md)
for typed execution, actual-root association, and implemented durable paths.

`bind_agent_runtime_v2` consumes an exact retained Project selection, an already
admitted ProgramRoot v1/v2/v3, source Agent and Step identities, a deployment,
typed effect registry, and one bounded invocation. It authenticates every
source-owned root segment against the retained Project and derives the Agent
Definition from the selected source before binding deployment or effect facts.
It compiles the [typed iterative product](AGENT-TYPED-EFFECTS-V3.md) directly.
The frozen Runtime v1 profile remains a compatibility carrier in source; this
path never instantiates the Runtime v1 action loop or translates typed calls
into Runtime v1 actions.

The operation registry is ordered and selected by an exact checked `usize`
Proposal field. Its operation, effect, argument and result identities must
match the deployed source contracts. Typed arguments come from the checked
Proposal projection; typed results pass exact ordered field/type checks. This
bounded version transports five scalar result kinds through canonical typed
fields in the existing Outcome Bytes carrier. It does not add arbitrary typed
nominal reducer results, provider transport, public ABI or ambient capabilities.

DeploymentRoot v3 binds the ProgramRoot, source selection, semantic definition,
deployment, binding, Step and complete typed registry digest. InstanceRoot v3
binds task bytes and budget, every ordered Proposal byte sequence including any
unused suffix, stage ceilings, effect ceilings, and effective deployment turn
and call ceilings. ExecutionRevision v3 joins those roots to the retained
Project revision. Registry reordering changes the deployment association;
narrowing an invocation budget changes its instance association.

The runtime retains all invocation inputs privately and is consumed by `run`.
Only that actual producer constructs EvidenceRoot v3, committing its immutable
typed-effect evidence together with the InstanceRoot and ExecutionRevision.
There is no method accepting caller-authored evidence or an arbitrary run.
Malformed host results stop subsequent dispatch and retain measured byte work.
Cancellation and budget exhaustion are also evidence-bearing outcomes. These
roots describe execution; they grant no authority to invoke a handler or publish
an artifact beyond the explicitly supplied live operation. Target request and
observation replay also bind the same non-authorizing authorization digest, so
a serialized observation cannot splice its authorization attribution from a
different retained request.

The focused `execution_revision::typed` integration test exercises three turns
and two distinct operations, verifies exact arguments and results, compares
registry order and budget roots, rejects a mistyped selector before dispatch,
and checks malformed-result settlement and a byte ceiling that prevents any
host dispatch. The additional durable integration passes full completed replay
with zero host calls, changed-task rejection before store access, uncertain
intent rejection, and observed-result recovery that executes only the remaining
two operations.

`bind_agent_runtime_v2_live` is an additive source-proposal binding with no
submitted proposal inventory. `AgentRuntimeV2::run_live` rejects a runtime that
was bound with frozen proposal bytes, routes the ordinary `ProposalSource` only
through the checked iterative lifecycle, and captures the canonical proposal at
the lifecycle's effect boundary. The typed effect dispatcher therefore cannot
run for a malformed stream, cancellation before adapter construction, or a
completed stream whose settlement bytes disagree. The supplied source remains
an explicit host capability; this route creates no transport, provider, or
checkpoint authority.

[Source Model Operation v1](SOURCE-MODEL-OPERATION-V1.md) adds the bounded
`run_live_bound_model` route. It commits one deployment-admitted
provider/model selection whose declared capabilities satisfy the source
requirements, adapter identity/profile, source revision, compiler Proposal
grammar and current instance before the adapter can start. It retains redacted
model-attempt evidence in additive EvidenceRoot v4. The ordinary `run_live`
surface and its v3 root remain the compatibility route. The one-pass bound
adapter route is local; the separate explicit checkpointed route below adds
only caller-store source-journal recovery. Neither claims a hosted provider or
target-runtime transport.

Its opt-in `new_bound_with_policy` route composes the existing
`ModelPolicyLedger` with a host request-bound `ModelAttemptQuote`: source and
deployment ceilings plus an invocation ceiling are intersected before a fresh
attempt can construct an adapter. Current deployment documents bind one
provider only; retry and failover transitions remain unavailable here.

`new_bound_checkpointed` and `run_live_bound_model_durable` add the unpriced
durable source route through the existing Source Live Journal v2 cursor. The
adapter derives the exact canonical prompt identity, waits for the durable
attempt-intent acknowledgement before factory construction or `start`, and
then persists the raw bounded settlement or closed failure row. A retained
unresolved intent refuses recovery rather than redispatching. The runtime
derives a journal program-root profile from its ProgramRoot, typed registry,
and effective effect ceilings, so a checkpoint cannot be reopened with a
wider effect budget or a different registry. The priced in-memory policy route
does not have durable reservation carry and is refused by this entry.

`run_durable` consumes the same bound producer and a caller-owned single-writer
checkpoint store. A retained snapshot must come from that authorized trusted
store; hashes do not authenticate host observations. The private producer
supplies its actual ProgramRoot and ExecutionRevision to the
[operation checkpoint implementation](AGENT-OPERATION-CHECKPOINT-V2.md).
EvidenceRoot v4 additionally binds the checkpoint digest and reserved-fuel
ceiling. Replay reserves additional fuel, so its evidence differs even when
its terminal value is unchanged. State migration is an implemented additive
contract; a suspended value alone still grants no resume authority. Hosted
evidence for the admitted runtime is green at the v0.4.0 baseline, without
promoting unimplemented provider transports or general public ABI support.
`run_durable` and its migration-seeded counterpart `run_durable_from_seed`
themselves select only the interpreter; a local, test-only backend selector
(`run_durable_on`/`run_durable_from_seed_on`, see the
[iterative lifecycle's checkpoint-route parity evidence](AGENT-ITERATIVE-LIFECYCLE-V2.md))
additionally proves the same canonical checkpoint bytes decode and continue
correctly under native C11 or Core Wasm, without changing either production
entry.

## Live Repair Smoke v1 and the repair publication boundary

[Live Repair Smoke v1](LIVE-REPAIR-SMOKE-V1.md) adds `agent_runtime_v2::live_smoke`
and `agent_runtime_v2::repair_approval`. Neither adds a runtime root, a transport,
or a provider: they are the gate in front of a paid call and the gate in front of
publishing a repaired candidate.

`LiveRepairSmokeTarget` binds one retained-Project selection, resolving its source
path inside the retained inventory and its repair target through the ordinary
candidate semantic-delta route, and fails closed on drift. `LiveRepairSmokePlan`
joins that target to one bound `SourceModelBinding` and *derives* its effective
ceiling through the existing `policy_binding` intersection rather than accepting
one. `LiveRepairSmokePlan::preflight` evaluates readiness, dispatches nothing, and
states `dispatched: false` and `provider_dispatch_count: 0` as facts of its own
receipt. `OperatorLiveSmokeGrant` is the separate human act: minted only by an
explicit host call or its own canonical replay, it binds one plan digest and may
only narrow, never widen, the derived ceiling. `LiveRepairSmokeRecord` records the
true outcome, including provider failure, budget exhaustion and cancellation, and
refuses a record whose reported usage exceeds what was authorized.

No live provider call has been made through this route in this repository. The
preflight's honest current answer is `ready: false`, blocked on
`operator_grant_present`.

`repair_approval` keeps publication a separate act again. `RepairCandidateReview`
regenerates the source diff, semantic delta and impact summary from the candidate
itself and records the host's validation results, declared blind spots and journal
binding. `RepairCandidateApproval::approve` is distinct from deriving a review and
names one exact candidate digest, and
`prepare_approved_repair_publication`/`apply_approved_repair_publication` refuse an
unnamed candidate before delegating to the unmodified `project` publication
boundary, which performs its own independent replay and remains the only authority
that pivots `ACTIVE`.

[Durable migration v3](AGENT-STATE-MIGRATION-V3.md) adds a persisted handoff
and trusted-store recovery for checked migrated State. Its joined evidence
retains the handoff digest and exposes the complete recoverable checkpoint.

[Workspace Execution Association v1](WORKSPACE-EXECUTION-ASSOCIATION-V1.md)
adds exact semantic-service generation selection, authority-free receipt replay,
and consuming producer/evidence associations without changing these root bytes.

[Project Linked Agent Lifecycle v1](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md)
and [Project Linked Agent Migration v1](PROJECT-LINKED-AGENT-MIGRATION-V1.md)
add authenticated imported-role and migration closures. They reuse this runtime
producer while retaining their separately versioned lifecycle and effect wires.
