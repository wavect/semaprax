# Hot Reload Session v1

Status: partial local library profile for HR-04. The prepared interpreter lane
has a checked revision coordinator. Source-Agent handoff selection now has a
same-supervisor lifecycle, while durable migration, checkpoint claim, and
destination execution remain owned by the source-live migration protocol.

## Boundary

`HotReloadSession` owns one prepared Project interpreter, its active checked
`ProjectRevision`, a monotonically increasing generation, one pending checked
candidate, and a terminal uncertainty bit. The caller supplies immutable
revisions and any capabilities needed to construct them. The session has no
watcher, source discovery, filesystem, network, provider, process, or
publication authority. It can replace the prepared interpreter only between
invocations. It cannot migrate an executing stack, native library, Wasm memory,
outstanding FFI resource, callback, or source-Agent state.

Candidate admission runs the ordinary Project check before retaining a pending
revision. Planning reads retained checked HIR; it does not touch the active
worker. The plan is an opaque in-memory value. Its JSON representation has
`authority: none` and cannot be parsed back into a plan. The plan digest binds
the generation, submission identity, exact Project and Program roots, selected
entry/test identities, decision, and reason. The digest is an integrity check,
not permission. Activation independently rebuilds the plan and delegates the
whole-state pivot to `PreparedProjectInterpreter::replace_revision`.

The session exposes its retained worker's opaque in-process identity so a
development UI can observe continuity across A-to-B-to-C activation. It is
neither a stable wire value nor activation authority. Each completed execution
keeps its ordinary revision-bound source trace: an A trace remains replayable
only against A after B or C is active.

The session also exposes one bounded in-process lifecycle observation. It
contains only the last lifecycle state, generation, active revision and an
optional pending revision. `candidate_admitted`, `waiting_for_safe_point`,
`activated`, ordinary `refused`, and `terminal_uncertainty` distinguish the
coordinator outcomes without retaining plans, source, traces, capabilities or
a second transport protocol.

## Current compatibility rule

The local prepared-interpreter lane derives the callable closure from each
checked entry/test root, including calls in checked pre/postconditions,
instantiated targets, function references, and every checked target compatible
with a reachable indirect invocation. It admits a changed code body only when
the entrypoints,
permit set, type and interface records, exact reachable callable stable-ID set,
return/parameter types and ownership, declared effects and yields, checked
pre/postconditions, cleanup inventory/plan, and loan plan agree. A missing or
ambiguous closure target fails closed. Unreachable functions remain outside
this local prepared-worker state compatibility decision.
For every source Agent, planning retains the predecessor and candidate
AgentDefinition, AgentGraph, Runtime v1 profile, State type identity, Proposal
and Observation schema digests in a stable-ID ordered opaque handoff row. Its
v2 digest binds the row schema and every endpoint fact. The row contains no
checkpoint bytes, lifecycle binding, store, host capability, or migration
function. It is therefore a selection record for the source-live migration
owner, never permission to restore or run a checkpoint. A source-Agent plan
enters the supervisor states `waiting_for_checkpoint` and
`migration_required`; the source-live adapter replays the row against both
retained Projects, then still authenticates the predecessor checkpoint,
selection, schema transition, pure migration and destination journal before
one destination traversal. Only that traversal may mark the supervisor
`activated`; the prepared interpreter never pivots or dispatches this Agent.
The bounded lifecycle observation records the corresponding
`waiting_for_safe_point`, `activated`, ordinary `refused`, or
`terminal_uncertainty` transition while the handoff-status accessor retains its
more specific source-Agent state.
An acknowledged-journal ambiguity terminalizes the supervisor as
`terminal_uncertainty`, without in-memory retry or rollback. The physical CLI
claim remains a cooperating-CLI single-destination rule: a post-claim,
pre-settlement crash needs explicit operator reconciliation. A changed row
does not grant a policy or capability widening.
This rule remains conservative and incomplete: it is a local
prepared-interpreter compatibility decision, and is not a general hot reload
guarantee.

