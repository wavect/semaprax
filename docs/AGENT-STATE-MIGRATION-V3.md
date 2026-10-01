# Durable Agent State Migration v3

Audience: runtime integrators and compiler contributors.

Migration v3 adds a durable handoff and trusted-store recovery to
[State Migration v2](AGENT-STATE-MIGRATION-V2.md). The destination commits its
initial migrated State before running a stage or handler.

Status: **HOSTED GREEN** for bounded v0.4.0 continuation under the
[release baseline](RELEASE-0.4.0-STATUS.md).

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

This is retained execution with injected stores and handlers, with hosted-green
release evidence. Distributed writer coordination, automatic reconciliation,
transactional migration preparation, and cross-store exactly-once handoff
remain separate work. The full Agent, language, standard-library and public
ABI goals remain open.

## Focused evidence

`cargo test --locked -p semaprax --test agent_runtime_v1 execution_revision::typed::migration`
selects the existing consuming migration case and durable integration cases.
The original three durable cases cover completed replay with zero repeated
host calls, rejected changed/swapped runtime bindings and altered handoff
usage, genesis and Intent/Observed/terminal-Transition lost acknowledgements,
and A→B→C State extension with both fresh and replayed B suspension. Both chains
finish with nine cumulative calls and iterations; replay raises the charged
stage count from 28 to 37 while preserving the predecessor handoff and State
fields.

`cargo test --locked -p semaprax --lib execution_revision::typed::migration`
selects the original pure migration check and closed handoff codec checks.
The existing durable-driver and frozen operation-checkpoint codec regressions
remain part of the evidence. The earlier focused local runs are historical
witnesses; the current released implementation is **HOSTED GREEN** under the
v0.4.0 baseline. This does not complete the broader gates listed above.

`cargo test --locked -p semaprax --lib
agent_lifecycle::iterative::effects::durable::tests::migration_seeded_checkpoint_restores_on_a_different_backend_than_it_saved_on`
adds local, test-only backend-selection evidence for the destination-side
durable driver this migration route resumes through: the same
`run_durable_from_seed_on` entry described in
[the iterative lifecycle's checkpoint-route parity evidence](AGENT-ITERATIVE-LIFECYCLE-V2.md),
exercised here against a hand-built seed rather than the full checked
handoff/snapshot pipeline. It requires an explicit held `clang` and `node`.
The default `resume_migrated_agent_runtime_v2`/`run_durable_from_seed`
route continues to select the interpreter. Additive `run_durable_with_backend`
on fresh and resumed migrated runtimes selects an explicitly held native or
Core Wasm stage host through the same checked handoff and persisted journal.
Invalid Core Wasm source selection is refused before the destination handoff
is staged. Additive
`migrate_suspended_agent_runtime_v2_with_backend` selects that same held target
for the consuming pure migration before the handoff is created. It records two
metered migration evaluations in the v4 migration root: instruction steps and
physical finalizer events remain target-specific, while semantic fuel is the
common charge. The root is immutable handoff input, so a recovered destination
does not repeat either evaluation or charge. The parity evidence above is
local; hosted migration evidence and full target cleanup-event parity remain
open. Recovery accepts finalizer rows only in producer-representable form: a
bounded ordered sequence of nonempty bounded function identities and `u32`
liveness flags. It refuses malformed target cleanup evidence before a durable
destination can treat the handoff as settled.

Fresh and resumed migrated runtimes also expose
`run_durable_metered_with_backend` for the explicit metered checkpoint profile.
It retains one authenticated semantic-work receipt per committed destination
stage and returns a distinct
`semaprax.evidence-root.durable-migration-metered.v1` association binding the
migration handoff, selected target-and-fuel binding, typed-effect evidence,
checkpoint and semantic-work digest. The selected binding identifies the held
target and fuel profile; its target-specific instruction observations stay
separate from the common semantic-work receipts. The ordinary durable migration
route and evidence schema remain separate.
Local same-Interpreter, caller-held Core Wasm, and caller-held native
migration/recovery selectors verify the association, reservation/receipt
pairing, and no retained host-work redelivery on each selected backend. A v4
target migration retains its exact metered target binding in the authenticated
handoff: the durable facade refuses an absent, unmetered, or differently bound
target before its destination handoff reservation or host work. The full
acceptance gate remains open.

On Linux, `sanitized_held_native_migration_and_recovery` opens a test-only held
clang wrapper which adds ASan and UBSan to every generated native stage compile,
requires both symbol families from the generated executable, and then exercises
pure migration plus durable recovery without redelivery. It grants no `PATH`
lookup. The local Darwin held-process attestation requires exactly one mapped
region for the registered executable before it runs; an ASan image maps none at
that point, so the ordinary held-native recovery selector remains the Darwin
gate. The sanitizer selector awaits Linux execution.
