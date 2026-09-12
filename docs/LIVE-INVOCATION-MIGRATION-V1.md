# Live Invocation Migration v1

Status: **LOCAL** bounded design + reference implementation, fixture-backed.

Audience: implementers of issue #115 ("Migrate live rich state with history
and budgets intact") and its neighbours in the #108–#116 lane, and reviewers
of the migration boundary this document adds around [Live Invocation
Contract v1](LIVE-INVOCATION-CONTRACT-V1.md) and [Live Invocation
Persistence v1](LIVE-INVOCATION-PERSISTENCE-V1.md).

This document assumes the reader already knows both of those: the causal
journal's record format and ordering rules, `kernel::run_live_invocation`'s
fresh-start/resume/replay/uncertain-intent behavior, and how a journal is
persisted and recovered across a process boundary. Everything below is
additive to that contract, not a restatement of it.

## What issue #115 asks for, and why it cannot live inside the kernel

Issue #115 asks for a live invocation to move onto a new ProgramRoot,
schema and deployment policy through a checked pure migration, preserving
conversation history, cumulative budget, and prior turn/call counts. The
prerequisite work this issue names —
[`execution_revision::typed_migration`](../src/execution_revision/typed_migration.rs)
— already proves the general shape a real checked migration takes: validate
an actual `Suspend`, bind old/new state schemas, evaluate a pure migration
function *twice* and reject any answer that differs between the two calls,
and charge every prior reservation forward rather than resetting it.

This module's file lease is `src/live_invocation/**` only —
`execution_revision`, `hir` and `interpreter` are frozen and out of reach
here. More importantly, `kernel::run_live_invocation` itself never holds an
Agent's actual state at all: it threads opaque bytes through the caller's
own `TurnObserver`/`TurnPolicy`, and a live invocation's identity and
journal are permanently bound together (`journal::validate` rejects any
entry naming a different invocation). So "migrate state" cannot mean
"rewrite the kernel's journal in place" — that would defeat the causal
journal's whole contract. [`src/live_invocation/migration.rs`](../src/live_invocation/migration.rs)
instead adds one pure function, [`migrate_live_invocation`](../src/live_invocation/migration.rs),
that reads a terminal, suspended predecessor journal and a destination
identity, and produces a durable *handoff* record plus the destination's
migrated state bytes — never touching the predecessor journal, never
dispatching a model or effect call itself.

## The checked-pure seam: `LiveStateMigration`

```rust
pub trait LiveStateMigration {
    fn migrate(&mut self, previous_state: &[u8]) -> Result<Vec<u8>, String>;
}
```

A real deployment binds this to a compiler-checked pure function over
retained source — the exact mechanism
`execution_revision::typed_migration::evaluate_migration` already
implements against HIR (reject effects, an illegal ownership mode, an
incompatible result shape, or an unbound source identity, before ever
producing a value). This module cannot call into that machinery directly
(it is outside this file lease), so `migrate_live_invocation` re-states the
one property it verifiably *can* enforce at this Rust-trait boundary
without HIR in hand: it calls `migration.migrate` **exactly twice** on the
identical input and refuses (`LiveMigrationError::NonDeterministicMigration`)
if the two answers differ — mirroring `evaluate_migration`'s own
`first != second` rejection precisely. This module ships only deterministic
fixtures (`fixture::FixtureStateMigration`,
`fixture::FixtureNondeterministicStateMigration`,
`fixture::FixtureRefusingStateMigration`); binding a real compiler-checked
migration function is downstream integration work against this trait, the
same declared boundary this lane already draws around
`ModelHandler`/`ProposalDecoder`/`AuthorizationGate`/`InvocationBudgetHook`.

## What "history and budgets intact" means at this layer

