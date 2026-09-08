# Execution root association v1

Status: local, partial; focused V1/V2/V3 and iterative root association tests pass.

Audience: compiler contributors and runtime integrators.

`execution_revision::bind_execution_revision` consumes only retained, checked
Project source and a compiler-produced ProgramRoot v1, v2, or v3. It checks the
expected root digest and independently compares all six source-owned segment
descriptors with a freshly derived retained Project root. Additional policy,
target, and projection nodes remain owned by their existing workspace producer.
No second semantic graph is created.

The selected source path must belong to the retained Project. Its checked Agent
must lower to the exact retained AgentDefinition. The existing v1-to-v2 migration
and deployment binder derive semantic definition and validate explicit deployment
bytes. The ordinary lifecycle compiler binds those deployed bytes to that same
retained source, including its ordinary proposal schema and authorization rules.

DeploymentRoot commits program, semantic definition, deployment binding, source
selection and lifecycle. InstanceRoot commits deployment, task/proposal digests,
task budget and stage fuel. ExecutionRevision joins those roots with the retained
Project revision. Task and proposal bytes are held privately, never rendered in
root documents. Each root uses compact sorted JSON with terminal LF, domain
separation by schema followed by NUL, and SHA-256 before its digest is inserted.

Consuming the opaque ExecutionRevision runs the existing lifecycle with the
explicit injected read operation and live cancellation. Only that producer can
construct EvidenceRoot, which commits the actual lifecycle evidence and exact
execution and instance roots. Failed binding invokes no host. These values grant
no filesystem, network, mutation, deployment, checkpoint, or publication authority.
They add no wire import or deserialization route and alter no frozen root bytes.

Focused gate: `cargo test --locked -p semaprax --test agent_runtime_v1
execution_revision`. This batch covers local retained-source association and
one acyclic lifecycle. Iterative execution, state migration and durable root-bound
checkpoint recovery require their own additive protocols and evidence.

## Iterative association v2

`execution_revision::iterative` adds a separate opaque
`IterativeExecutionRevision`, preserving the v1 API and bytes. It binds the exact
retained Agent and deployed Step reducer through the existing iterative lifecycle
compiler, without instantiating Runtime v1. DeploymentRoot v2 additionally binds
the Step type and iterative lifecycle digest. InstanceRoot v2 commits all ordered
proposal digests, including unused suffix entries, plus task bytes, task budget,
iteration count, stage count and per-stage fuel ceilings. At most 4096 proposals,
262144 bytes per proposal, and 2 MiB in aggregate are retained.

Consuming this revision executes its privately retained invocation and constructs
EvidenceRoot v2 from that actual producer. Even cancellation before initialization
remains bound to its exact invocation and roots. No API accepts an arbitrary run
or caller-supplied evidence to mint this association. Suspend is evidence only;
this API creates no resume or migration token.

The local focused integration selector `execution_revision` now also covers the
iterative association and passes locally. The retained interpreter
adds exact flat Copy-record calls over its five existing scalar leaves solely to
its retained-call function map, allowing Copy-only Observation stages while keeping
ordinary interpreter entry admission unchanged. Ordinary HIR validation, exact
function identity and reachable-body scanning still apply; a focused unit test
checks a make/relay call chain and rejects a forged scalar field type.

The iterative runner intersects requested iterations with the bound deployment's
`max_turns` and `max_tool_calls`, since this profile performs one read per turn.
InstanceRoot keeps the requested ceilings and separately commits deployed and
effective iteration limits. Narrowing either deployment ceiling stops before a
second read; the successful three-turn fixture explicitly admits three turns and
three calls in its source profile.

[Workspace Execution Association v1](WORKSPACE-EXECUTION-ASSOCIATION-V1.md)
adds exact semantic-service generation selection, authority-free receipt replay,
and consuming producer/evidence associations without changing these root bytes.
