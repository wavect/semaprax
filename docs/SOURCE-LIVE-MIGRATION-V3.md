# Source live migration journal v3

Status: **implemented with local executable conformance.** This is
the source-mode extension of [Source Live Journal v2](SOURCE-LIVE-JOURNAL-V2.md),
not a generic kernel journal, a provider receipt, or a new authority source.

Audience: source runtime, embedding, and durable migration contributors.

## Admission and handoff identity

The host supplies retained predecessor and destination `ProjectRevision`s,
their selected source Agent identities, compiled checked lifecycles, one task,
the predecessor's latest authoritative source checkpoint, and an independently
bound predecessor `SourceInvocationBinding`. The bridge derives both actual
`ProgramRoot`s and reselects the linked Agent role closures from those retained
Projects. Exact recompiled lifecycle bytes and digest must match. The
destination Project must explicitly declare or import the selected migration
function; the checked linked closure admits only its pure signature
`OldState -> NewState`. Both nominal State IDs and every persistent flat field
identity and leaf type are checked against the predecessor and destination
lifecycles. Submitted HIR and caller-asserted roots cannot select code.

A `semaprax.hot-reload-source-agent-handoff.v2` row may restrict this
selection when it replays against both retained Projects. It commits each
endpoint's AgentDefinition, AgentGraph, Runtime profile, Proposal and
Observation facts but carries no checkpoint, writer, provider, capability, or
dispatch authority. After it matches, this protocol still performs every
ordinary predecessor-binding, checkpoint, State-schema, pure-migration and
destination-journal check before it can dispatch a successor.

The predecessor must have an actual committed Suspend terminal with a retained
canonical State carrier. The carrier is decoded under the exact predecessor
flat State schema, then re-encoded byte-for-byte. Its terminal chain and
generation, invocation, schema, Project roots, checked migration closure,
State identities, task, cumulative counters, unit contract, absolute deadline
and clock floor form the v3 handoff digest. Hashes bind bytes; they do not prove the store generation is latest or grant
exclusive writer authority. The caller must independently ensure the
predecessor snapshot is latest and cannot be consumed twice across destination
stores. The destination must be fresh or supply its own latest, exclusively
owned v3 snapshot. A caller-derived expected handoff can further pin migration,
but copying it from submitted destination checkpoint bytes is not independent
evidence.

The destination model ceiling, iteration limit, logical stage limit,
per-stage fuel limit and cumulative fuel ceiling may tighten but cannot exceed
their predecessor limits. The reservation unit, per-attempt charge and clock
domain stay the same. The absolute deadline cannot extend. The destination
start and every later clock
sample must not precede the predecessor or journal floor. Model reservations,
logical turns/stages/effects/attempts and reserved stage fuel carry forward
without refunds. Prior raw model text and effect feedback remain provenance
under the predecessor binding; the destination's first new stage is Observe on
the migrated State, with no Initialize or implicit old-schema feedback.

## One append-only destination family

Schema `semaprax.live-invocation.source-persisted-journal.v3` has a distinct
invocation and chain domain. It uses the v2 source causal entries after this
mandatory prefix:

1. `MigrationOpened(handoff_digest)` is ACKed before migration evaluation.
   It contains no migrated State.
2. `MigrationEvaluationIntent(attempt,fuel)` is ACKed before each checked pure
   evaluation. `fuel = 2 * max_migration_steps`, because the existing checked
   migrator evaluates twice and compares outcomes. Every intent consumes the
   full nonrefundable fuel, even on failure or crash.
3. `MigrationEvaluationSettled(attempt,canonical State,digest)` seals the one
   checked result before any destination stage. It is schema-checked again on
   recovery. `MigrationEvaluationFailed(attempt,closed reason)` instead stops
   the prefix with the charge retained. An unresolved pure intent may be
   retried only after a fresh numbered intent and ACK. The pure evaluator has
   no physical effect. An unresolved **model or effect** intent later in the
   source stream remains uncertain and is never redispatched.
4. `RunOpened` begins the ordinary source causal stream. The first reservation
   must be Observe at the carried turn. Original stages count once toward the
   logical stage ceiling; all original and replay reservations consume the
   same cumulative fuel ceiling. Completion retains the v2 terminal snapshot
   evidence shape, with cumulative committed counters and bounded carrier.

