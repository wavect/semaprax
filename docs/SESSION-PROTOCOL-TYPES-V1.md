# Session/protocol types v1

Status: **reference validator**, not yet source syntax. Issue #206 asks for
a bounded session/protocol type model applied to two real subsystems. This
slice delivers a Rust-level protocol declaration, an affine typed endpoint,
and a runtime engine (`src/session_protocol/`) proving the message-order,
ownership and authority properties, plus this design. It adds no `.spx`
syntax, HIR node, verifier rule, or graph/architecture/Assurance-Manifest
projection -- see [Scope boundary](#scope-boundary).

Audience: compiler contributors implementing the source-syntax/HIR/backend
generalization this document specifies, and reviewers auditing what this
slice of #206 delivered versus what remains.

## What already exists on `main`

Before this change, three real mechanisms already ship, each checked
end to end for exactly one closed shape, and each explicitly outside this
module's lease -- nothing in `src/session_protocol/` edits any of them:

- `src/resumable_effects/core.rs` (issue #204) already proves a generic
  suspend/resume driver: a typed journal, `Intent`/`Observed`/`Transition`
  entries, replay that dispatches zero new host calls for an
  already-completed run, and sticky terminal-status cleanup. Its
  `EffectScope { program_root, invocation_id, policy_epoch }` is the direct
  model `session_protocol::engine::SessionTable`'s per-session `generation`
  counter follows: a stale credential must never be replayable as a bearer
  token.
- `src/live_invocation/` generalizes one closed effect boundary
  (`model.invoke`) with its own causal journal and budget/migration
  machinery.
- `src/project/candidate/multi_agent_coordination.rs` (issue #207) proves a
  *different* shape entirely: a bounded, one-shot coordination-session
  evidence document (participants, granted scopes, proposal evaluation)
  that is explicitly "proof data, not a scheduler" and never itself enforces
  a live message-order protocol on a resource across multiple round trips.

None of the three checks legal message *order* for an arbitrary
long-running interaction as a general, reusable, declared model -- named
states/transitions a caller declares once, checked by an engine every
time. That is exactly what #206 asks for and what
`session_protocol::spec::ProtocolSpec`,
`session_protocol::engine::SessionTable`, and `session_protocol::duality`
add.

`rg -l "protocol" src/` before this change surfaces one unrelated existing
use of the word: `src/protocol_check.rs` projects `.spx` `protocol`
*interface* declarations (a closed-vocabulary, body-less method-signature
construct -- see its own module doc) into JSON for tooling. It has no
relation to message order, states, or transitions, and this module does not
touch it, extend it, or rename around it.

## Design: the general shape

A declared protocol is plain owned data (`ProtocolSpec`): a name, a state
set, an initial state, a terminal-state set, a list of transitions, and a
canonical (never sorted, never repaired) cleanup inventory per terminal
state. One transition is:

```text
Transition {
    from:                 StateId,
    label:                Label,               // the message/operation name
    kind:                 Kind,                 // Send | Receive | Call | Return | Cancel | Timeout | Fail
    payload_type:         &'static str,         // a lightweight payload tag, not full type checking
    required_capability:  Option<&'static str>, // authority the caller must separately present
    ownership:            OwnershipMove,        // None | ConsumesResource
    next:                 Next,                 // Then(state) | Choice([(label, state); N>=2])
}
```

`ProtocolSpec::validate` collects every static defect in one pass rather
than stopping at the first (`SpecError`, nine variants): unknown
initial/terminal/next states, a duplicate label from one state, a
single-entry or duplicate-labeled branch, a terminal state with an outgoing
transition, a non-terminal state with no outgoing transition at all
(`DeadEnd`), a non-terminal state whose outgoing transitions never include a
`Cancel`/`Timeout`/`Fail` escape (`MissingEscape` -- "no abandoned
nonterminal endpoint unless cancellation/cleanup is defined", read as a
static declaration-level rule), and a terminal state missing its cleanup
entry.

### Two enforcement layers, not one

1. **Compile-time (Rust ownership).** `SessionTable::advance` consumes its
   `Endpoint` argument by value. Presenting the same live binding to a
   second operation is not a runtime check this module performs -- it is
   `rustc` rejecting a use of a moved value. `Endpoint`'s `Drop` panics if a
   live, non-terminal endpoint is ever simply dropped: "no abandoned
   nonterminal endpoint unless cancellation/cleanup is defined" is enforced
   as a linear-type drop bomb, the same idiom this codebase's affine
   resource types already use elsewhere, not a lint a caller can silently
   ignore.
2. **Runtime (`SessionTable`).** A protocol's states/transitions are
   declared *data* -- issue #206 asks for one general model, not one Rust
   type per protocol -- so the table independently checks message order,
   payload tag, required capability, ownership movement, and handle
   freshness (a per-session `generation` counter) every time. This is the
   layer that catches a handle that crossed a serialization boundary and so
   is no longer the same Rust value the move-checker can reason about (see
   [Failure and security cases](#failure-and-security-cases) below).

### A protocol state is not authority

Reaching a state, in the right order, proves only that a message sequence
was legal so far. `Transition.required_capability`, when set, is an
independent check: `SessionTable::advance` refuses `MissingAuthority` when
the caller does not present it, *even when the state and order are exactly
correct* -- state and authority are asserted as two different, independently
failing checks in every test that exercises this
(`missing_authority_is_refused_even_in_correct_order`).

For the one concrete capability shape this reference module can pin ahead
of time to a real Rust type (rather than the data-driven engine's string
tag), `session_protocol::capability::Grant<C>` makes the same principle a
compile-time rejection: a `Grant` minted for one capability marker type does
not type-check at a call site declared for a different one -- the same shape
`crate::model_call_receipt` already ships for "a receipt is not
authorization" (its own `compile_fail` doctest is the pattern this module's
three `compile_fail` doctests -- two in `src/session_protocol.rs`, one in
`src/session_protocol/capability.rs` -- copy).

### Ownership transfer

`OwnershipMove::ConsumesResource` transitions require a caller-presented
`ResourceToken { id: String }`. `ResourceToken` is deliberately `Clone`
(unlike `Endpoint`): it models the exact hazard this document's failure-case
list names -- "serializing endpoints can recreate authority" -- by letting a
test construct a literal duplicate of a token identity and prove the table
still refuses it. `SessionTable` tracks every consumed token id centrally
(`consumed_tokens: BTreeSet<String>`), independent of which session
presented it, so a cloned/duplicated token id is refused even when
presented by a wholly separate, otherwise-legal session
(`a_consumed_resource_token_cannot_be_reused_even_by_a_different_session`).

### Cancellation, timeout, and remote failure

`Cancel`, `Timeout`, and `Fail` are ordinary declared transitions with their
own `Next` target and their own cleanup entry -- never an implicit exception
path that bypasses the state machine. The two applied protocols
(`protocols::model_stream_protocol`, `protocols::resource_transaction_protocol`)
route a `Timeout` to a state distinct from an explicit `Cancel`
(`Uncertain`, not `Cancelled`/`RolledBack`): the remote side's actual status
after a timeout is genuinely uncertain, and this module never claims
otherwise by reusing the "cleanly cancelled" terminal for it. Reaching any
terminal state runs that state's declared cleanup inventory, in exact
declared order, exactly once; a cleanup entry's own failure is recorded in
`TerminalOutcome::cleanup` but never replaces `TerminalOutcome::terminal_kind`
-- failure selection is sticky, proven by
`remote_failure_terminal_status_is_sticky_against_cleanup_failure`, which
engineers every cleanup op to fail and asserts the terminal status is
unchanged, plus an exact-order assertion using op names whose alphabetical
order is the *opposite* of their declared order (proving nothing sorted
them).

### Checkpoint/resume restriction

A `Call` transition (e.g. the transaction protocol's `read`/`write`) opens a
pending in-flight operation that only its own matching `Return`, or an
escape, resolves. `SessionTable::checkpoint` refuses with
`CheckpointError::InFlightCall` while one is outstanding: only a settled,
non-pending protocol state may be automatically resumed, mirroring
`resumable_effects::Journal`'s identical `UnterminatedIntent` rule for "the
physical dispatch's outcome is uncertain, so this cannot be trusted for a
later resume."

### Duality/compatibility

`duality::check_duality` is a local, two-party check (multi-party global
protocols are explicitly out of scope per the issue). Given a spec pair, it
requires every transition on each side to have a counterpart on the other
(same state, same label) with a complementary kind (`Send`/`Receive`,
`Call`/`Return`, or identical escape kind), an identical payload tag, *and*
an identical required capability. The capability check is not incidental:
"duality can be unsound when payload effects or capabilities differ" (issue
#206's own failure-case list) is exercised directly by
`capability_divergence_is_refused_distinctly_from_a_generic_mismatch`, which
constructs a pair that is dual in every other respect and asserts the
specific `DualityError::CapabilityDivergence` variant, never the generic
`MissingCounterpart`/`KindNotComplementary` a naive structural-only check
would conflate it with.

## What this reference module implements

- `spec.rs`: `Kind`, `OwnershipMove`, `Next`, `Transition`, `ProtocolSpec`,
  `SpecError` (9 variants), `ProtocolSpec::validate`.
- `capability.rs`: `Grant<C>`, marker capability types, and the module's
  first `compile_fail` doctest.
- `engine.rs`: `Endpoint` (affine, drop-bomb-guarded), `ResourceToken`,
  `ProtocolError` (10 variants), `CheckpointError` (3 variants),
  `Checkpoint`, `TerminalOutcome`, `AdvanceOutcome`, `CleanupHandler`,
  `SessionTable` (`open`, `advance`, `checkpoint`).
- `duality.rs`: `DualityError` (4 variants), `check_duality_one_way`,
  `check_duality`.
- `protocols.rs`: `model_stream_protocol` (a model/tool streaming session:
  `Send`/`Receive`/`Branch`/`Cancel`/`Timeout`/`Fail`) and
  `resource_transaction_protocol` (a bounded database transaction:
  `Call`/`Return` with pending tracking, and `ConsumesResource` ownership on
  `commit`) -- the two applied subsystems issue #206 requires, at this
  module's Rust reference-kernel layer (see [Scope boundary](#scope-boundary)).
- `tests.rs`: every category below.

## What matters, and how it is tested

- **Out-of-order operations, operations after close, duplicate opens, and a
  stale/closed handle each get a specific, distinguishable reason.**
  `duplicate_open_is_refused_distinctly`,
  `out_of_order_operation_is_refused_distinctly`,
  `use_after_terminal_is_refused_distinctly_from_stale_handle`, and
  `stale_handle_is_refused_distinctly_from_use_after_terminal` each assert
  the exact `ProtocolError` variant **and** assert the rendered `Debug`
  string does not mention a named confusable neighbour's variant (mirroring
  the audited `Reason::InstanceTemplateChanged`/`ArgumentsChanged` defect
  this codebase's own review history calls out: a comparison that can never
  actually distinguish two cases is not a test). Every one of these is
  paired with the corresponding legal sequence succeeding on the same live
  endpoint the failed attempt handed back.
- **A protocol state is not authority.** `missing_authority_is_refused_even_in_correct_order`
  drives the exact right state, in the exact right order, with the exact
  right payload, and shows the operation is still refused without the
  declared capability -- and refused identically for a *wrong* capability,
  not silently accepted because "some" grant was present -- then succeeds
  once the correct capability is presented. The `Grant<C>` `compile_fail`
  doctest proves the same principle at compile time for the one concrete
  shape pinned to a Rust type.
- **Wrong branch, missing branch.**
  `unrecognized_branch_choice_is_refused_distinctly` and
  `missing_branch_choice_is_refused` are paired with **both** declared
  choices of the same branch succeeding and landing in their distinct
  states.
- **Payload mismatch.** `payload_type_mismatch_is_refused_distinctly`,
  paired with the declared tag succeeding.
- **Ownership/resource consumption.**
  `commit_without_a_resource_token_is_refused` and
  `a_consumed_resource_token_cannot_be_reused_even_by_a_different_session`
  (the latter using a cloned token id from an entirely separate, otherwise-
  legal session -- the "serialized identity" hazard, not a vacuous
  same-session double-use).
- **Cancellation/timeout/failure are explicit, and failure selection is
  sticky.** `cancellation_runs_its_declared_cleanup_in_exact_order`,
  `timeout_reaches_its_own_distinct_uncertain_terminal_state` (with an
  explicit `assert_ne!` against the Cancel terminal), and
  `remote_failure_terminal_status_is_sticky_against_cleanup_failure`.
- **Checkpoint/resume allowed and refused.**
  `checkpoint_is_refused_while_a_call_is_in_flight_and_allowed_once_settled`
  drives a `Call` to its pending intermediate state, asserts `InFlightCall`,
  resolves it with the matching `Return`, and asserts checkpoint now
  succeeds -- both directions in one test, on the same session.
- **Duality compatible/incompatible, capability divergence specifically.**
  `a_dual_client_server_pair_is_compatible`,
  `capability_divergence_is_refused_distinctly_from_a_generic_mismatch`, and
  `missing_counterpart_is_refused_distinctly_from_capability_divergence`
  each assert the exact `DualityError` variant present and assert the
  confusable neighbour variant is absent from every error the check
  returned.
- **The affine layer's own drop bomb.**
  `abandoned_nonterminal_endpoint_panics_on_drop` proves dropping a live,
  non-terminal `Endpoint` panics (via `catch_unwind`), with a message
  naming the abandonment invariant; `cancelled_endpoint_does_not_panic_on_drop`
  is the negative control, proving a *terminal* endpoint's drop is
  harmless. The module's second `compile_fail` doctest proves the stronger,
  earlier-catching half: presenting the same live `Endpoint` binding to a
  second operation does not compile at all.
- **Static declaration defects are each specific.** `spec_validation::*`
  (10 tests) exercises every `SpecError` variant, including two pairs of
  tests that specifically prove two rules are *not* conflated
  (`DeadEnd` vs. `MissingEscape`) and confirm both applied example
  protocols themselves validate cleanly
  (`both_applied_protocols_validate`) -- a direct regression against
  shipping a broken example.

## Failure and security cases

- **Protocol types can become verbose without inference/sugar.** Named,
  not solved, by this slice: `ProtocolSpec` is hand-authored Rust data;
  `.spx` surface syntax and sugar are exactly the parser-level follow-up
  this document does not implement (see [Scope boundary](#scope-boundary)).
- **Implicit error paths can escape the state machine.** Addressed:
  `Cancel`/`Timeout`/`Fail` are ordinary declared transitions with their own
  target and cleanup entry, never a side channel around `advance`.
- **Serializing endpoints/tokens can recreate authority.** Addressed at the
  reference-kernel level: `Endpoint` is not `Clone`, and any construction
  that reintroduces a duplicate identity (the "stale handle" test's literal
  duplicate, or a cloned `ResourceToken`) is caught by the table's
  generation counter or consumed-token set, respectively -- not prevented
  from being constructed (this module cannot prevent a future real
  serialization codec from existing), but refused when replayed.
- **Duality can be unsound when payload effects or capabilities differ.**
  Addressed directly: `check_duality` compares required capability
  identity, not mere presence, per transition pair.
- **Timeout may leave the remote side in an uncertain state.** Addressed by
  routing every declared `Timeout` transition in both applied protocols to
  a distinct `Uncertain` terminal, never conflated with a clean `Cancel`.

## Scope boundary

Explicitly **not** done in this slice, and why:

- **No `.spx` syntax, HIR node, verifier rule, or graph/architecture/
  Assurance-Manifest projection.** The repository's change protocol
  requires parser, canonical formatter, resolver/HIR, verifier, semantic
  graph, native backend, and Wasm backend to move together once syntax
  carries runtime meaning; landing a half-wired parser rule with no checked
  HIR consumer would violate that protocol rather than satisfy it. This is
  the same scope boundary `resumable_effects` and `live_invocation` already
  document for their own boundaries, and this document is the design a
  follow-up parser/HIR/graph tranche implements against.
- **No compiler-checked ownership analysis.** `Endpoint`'s affinity is
  enforced by Rust's own move checker and a runtime drop bomb over a
  reference kernel's own values, not the compiler's alias/uniqueness
  analysis over real checked HIR locals.
- **No multi-party global protocol verification.** `duality` is
  deliberately two-party/local only, per the issue's own explicit
  out-of-scope list.
- **No wire/byte format for a serialized `Endpoint` or `Checkpoint`.** Both
  are in-memory Rust values in this slice; a future canonical-JSON or
  byte-wire codec is a seam this design leaves for later work, not
  something this slice claims to have solved (see the "serializing
  endpoints can recreate authority" discussion above -- the risk is named
  and the *replay-time* check is proven, but no codec exists here to be
  hostile-input-tested).
- **No native/Wasm lowering, no migration of an existing subsystem (Agent
  lifecycle, network streams, publication workflows) onto this
  mechanism.** Both are downstream of the syntax/HIR tranche above.

## Acceptance criteria: met here versus open

| Criterion (from issue #206) | Status |
| --- | --- |
| At least two real interaction lifecycles are checked by the general protocol type system | **Reference-kernel level only.** `model_stream_protocol` and `resource_transaction_protocol` are both checked by the same `ProtocolSpec`/`SessionTable` machinery; no existing subsystem's own code (Agent lifecycle, network streams, a real transaction backend) was migrated onto it. |
| Invalid order is rejected before runtime | **Static declaration defects**: at spec-validation time (`SpecError`, before any session opens). **Message-order defects**: at the engine's own runtime check (`IllegalTransition` etc.) -- not before compilation, since the protocol is declared data in this slice, not `.spx` source the compiler itself parses. The one case genuinely caught by `rustc` at compile time is presenting an already-consumed `Endpoint` binding a second time, and presenting a `Grant` for the wrong capability marker type. |
| Ownership and authority are coupled to protocol state | **Met at the reference-kernel level**: `required_capability` and `OwnershipMove` are per-transition fields the engine checks alongside state/order, and are proven independently failing from state/order correctness (`missing_authority_is_refused_even_in_correct_order`). |
| Failure/cancellation/uncertainty remain explicit | **Met**: `Cancel`/`Timeout`/`Fail` are ordinary declared transitions with their own cleanup; `Timeout` is routed to a distinct `Uncertain` terminal in both applied protocols. |
| Protocol facts appear in context, graph, architecture, and assurance outputs | **Open.** No projection into `graph`, `architecture_claims`, or `assurance_manifest` exists in this slice -- each requires the parser/HIR/graph tranche above to have a real `.spx`-declared protocol to project in the first place; projecting a Rust-only reference kernel would be a second, disconnected source of truth. |

## Gate

`cargo test --locked -p semaprax --lib session_protocol::` (30 unit tests)
and `cargo test --locked -p semaprax --doc session_protocol` (3
`compile_fail` doctests: grant-for-wrong-capability, double-use of a
consumed `Endpoint`, and the capability module's own copy of the
grant-for-wrong-capability example) are this module's focused selectors.
