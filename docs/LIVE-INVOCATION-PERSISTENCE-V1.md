# Live Invocation Persistence v1

Status: **LOCAL** bounded design + reference implementation, fixture-backed.

Audience: implementers of issue #114 ("Persist and recover live Agent
conversations without redispatching recorded work") and its neighbours in the
#108–#116 lane, and reviewers of the persistence/recovery boundary this
document adds around [Live Invocation Contract v1](LIVE-INVOCATION-CONTRACT-V1.md).

This document assumes the reader already knows Live Invocation Contract v1:
the causal journal's record format, its ordering rules, and
`kernel::run_live_invocation`'s fresh-start/resume/replay/uncertain-intent
behavior. Everything below is additive to that contract, not a restatement
of it.

## What issue #114 asked for, and what already existed

Issue #114's title names exactly the property [Live Invocation Contract
v1](LIVE-INVOCATION-CONTRACT-V1.md) already provides **in memory**:

- A causal journal with immutable invocation identity, separated from model
  responses that arrive after execution starts — [`identity`](../src/live_invocation/identity.rs)
  and [`journal`](../src/live_invocation/journal.rs).
- Replay emits zero dispatches and reproduces the terminal outcome —
  `kernel::run_live_invocation`, proved by
  `tests::replaying_a_terminal_journal_makes_zero_dispatches_and_reproduces_the_outcome`
  (`src/live_invocation/tests.rs`), which wires every seam to a fixture that
  panics if touched.
- Resume-after-continue redispatches nothing — proved by
  `tests::resuming_a_journal_after_continue_does_not_redispatch_the_completed_turn`.
- An uncertain intent and a mid-turn-stuck prefix are each refused rather than
  guessed — `LiveKernelError::UncertainIntent` /
  `LiveKernelError::UnresolvedPrefix`, proved by
  `tests::an_uncertain_intent_journal_is_refused_before_any_redispatch` and
  `tests::an_unresolved_mid_turn_prefix_is_refused_rather_than_guessed`.
- Duplicate, reordered, omitted and post-terminal journal entries are each
  rejected — `journal::validate`, proved by
  `journal::tests::duplicate_turn_opened_is_rejected`,
  `journal::tests::reorder_is_rejected`, `journal::tests::omission_is_rejected`,
  `journal::tests::continuation_after_terminal_outcome_is_rejected`.

None of this was rebuilt for #114. What a process boundary adds, and what
this document's [`src/live_invocation/persistence.rs`](../src/live_invocation/persistence.rs)
module provides, is the part the in-memory contract cannot: getting the
journal `kernel::run_live_invocation` builds in memory onto durable storage,
**at the points that matter** (before every dispatch, after every
settlement), through the same caller-owned store contract
`agent_lifecycle::durable::CheckpointStore` already defines — reused, not
reinvented, exactly as this issue's bounded scope named.

## What this module adds

### The write-side seam: `JournalSink`

```rust
pub trait JournalSink {
    fn persist(&mut self, journal: &[JournalEntry]) -> Result<(), CheckpointStoreError>;
}
```

`kernel::run_live_invocation` calls a bound `LiveInvocationHandlers::sink`
immediately after every journal append, in the same order the entries are
produced. `sink: None` (the field every existing caller in
`src/live_invocation/tests.rs` sets) makes every call a no-op: the 39
existing `live_invocation` lib tests still pass unmodified with this field
added, because nothing about in-memory behavior changed for them — see
`cargo test --locked -p semaprax --lib live_invocation` below.

The critical placements — the ones that make "survive interruption around
model calls" true rather than aspirational — are in `kernel.rs`:

- After `RequestIntent` is appended, **before** `ModelHandler::invoke` is
  called. A store failure here is an ordinary, safe refusal: nothing
  external has happened yet.
- After `ResponseRecorded`/`ResponseFailed` is appended, **before**
  `ProposalDecoder::decode` runs. The dispatch already happened by this
  point and cannot be undone.