## Transition table

| State and request | Result | Active runtime |
| --- | --- | --- |
| Live; admit a valid candidate | Replace the one pending candidate and advance submission identity | Unchanged |
| Live; invalid candidate or exhausted submission identity | Reject with `invalid_candidate` or `generation_exhausted` | Unchanged |
| Live; plan pending candidate | Read-only eligible, unchanged, unsupported, or rejected decision | Unchanged |
| Live; activate matching eligible plan | Delegate one worker pivot; advance generation on acknowledgement | New complete revision |
| Live; plan source-Agent-compatible candidate | Emit checkpoint-handoff selection facts; source-live coordinator may enter `waiting_for_checkpoint` | Unchanged |
| Waiting; authenticated checkpoint and checked State migration prepared | Enter `migration_required`; no destination dispatch yet | Unchanged |
| Migration required; source-live destination traversal acknowledges | Mark `activated`; advance supervisor generation without pivoting the prepared interpreter | New source-journal generation |
| Waiting or migration required; source journal has ambiguous acknowledgement | Enter `terminal_uncertainty`; no in-memory retry or rollback | Unknown; explicit journal recovery/reconciliation required |
| Live; activate stale generation or superseded candidate | Reject `stale_generation` or `stale_candidate` | Unchanged |
| Live; worker has an outstanding invocation | Reject `busy_boundary`; preserve pending plan for an explicit later attempt | Unchanged |
| Live; activate identical or incompatible plan | Reject `identical_revision`, `incompatible_closure`, `policy_changed`, or `unsupported_target` | Unchanged |
| Live; worker rejects candidate before pivot | Clear pending candidate and return its diagnostics | Old complete revision |
| Live; worker panic or lost acknowledgement | Enter `terminal_uncertainty`; refuse later operations | Unknown; no rollback claim |
| Terminal; any admission, plan, activation, or execution | Refuse `terminal_uncertainty` | Unknown |

At most one candidate and one worker request are retained. Generation and
submission counters use checked `u64` increments; their first over-bound
increment refuses before changing session state. Candidate source, checked HIR,
worker stack, trace, and report bounds remain the ordinary Project and
prepared-interpreter limits. No queue or automatic retry is introduced.

The session emits `SPX-HR400` for its own typed refusals and preserves the
underlying Project and prepared-interpreter diagnostics for those owners'
failures. A caller must inspect the typed reason as well as the diagnostic.

The focused local gates use transition tables at
`project::hot_reload::tests::coordinator_transition_table_preserves_the_active_revision`
and `agent_runtime_v1::source_migration::source_agent_handoff_supervisor_activates_once_and_terminalizes_lost_ack`.
They cover admitted and activated code replacement, incompatible closure,
checked stale and identical refusal, generation-overflow and first-over-bound
submission refusal, busy safe-boundary retention, and terminal uncertainty;
the source-Agent table covers checkpoint waiting, activation, clean refusal and
terminal uncertainty. Every ordinary refusal preserves the active revision. The
`project::hot_reload::tests::indirect_changed_effect_is_refused_by_a_session_and_keeps_active_worker_usable`
gate carries a changed declared effect on a compiler-derived indirect target
through session admission, planning, refusal and a subsequent active-worker
execution. Separate cases cover changed contract
and entry identity, missing imported stable IDs, forged plans, and one
physically paused A invocation: B remains pending with
`waiting_for_safe_point`, A's trace remains bound to A, then the same worker
activates B. They also check retained-worker release when that session ends,
replacement panic before the pivot, and a post-pivot lost acknowledgement that
terminalizes without retry. This is local library evidence, not hosted or
source-Agent handoff evidence.

## Completion work

HR-01 still needs physical source-Agent durable-checkpoint execution evidence
and broader adversarial effects coverage.
