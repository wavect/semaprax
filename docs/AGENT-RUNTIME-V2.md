# Direct Agent Runtime v2

Status: local, partial; focused typed execution and actual-root association pass.

Audience: runtime integrators and compiler contributors.

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
an artifact beyond the explicitly supplied live operation.

The focused `execution_revision::typed` integration test exercises three turns
and two distinct operations, verifies exact arguments and results, compares
registry order and budget roots, rejects a mistyped selector before dispatch,
and checks malformed-result settlement and a byte ceiling that prevents any
host dispatch. The additional durable integration passes full completed replay
with zero host calls, changed-task rejection before store access, uncertain
intent rejection, and observed-result recovery that executes only the remaining
two operations.

`run_durable` consumes the same bound producer and a caller-owned single-writer
checkpoint store. A retained snapshot must come from that authorized trusted
store; hashes do not authenticate host observations. The private producer
supplies its actual ProgramRoot and ExecutionRevision to the
[operation checkpoint implementation](AGENT-OPERATION-CHECKPOINT-V2.md).
EvidenceRoot v4 additionally binds the checkpoint digest and reserved-fuel
ceiling. Replay reserves additional fuel, so its evidence differs even when
its terminal value is unchanged. State migration remains a separate addition;
a suspended value alone grants no resume authority. Hosted support requires
exact-commit evidence and is not inferred from local execution.

[Durable migration v3](AGENT-STATE-MIGRATION-V3.md) adds a persisted handoff
and trusted-store recovery for checked migrated State. Its joined evidence
retains the handoff digest and exposes the complete recoverable checkpoint.

[Workspace Execution Association v1](WORKSPACE-EXECUTION-ASSOCIATION-V1.md)
adds exact semantic-service generation selection, authority-free receipt replay,
and consuming producer/evidence associations without changing these root bytes.
