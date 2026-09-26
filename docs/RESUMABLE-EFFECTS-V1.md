# Resumable effects v1

Audience: compiler contributors implementing the source-syntax/HIR/backend
generalization this document specifies, and reviewers auditing what #204
delivered versus what remains.

Status: **a minimal `.spx` slice with compiler-owned lowering, interpreter
execution, authority-free production target preparation, and private
cross-backend execution parity evidence**, on top of a Rust reference
validator. Issue #204 asks for typed resumable effects in ordinary non-Agent
functions, with the generality of Aver's `yield` lowering. The current pieces
have different roles:

- `src/resumable_effects/` — a Rust-level generic driver, journal, capability
  gate, per-effect signature table and checkpoint codec, proving the
  suspend/resume semantics for any caller-chosen Rust types. No `.spx` source
  drives it.
- The admitted `.spx` `yields`/`yield` slice — parser, canonical formatter,
  resolver/HIR, verifier, semantic graph, native backend and Wasm backend,
  landed together.
- `resumable_effects::lowering` — one deterministic ordered-state HIR plan for
  one to eight direct sequential yield sites, including independently
  validated yield-free start and per-site resume projections.
- `interpreter::resumable` — the public compiler engine that executes the
  source suspension and consumes the plan's state and invocation identities.
- `resumable_effects::backend` — `cfg(test)`-only, crate-private parity runners
  that execute those yield-free projections through real native `-O0`/`-O2`
  and Core Wasm target paths. Ordinary native (`SPX-B116`) and Wasm
  (`SPX-W126`) emission still refuses a `yields` function; this is bounded
  test evidence, not production compiler authority, a public resumable ABI or
  a scheduler.
- `resumable_effects::target` — a production, authority-free preparation seam
  for the same admitted scalar slice. Given an already resolved program, its
  selected persistent function identity, and `NativeC11` or `CoreWasm`, it
  rederives the exact sequential plan and returns a bounded in-memory ordered
  inventory: one yield-free start projection followed by one yield-free resume
  projection per suspension site. Each artifact digest commits to the target,
  selected function, exact plan identity, state identity, projection position,
  selected entry symbol, and emitted bytes; the enclosing typed profile commits
  to the ordered digest vector. Native preparation disables the ordinary
  program-entry wrapper and adds a plan/site-specific selected wrapper. Backend
  output is capped during emission, not only after allocation. `verify()`
  independently re-lowers and re-emits the full inventory,
  rejecting source drift, forged/reordered plan state, target drift, or changed
  bytes. It is not a serialized manifest, a publisher, an executor, a host
  adapter, a continuation ABI, a scheduler, deployment evidence, or hosted
  support. It performs no filesystem, process, network, environment, secret,
  key, or signing operation.
- `resumable_effects::source_checkpoint` — a public ambient-authority-free envelope
  around the admitted sequential scalar continuation. The canonical bounded
  bytes authenticate the inner plan/site/argument/history proof and
  caller-supplied ProgramRoot, invocation identity, and policy epoch facts with
  a caller-owned 256-bit HMAC key. Decode requires that key and those current
  facts plus the checked program, selected function, and original arguments
  again. It returns only an inert continuation: the ordinary resume path must
  still receive a separate answer and replay every request. The codec performs
  no dispatch, storage, scheduling, publication, or authority minting. Its
  explicit v2 migration API authenticates the old envelope, replays both old
  and destination revisions without a handler, requires compatible persistent
  function/channel/site structure and bit-exact historical/current requests,
  then emits canonical v2 bytes under separately supplied destination scope
  and key. Migration is fuel-bounded proof work, not execution authority.
- `resumable_effects::source_signature` — a pure bridge from the selected
  persistent source function and its checked `yields Request -> Response`
  clause to the runtime `EffectSignatureTable`. It uses versioned
  `ResolvedType::identity_key()` shapes, records the exact number of lowered
  sites, and binds the result to the deterministic lowering identity. Its
  `verify()` path re-lowers rather than trusting stored strings. It creates no
  handler and grants no effect or resume authority.
- `resumable_effects::source_driver` — a fresh, synchronous interpreter driver
  for that admitted scalar source lane. The caller injects both an
  `EffectHandler` and an explicit `CapabilityPolicy`; the driver derives the
  source signature, checks every request and answer tag, and enforces
  cancellation plus per-segment/cumulative fuel, call, request-byte and total
  channel-byte budgets before each physical dispatch. Its bounded ordered
  records retain exact scalar bits but no host error strings. They are inert
  run evidence, not a journal, checkpoint, retry token, scheduler, target
  runtime, or crash-safe exactly-once claim.

