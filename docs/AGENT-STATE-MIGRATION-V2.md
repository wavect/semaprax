# Agent State Migration v2

Status: local, partial; pure-call, cumulative-accounting and joined-runtime
integration checks pass for migration from an actual durable `Suspend`.

Audience: runtime integrators and compiler contributors.

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
The destination continuation currently produces in-memory execution evidence;
persisted migration handoff, checkpoint recovery of that continuation, repeated
migration chains, reconciliation and hosted support remain separate work.

[Durable migration v3](AGENT-STATE-MIGRATION-V3.md) extends this consuming
preparation with persisted handoff and destination recovery. Its evidence
is tracked separately.
