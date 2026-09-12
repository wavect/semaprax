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

## Rich schema: an unknown or future revision is refused, not adopted

The base implementation above binds identity, program root and journal
state, but says nothing about the *shape* of the state bytes it carries
forward — a migration function could be handed any previous/destination
schema pair with no check that it was ever compiled or checked against that
exact pair. `LiveStateMigration::known_schema_transitions` closes that gap:
a real migration function declares exactly which
`(previous_schema, destination_schema)` digest pairs it is checked to
interpret (mirroring `execution_revision::typed_migration`'s own "the
destination must retain the old state schema and provide a second bounded
flat state schema" binding). When it declares a set,
`migrate_live_invocation` refuses (`LiveMigrationError::
UnknownSchemaRevision`) any previous/destination `interaction_schema_digest`
pair outside it, **before** `LiveStateMigration::migrate` is ever called —
so an unknown or future schema revision is refused rather than silently
reinterpreted as the destination's schema.
`fixture::FixtureSchemaBoundStateMigration` is the fixture that declares a
restricted set; `migration::tests::
an_unknown_or_future_destination_schema_revision_is_refused_before_the_migration_function_is_ever_called`
proves the refusal happens with `calls == 0` (never invoked), and
`migration::tests::a_migration_bound_to_the_declared_schema_pair_migrates_cleanly_and_records_it`
proves the matching known pair still migrates and is recorded on the
handoff (`LiveMigrationHandoff::previous_schema_digest`/
`destination_schema_digest`). This is what makes "schema interpretation
remains revision-specific" true at this layer: a migration bound to
`(SCHEMA_A, SCHEMA_B)` never silently reinterprets state under `SCHEMA_C`,
no matter how similar the bytes look. `FixtureStateMigration` and its
siblings keep the prior permissive default (`known_schema_transitions`
returning `None`) unchanged, so every pre-existing test in this module is
unaffected by this addition.

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
8. `UnknownSchemaRevision` — the bound `LiveStateMigration` declared a
   restricted set of known schema pairs, and the exact previous/destination
   pair is not one of them.
9. `MigrationRefused`/`NonDeterministicMigration` — the bound
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

## Durable destination handoff checkpoint

A successful pure migration is not itself permission to dispatch the
destination. `persist_migration_handoff` first writes one complete
`semaprax.live-invocation.persisted-migration-handoff.v1` document through
the existing caller-owned `agent_lifecycle::CheckpointStore` contract. The
versioned document binds the destination identity and generation to the
canonical handoff, its digest, the exact migrated-state hex bytes and their
digest, plus the destination journal and its chain digest.

`recover_migration_handoff` requires the exact destination identity and
recomputes every handoff, state, and journal link before returning a
validated `RecoveredMigrationHandoff` control record. It refuses a different
or future schema, a substituted destination, a reminted handoff, modified
state bytes, or a changed journal. Zero and exhausted (`u64::MAX`) generations
are refused, and documents larger than 2,097,152 bytes are rejected before
JSON parsing. Exact canonical re-rendering, including the terminal LF, is
required, so duplicate keys and alternate encodings are not adopted.

This record is evidence, not authority. Its hashes prove self-consistency but
not who produced it; `RecoveredMigrationHandoff` is cloneable and recovery
accepts caller-supplied bytes. Replay resistance and rollback detection depend
on a separately trusted store that reads the latest record and enforces
monotonic generations. `CheckpointStore` supplies atomic replace-or-retain
writes, but does not by itself supply a read API or promise to reject an older
caller-supplied generation. Journal validation and authorization remain
independent checks.

`run_migrated_destination` is the migration-specific dispatch route. It
accepts that validated record, checks the destination identity and schema again,
and installs a combined checkpoint sink before it calls the generic kernel.
Every journal write replaces the same document at the next generation while
preserving the handoff and migrated bytes. Therefore the first destination
`RequestIntent` is durable together with its handoff before model dispatch,
and a recovered terminal destination journal replays with zero new dispatches.
The generic kernel remains an intentionally separate route for fresh,
non-migrated live invocations; it cannot be cited as satisfying this migration
checkpoint requirement. This adapter does not make a caller who bypasses it
safe, and does not grant the recovered record any authority.

`migration::tests::a_migration_handoff_checkpoint_recovers_only_when_every_bound_byte_replays`
proves the round trip and rejects state, handoff, and schema tampering.
`migration::tests::a_persisted_or_recovered_handoff_drives_destination_dispatch_and_replay`
proves the real adapter sequence: persist, fail a checkpoint before dispatch,
dispatch through the combined sink, recover, then replay the terminal
destination with zero handler calls.

## Non-goals and known limitations (this round)

- **No distributed multi-writer transaction, no automatic cross-store
  reconciliation.** The combined checkpoint uses the existing atomic
  replace-or-retain `CheckpointStore` operation for one caller-selected
  destination record. It does not coordinate independent stores or mutate
  the predecessor journal.
- **No real compiler-checked migration function.** `fixture::FixtureStateMigration`
  and its siblings are deterministic fixtures for exercising the checked-pure
  double-evaluation boundary; binding a real one against retained HIR is
  downstream integration work, exactly as this lane already documents for
  `ModelHandler`/`ProposalDecoder`/`AuthorizationGate`.
- **No live network call, no real provider credential, no model spend.**
  Every test in `src/live_invocation/migration/tests.rs` runs entirely
  against fixtures.
- **"Live" here names this kernel's causal-journal contract, not a running
  compiled conversation.** [Live Invocation Contract
  v1](LIVE-INVOCATION-CONTRACT-V1.md#non-goals-and-known-limitations-this-round)
  records that no parser or HIR syntax for `model.invoke` exists yet, and
  `kernel::run_live_invocation` is called for real only from this crate's
  own tests (`src/agent_interaction_schema/live_bridge/tests.rs`) — nothing
  outside `src/live_invocation/**` and its own test tree calls it. This
  migration module is honestly a persistence/migration layer that is fully
  exercised and correct against that kernel today; it is not evidence that
  a source-native Agent conversation can be moved between ProgramRoots yet,
  because no source-native Agent conversation drives this kernel yet. See
  "What issue #115 asks for, and why it cannot live inside the kernel"
  above for why that gap is inherent to the file lease, not an oversight.
- **Accumulating a whole chain's committed budget is the caller's job.****
  `LiveMigrationHandoff::previous_committed_budget` reports exactly the
  immediately preceding generation's total (A's carried total when
  migrating A→B; B's own total when migrating B→C). A caller assembling
  one running total across a longer chain sums every hop's own value, the
  same way `migration::tests::an_a_to_b_to_c_chain_preserves_call_counts_and_never_refunds_committed_budget`
  does; this module does not itself track a chain-wide running total,
  since it has no persistent state across separate calls to migrate. The
  migration adapter reborrows the caller's budget hook unchanged; it does not
  reconstruct or authenticate a chain-wide cumulative ledger. A destination
  caller must seed that hook/ledger with predecessor totals before dispatch.
- **This does not close issue #115.** The local checkpoint reference now
  makes its own durable handoff sequence recoverable, but a real
  compiler-checked HIR migration and trusted cumulative chain accounting are
  still downstream work. #177's source/HIR live-conversation integration is
  deliberately outside this module.
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

93 tests (the 73 tests Live Invocation Contract v1 and Live Invocation
Persistence v1 already established, unchanged, plus 20 in
`migration::tests` — the original 16, two rich-schema cases, and two durable
handoff-checkpoint regressions), all fixture-backed, no network access, no model spend.