- **History is intact because migration never touches the predecessor's
  journal.** `migrate_live_invocation` takes it by shared reference, reads
  it, and returns. `migration::tests::the_predecessors_journal_still_replays_with_zero_dispatches_after_migration`
  proves this is not merely a documented intention: it migrates a suspended
  journal, then replays the *same* journal reference through the ordinary
  `kernel::run_live_invocation` with every seam (`TurnObserver`,
  `TurnPolicy`, `ModelHandler`) wired to panic if touched, and asserts
  `dispatched == 0` and `replay.journal == journal` (byte-identical).
- **Budgets are intact because the predecessor's total committed spend is
  carried, never reset.** `LiveMigrationHandoff::previous_committed_budget`
  is `budget::committed_from_journal(previous_journal)` — the exact fold
  `budget::CumulativeBudgetLedger::resume` already uses, extracted once so
  the two call sites can never drift onto two slightly different folds.
  `CumulativeBudgetLedger::migrated`/`resume_migrated` start a destination
  ledger's `committed` at that carried total instead of at zero. A
  destination *ceiling* is the new deployment's own independent policy
  decision (it may narrow or widen); what it can never do is pretend the
  carried spend did not happen — there is no constructor here that starts a
  migrated ledger at zero.
  `migration::tests::resuming_the_destination_after_a_simulated_crash_never_refunds_the_carried_predecessor_spend`
  is the fault-injection proof: it builds a destination journal ending in
  an uncertain `RequestIntent` (the exact shape a crash between the
  "before dispatch" write and the "after settlement" write leaves behind —
  matching `budget::tests::resuming_after_a_simulated_crash_never_refunds_the_already_committed_reservation`'s
  own pattern, crossed over the migration boundary) and shows
  `resume_migrated` reconstructs `carried + this reservation`, refusing a
  retry that would only fit if either half had been silently refunded.
- **Prior turn/call counts are intact as carried evidence, not as
  continued numbering.** A migrated invocation gets a genuinely new
  identity and a fresh causal chain starting at turn 0 (identity and
  journal are permanently bound; there is no way to splice a new identity
  onto an old chain). What issue #115 actually asks for — "A→B→C preserves
  previous call counts" — is satisfied by `LiveMigrationHandoff` carrying
  `previous_turns`/`previous_model_calls`/`previous_model_failures`/
  `previous_effect_calls` (all folded from `journal::receipt_projection`)
  forward at every hop, so a caller assembling one end-to-end audit trail
  across A→B→C never loses any hop's counts.
  `migration::tests::an_a_to_b_to_c_chain_preserves_call_counts_and_never_refunds_committed_budget`
  exercises the full three-generation chain: A migrates into B (a changed
  rich-state field), B's own subsequent model request observes the
  migrated field, B migrates into C, and the totals from both hops sum
  without loss.

## Refusal ordering, and what "before any host dispatch" means here

Every [`LiveMigrationError`](../src/live_invocation/migration.rs) variant is
checked, in this order, before `LiveStateMigration::migrate` is ever
called:

1. `PreviousIdentityMismatch` — the supplied seed does not derive to the
   claimed previous identity (a wrong original execution association).
2. `StaleDestination` — the supplied destination seed does not derive to
   the claimed destination identity (a stale or reminted destination
   generation).
3. `UnchangedProgramRoot` — predecessor and destination name the same
   `program_root`: not a version change, and migrating in place would
   silently approve what is really the same deployment re-running
   `initialize`.
4. `StateCapacity` — `previous_state` (or the migrated result) exceeds
   `MAX_MIGRATED_STATE_BYTES` (262 144 bytes, mirroring
   `execution_revision::typed_migration`'s own state cap rather than
   inventing a new number).
5. `InvalidPreviousJournal` — the previous journal does not causally
   validate against the claimed previous identity at all.
6. `NotTerminal` — the previous journal is uncertain, stuck mid-turn
   (including an "uncertain effect" ending right after `EffectIntent` with
   no `EffectObserved`/`EffectFailed`), or otherwise still in flight. Only
   a journal that has already reached a recorded terminal outcome is
   migratable — in-flight or uncertain work must first reach the reviewed
   suspend/reconciliation state, matching issue #115's bounded scope.
