# Project Linked Agent Lifecycle v1

Status: private additive binding; focused local linked-role and standard-package
evidence passes, while broader lifecycle support remains pending.

Audience: Project, Agent lifecycle, runtime, and semantic-graph contributors.

This profile binds the typed iterative Agent runtime to an authenticated Project
role closure. It lets the runtime consume deterministic Agent roles retained by
the Project dependency closure while preserving the existing direct binder and
all older Runtime, Definition, and lifecycle bytes.

## Authenticated Project role closure

`ProjectRevision::linked_agent_program` selects a retained source path and
stable Agent identity, derives its Definition from the ordinary checked source,
and compares that canonical Definition with a retained Project AgentDefinition.
It replays the complete retained source set through the ordinary bounded
semantic-workspace preflight and requires the replayed manifest and workspace
revision to equal the retained Project. The resulting linked program is an
internal role closure, not an exported owned signature.

The closure retains the four deterministic roles `initialize`, `observe`,
`authorize`, and `reduce`. Each role must be present in the checked closure and
effect-free. Existing linker, HIR validation, ownership, contract, and cleanup
checks remain authoritative. The association is the compiler-produced
`semaprax.agent-linked-source.v1` root, binding Project and workspace revisions,
the selected source graph revision, source path and revision, Agent identity,
Definition digest, role IDs, and the retained function inventory. The complete
Project association is held separately by the
`semaprax.agent-interaction-contract-facts.v1` Interaction Facts product;
the linked-source root does not replace that Project-owned association.

`ProjectRevision::linked_agent_proposal_schema` derives the existing Proposal
schema from this same checked linked closure. Proposal and Observation retain
their frozen v1 schema identities and are bound to the actual selected source
graph revision; the method is read-only and conveys no runtime or provider
authority.

Source-authored `std.agent` data, when supplied, is ordinary checked source
input. It is not sealed authority and cannot replace the retained Project
closure, mint execution evidence, or authorize a host operation.

## Source package data

The private `std.agent` package selects `owned-data-api.v1` with no public
exports or effects. Its ordinary records are `Task { objective: Bytes,
budget: i64 }`, `Context { objective: Bytes, budget: i64, epoch: i64 }`,
Copy `Observation { budget: i64, epoch: i64 }`, and `Outcome { value: Bytes,
status: i64 }`. Their explicit `std.agent.*` field and type identities are
retained in the [generated catalog](STANDARD-LIBRARY-CATALOG.md).

`initialize` consumes Task, transfers its objective and budget to Context,
and initializes epoch to zero. `observe` borrows Context and returns its
budget and epoch as Copy observations. `advance` consumes Context, preserves
objective and budget, and increments epoch; its precondition is
`0 <= epoch < i64::MAX`. `outcome_status` borrows Outcome and returns status;
`outcome_bytes` consumes Outcome and transfers its exact Bytes value.
These signed values are application data, not runtime budget grants or
status authority. The library introduces no allocation site or host operation.
Applications retain their own Proposal, Decision, authorization, and Step
semantics, while the checked lifecycle selects the explicit role/type IDs.

## Additive runtime binder

`bind_linked_agent_runtime_v2` is an additive sibling of
`bind_agent_runtime_v2`. It accepts the authenticated Project and
`ProgramRootRef`, expected ProgramRoot digest, source and Agent identities,
Step and selector identities, typed operation registry, deployment source,
bounded task and proposal inputs, iterative budget, and effect budget. It uses
the linked closure to compile the additive lifecycle and typed effect registry
described below, then creates the existing typed runtime producer.

The linked path authenticates all retained ProgramRoot segments: source
projection, semantic program, stable identity index, dependency closure,
contracts and tests, and Agent definitions. It checks the selected source and
Definition again, binds the deployment, narrows the effective turn/call
budget, and rejects stale roots, missing roles, mismatched definitions, or
invalid capacities before execution.

The linked source association is embedded into the additive lifecycle document
as `semaprax.agent-iterative-lifecycle.v3`. Linked typed effects use
`semaprax.agent-typed-effects.v4`; the direct lifecycle v2 and typed-effects v3
schemas and bytes remain unchanged. The resulting roots are the existing
additive typed-runtime forms:
`semaprax.deployment-root.v3`, `semaprax.instance-root.v3`, and
`semaprax.execution-revision.v3`. The consuming runtime `run` alone may join
the actual typed lifecycle result into `semaprax.evidence-root.v3`; caller
authored evidence and arbitrary runs are not accepted. Proposal bytes, task
bytes, budgets, registry identities, source revisions, and Project roots remain
digest-bound in the same order as Direct Agent Runtime v2.

The runtime still executes through the caller-supplied typed effect handler and
cancellation input. This profile does not add ambient authority, a provider,
or a new operation registry. Its current execution model is the injected
driver path; no native C11 or Wasm Agent-stage execution claim follows from
this binding.

The linked runtime may use the existing durable producer and its caller-owned
checkpoint boundary. Migration to another ProgramRoot revision and migration resume remain rejected for this linked path
until an additionally authenticated migration-function closure is supplied;
a suspended value or an unbound linked closure cannot create migration
authority.

## Compatibility and boundaries

`bind_agent_runtime_v2` and the prior iterative lifecycle remain available with
their existing direct source path and canonical bytes. This profile does not
translate or rewrite the frozen Runtime v1 action loop, and it does not alter
AgentDefinition v1, AgentGraph, existing Project manifests, public descriptors,
or public nominal/owned ABIs. Linked role closure is private Project
composition only.

Focused local evidence now passes all six `linked_agent_*` cases in
`agent_runtime_v1::execution_revision::typed`, covering imported three-turn
roles, standard-package roles, imported-body/root drift, intent/observed-ack
recovery with completed replay, suspend-terminal replay, and migration refusal.
The four `testing` package cases also pass across the interpreter, native C11
`-O0`/`-O2`, and Core Wasm, including the `std.agent` epoch `-1`/maximum
boundaries and the `std.test.bytes` regression. This evidence is local and
injected-driver scoped; linked migration, native/Wasm Agent-stage execution,
live providers, hosted support, and the full `std.agent` scope remain open.

## Owning implementation

`src/project/agent_linked.rs` owns retained Project source replay, linked role
closure authentication, deterministic-role selection, the linked-source
association, and the read-only linked Proposal schema method.
`src/project/agent_contract_facts.rs` owns the complete Project Interaction
Facts association. `src/agent_lifecycle/iterative.rs` owns the v2 stage and
Step binding plus linked lifecycle v3 embedding; its effect extension owns
linked typed operation compilation and v4 schema selection.
`src/execution_revision/typed.rs` owns `bind_linked_agent_runtime_v2`, the
typed DeploymentRoot/InstanceRoot/ExecutionRevision v3 products, and consuming
EvidenceRoot v3 publication. [Agent iterative lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md)
and [Direct Agent Runtime v2](AGENT-RUNTIME-V2.md) retain their respective
stage and runtime contracts.
