# Resumable effects v1

Status: **reference validator**, not yet source syntax. Issue #204 asks the
compiler to let an ordinary, non-Agent function declare typed resumable
effects with the same generality Aver's `yield` lowering has. This document
records the full design that generalization needs and states, exactly, how
far this slice carries it: a Rust-level generic driver and journal
(`src/resumable_effects/`) that proves the required suspend/resume semantic
properties end to end, plus this design. It does **not** add `.spx` syntax,
an HIR node, a verifier rule, a semantic-graph projection, or native/Wasm
lowering — see [Scope boundary](#scope-boundary).

Audience: compiler contributors implementing the source-syntax/HIR/backend
generalization this document specifies, and reviewers auditing what #204
delivered versus what remains.

## What already exists on `main`

Before this change, three things already ship, HOSTED GREEN, for exactly one
closed shape:

- `agent_lifecycle::iterative::compile_agent_lifecycle_v2`
  ([Agent iterative lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md)) lowers
  one AgentDefinition's fixed six-role
  `initialize`/`observe`/`propose`/`authorize`/`execute`/`reduce` operations
  into a `Continue`/`Complete`/`Suspend`/`Fail` `Step` state machine.
- `agent_lifecycle::iterative::effects::compile_typed_effects`
  ([Agent typed effects v3](AGENT-TYPED-EFFECTS-V3.md)) binds a bounded,
  ≤64-row operation registry of typed host effects to that same state
  machine, checked against a deployment's allowed tool/effect IDs.
- `agent_runtime_v2::checkpoint`
  ([Agent operation checkpoint v2](AGENT-OPERATION-CHECKPOINT-V2.md))
  durably journals each turn as `Intent`/`Observed`/`Transition` entries and
  proves replay against a trusted store dispatches zero new host calls for
  an already-completed run.

All three are real, tested, and **outside this module's lease** — nothing in
`src/resumable_effects/` edits them. What none of them do, and what #204
requires, is expose this shape to an arbitrary function: every type in the
existing stack is the AgentDefinition's `task`/`state`/`observation`/
`proposal`/`outcome`/`result` roles, decoded from one closed
`semaprax.agent-definition.v1` JSON document. There is still no general
"a function declares a typed effect, the compiler lowers it" mechanism on
`main`. `rg -l "suspend|resumable|yield" src/` before this change confirms
this: every hit is either this closed Agent stack, an unrelated iterator/
for-loop `yield`-shaped identifier, or `live_invocation`'s single
`model.invoke` boundary (`docs/LIVE-INVOCATION-CONTRACT-V1.md`), which is
itself only a generalization of *one* effect, not of suspend/resume itself.

## Design: the general shape

A resumable computation over caller-chosen `State`, `Result`, `Request`,
`Observation`, `CleanupOp` types is:

```text
request:    State -> Option<Request>                       // is this turn a suspension point?
transition: (State, Option<&Observation>) -> Step<State, Result>
cleanup_plan: State -> Vec<CleanupOp>                       // canonical order, terminal-only
```

`Step` is the existing `Continue`/`Suspend`/`Complete`/`Fail` vocabulary,
unchanged in shape from the Agent profile. `transition` is the one place all
of a program's semantics live, and it is called *identically* whether its
`observation` was just physically dispatched or replayed from a trusted
journal entry — that identity of call is what makes replay a re-*use* of
trusted recorded observations rather than a second, possibly divergent,
re-*execution* (see [What matters](#what-matters-and-how-it-is-tested)
below). This is the same principle
`agent_runtime_v2::checkpoint`'s recovery already documents ("recovery
re-executes deterministic stages under new persisted fuel reservations...
compares every retained transition against the newly checked reducer
result"), generalized from one Agent reducer to an arbitrary `transition`.

A journal entry additionally carries an `EffectScope { program_root,
invocation_id, policy_epoch }`, generalizing
`live_invocation::identity::LiveInvocationSeed` from one effect
(`model.invoke`) to an arbitrary program. `Journal::validate` rejects any
entry whose scope disagrees with a caller-supplied *expected* scope — the
caller re-derives that scope independently every time, exactly as
`LiveInvocationSeed` is re-derived rather than read back off a journal — so
a copied or replayed journal can never become a bearer credential for a
different program root, invocation, or policy epoch (the "reminted resume"
failure case #204 names explicitly).

## What this reference module implements

`src/resumable_effects/core.rs`:

- `ResumableEffectProgram`: the trait above. `State`, `Result`, `Request`,
  `Observation`, `CleanupOp` are each bound `Clone + Eq + Debug + 'static`.
- `Step<S, R>`: `Continue | Suspend | Complete | Fail(i64)`.
- `EffectHandler<Req, Obs>` / `CleanupHandler<Op>`: the only injected
  physical boundaries. Nothing in the driver itself opens a file, spawns a
  process, contacts a network, or otherwise acquires ambient authority
  merely by running a suspend/resume program — the driver's own settlement
  of a `Step` is proof data about what the program decided, never a grant to
  act on it.
- `Journal<P>` / `JournalEntry<P>`: an append-only, canonical-order record
  of `Intent`/`Observed`/`ObservationFailed`/`Transition` entries.
  `Journal::validate` rejects a journal before it is trusted for replay:
  `StaleProgramRoot`, `WrongInvocation`, `WrongPolicyEpoch` (three distinct
  scope-field mismatches, not one merged variant), `OutOfOrder`,
  `RequestMismatch`, `NonSequentialTurn`, `EntryAfterTerminal`, and
  `UnterminatedIntent` (a journal ending on a bare `Intent` is uncertain —
  whether the physical dispatch happened is unknown — and is rejected
  before any stage, mirroring `agent_runtime_v2::checkpoint`'s identical
  rule).
- `run` / `resume`: the driver. `resume` replays every entry the journal
  already has — recomputing `request`/`transition` and asserting they
  agree with the recorded entries (`RequestDrift`/`TransitionDrift` if not)
  — without ever calling the injected handler for a replayed turn, then
  performs genuinely new turns past the journal's recorded tail as fresh
  dispatches with their own accounting. Both functions return the journal
  on *every* path, including failure, so a caller keeps the latest durable
  checkpoint candidate even when a call fails (mirroring
  `DurableTypedFailure`).
- Cleanup only runs for `Complete`/`Fail` (a `Suspend` deliberately keeps a
  computation's resources live for a later resume); cleanup ops run in
  exact `cleanup_plan` order, exactly once, and a cleanup failure is
  recorded in `Outcome::cleanup` but never replaces the already-selected
  `terminal` — failure selection is sticky by construction, not by a
  downstream check.

## What matters, and how it is tested

- **Replay is not re-execution.** `resume_from_a_truncated_journal_only_dispatches_the_new_tail`
  proves a 3-turn run resumed from a 1-turn-recorded prefix dispatches
  exactly the 2 remaining turns, never the first. `replay_of_a_completed_run_makes_zero_new_effect_dispatches`
  and `replay_of_a_suspended_run_reproduces_the_same_suspension_with_zero_dispatch`
  wire a handler that **panics** if called, so a stray dispatch fails loudly
  rather than silently passing. `replaying_a_recorded_failed_dispatch_never_recontacts_the_host`
  proves the same holds for a recorded *failure*: replay reports the same
  deterministic failure again rather than re-attempting the call.
- **A settlement is proof data, not permission.** The driver never holds a
  file handle, socket, or process; `EffectHandler`/`CleanupHandler` are the
  only injection points, matching every other effect boundary in this
  codebase (`live_invocation::model_invoke::ModelHandler`,
  `agent_lifecycle`'s `TypedEffectHandler`).
- **Failure selection is sticky.** `cleanup_failure_never_overrides_the_already_selected_terminal_status`
  runs both a `Fail` and a `Complete` case, each with one cleanup entry
  engineered to fail, and asserts the terminal status is unchanged in both
  — plus a negative-control `assert_ne!` against the other status in each
  case, so the two paths cannot be silently confused with each other.
- **Cleanup-plan vectors are canonical runtime order.** `cleanup_plan`
  returns a fixed `Vec`; the driver iterates it front-to-back and every
  cleanup-asserting test checks the exact recorded order, not just
  membership.
- **Typed effects are a compile-time diagnostic, not a runtime surprise.**
  `ResumableEffectProgram` fixes one `Request`/`Observation` pair per
  program (a monomorphic type parameter, not a boxed/erased channel), so
  presenting an observation of the wrong type is a `rustc` type error. The
  module-level `compile_fail` doctest in `src/resumable_effects.rs` (run by
  `cargo test --doc`) proves this executes as a real compiler rejection,
  not just a documentation claim. A second `compile_fail` doctest proves the
  `'static` bound rejects a borrowed local as a program's `State` — the
  reference validator's compile-time ownership gate.
- **Specific, not merged, diagnostics.** `validate_rejects_a_stale_program_root_specifically`,
  `..._a_wrong_invocation_specifically` and `..._a_wrong_policy_epoch_specifically`
  each assert the exact `JournalError` variant returned **and** assert the
  other two variants are *not* what was returned, so the three scope fields
  cannot be silently conflated into one underspecified rejection.
- **Corruption and reminted-resume refusal are caught before any dispatch.**
  `resume_rejects_a_replayed_request_that_disagrees_with_recomputation` and
  `..._a_replayed_transition_that_disagrees_with_recomputation` hand-tamper
  a completed journal's recorded request/transition (in a way that still
  passes `Journal::validate`'s structural check, so the drift is caught only
  by the driver's own recomputation) and assert `RequestDrift`/
  `TransitionDrift`, with a panicking handler proving zero new dispatches
  happened first. `resume_refuses_a_journal_presented_under_a_different_invocation_scope`
  proves a genuine, valid, completed journal cannot simply be replayed under
  a different scope — the journal itself carries no authority to be
  resumed; only a caller-supplied, independently-derived matching scope
  does.
- **Owned locals transfer intact across suspension.**
  `owned_state_transfers_intact_across_a_suspension` carries a growing,
  non-`Copy` `Vec<String>` log through two suspensions and asserts its exact
  contents in the `Suspend` carrier.

## Scope boundary

Explicitly **not** done in this slice, and why:

- **No `.spx` syntax, HIR node, verifier rule, or graph projection.** The
  repository's change protocol requires parser, canonical formatter,
  resolver/HIR, verifier, semantic graph, native backend and Wasm backend to
  move together once syntax carries runtime meaning. That is a
  multi-subsystem change; landing a half-wired parser rule with no checked
  HIR consumer, or an HIR node no backend lowers, would violate that
  protocol rather than satisfy it. This document is the design that
  follow-up work implements against — the same relationship
  `LIVE-INVOCATION-CONTRACT-V1.md` already has to the HIR-integration issues
  it exists to unblock (#109–#116, #178–#181 per that document).
- **No compiler-checked ownership analysis.** `'static + Clone + Eq + Debug`
  is this reference module's own approximation of "plain owned, transferable
  data" — it rejects a borrow or a non-`'static` handle the same way a real
  ownership checker would reject an escaping reference, but it is not the
  compiler's alias/uniqueness analysis over real HIR locals. Wiring a real
  `yield` point to the existing ownership/ cleanup-plan machinery
  (`cleanup_plan::build`) so the compiler itself computes which locals cross
  a suspension is follow-up work this design enables but does not perform.
- **No state migration across ProgramRoot revisions.** `execution_revision::typed_migration`
  and `live_invocation::migration` already prove the shape a real checked
  migration takes (evaluate a pure migration function twice, reject
  disagreement, carry cumulative budget forward). A `resumable_effects`
  migration hook would follow that exact pattern once real checked state
  types exist to migrate between; adding a placeholder hook ahead of that
  would be a second source of truth this document's design explicitly
  avoids.
- **No native/Wasm lowering, no Agent-runtime migration onto this
  mechanism.** Both are named in #204's implementation sequence as steps 8
  and "interpreter first, then native/Wasm" — downstream of the
  syntax/HIR/verifier tranche above, not reachable without it.
- **No checkpoint byte-wire format.** `Journal`/`JournalEntry` are in-memory
  Rust values in this slice; `Journal::from_entries` is the seam a future
  canonical-JSON or byte-wire codec (matching
  `agent_runtime_v2::checkpoint`'s compact-JSON, sorted-key, bounded wire)
  would decode into, but no such codec exists yet here.

## Acceptance criteria: met here versus open

| Criterion (from issue #204) | Status |
| --- | --- |
| A non-Agent function can yield typed requests and resume safely | **Reference-validator level only.** `ResumableEffectProgram` is a Rust trait any non-Agent Rust type can implement and drive; no `.spx` source can do this yet. |
| Generated state machines are deterministic semantic projections | Proven at the reference level: `transition` is required to be a pure function and drift from that requirement is caught (`RequestDrift`/`TransitionDrift`). Not yet a compiler-generated projection from source. |
| Ownership, effects, contracts and authority survive suspension correctly | Ownership: reference-level `'static`/`Clone` gate only (see above). Effects: `EffectHandler` is the sole authority boundary. Contracts (pre/postconditions) and real compiler-checked ownership: **open**, need HIR integration. |
| Checkpoint/recovery never grants effect authority by itself | **Met**, including at the "reminted resume" level: `Journal`/`resume` never dispatch on a replayed entry, and a valid journal is refused outright under a scope the caller did not itself derive. |
| Agents can progressively reuse the mechanism rather than remain a separate runtime island | **Open.** `agent_lifecycle`/`agent_runtime_v2` are untouched (outside this module's lease); migrating even one Agent fixture onto `resumable_effects` is follow-up work once the syntax/HIR tranche exists for it to lower into. |

## Gate

`cargo test --locked -p semaprax --lib resumable_effects::` (19 unit tests)
and `cargo test --locked -p semaprax --doc resumable_effects` (2
`compile_fail` doctests proving the typed-resume and ownership compile-time
rejections) are this module's focused selectors.
