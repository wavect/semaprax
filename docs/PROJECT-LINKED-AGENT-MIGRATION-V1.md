# Project Linked Agent Migration v1

Status: private additive migration; focused local imported migration, durable
recovery, selection-refusal, and workspace association checks pass.

Audience: Project, Agent lifecycle, runtime, and semantic-graph contributors.

This profile extends the authenticated linked Agent lifecycle with a checked
state migration between two retained Project source revisions. It composes the
linked lifecycle v3 and typed-effects v4 products while preserving the direct
migration v2/v3 products and all older bytes.

## Migration admission

Migration starts only from an actual durable `Suspend` produced by the previous
authenticated runtime. At least one side uses linked Project roles; a direct
source runtime may occupy the other side. The previous runtime, its durable evidence, and
the newly authenticated destination runtime are checked against their expected
execution revisions. Each linked side is bound through its retained Project
closure, and no submitted checkpoint or evidence bytes supply migration
authority.

The migration function is selected by an explicit stable migration-function ID
declared in the selected destination Agent source or explicitly function-imported
there. The selected declaration and its transitive callees form the retained
destination Project closure; unrelated functions are not selectable by alias.
That closure is authenticated with the source and Project association before
the function is reachable. The function is pure and deterministic and accepts one `OldState` parameter.
An owning State requires `own`; the existing value parameter remains admitted
for an independently checked Copy State. A borrowed parameter is rejected.
The function returns the destination `NewState` value. It is
evaluated twice against the exact retained suspended state; both results must
match. No ambient effect, typed operation, provider, or host call is admitted
inside the migration closure.

The destination source must retain the old State nominal schema exactly,
including field identities and leaf types, and must also provide the new State
schema. The migrated result must be the destination NewState nominal record.
Schema drift, an absent or forged migration ID, an import outside the
authenticated closure, an effectful callee, a non-`OldState -> NewState`
signature, or a non-deterministic result fails before destination execution.

## Roots, budgets, and state

The linked migration root is additive `semaprax.agent-state-migration.v3`.
Its `linked_sources` object contains `previous` and `destination` associations,
with null for a direct source side. The previous linked association is
`semaprax.agent-linked-source.v1`; the destination migration association is
`semaprax.agent-linked-migration-source.v1`, which additionally binds
`migration_function` and `migration_source_path`. The migration root binds
execution revisions, actual previous evidence/checkpoint,
migration-function ID and authenticated import closure, old and migrated State,
and migration limits. Its predecessor association is nullable for the first
linked migration and is required and digest-bound when the source Suspend came
from a prior migration handoff. The existing state-migration v1/v2 roots and
the ordinary linked lifecycle v3 and typed-effects v4 documents remain
unchanged.

All prior calls, argument/result bytes, iterations, stages, and reserved fuel
carry into the destination ceilings. The migration reservation is checked with
the cumulative usage before the pure call; no valid reservation is refunded.
Failure selection is sticky, including pure-call, destination-capacity, and
settlement failures, and no partial migrated result is published.

On success, the destination receives the checked NewState through an internal
seed. The destination skips `initialize`, obtains fresh stage authorizations,
and continues through the existing injected typed effect driver. A new
execution evidence root records the migration root, destination revision,
typed-effect evidence, and cumulative usage.

## Durable handoff and recovery

The existing durable producer supplies the caller-owned destination checkpoint
store and handoff protocol. Generation zero commits the complete migrated
state, migration root, predecessor association, cumulative counters, and
reserved-fuel ceiling before any destination stage or handler executes. Lost
acknowledgements remain uncertain and stop further dispatch.

Recovery requires a trusted snapshot, trusted expected handoff digest, both
expected execution revisions, and the authenticated previous and destination
runtimes. It rechecks the roots, source/import closure, old and new
schemas, migrated field identities and carried counters. Recovery restores the
persisted migration result and does not evaluate the migration function again.
Each replayed stage consumes cumulative ceilings and obtains a fresh checked
authorization; failures cannot refund prior work or replace a selected
terminal result.

Migration and recovery add no filesystem, network, process, provider,
publication, or checkpoint authority. A suspended value, submitted handoff,
digest, or evidence capsule cannot mint the migration binding.

## Boundaries and ownership

This is a private Project association. It does not add a public ABI, hosted
support, native/Wasm Agent-stage execution, live provider, distributed writer
coordination, or general `std.agent` completion. The focused `linked_agent_imported_migration_durable_recovery_preserves_state_and_usage`
and `linked_agent_migration_selection_rejects_alias_and_unimported_stable_id`
cases cover imported State migration, renamed imports selected by stable ID,
old payload preservation, skipped initialization, cumulative usage, completed
recovery without redispatch, and refusal of aliases or unimported declarations.
The existing linked-role and workspace currentness checks pass alongside them.

`src/project/agent_linked.rs` owns linked source and migration-function closure
selection from retained Project inputs. `src/project/agent_contract_facts.rs`
owns the complete Project Interaction Facts association.
`src/agent_lifecycle/iterative.rs` and its durable/effect extensions own the
linked lifecycle and typed continuation contracts. `src/execution_revision/typed_migration.rs`
and its durable handoff modules own migration roots, cumulative settlement,
trusted recovery, and the no-re-evaluation boundary. [Project Linked Agent
Lifecycle v1](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md), [Agent State Migration
v2](AGENT-STATE-MIGRATION-V2.md), and [Durable Agent State Migration
v3](AGENT-STATE-MIGRATION-V3.md) retain their respective contracts.