What is still open is in [Scope boundary](#scope-boundary).

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
  `RequestMismatch`, `NonSequentialTurn`, `EntryAfterTerminal`,
  `EntryAfterObservationFailure`, and `UnterminatedIntent` (a journal ending
  on a bare `Intent` is uncertain —
  whether the physical dispatch happened is unknown — and is rejected
  before any stage, mirroring `agent_runtime_v2::checkpoint`'s identical
  rule).
- `run` / `resume`: the driver. `resume` replays every entry the journal
  already has — recomputing `request`/`transition` and asserting they
  agree with the recorded entries (`RequestDrift`/`TransitionDrift` if not)
  — without ever calling the injected handler for a replayed turn, then
  performs genuinely new turns past the journal's recorded tail as fresh
  dispatches with their own accounting. Both functions return the journal
  on *every* path, including failure, so a caller can persist the latest
  checkpoint candidate even when a call fails. `Journal` itself is only an
  in-memory vector: this module supplies no durable append sink and makes no
  crash-recovery claim.
- Cleanup only runs for `Complete`/`Fail` (a `Suspend` deliberately keeps a
  computation's resources live for a later resume); cleanup ops run in
  exact `cleanup_plan` order for a fresh terminal, and a cleanup failure is
  recorded in `Outcome::cleanup` but never replaces the already-selected
  `terminal` — failure selection is sticky by construction, not by a
  downstream check. Replaying an in-memory returned terminal runs no cleanup
  again. The v1 wire does not record cleanup settlement, so a decoded or
  externally reconstructed terminal is cleanup-ambiguous and cannot support an
  exactly-once crash-recovery claim.

`src/resumable_effects/capability.rs`:

- `CapabilityPolicy`: a bounded (≤64, mirroring
  `AGENT-TYPED-EFFECTS-V3.md`'s own registry ceiling), ordered, duplicate-
  and empty-id-rejecting allowlist of capability ids.
- `CapabilityGatedHandler`: wraps an already-injected `EffectHandler` and
  refuses, before the wrapped handler is ever called, any request whose
  caller-supplied `capability_of` mapping names an id outside the current
  policy. A denial is reported through the driver's existing
  `HandlerFailed`/`ObservationFailed` path — the same one a genuine host
  failure already takes — so it is explicit evidence in the returned journal,
  not a silently dropped decision, and replaying a denied journal reports the same denial
  again without a second call to the wrapped handler (or the gate itself).
  This implements the "Effect declarations and capability requirements"
  scope bullet as a decorator at the existing effect-authority boundary,
  rather than a change to `run`/`resume` or the `ResumableEffectProgram`
  trait.

`src/resumable_effects/migration.rs`:

- `StateMigration<From, To>` / `migrate_suspended`: a pure state-migration
  function between two `ResumableEffectProgram`s' `State` types, evaluated
  *twice* and rejected on disagreement, mirroring the exact pattern
  `execution_revision::typed_migration` and `live_invocation::migration`
  already prove for real checked state types ("evaluate a pure migration
  function twice, reject disagreement, carry cumulative budget forward").
  Only a genuinely `Suspend`ed outcome is a migration candidate; migrating
  under an unchanged `program_root` is refused (`resume`'s journal-replay
  path is correct there, not migration). A successful migration carries the
  old revision's cumulative `Outcome::dispatched` count forward in
  `MigratedState` rather than silently resetting it. This function mints no
  effect authority and performs no dispatch itself — its result is a new
  `State` a caller must still drive through the ordinary `run`/`resume`
  machinery under a freshly derived scope. This implements implementation-
  sequence step 7 ("Implement pure state migration functions between
  compatible ProgramRoot revisions and reject incompatible state changes")
  at the reference level; it does not migrate the compiler plan's state or
  bind a migration to its checked-HIR identity (see
  [Scope boundary](#scope-boundary)).

`src/resumable_effects/signature.rs`:

- `EffectSignatureTable`: a bounded (≤64, the same ceiling), ordered,
  duplicate- and empty-rejecting table of `EffectSignature { effect_id,
  request_shape, answer_shape }`. `answer_shape` is the "what resuming this
  suspension must supply" half of a typed resumable effect — the half
  `core`'s whole-program `Observation` type parameter cannot express.
- **Why it is needed.** `ResumableEffectProgram` fixes exactly one
  `Request`/`Observation` pair per program, so `rustc` rejects an
  observation of the wrong *Rust* type. That is a whole-program channel
  type, not a per-effect one. A computation that waits on several distinct
  effects must model `Request`/`Observation` as enums or tagged records, and
  at that point Rust sees one type and checks nothing about *which* effect a
  given answer answers: a `clock` suspension can be resumed with a `model`
  answer, and both the handler boundary and a recovered journal accept it
  silently. This module is that missing check, and only that check.
- `SignatureCheckedHandler`: a decorator over an already-injected
  `EffectHandler`, in the same shape as `CapabilityGatedHandler` and
  composable with it in either order. It refuses an undeclared effect id or
  a request whose shape disagrees with its declaration *before* the wrapped
  handler — the only physical effect boundary — is called at all, and
  refuses an answer that names a different effect, or the right effect in
  the wrong shape, before that answer can become the observation a
  `transition` reads. Both refusals travel the driver's existing
  `HandlerFailed`/`ObservationFailed` path, so `run`/`resume` are unchanged,
  the refusal is explicit evidence in the returned journal, and it can never replace an
  already-selected terminal status. The answer check necessarily runs after
  the wrapped handler returns — an answer cannot be inspected before it
  exists — so a genuinely authorized physical effect may already have
  happened when an answer is refused; what the refusal guarantees is that a
  mismatched answer never becomes an `Observed` entry or reaches a
  `transition`.
- `validate_journal_signatures`: the same check re-applied to a *recovered*
  journal. `Journal::validate` checks scope, ordering and request identity,
  but it compares an `Observed` entry's observation against nothing —
  observation and request are the program's own two Rust types and any pair
  of them is structurally legal. A hand-tampered or corrupted checkpoint can
  therefore pass `Journal::validate` and still be refused here, reported as
  the exact entry index plus a distinct `SignatureMismatch` variant
  (`UnknownEffect`, `RequestShapeMismatch`, `AnswerForWrongEffect`,
  `AnswerShapeMismatch`). Journal order is preserved: the first offending
  entry is reported, and the journal is never sorted, skipped past, or
  repaired.
- **Nothing here runs anything.** Constructing a table, checking a shape and
  validating a journal are pure functions over caller-supplied data: they
  dispatch no effect, spawn no work, and mint no authority. A signature is
  proof data about what an answer must look like, never permission for
  anything to produce one. Shapes in the general Rust API remain
  caller-supplied opaque strings compared for exact equality. Source callers
  instead use `source_signature::derive_source_effect_signature`, which
  reuses the compiler-owned lowering and derives the effect id and versioned
  request/answer shapes from checked HIR. The two layers remain separate so
  arbitrary Rust programs can use the checking discipline without pretending
  that their caller-owned strings came from `.spx` source.

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
- **A resume that does not match is refused, twice over.**
  `an_undeclared_effect_is_refused_before_the_wrapped_handler_is_called` and
  `a_request_whose_shape_disagrees_with_its_declaration_is_refused_before_the_wrapped_handler`
  wire a wrapped handler whose `dispatch` **panics** if called at all, so
  "refused before the effect boundary" is proven rather than asserted.
  `an_answer_naming_a_different_effect_never_becomes_an_observation` offers a
  perfectly well-formed `model` answer against a pending `clock` suspension —
  a swap `rustc` cannot see, because both are the same Rust type — and
  asserts the run fails and the journal contains **no** `Observed` entry.
  `a_tampered_answer_that_structural_validation_accepts_is_refused_by_signature_checking`
  asserts `Journal::validate` returns `Ok` for the tampered journal and
  `validate_journal_signatures` still refuses it at the exact entry index,
  so the new check demonstrably catches something the existing structural
  validator cannot.
  `a_journal_recovered_under_a_table_that_no_longer_declares_its_effect_is_refused`
  is the "resuming after a code change can execute state under incompatible
  semantics" case. `a_refused_answer_is_replayed_as_the_same_refusal_without_a_second_dispatch`
  proves non-redelivery for the supplied in-memory journal: the replay wires
  a panicking handler. It is not a crash-safe exactly-once claim.


## Compiler-owned lowering, target preparation, and private execution parity

`src/resumable_effects/lowering.rs` derives a `SequentialResumablePlan` from
the exact checked HIR program. The plan has explicit `entry`, per-site
`suspended` and `complete` state identities, every authored yield-site identity
and type pair, and an ordered family of yield-free HIR programs. The original
one-site `ResumablePlan` shape and `lower` entry point remain intact;
`lower_sequential` is the additive multi-site surface:

- the **start/request projection** evaluates the authored precondition and
  prefix, then returns the request instead of crossing the yield;
- each **resume projection** adds the ordered compiler-owned answers through
  one site, replays the pure scalar prefix (including request evaluation), and
  either returns the next request or evaluates the final suffix and
  postcondition. The bound is eight sites to cap the precomputed HIR family.

Both projections rebuild loan and cleanup proof attachments and pass ordinary
HIR validation before any backend sees them. Moving a request subtree also
rederives its structural expression and local identities at the new canonical
path; copied HIR identities are not accepted as a shortcut. The plan refuses
an entrypoint or any retained incoming caller because replacing a function's signature
inside an otherwise callable program would be unsound. It walks the direct-call
closure required by the selected function and authored entrypoint, retains only
that closure, and diagnoses a retained `yields`-declaring callee specifically.
Disconnected yielding functions are therefore isolated rather than rejected.
The retained closure must itself stay inside the explicit, effect-free
Copy-scalar profile with no owned cleanup; authored nominal/authority declarations,
generic calls and function references remain outside this projection.

The plan identity commits to deterministic checked-HIR bytes, the selected
function and the ordered yield sites. A pending suspension additionally binds
that identity, its suspended-state/site identity, the exact tagged bits of
every original scalar argument, and every prior answer. Request equality is insufficient:
`yield 0; answer + seed` can produce the same request for two invocations whose
suffix result differs. A state or binding mismatch fails closed with
`SPX-F115`; the digest is proof data and confers no authority.

`src/resumable_effects/target.rs` exposes the production-library preparation
API for this exact plan. It independently re-lowers the selected function,
emits one start plus one artifact per resume site through the ordinary native
C11 or Core-Wasm scalar generator, and returns only a typed, bounded in-memory
inventory. Each artifact and the ordered profile carry domain-separated
digests; `verify()` re-lowers and re-emits rather than trusting those stored
digests. The API performs no compilation, target execution, filesystem write,
publication, scheduling, checkpoint recovery, or host call. Ordinary native
and Wasm emitters still refuse the original yielding program.

`src/resumable_effects/backend/` is compiled only under `cfg(test)` and is
crate-private. Its runners replay each recorded request projection and compare
exact request bits before executing the current resume projection through the real C/native
and Core Wasm generators. The test process explicitly uses local temporary
storage plus `clang`/Node from its environment; none of that ambient authority
is present in a production library build or granted to generated code. This
proves target agreement for the bounded plan without changing the ordinary
emitters' `SPX-B116`/`SPX-W126` refusal or inventing a public checkpoint ABI.
The Core Wasm runner keeps the Public Scalar Export Profile's existing
`usize` refusal (`SPX-W115`); native evidence covers it with the target's
64-bit carrier. Because its test adapter crosses the JavaScript `Number`
boundary, arbitrary NaN payload/signaling-bit preservation is not a portable
Wasm claim. Arithmetic failures in both the request prefix and resumed suffix,
plus failing `requires` and `ensures`, preserve the interpreter's exact
normalized status across native `-O0`/`-O2` and Core Wasm. These runners make
no fuel, cleanup-event, scheduling or durability claim.

This is intentionally separate from `resumable_effects::core`. That reference
driver synchronously dispatches a request through an injected handler; its
`Step::Suspend` is a terminal/replayed outcome, not a pending source `yield`
that accepts an externally supplied answer. Reusing it for source suspension
would require a distinct external-await journal phase and runtime protocol,
neither of which this tranche adds.

## Interpreter execution

`src/interpreter/resumable.rs` runs `.spx` suspensions. The exhaustively
matchable legacy one-site `ResumableStep` and its start/resume functions remain
unchanged and reject multi-site input. Multi-site programs use the additive
`SequentialResumableStep`, an opaque `ResumableContinuation`, and
explicit sequential start/resume functions:

```text
run_resumable_effect(program, function_id, arguments, max_steps)
    -> Suspended { state, binding, request }
     | Completed { state, result } | LanguageFailure | ...
resume_resumable_effect(
    program, function_id, arguments, state, binding, request, answer, max_steps
)
    -> Completed { state, result } | ...
run_sequential_resumable_effect(program, function_id, arguments, max_steps)
    -> Suspended { continuation }
     | Completed { state, result } | LanguageFailure | ...
resume_sequential_resumable_effect(
    program, function_id, arguments, continuation, answer, max_steps
)
    -> Suspended { continuation }
     | Completed { state, result } | LanguageFailure | ...
```

**How a resume works, and why it is sound here.** A suspension is resumed by
re-executing the function *from its entry* with the answer substituted at the
yield site. That is not a general continuation, and it would be wrong for a
general one. It is correct for exactly this slice because the slice
forecloses every way a re-execution could differ from or duplicate the
original prefix: a `yields`-declaring function may declare no `uses` effects
(`SPX-T302`), so the replayed prefix contacts no host and can redispatch
nothing; every parameter and intermediate value is an admitted Copy scalar
(`SPX-T301`/`SPX-T303`), so nothing owned is live across the suspension and
the replay allocates and frees nothing; and one to eight `yield` sites execute
in authored top-level sequence (`SPX-T297`/`SPX-T298`), so control cannot branch
around or repeat a site. The opaque continuation carries prior scalar
request/answer pairs; its binding commits prior answer bits, and replay checks
every historical request before consuming the current answer. Finally, lowering rejects a reachable
`yields` callee, so the call closure cannot hide a second suspension. Widening
any of those restrictions invalidates this execution model and requires a
general continuation lowering.

**Replay is proven, not assumed.** Every resume recomputes the request from
the replayed prefix and requires it to equal the request the suspension
recorded. Disagreement is refused (`SPX-F114`) rather than answered — the
same `RequestDrift` discipline `resumable_effects::core` enforces at the
reference level, now applied to real source. Request comparison is the second
check, not the identity check: before replay, the suspension's `state` and
`binding` must match the exact checked program, yield site and bit-exact
original arguments and prior answer bits (`SPX-F115`). This rejects different
arguments or histories even when they deliberately compute the same request.

**A resumed computation is checked, not trusted.** Both the recorded request
and the supplied answer are checked against the function's declared
`yields Request -> Response` types before the program is entered at all; a
mismatch is `SPX-F113`. Floats compare by bits, so `-0.0` is never silently
accepted for `0.0` and a replayed `NaN` request still matches itself.

**A suspension carries no authority.** This lane opens no file, spawns no
process and contacts no network. A suspension is proof data about what the
program asked for, never permission to satisfy it; who answers a request, and
whether they were entitled to, stays the caller's concern
(`resumable_effects::capability`). Every *other* interpreter lane still
refuses a `yield` outright — `Resumption::Refused` is the default every other
evaluator carries — so nothing gained the ability to suspend by accident.

New diagnostics: `SPX-F113` (a resume value's type disagrees with the
declared `yields` signature), `SPX-F114` (the replayed prefix recomputed a
different request than the suspension recorded), and `SPX-F115` (the state
or invocation binding is not for this exact program, yield site and original
argument bits). All fail closed before a resumed result is produced.

**Closed recovery bytes for the admitted source lane.** The crate-private inner
`interpreter::resumable::checkpoint` module additionally encodes a sequential
continuation as bounded (16 KiB), closed canonical JSON under
`semaprax.source-resumable-sequential-checkpoint.v1`. It records only scalar
request/answer history and the selected suspension state; float values carry
their exact bits. On recovery the caller supplies the current checked program,
function ID, and original arguments again. The decoder re-lowers that program,
selects its exact site, derives the binding from the independently supplied
arguments and decoded prior-answer bits, verifies every scalar type, checks a
corruption-detecting self-digest, and rejects noncanonical, truncated, and
oversize bytes. The digest is not a MAC: a deliberately rewritten canonical
checkpoint can be decoded, but the ordinary resume path deterministically
replays and rejects any forged historical/current request before publishing a
result. Decode itself does not run source, dispatch an effect, or grant an
answer authority. `resumable_effects::source_checkpoint` is the public scoped
envelope around this structural representation; the inner codec stays private
so callers cannot bypass the required ProgramRoot/invocation/policy binding.
The original `semaprax.source-resumable-checkpoint.v1` API and bytes remain an
explicit compatibility lane. The opt-in
`semaprax.source-resumable-checkpoint.v2` API additionally authenticates the
compiler-derived request and answer type identities, sequential-plan identity,
and exact yield count under a distinct HMAC domain. Its encoder first decodes
the continuation against the current checked program and original arguments;
its decoder authenticates and proves canonical bytes before re-deriving those
compiler facts. V1 and v2 reject one another rather than silently upgrading or
downgrading a stored document.
Neither layer is a public continuation ABI, durable production runtime,
migration format, or target scheduler.

## Durable continuation (separate contract)

[Resumable Effects Continuation v1](RESUMABLE-EFFECTS-CONTINUATION-V1.md)
defines the first public bounded continuation route for the two-to-eight-site
sequential scalar profile: a non-bearer request/answer exchange, explicit host
capability at dispatch and answer, sticky settlement, and a durable
append-only journal with crash recovery from the observed tail. It reuses the
v2 envelope and the interpreter plans above unchanged. The scope-boundary
items below remain open except where that contract states otherwise.

## Scope boundary

Explicitly **not** done in this slice, and why:

- **The `.spx` slice is minimal by construction, not by accident.** It admits
  one to eight `yield` sites only in authored sequence at the function's own
  top level (never in a loop, a conditional branch,
  a call argument or any nested expression), scalar request/response/
  parameter/local types, no `uses` effects, no generics, free functions only.
  The closed projection can isolate a selected function from disconnected
  yielding functions, but every retained entrypoint/helper must satisfy the
  same explicit, effect-free Copy-scalar target-neutral profile. Authored
  nominal/authority surfaces anywhere in the source program still fail closed
  until their correlated dependency graphs can be pruned exactly. Widening any
  one of these constraints is its own tranche across the same seven layers.
- **The admitted source channel now derives and enforces checked signature
  shapes, but the general Rust API remains open-ended.**
  `source_signature` derives the selected stable function id and versioned
  request/answer `ResolvedType` identities into a one-row
  `EffectSignatureTable`, bound to the exact sequential lowering identity and
  site count. Reverification rejects changed source, types, dependencies, or
  plan meaning rather than repairing the binding. The fresh synchronous source
  driver reuses that exact table with an injected handler, explicit capability
  policy, cancellation and bounded accounting. Tagged multiple effect families,
  owned aggregate channels, durable recovery driving, asynchronous scheduling,
  and public native/Wasm handler integration remain open.
- **No compiler-checked owned suspension state.** `'static + Clone + Eq + Debug`
  is this reference module's own approximation of "plain owned, transferable
  data" — it rejects a borrow or a non-`'static` handle the same way a real
  ownership checker would reject an escaping reference, but it is not the
  compiler's alias/uniqueness analysis over real HIR locals. The bounded plan
  independently rebuilds loan and cleanup attachments, but admits only Copy
  scalars. Sequential suspension commits exact scalar answer history into its
  derived binding, and public recovery additionally authenticates serialized
  history with the caller-owned checkpoint key; it does not compute liveness or
  carry a live local frame. Computing and
  carrying a real owned suspension frame remains follow-up work.
- **General owned-state migration exists only at the reference level.**
  `src/resumable_effects/migration.rs`'s `migrate_suspended` now proves the
  checked-migration *pattern* (`execution_revision::typed_migration` and
  `live_invocation::migration`'s "evaluate twice, reject disagreement,
  carry cumulative budget forward") for an arbitrary caller-chosen `State`
  pair. The public scalar checkpoint v2 lane now supports narrower checked
  source migration: old bytes are authenticated, both revisions replay the
  recorded answer prefix without dispatch, and every historical and pending
  request must agree bit for bit before the destination continuation is signed.
  Persistent function and invocation identities, parameter/result/channel
  types, ordered site positions and state identities remain compatible; the
  ProgramRoot must change, while destination key and policy epoch are explicit
  caller choices. Per-segment and combined old/new replay fuel are bounded.
  The current plan still has only compiler-owned scalar control identities and
  no migration-compatible owned local frame; defining compatibility and pure
  transforms for future owned state remains downstream work.
- **No public native/Wasm resumable execution runtime or Agent-runtime
  migration onto this mechanism.** The public preparation API emits bounded
  authenticated in-memory C11/Wasm projection artifacts, and the private
  parity runners execute those semantics through real native and Core Wasm
  target paths. Ordinary emission still refuses a `yields`-declaring function
  outright — `SPX-B116` and `SPX-W126`. A prepared artifact grants no target
  execution or publication authority. There is no public continuation ABI,
  external-await runtime seam, durable checkpoint store/service, or target
  scheduler. The scoped scalar checkpoint envelope above is deliberately not
  any of those seams. Migrating an Agent fixture onto the
  mechanism is untouched.
- **Bounded public checkpoint envelopes exist for the admitted scalar
  source lane, not a durable runtime.** A caller-owned 256-bit HMAC key
  authenticates the closed sequential continuation together with exact external
  ProgramRoot, invocation, and policy-epoch facts supplied independently again
  at recovery. V2 also binds the checked source channel shapes, lowering-plan
  identity, and yield count; it inherits the inner sequential codec's current
  two-to-eight-site checkpoint admission even though the signature bridge can
  describe a one-site plan. An untrusted store cannot rebind history across
  those scopes or signed compiler facts without the key. Decode re-lowers the
  current program and derives the suspension binding; normal resume replay
  remains the only path that can accept an answer. The key is zeroized on drop
  and grants no effect or resume authority. Neither envelope stores bytes or
  dispatches work, and neither is an endpoint, scheduler, checkpoint service,
  or compatibility promise for broader owned/control-dependent state.
- **A checkpoint byte-wire codec also exists at reference level.**
  `src/resumable_effects/codec.rs` encodes a `Journal` bound to its
  `EffectScope` into closed, deterministic JSON bytes and back, matching
  `agent_runtime_v2::checkpoint::codec`'s "closed deterministic JSON,
  canonical byte equality rejects duplicate keys" discipline: an explicit
  versioned schema (`RESUMABLE_EFFECTS_CHECKPOINT_SCHEMA`), a size bound
  (1 MiB) and an entry-count bound (4096) checked before parsing entries,
  a self-consistency digest, and three-way scope-binding checks
  (`StaleProgramRoot`/`WrongInvocation`/`WrongPolicyEpoch`) against the
  caller's independently derived `expected` scope before a single entry is
  reconstructed. Every caller-chosen `State`/`Request`/`Observation`/
  `Result` type opts in through a new, narrower `EffectCodec` trait rather
  than the base `ResumableEffectProgram` bound growing a codec requirement
  every program pays for. Decoding still performs none of
  `Journal::validate`'s ordering/turn-sequencing/request-identity checks —
  a decoded journal is recovered bytes, not a trusted one, and the caller
  must still call `validate` (and `run`/`resume`) before anything is
  granted. This remains a reference-level wire format for the Rust-trait
  driver only: it has no ProgramRoot-derived schema hash and does not bind
  to a real checked source type, which is downstream of the syntax/HIR
  tranche below.

## Acceptance criteria: met here versus open

| Criterion (from issue #204) | Status |
| --- | --- |
| A non-Agent function can yield typed requests and resume safely | **Met only for the bounded `.spx` slice.** A selected ordinary free function declares one typed channel and one to eight direct sequential `yield` sites; `interpreter::resumable` runs them through an opaque continuation, the public authority-free preparation API emits exact native C11/Core-Wasm projection inventories, and `cfg(test)`-only native `-O0`/`-O2` and Core Wasm runners execute the same staged projections. A public bounded checkpoint envelope HMAC-authenticates its private structural continuation with independently supplied ProgramRoot/invocation/policy facts and re-derives the binding from the current program/function/arguments; it is not a public runtime ABI or durable store. Resume checks answer types (`SPX-F113`), every replayed request (`SPX-F114`), and exact program/site/argument/prior-answer binding (`SPX-F115`). Ordinary native/Wasm emission still refuses (`SPX-B116`/`SPX-W126`). Disconnected yielding functions are pruned; nested or control-dependent yields, owned state and effectful prefixes remain open. Distinct request/response types are admitted for direct `let`, mutable whole-binding assignment, and tail sites; assignment checks its target against the retagged response type rather than the request placeholder. |
| Generated state machines are deterministic semantic projections | **Met only for the bounded ordered replay plan.** `SequentialResumablePlan` deterministically derives entry/per-site-suspended/complete identities and independently validated yield-free start/per-site-resume HIR projections while the original one-site `ResumablePlan` remains source-compatible. The interpreter consumes its identities and opaque history; the public preparation profile binds target artifacts to the plan/state/role/bytes and independently re-emits them during verification; private native and Wasm runners execute its projections. Live-frame/liveness lowering and control-dependent yields remain open. |
| Ownership, effects, contracts and authority survive suspension correctly | For the `.spx` plan, only Copy scalars are admitted, ordinary effects and reachable yielding callees are refused, the start projection owns precondition evaluation, the resume projection owns the suffix/postcondition, and suspension bindings confer no authority. The checked source `yields` clause derives its exact versioned runtime signature table and plan binding rather than relying on caller-authored shape strings; the fresh synchronous driver checks that table and explicit capability policy before every injected host call and makes failures sticky without retry. Owned values across suspension, effectful prefixes, durable recovery driving, asynchronous scheduling, and public native/Wasm handler integration remain **open**. At the separate Rust-reference level, `EffectHandler`, `CapabilityGatedHandler` and `SignatureCheckedHandler` prove the more general checking discipline. |
| Checkpoint/recovery never grants effect authority by itself | **Met** for the reference journal and the bounded source continuation proof. `Journal`/`resume` never dispatch on a replayed entry, and a valid journal is refused outright under a scope the caller did not itself derive. `decode_checkpoint` performs the identical three-way scope check before reconstructing any entry, and a decoded-then-validated journal still cannot be resumed under a scope the caller did not itself derive. Separately, the public sequential-source v1 envelope requires a caller-owned key to authenticate exact ProgramRoot/invocation/policy scope; the additive v2 envelope also authenticates compiler-derived request/answer shapes, lowering-plan identity, and yield count. Both recover through the same private structural codec, which re-derives plan/site/binding from caller-supplied checked program/function/arguments. V2 migration additionally replays both revisions and compares all recorded requests before signing a destination checkpoint. None of decode, migration, or evidence performs host dispatch or grants answer authority. |
| Agents can progressively reuse the mechanism rather than remain a separate runtime island | **Open.** `agent_lifecycle`/`agent_runtime_v2` are untouched (outside this module's lease); migrating even one Agent fixture requires a public external-await/runtime seam, durable source checkpointing and a broader state profile than this private scalar plan provides. |

## Gate

`cargo test --locked -p semaprax --lib interpreter::resumable::tests::`
runs 20 unit tests driving real `.spx` source through parse, resolve, start,
exactly bound resume and same-request/different-argument refusal.

`cargo test --locked -p semaprax --lib resumable_effects::lowering::tests::`
runs 15 deterministic-plan, closed HIR-projection, provenance,
canonical-identity and hostile-mutation tests. The required physical target
selector is:

`cargo test --locked -p semaprax --lib resumable_effects::lowering::sequential_tests::`
runs 5 ordered-site, eight-site-bound, history-binding, contract-placement and
distinct-type projection tests.

`cargo test --locked -p semaprax --lib resumable_effects::source_checkpoint:: -- --nocapture`
runs 10 v1/v2 scope, signature-drift, canonicality, downgrade, corruption,
bound and replay tests. The v2 cases exercise recovery through both suspension
sites and preserve the unchanged v1 compatibility wire.

`cargo test --locked -p semaprax --lib resumable_effects::source_checkpoint::migration::tests:: -- --nocapture`
runs 12 old/new replay, forged-history, exact scalar-bit, compatibility,
argument-change, key/scope rotation, and exact fuel-boundary migration tests.

`cargo test --locked -p semaprax --lib resumable_effects::source_driver::tests:: -- --nocapture`
runs 10 fresh-run success and refusal tests for compiler-derived tags, exact
scalar bits, explicit capability policy, cancellation, fuel/call/byte bounds,
sticky host/language failures, and one physical dispatch per source site.

```sh
SEMAPRAX_REQUIRE_RESUMABLE_BACKENDS=1 \
  cargo test --locked -p semaprax --lib \
  resumable_effects::backend::tests:: -- --nocapture
```

It runs 10 private parity tests and fails rather than skips if local `clang` or
Node is absent. The combined
`SEMAPRAX_REQUIRE_RESUMABLE_BACKENDS=1 cargo test --locked -p semaprax --lib resumable_effects::`
selector runs 97 tests, including the 23 reference-driver replay tests.
`cargo test --locked -p semaprax --doc
resumable_effects` retains the two `compile_fail` doctests proving the
typed-resume and ownership compile-time refusals.

These are local, offline, re-runnable results. They are not hosted evidence,
do not promote the private projection runners to public native/Wasm support,
and do not close #204's general lowering or runtime work.