7. `NotSuspended` — the previous journal is terminal, but its case is
   `complete` or `fail`, not `suspend`. A completed or failed invocation
   has nothing left to migrate into a new generation.
8. `MigrationRefused`/`NonDeterministicMigration` — the bound
   `LiveStateMigration` itself refused, or disagreed with itself across its
   two calls.

Since `migrate_live_invocation` performs no model, effect, or journal-sink
call of its own — the only injected call it ever makes is to
`LiveStateMigration::migrate` — every refusal above is structurally "before
any host dispatch or store effect": there is no dispatch inside this
function for a refusal to come *after*. `verify_destination_binding` gives
a recovering caller the same zero-cost check independent of a fresh
migration call: it rejects a handoff bound to the wrong destination before
the caller reconstructs a budget ledger, opens the destination's first
turn, or performs any effect of its own.

## Idempotent handoffs: what "repeated recovery" means here

`LiveMigrationHandoff::digest` is a pure function of every field the
handoff carries. `migration::tests::migrating_twice_with_identical_inputs_produces_a_byte_identical_handoff`
proves that calling `migrate_live_invocation` twice with byte-identical
inputs produces a digest-identical handoff — the mechanism that keeps a
repeated recovery from ever reading as "a new migration continuation" the
second time. `migration::tests::changing_the_migration_function_name_changes_the_handoff_digest`
proves the converse: any differing bound input (here, which migration
function was named) changes the digest, so a reminted handoff is
detectable rather than silently accepted as the same one.

## Non-goals and known limitations (this round)

- **No distributed multi-writer transaction, no automatic cross-store
  reconciliation.** Like `execution_revision::typed_migration`, this is a
  pure function over caller-supplied, already-validated inputs. A caller
  who wants a persisted handoff (so a crash between computing it and
  acting on it is itself recoverable) layers `persistence::JournalSink`
  underneath the destination's own fresh journal, unchanged from [Live
  Invocation Persistence v1](LIVE-INVOCATION-PERSISTENCE-V1.md); this
  module adds no second store contract of its own.
- **No real compiler-checked migration function.** `fixture::FixtureStateMigration`
  and its siblings are deterministic fixtures for exercising the checked-pure
  double-evaluation boundary; binding a real one against retained HIR is
  downstream integration work, exactly as this lane already documents for
  `ModelHandler`/`ProposalDecoder`/`AuthorizationGate`.
- **No live network call, no real provider credential, no model spend.**
  Every test in `src/live_invocation/migration/tests.rs` runs entirely
  against fixtures.
- **Accumulating a whole chain's committed budget is the caller's job.**
  `LiveMigrationHandoff::previous_committed_budget` reports exactly the
  immediately preceding generation's total (A's carried total when
  migrating A→B; B's own total when migrating B→C). A caller assembling
  one running total across a longer chain sums every hop's own value, the
  same way `migration::tests::an_a_to_b_to_c_chain_preserves_call_counts_and_never_refunds_committed_budget`
  does; this module does not itself track a chain-wide running total,
  since it has no persistent state across separate calls to migrate.
- **The #228 boundary is unchanged by this module.** `migrate_live_invocation`
  performs no fallible host I/O of its own (no journal-sink write, no model
  or effect dispatch); it is a pure function over in-memory inputs. The
  after-dispatch persistence-failure boundary [Live Invocation Persistence
  v1](LIVE-INVOCATION-PERSISTENCE-V1.md#the-228-boundary-restated-for-this-module)
  already names is neither widened nor narrowed here.

## Executable reference

```sh
cargo test --locked -p semaprax --lib live_invocation
```

89 tests (the 73 tests Live Invocation Contract v1 and Live Invocation
Persistence v1 already established, unchanged, plus 16 new in
`migration::tests`), all fixture-backed, no network access, no model spend.