- After `EffectIntent` is appended, **before** `TurnEffect::call` is
  invoked — the same "before dispatch" discipline for the one further
  effect a turn may perform.
- After every other append (`TurnOpened`, `ProposalAdmitted`/`ProposalRefused`,
  `AuthorizationConsumed`, `EffectObserved`, `Transition`, and once more via
  `finish` for `TerminalOutcome`), so a bound sink's store always reflects
  either the exact prefix a fresh process would see after a crash at that
  point, or (with `sink: None`) nothing changes at all.

`LiveKernelError` gains one new variant, `PersistenceFailed { dispatched:
usize }`, carrying the confirmed dispatch count *at the moment of the failed
write* — not the lost, unwritten entries. That is deliberate: a real crash
does not let a recovering process read memory that was never written either,
so returning "how many real dispatches happened" and nothing else mirrors
what a fresh process can actually know, rather than inventing a channel to
recover bytes that were never made durable.

### The store adapter: `CheckpointJournalSink`

`CheckpointJournalSink` adapts a caller-owned
`agent_lifecycle::CheckpointStore` (the *exact* trait
`agent_runtime_v2::checkpoint`'s per-operation journal already uses) into a
`JournalSink`, by re-rendering the whole journal through
[`journal::render`](../src/live_invocation/journal.rs) and committing it as
one incrementing generation each call — the same "replace the whole
generation, or leave the previous one intact" contract
`CheckpointStore::commit` already documents. `CheckpointJournalSink::resume`
continues an existing sink from a previously recovered generation, so a
resumed process's next successful write is `generation + 1`, never a
collision with what the store already holds.

This is the whole of what #114 needed to add here: no second store
contract, no second effect-journal owner. `agent_runtime_v2::checkpoint`'s
`OperationCheckpoint` and `src/live_invocation/journal.rs`'s causal journal
remain two separate, independently-owned journals for two separate
concerns (typed-effect per-operation checkpointing vs. the live-invocation
model/effect journal), exactly as the issue's "Current evidence and
distinction from existing work" section states; this module does not merge
them, only reuses their shared store trait.

### The envelope and recovery: `encode_envelope` / `recover_journal`

A persisted document is:

```json
{"schema":"semaprax.live-invocation.persisted-journal.v1",
 "invocation":"sha256:<the exact LiveInvocationId this document is bound to>",
 "generation":<u64>,
 "chain":"sha256:<journal::chain of the entries below>",
 "entries":[...journal::render's existing canonical array...]}
```

`recover_journal(document, identity)` checks exactly what the envelope adds
and nothing `journal::validate` already checks:

- **Schema** — rejects a document from an unrelated format
  (`RecoveryError::SchemaMismatch`).
- **Invocation binding** — rejects a document written for a different live
  invocation (`RecoveryError::InvocationMismatch`), independent of, and in
  addition to, every `TurnOpened` entry's own `invocation` field (which
  `journal::validate` already checks once the entries reach the kernel).
