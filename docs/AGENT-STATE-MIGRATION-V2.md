# Agent State Migration v2

Audience: runtime integrators and compiler contributors.

Migration v2 moves an actual durable `Suspend` to a newly bound runtime by
calling a checked, deterministic State migration function. It carries usage
forward but grants no host or store authority.

Status: **HOSTED GREEN** under the [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md)
for the pure call, cumulative accounting, and joined-runtime integration.

`migrate_suspended_agent_runtime_v2` consumes three independently bound
inputs: the previous `AgentRuntimeV2`, the `AgentRuntimeV2DurableEvidence`
produced by that runtime's live durable invocation, and a newly bound
destination runtime. The previous and destination execution revisions are
checked against caller supplied expectations; the durable evidence must belong
to the previous revision; and the program roots must differ. A terminal result,
an effect failure, a fabricated checkpoint, or stale revision is refused
before migration or destination host work.

The producer must have reached an actual `Suspend`. Its checkpoint journal is
parsed only to count the producer's acknowledged stage reservations, while the
retained state is taken from the checked lifecycle run. Evidence and checkpoint
bytes carry no migration authority. The old state is required to be a bounded
flat record with persistent identity and scalar or byte leaves. The destination
must retain the old state schema and provide a second bounded flat state schema.

The selected migration function is checked as a pure, deterministic
`OldState -> NewState` call. It is evaluated twice against the exact retained
state and both outcomes must match. The result must be a destination nominal
record. Migration fuel is reserved cumulatively with prior usage and bounded by
the supplied ceiling; prior calls, bytes, iterations, and stage reservations
must fit the destination ceilings before the pure call is admitted.

On success the destination runtime receives the checked migrated state through
an internal seed. Its next live invocation obtains fresh stage authorizations
and may perform new injected host work. Migration evidence reports the
previous and destination roots, the pure migration root, cumulative usage, and
the destination typed effect evidence. No checkpoint authority, host
capability, filesystem, network, provider, or publication authority crosses
the migration boundary.

The focused integration gate uses the shared typed fixture, mutates the old
reducer to produce a real `Suspend`, adds a pure state extension to the
destination source, and verifies three prior calls plus three post-migration
calls, cumulative bytes and reserved fuel, increased stage counts, stale
revision rejection, and destination ceiling exhaustion before host work.

The integration also verifies that the destination does not execute initialize,
that the migrated State survives a different destination task input, and that
an exhausted pure migration returns its reserved fuel in failure accounting.
This v2 destination continuation produces in-memory execution evidence.
Persisted handoff, checkpoint recovery, and repeated migration chains are
implemented by the additive v3 contract rather than added to this frozen v2
profile. Automatic reconciliation remains separate functionality.

[Durable migration v3](AGENT-STATE-MIGRATION-V3.md) extends this consuming
preparation with persisted handoff and destination recovery. Its release
evidence is hosted green under the same v0.4.0 baseline, with its own wire and
trust boundaries.

## Held-target destination continuation

The additive `MigratedAgentRuntimeV2::run_with_backend` library method accepts
`TargetStageBackend::Interpreter`, `Native` with a caller-held compiler, or
`CoreWasmHeld` with a caller-held Node runtime. It continues the already migrated
State through the same seeded driver and sealed stage dispatch as `run`.
`run` retains its interpreter default. No destination initialize is executed;
the continuation retains prior calls, bytes, iterations and stage reservations,
and obtains fresh authorizations for its new effects. Evidence schemas and
reservation accounting are unchanged. Reservation totals are target-neutral;
lifecycle evidence retains backend-specific instruction counts, so the resulting
evidence roots need not be byte-identical across targets.

The selected Wasm route requires the destination registry's own retained source
before any destination reservation or handler call; missing source is refused,
with no fallback. Cancellation present before the first destination stage
returns `Cancelled` with no stage or effect work and preserves all prior charges,
including the already completed pure migration call's reservations.

The focused local target gate is
`cargo test --locked -p semaprax --test agent_runtime_v1 selected_migration_continuation_preserves_state_usage_and_precancellation`.
It compares the default and three public selectors using an actual durable
predecessor suspension and checked migration, including identical continuation
results, stage outcomes, cumulative accounting and cancellation before work.
It independently verifies each evidence root while checking instruction counts
separately: interpreter stages report their steps; native and Wasm report zero.
The library refusal gate is
`cargo test --locked -p semaprax --lib selected_migration_missing_wasm_source_refuses_before_reservation_or_dispatch`.
Held-target tests require explicitly opened `clang` and `node` tools and skip
when those optional tools are unavailable; skipped legs are not target evidence.
Executed local legs do not establish hosted evidence. The pure `OldState -> NewState` migration call
still executes twice on the interpreter during ordinary preparation.

## Held-target pure migration and durable metering

`migrate_suspended_agent_runtime_v2_with_backend` evaluates the same checked
pure call twice on the caller-selected Interpreter, held native C11, or held
Core Wasm target. It requires a separate semantic-fuel limit in the admitted
`1..=1_000_000` interval. This limit charges the existing semantic events, not
interpreter instructions: the two compiled backends report zero instruction
steps while all three targets report the same semantic fuel and termination
facts. Native and Wasm additionally retain their ordered physical cleanup-plan
finalizer events; the interpreter reports no physical finalizer sequence.

The selected target and semantic-fuel limit are checked before migration fuel
is reserved. Missing retained Wasm source or an unsupported semantic closure
therefore refuses before a compiler, runtime, or destination handoff can run.
The resulting additive `semaprax.agent-state-migration.v4` root binds both
metered evaluations, their target-specific instruction and cleanup observations,
and the target/registry binding. Its two receipts must agree on common semantic
work and copy-out cleanup before they can become a handoff; instruction counts
remain target-specific observations. The v4 root becomes part of the existing
durable handoff, so recovery restores the already charged migration result and
does not repeat either target evaluation. The ordinary v1-v3 roots, reservation
accounting, handoff wire, and interpreter-default API remain unchanged.

The focused target gate is
`cargo test --locked -p semaprax --test agent_runtime_v1 selected_migration_continuation_preserves_state_usage_and_precancellation`.
It uses a real durable suspension, checks all three held target selections for
both pure migration and continuation, and distinguishes target instruction
counts from common semantic charges and target cleanup observations.
The linked-source regression
`linked_wasm_pure_migration_refuses_before_reserving_or_evaluating` selects a
held Wasm target for an imported migration closure, which deliberately has no
single-module Wasm source. It asserts refusal before either the pure-call fuel
reservation or a target evaluation, preserving the predecessor's usage,
iterations, and stage count.
