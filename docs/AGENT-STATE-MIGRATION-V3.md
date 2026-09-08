# Durable Agent State Migration v3

Audience: runtime integrators and compiler contributors.

Status: locally exercised; hosted promotion remains pending. This adds durable
continuation to
[State Migration v2](AGENT-STATE-MIGRATION-V2.md).

## Handoff and authority

The existing checked migration consumes an actual durable Suspend, preserves
its cumulative usage, validates both retained programs and State schemas, and
evaluates the pure migration twice. Its opaque migrated runtime now exposes
`handoff_digest` and consuming `run_durable` methods. The latter first commits
generation zero to a caller-owned destination store. That generation contains
the complete migrated State, checked migration root, prior call/byte/fuel
usage, prior iteration/stage counts and reserved-fuel ceiling. No destination
stage or handler executes before this handoff commit acknowledges.

The caller must provide exclusive writer authority and a fresh destination
store. Any lost acknowledgement stops the invocation. Its failure exposes the
latest candidate because the store may have committed it. Recovery reads the
actual trusted-store snapshot; it does not assume a failed commit left the
old generation in place. The adapter obtains no filesystem, network, provider,
process or publication authority.

The handoff wire is `semaprax.agent-migration-handoff.v1`, bounded to 1 MiB,
with closed flat typed State, bounded counters and exact canonical JSON.
Its independently retained digest binds the complete document. The outer
`semaprax.agent-migrated-checkpoint.v1` snapshot contains that immutable
handoff plus the destination operation journal, or null at generation zero.
It is bounded to 8 MiB and rejects unknown fields and noncanonical bytes.

`resume_migrated_agent_runtime_v2` consumes independently bound previous and
destination runtimes, the trusted snapshot, a trusted expected handoff digest,
and both expected execution revisions. It checks the exact roots, migration
function and pure call closure, old/new nominal schemas, migrated field
identities/types and carried counters before returning an opaque recovered
runtime. The recovered object only offers durable execution.

Stored State is trusted producer input. Hashes and submitted checkpoint bytes
alone do not prove that migration or a physical effect occurred. The expected
handoff digest must come from the caller's trusted migration binding, not be
copied uncritically from the submitted document. Recovery restores the already
persisted migration result; it does not evaluate the migration function again.
Initial pure migration preparation remains the consuming v2 operation; this
profile does not make that pre-handoff computation itself a transactional
source-store operation.

## Destination execution and recovery

The inner operation checkpoint retains its frozen v2 wire. Its identity uses
a distinct migrated invocation domain that binds the migration root, State,
exact invocation and requested budgets. Its local ceilings subtract all carried
calls, bytes and reserved fuel. Reported usage adds local work back to the
immutable handoff baseline, with checked arithmetic.

The destination skips initialize and obtains a new checked grant on every
iteration. Its policy binds the migration root. Every retained or replayed
stage durably reserves fuel; every reservation also consumes the cumulative
stage ceiling. Repeated recovery cannot refund stage or fuel usage.

An uncertain Intent stops before any stage, store write or host redispatch.
Observed and Transition prefixes replay the checked producer under fresh grants,
compare exact context and results, and reuse retained observations without host
calls. New effects are reachable only after the prefix is consumed. A failed
terminal Transition commit preserves the already selected terminal result in
the returned failure.

`AgentRuntimeV2DurableEvidence::checkpoint()` returns the complete recoverable
snapshot; `run().checkpoint()` remains the inner journal for inspection.
Migrated typed evidence uses additive v3 bytes with explicit prior, local and
cumulative counters. The joined durable migration evidence binds the handoff,
migration, destination revision, instance and actual execution evidence.
Ordinary durable execution retains its v2 bytes.

## Repeated revisions and limits

A destination that actually suspends can supply its durable evidence to the
next checked migration. Cumulative iteration and stage counts include prior
handoffs and all replay reservations. A chained migration uses an additive v2
migration root that also binds the predecessor handoff digest. Each destination
acquires its own caller-owned store and fresh stage authorizations.

This is local retained execution with injected stores and handlers. Distributed
writer coordination, automatic reconciliation, transactional migration
preparation, cross-store exactly-once handoff and hosted promotion remain
separate work. The full Agent, language, standard-library and public ABI goals
remain open.

## Focused local evidence

`cargo test --locked -p semaprax --test agent_runtime_v1 execution_revision::typed::migration`
passes the existing consuming migration case and three durable integration
cases. They cover completed replay with zero repeated host calls, rejected
changed/swapped runtime bindings and altered handoff usage, genesis and
Intent/Observed/terminal-Transition lost acknowledgements, and A→B→C State
extension with both fresh and replayed B suspension. Both chains finish with
nine cumulative calls and iterations; replay raises the charged stage count
from 28 to 37 while preserving the predecessor handoff and State fields.

`cargo test --locked -p semaprax --lib execution_revision::typed::migration`
passes the original pure migration check and two closed handoff codec checks.
The four existing durable-driver tests and eight frozen operation-checkpoint
codec tests also pass. These are focused local checks; no full gate or hosted
success is claimed.