- **Chain integrity** — recomputes `journal::chain` over the decoded entries
  and rejects a mismatch (`RecoveryError::ChainMismatch`). This is the check
  `journal.rs`'s own per-entry digest fields cannot provide: `journal::decode`
  never recomputes an entry's `response_digest`/`observation_digest`/etc.
  from its recorded bytes (a declared nonclaim — without a signature, a
  party who could rewrite one could recompute the other too), so a document
  that is internally self-consistent per entry but was truncated, reordered,
  or had one entry's recorded content silently substituted is *only* caught
  by the whole-journal chain link. Two hostile-input tests prove this
  distinction is real rather than assumed:
  `persistence::tests::recovery_rejects_a_document_whose_entries_were_tampered_with_after_writing`
  substitutes one still-well-formed digest string in place of another (a
  change `journal::decode`'s shape check alone would accept), and
  `persistence::tests::recovery_rejects_reordered_entries_even_though_each_entry_individually_decodes`
  builds a document with reordered, individually-valid entries and a stale
  `seq`-consistent-but-order-wrong array. Both are rejected by the chain
  check before the entries ever reach `kernel::run_live_invocation`.

Recovery deliberately does **not** re-run `journal::validate`'s causal
ordering check itself. That check has exactly one owner —
`kernel::run_live_invocation`, the moment it is handed `recover_journal`'s
returned entries as a starting journal — so a reordering or omission that
happens to keep a valid chain (impossible for `journal::chain` by
construction, since it folds every entry's own position into the link, but
worth stating precisely) is still not something this module attempts to
catch a second, competing way. This is the direct answer to "avoid two
competing owners of the same effect journal."

## Recovery reserves fresh replay fuel and never erases prior evidence

Because recovery hands `kernel::run_live_invocation` the exact recorded
entries as a *starting* journal — the same parameter fresh calls pass an
empty `Vec` for — every existing in-memory guarantee applies unchanged to a
recovered document:

- A **fully recorded completed run** recovers and replays with zero model
  and effect calls: `dispatched == 0`, and a handler wired to panic if
  touched proves it, in
  `persistence::tests::a_fully_persisted_completed_run_recovers_and_replays_with_zero_dispatches`.
  This is the literal "A fully recorded completed run recovers with zero
  model and effect calls" required-test line from the issue.
- A **resumable** document (ending cleanly after a `continue` transition)
  resumes at the next turn without redispatching the recorded prefix — the
  same `resumable_turn` mechanism Live Invocation Contract v1 already
  documents, now reachable from a document instead of only an in-memory
  `Vec<JournalEntry>`.
- An **uncertain-intent** document (ending right after `RequestIntent` with
  no recorded response — exactly the shape a crash between the "before
  dispatch" write and the "after settlement" write leaves behind) is refused
  before any redispatch, by the same `LiveKernelError::UncertainIntent` path.
  Nothing about persistence changes this: the persisted document simply
  *is* the in-memory journal a crash at that exact point would have left,
  and the existing uncertain-intent refusal already covers it without any
  new code.
- Total dispatch/usage accounting is not reset by recovery: `dispatched`
  counts only *this call's* new dispatches (unchanged from before #114), and
  every prior turn's `InvocationBudgetHook::record` call already happened
  during the run that produced the recovered document — recovery does not
  re-run it, and does not need to, because the budget hook this kernel binds
  is (per Live Invocation Contract v1's own non-goals) not itself a
  cross-invocation cumulative store; a caller layering cumulative accounting
  on top of `InvocationUsage` is unaffected by whether a given call started
  from an empty or a recovered journal.

## The two shapes of persistence failure, made concrete

Two fault-injection tests give `LiveKernelError::PersistenceFailed` exact,
checked meaning rather than a documented intention:

- `persistence::tests::a_store_failure_before_the_first_dispatch_makes_zero_model_calls`
  uses a `CheckpointStore` that fails on its second `commit` call (the
  `RequestIntent` write, still before dispatch) and asserts both
  `PersistenceFailed { dispatched: 0 }` and `handler.calls == 0` — the
  handler is `FixtureModelHandler::must_not_be_called()`, so a spurious
  dispatch would panic the test, not just fail an assertion.
- `persistence::tests::a_store_failure_immediately_after_the_response_still_reports_the_real_dispatch_count`
  fails on the third `commit` call (the `ResponseRecorded` write, right
  after a real dispatch) and asserts `PersistenceFailed { dispatched: 1 }`
  and `handler.calls == 1` — proving the kernel does not, and structurally
  cannot, pretend a dispatch that already happened did not. It also recovers
  the store's last successfully-committed document and confirms it ends at
  `RequestIntent` (the response never reached storage), matching exactly
  what a real process crash at the same point would leave behind.

## The #228 boundary, restated for this module

[Durable Jobs v1](DURABLE-JOBS-V1.md#the-228-boundary-what-blocks-a-checked-semaprax-caller)
records the same boundary this module runs into, for a different domain: a
fallible host write that can fail *after* an irrevocable external action
already happened has no in-language way for a checked SEMAPRAX-authored
caller to report "outcome uncertain" rather than either "definitely failed"
or a silently swallowed success, because every fallible host operation in
this repository aborts its enclosing invocation on failure instead of
returning an inspectable value (issue #228).

This module's own after-dispatch write — `CheckpointJournalSink::persist`
called after `ResponseRecorded`/`ResponseFailed`/`EffectObserved` — is
exactly that shape of write: the model or effect call already happened, and
`commit` can still fail. Today that is not blocked by #228, and this
document does not claim otherwise, for one precise reason: `src/live_invocation/`
is Rust-host-side code exercising a Rust-host-side kernel (per Live
Invocation Contract v1's own scope: "It does not touch `agent_lifecycle`,
`agent_runtime_v2`, HIR, or the parser"), so `Result<(), CheckpointStoreError>`
propagating up to `LiveKernelError::PersistenceFailed` is already exactly as
expressive as a Rust caller needs — no checked SEMAPRAX source calls into
this kernel yet, so there is no in-language caller lacking a channel to lose.

The gap becomes live, not hypothetical, the day a checked Agent's `propose`
role is wired through this kernel (#109–#116's scope, not this module's) and
a checked handler needs to react to its *own* persistence outcome rather
than a host runner reacting on its behalf. At that point, this exact
after-dispatch write is the "published then I/O failed, outcome uncertain"
case #228 names, and closing it needs #228's value-typed host operation —
not a sentinel invented here. This document deliberately does not invent
one, matching the same restraint `docs/DURABLE-JOBS-V1.md`'s own "#228
boundary" section already exercises for its structurally identical problem.

## Non-goals and known limitations (this round)

- **No real filesystem, database, or network-backed `CheckpointStore`.**
  Every test in `src/live_invocation/persistence/tests.rs` uses an in-memory
  `RecordingStore`. This matches the existing convention in this codebase:
  neither `agent_lifecycle::durable` nor `agent_runtime_v2::checkpoint` ships
  a concrete filesystem `CheckpointStore` either — both document the
  "atomic rename" contract a real store must satisfy and test only against
  an in-memory fixture. A real store implementation is caller-owned
  infrastructure, not part of this contract.
- **No real subprocess termination/restart test.** The issue's own
  implementation steps name this as a further step "once fixture tests
  pass"; this round's fixture-level fault injection (failing a specific
  `commit` call number) proves the *logical* recovery properties precisely
  and repeatably, but is not a claim about a real process crash, a real
  filesystem `fsync` boundary, or power-loss durability. `docs/DURABLE-JOBS-V1.md`'s
  own non-claims section draws this identical line for the same reason.
- **No cumulative cross-invocation budget or receipt changes.** Recovery
  passes decoded entries into the same `kernel::run_live_invocation` that
  existed before this round; the budget hook and receipt projection
  mechanisms are unchanged.
- **The #228 boundary above is not closed here** and is not attempted here;
  it is out of scope for a Rust-host-side kernel with no checked-source
  caller yet, and is named precisely so a later tranche does not have to
  rediscover it.
- **Migrating a persisted invocation onto a new identity is a separate
  document.** [Live Invocation Migration v1](LIVE-INVOCATION-MIGRATION-V1.md)
  (issue #115) builds `migration::migrate_live_invocation` on top of this
  module's `budget::committed_from_journal` fold and the same journal
  format, without changing this module's own recovery envelope or write
  path.

## Executable reference

```sh
cargo test --locked -p semaprax --lib live_invocation
```

50 tests (the 39 Live Invocation Contract v1 tests, unchanged, plus 11 new
in `persistence::tests`), all fixture-backed, no network access, no model
spend.