The writer validates each exact canonical candidate and reserves worst-case
settlement and terminal room before acknowledging an intent. A failed commit makes the writer unusable. Its receipt records only the last
ACK and cannot tell whether the store accepted a later unacknowledged write.
Recovery must use the latest authoritative store snapshot. A committed
terminal is returned as an opaque receipt before attempting a continuation
ledger; it is not reconstructed as a fresh `IterativeRun`.

## Executable evidence and remaining acceptance

The `source_journal` selector passed 25 local unit tests, including v3
intent/settlement ordering, carried fuel, ACK loss, cross-version refusal and
canonical recovery. The `source_live` selector passed 19 local unit tests,
including the checked State decoder's non-ASCII hostile input. The
`agent_runtime_v1` `source_migration` selector passed seven retained-Project
tests: A→B→C execution through replayed hot-reload selection rows, binding and
budget refusals, lost-ACK charged retry and uncertain model refusal, post-ACK
deadline, failed-evaluator clock regression, post-ACK cancellation, and
independently schema-checked recovered settlement. The chain recovers B's
Suspend before C and C's terminal after expiry without redispatch, preserves
the carried State marker and cumulative reservations, retains the same absolute
deadline, and inspects each successor's first model request for its migrated
State and destination schema. These are local injected host/store gates;
fixed reservation units and unknown attempts are charged, not verified provider
billing. Predecessor response text stays under its original journal binding;
it is not automatically concatenated into the destination model request.

The local gates and coordinator semantic review do not establish cross-store
single-consumer handoff, hosted/provider execution, or nested State support.
A failed or unacknowledged model/effect intent remains uncertain; recovery
never treats an opaque receipt as physical exactly-once proof.

## Priced v4 successor boundary

The private priced source profile uses the distinct v4 journal and an integer
operator quote. A priced predecessor may migrate only to a priced v4
destination. The destination derives `PricedMigrationCarryV4` from the
validated predecessor binding and recovered checkpoint, never from a receipt
or caller-supplied totals. Its distinct v4 handoff digest binds the v3 base
handoff plus pricing identity (work unit, currency, minor-unit exponent and
integer rate), predecessor money ceiling, cumulative reserved, observed,
unknown and observed-over-reservation minor units, and the next global money
ordinal.

The v3 base carry must match the recovered predecessor invocation, generation,
chain, committed work and fuel, terminal turn/stage/effect/attempt counters,
limits, unit, clock floor, deadline, and v4 schema. The predecessor must have
a committed terminal snapshot. The destination retains the exact work unit,
currency, exponent and rate; it may narrow its money ceiling but may not lower
it below carried reservations. Historical observed overage can exhaust further
admission, but is neither refunded nor erased.

The `MigrationOpened` / checked pure-evaluation / `RunOpened` prefix remains
the single destination prefix. Under v4 its `MigrationOpened` row names the
priced handoff digest while the retained v3 digest binds ordinary migration
facts. Destination priced attempts start at the carried global ordinal and
continue it across recovery. Replayed totals retain previous `Unknown` and
`Observed` evidence; receipts remain observational and grant no accounting
authority.

The private CLI admits one priced predecessor-to-destination handoff. It
refuses unpriced-to-priced and priced-to-unpriced conversion rather than
treating prior work as zero-priced, observed, or free. This does not claim
provider billing, refunds, cross-store single consumption, or hosted execution.

## Additive V6 policy carry

The V6 source-policy successor uses the same checked migration authority and
V3 preamble, but its handoff separately commits the predecessor V6 binding
identity, exact policy limits, nonrefundable reserved/observed/exposure totals,
next global policy ordinal, and a digest of all prior reservations. Recovery
reconstructs these values only from an independently validated predecessor
checkpoint. The destination must keep the same selected provider and narrow
every policy ceiling; a provider switch is refused. Source/model/policy digest
changes are permitted only after this accepted handoff. No V1--V5 migration
wire is reinterpreted as V6.

Typed V6 migration prepares through
`AgentRuntimeV2::prepare_live_bound_model_durable_policy_migration_with_io_limits`
(or the policy-only form). `source_live_model_policy` produces an owned checked
policy with the exact typed effect root and absolute deadline;
`source_live_migration_endpoint` borrows the retained typed lifecycle. The
destination clock floor must be at least the predecessor's last checked time.
The facade reuses linked runtime migration validation and the source journal's
acknowledged migration/effect dispatcher. Terminal replay neither requotes nor
redispatches provider or effect work. Raw V6 policy facts are crate-private.
