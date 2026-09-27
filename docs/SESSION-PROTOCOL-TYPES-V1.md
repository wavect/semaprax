# Session/protocol types v1

Audience: compiler contributors implementing the source-syntax/HIR/backend
generalization this document specifies, and reviewers auditing what this
slice of #206 delivered versus what remains.

Status: **reference validator plus a checked, erased `.spx` declaration**.
Issue #206 asked for a bounded session/protocol type model applied to two
real subsystems. `src/session_protocol/` delivers a Rust-level protocol
declaration, an affine typed endpoint, and a runtime engine proving the
message-order, ownership and authority properties. Two real subsystems --
`project_transport::session` and `database_fixture`'s transaction -- hold
live `SessionTable`s and take every lifecycle decision through this engine
at runtime.

Issue #297 adds the source half: a `session protocol` declaration that the
parser, canonical formatter, verifier (`SPX-K1xx`), per-source semantic
graph (`semaprax.graph.v48`), `context`, Architecture Claims
(`protocol_realizers_bound`) and Assurance Manifest (`session_protocol`
obligations) understand -- and no other output (see
[Non-claims](#non-claims)) -- bound to the declaration's `@id`, its source
span, and the checked HIR functions its `via` clauses name. The declaration
is checked and erased: it has no runtime representation, lowers to nothing on
the native or Wasm backend, and grants no authority. See
[Declared session protocols](#declared-session-protocols-issue-297). Typestate
checking of `.spx` endpoint *values* (use-after-close rejected at compile time
in `.spx` source) is still not implemented -- see
[Scope boundary](#scope-boundary).

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
`Call`/`Return`, or identical escape kind), an identical payload tag, an
identical required capability, an identical `OwnershipMove`, *and* an
agreeing continuation. The capability check is not incidental:
"duality can be unsound when payload effects or capabilities differ" (issue
#206's own failure-case list) is exercised directly by
`capability_divergence_is_refused_distinctly_from_a_generic_mismatch`, which
constructs a pair that is dual in every other respect and asserts the
specific `DualityError::CapabilityDivergence` variant, never the generic
`MissingCounterpart`/`KindNotComplementary` a naive structural-only check
would conflate it with.

#### Continuation duality

Comparing only kind, payload and capability makes the compatibility proof
hold for exactly *one* message: nothing stops the two roles from landing in
different states afterwards and reading their next legal moves from
disagreeing states. The module's internal `check_continuation` closes that, with three
separate refusals so a caller can tell which property broke:

- `BranchNotOffered` -- the side that **selects** a branch may choose a
  label the peer's counterpart has no case for. This is the "a branch the
  peer never offers" defect. The rule is directional by kind, not by
  argument order: for the unambiguous `Send`/`Receive` pair the sender
  chooses what to emit and the receiver must have a case for every choice
  the sender may make, so it is a **subset** rule, not an equality rule --
  a receiving role that offers *extra* branches the sender can never select
  is safe, and `an_offering_peer_with_extra_branches_stays_compatible` pins
  that, so the refusal cannot degrade into "the two choice sets differ".
  Only `Receive` is treated as purely offering. `Call`/`Return` is
  deliberately **not** relaxed: which half selects a branching continuation
  depends on what the branch encodes (the caller issues the operation, the
  returning side decides its outcome), so both halves are treated as
  selecting and the effective rule for such a pair is set equality. Escape
  kinds (`Cancel`/`Timeout`/`Fail`) are identical on both roles and reach
  set equality the same way
  (`an_escape_branch_is_required_on_both_sides_because_escapes_are_symmetric`).
  Equality can only refuse more pairs than the subset rule, never fewer, so
  this costs precision on `Call`/`Return` branches and never soundness;
  narrowing it is a stated future refinement, not a hidden defect.
- `ContinuationShapeDivergence` -- one role continues unconditionally
  (`Next::Then`) where the other branches (`Next::Choice`). Rendered as the
  closed tags `"then"`/`"choice"`, never a formatted state list, so the
  refusal is deterministic.
- `ContinuationDivergence` -- the two roles agree on the message and (for a
  branch) on the choice label, but declare **different next states**.
  `choice` is `None` for a `Then` continuation and names the branch
  otherwise.

`OwnershipDivergence` completes the per-transition comparison in the same
pass: payload, capability and ownership are the three things a transition
carries besides its kind, and ownership was the one the check did not
compare. A pair that agrees on the first two while one side alone consumes a
`ResourceToken` is not dual -- one role believes a resource was transferred
and the other does not.

This layer adds no authority and no transport: it compares two declared
`ProtocolSpec` values and returns refusals. Agreeing on a continuation is
not permission to take it.

### Bounded model-checking

`model_check::check_bounded` is a separate, whole-graph BFS over a declared
`ProtocolSpec` (issue #206 step 6, "optionally model-check bounded
traces"), independent of any live `SessionTable`/`Endpoint`. It catches two
defects `ProtocolSpec::validate`'s per-transition checks cannot see, because
each is a property of the graph as a whole rather than of one transition in
isolation:

1. A declared state no transition, anywhere in the spec, ever names as a
   `Next` target -- an orphan `initial` can never actually reach, even
   though the orphan's own outgoing transitions (including its own required
   escape) are individually well-formed
   (`an_orphan_state_no_transition_ever_reaches_is_flagged_by_bounded_model_check_but_not_by_static_validate`).
2. A nonterminal state whose only paths loop forever among other
   nonterminal states without ever reaching a declared terminal state.
   `SpecError::MissingEscape` only requires a state to have *some*
   `Cancel`/`Timeout`/`Fail`-kind outgoing transition; nothing in that
   single-transition check stops the escape's own `next` from pointing at
   another nonterminal state whose own escape points right back, so every
   state on such a cycle can individually satisfy `MissingEscape` while the
   cycle as a whole never lets an endpoint finish
   (`an_escape_cycle_that_never_reaches_terminal_is_flagged_by_bounded_model_check`).

Both applied protocols are asserted to model-check cleanly
(`both_applied_protocols_model_check_cleanly`) with `bound = states.len()`
-- large enough that an ordinary BFS, which never needs to revisit a state,
answers "reachable at all," not merely "reachable within an arbitrarily
small search."

### Determinism

`the_same_message_sequence_produces_byte_identical_results_on_independent_runs`
drives an identical message/payload/capability sequence -- including one
deliberately illegal message mid-sequence -- against two wholly independent
`SessionTable`s and asserts the rendered states, the illegal-message error,
and the final terminal cleanup inventory are byte-identical once the one
caller-chosen difference (the session id, which `SessionTable::open`
requires to be unique) is normalized out of both traces.

## What this reference module implements

- `spec.rs`: `Kind`, `OwnershipMove`, `Next`, `Transition`, `ProtocolSpec`,
  `SpecError` (9 variants), `ProtocolSpec::validate`.
- `capability.rs`: `Grant<C>`, marker capability types, and the module's
  first `compile_fail` doctest.
- `engine.rs`: `Endpoint` (affine, drop-bomb-guarded), `ResourceToken`,
  `ProtocolError` (10 variants), `CheckpointError` (3 variants),
  `Checkpoint`, `TerminalOutcome`, `AdvanceOutcome`, `CleanupHandler`,
  `SessionTable` (`open`, `advance`, `checkpoint`).
- `duality.rs`: `DualityError` (8 variants), `check_duality_one_way`,
  `check_duality`, and the internal `check_continuation` (see
  [Continuation duality](#continuation-duality)).
- `model_check.rs`: `ModelCheckError` (2 variants), `check_bounded` -- a
  whole-graph bounded BFS, distinct from `ProtocolSpec::validate`'s
  per-transition checks (see [Bounded model-checking](#bounded-model-checking)).
- `protocols.rs`: `model_stream_protocol` (a model/tool streaming session:
  `Send`/`Receive`/`Branch`/`Cancel`/`Timeout`/`Fail`),
  `resource_transaction_protocol` (a bounded database transaction:
  `Call`/`Return` with pending tracking, and `ConsumesResource` ownership on
  `commit`), and `project_agent_session_protocol` -- a state-for-state,
  transition-for-transition transcription (cited against exact file:line
  spans in its own doc comment) of `SessionState`/`Session` in
  `src/project_transport/session.rs`, the already-shipped Agent-to-tool
  JSON-RPC transport, rather than a scenario invented for this kernel --
  the applied subsystems issue #206 requires, at this module's Rust
  reference-kernel layer (see [Scope boundary](#scope-boundary)).
  `project_agent_session_protocol` and `database_transaction_protocol` are
  no longer transcriptions only: `project_transport::session` and
  `database_fixture` now hold live `SessionTable`s over them and take every
  lifecycle decision through `admits`/`advance`. `shared.rs` hands each
  subsystem the one validated `&'static ProtocolSpec` it runs on.
- `tests.rs` and `tests/model_check_and_determinism.rs`: every category
  below (split across two files to stay under this repository's
  1500-line-per-file budget).

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
- **Duality: a branch the peer never offers, a divergent continuation, and
  an ownership mismatch.** The `tests::duality_continuation` submodule pairs
  a compatible branching client/server (`a_dual_branching_pair_is_compatible`,
  with `the_branching_fixtures_are_well_formed_and_model_check_cleanly`
  proving the fixtures themselves validate and model-check, so no refusal
  below is an artifact of a malformed declaration) with
  `a_branch_the_peer_never_offers_is_refused_distinctly`,
  `an_offering_peer_with_extra_branches_stays_compatible` (the subset rule's
  other side),
  `an_escape_branch_is_required_on_both_sides_because_escapes_are_symmetric`,
  `a_divergent_then_target_is_refused_distinctly_from_a_missing_counterpart`,
  `a_shared_branch_landing_in_different_states_is_refused_distinctly_from_an_unoffered_branch`,
  `a_continuation_shape_mismatch_is_refused_distinctly_from_a_target_divergence`,
  and
  `ownership_divergence_is_refused_distinctly_from_payload_and_capability_divergence`.
  Each asserts its exact variant *and* asserts every confusable neighbour is
  absent from the rendered errors, and
  `continuation_duality_errors_are_deterministic_across_runs` compares the
  exact rendered error vector across two independent runs while asserting
  the fixture really produces both refusal families.
- **The affine layer's own drop bomb.**
  `abandoned_nonterminal_endpoint_panics_on_drop` proves dropping a live,
  non-terminal `Endpoint` panics (via `catch_unwind`), with a message
  naming the abandonment invariant; `cancelled_endpoint_does_not_panic_on_drop`
  is the negative control, proving a *terminal* endpoint's drop is
  harmless. The module's second `compile_fail` doctest proves the stronger,
  earlier-catching half: presenting the same live `Endpoint` binding to a
  second operation does not compile at all.
- **Static declaration defects are each specific.** `spec_validation::*`
  (9 tests) exercises every `SpecError` variant, including two pairs of
  tests that specifically prove two rules are *not* conflated
  (`DeadEnd` vs. `MissingEscape`) and confirm both applied example
  protocols themselves validate cleanly
  (`both_applied_protocols_validate`) -- a direct regression against
  shipping a broken example.
- **Bounded model-checking catches whole-graph defects static validation
  cannot see, and both applied protocols pass it.** See
  [Bounded model-checking](#bounded-model-checking) above.
- **Determinism.** See [Determinism](#determinism) above.

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
  identity, not mere presence, per transition pair, and likewise compares
  `OwnershipMove`. Duality is also unsound when the two roles agree on a
  message but not on where it leaves them -- see
  [Continuation duality](#continuation-duality).
- **Timeout may leave the remote side in an uncertain state.** Addressed by
  routing every declared `Timeout` transition in all three applied
  protocols (including `project_agent_session_protocol`'s `apply_timeout`)
  to a distinct `Uncertain` terminal, never conflated with a clean
  `Cancel`.

## Declared session protocols (issue #297)

### Syntax

```text
@id("<persistent-id>")
session protocol "<versioned-name>" {
    states { S, ... }
    initial S;
    terminal S cleanup { op, ... }                         // zero or more
    on S label: kind Payload [requires capability cap.name]
        [consumes resource] [via "<function-id>"] -> S | choice { label: S, ... };
}
```

`kind` is one of `send`, `receive`, `call`, `return`, `cancel`, `timeout`,
`fail`, mirroring `spec::Kind`. The clause order is fixed and closed, so the
canonical formatter (`src/format/session_protocol.rs`) has exactly one
spelling; a declaration is one comment-placement leaf, like a static
`protocol`. Parsing lives in `src/parser/session_protocol.rs`; the AST in
`src/ast/session_protocol.rs`; the sealed cache codec carries it
(`src/cache_codec/carriers.rs`).

### Comments

A declaration is one comment-placement leaf, like a static `protocol`. A
comment before its `@id` leads it; a comment anywhere inside its body is
hoisted, in source order, above its `@id`. This is deterministic and a fixed
point (`comments_inside_the_declaration_body_hoist_above_it_deterministically`),
but in-body comment position is not preserved.

### Checking

`crate::session_protocol::source::check` runs inside `verify::verify`
(and therefore inside `hir::resolve`). It lowers the declaration onto the
kernel's own `ProtocolSpec` -- names map through a bounded, process-lifetime
static symbol pool, so the kernel is reused unchanged and no compile leaks --
and reports:

| Code | Meaning |
| --- | --- |
| `SPX-K101` | Duplicate protocol name or identity, an identity colliding with another declaration, or a repeated state, terminal, or cleanup operation. |
| `SPX-K102` | `ProtocolSpec::validate` refused the declared graph (unknown state, duplicate label, one-branch or duplicate-branch choice, terminal with an outgoing transition, dead end, missing `cancel`/`timeout`/`fail` escape, terminal without cleanup). |
| `SPX-K103` | `model_check::check_bounded` (bound = state count) refused it: an unreachable state, or no bounded path to a terminal. |
| `SPX-K104` | A `via` names no ordinary monomorphic function of this module; also the fail-closed HIR recheck when a projection finds a `via` the checked HIR does not retain. |
| `SPX-K105` | A `requires capability` names an effect its `via` function does not declare in `uses { ... }`. |
| `SPX-K106` | Capacity: more than 64 declarations per module, 64 states/terminals/cleanup operations, 256 transitions, or 64 choice branches (parser); or, for a declaration built outside the parser, more distinct names than the lowering pool admits (verifier, never a panic). |

### Legal order is not authority

A transition's `requires capability` is ordering metadata. With a `via`, it
must already be one of that function's declared effects, and the ordinary
effect rules (`SPX-E101`/`SPX-E102`/`SPX-E103`) stay authoritative: a caller
of a `via` function still needs its own `uses` set, whatever the protocol
says (`a_protocol_capability_never_satisfies_the_ordinary_effect_check`).
Without a `via`, the capability is realized outside checked source (for the
two canonical declarations, by the Rust subsystems) and every projection
labels it `"capability_binding":"unattributed"`. Nothing in a declaration
adds an effect, a capability, or a resource token to anything, and every
projected fact carries `"authority":"none"`.

### Projections

- **Per-source graph.** `graph::to_json` selects `semaprax.graph.v48` only
  when the program declares at least one session protocol. The document is
  the program's otherwise-selected graph with the v48 header and one trailing
  `session_protocols` object: `base_schema` (the schema it extends),
  `authority: "none"`, and one fact per declaration (stable id, name, span,
  states, initial, terminals with ordered cleanup, transitions with payload,
  capability, capability binding, ownership, `via`, continuation and span,
  plus `static_validation` and `bounded_reachability`). Every `via` is first
  bound against the checked HIR of the same program. A program without a
  declaration keeps its existing schema and bytes; `to_legacy_json` refuses a
  declaring program.
- **Context.** With `--filters session_protocol`, the envelope's
  `session_protocol_kernel` object gains a `declared` array of the same facts
  when, and only when, the queried program declares a protocol.
- **Architecture.** `ArchitectureClaim::protocol_realizers_bound(id, protocol)`
  (see [Architecture Claims v1](ARCHITECTURE-CLAIMS-V1.md)). It attests only
  that every `via` target is a checked function node of the evaluated
  revision's call graph. It says nothing about message order or call order.
- **Assurance.** One `session_protocol` obligation per declaration
  (see [Assurance Manifest v1](ASSURANCE-MANIFEST-V1.md)), `compiler_proved`
  for static validation only; never `model_checked`.
- **Workspace Semantic Graph (R21, issue #297 follow-on).**
  `workspace_graph::snapshot` selects `semaprax.workspace-semantic-graph.v2`
  only when at least one managed module declares a session protocol; a
  protocol-free workspace keeps `.v1` and byte-identical output, mirroring the
  per-source graph's own v48 gating. A declaring workspace gains one trailing
  `session_protocols` object: `base_schema` (the `.v1` schema it extends),
  `authority: "none"`, and one fact per declaration, each the same canonical
  fact the per-source graph emits plus `module` and `path` naming the exact
  managed source that owns it. Every `via` is bound against the checked HIR
  built from that same module before it is recorded, in the same per-module
  pass that already binds cross-file call edges
  (`src/workspace_graph/session_protocol_decl.rs`).
- **Package Semantic Graph (R21, issue #297 follow-on).**
  `PackageSemanticGraph::derive` selects `semaprax.package-semantic-graph.v2`
  only when at least one selected package declares a session protocol; a
  protocol-free package graph keeps `.v1` and byte-identical output, the
  identical gating discipline the per-source graph and the Workspace Semantic
  Graph already use. A declaring graph gains one trailing `session_protocols`
  object: `base_schema`, `authority: "none"`, and one fact per declaration,
  each the same canonical fact plus `package` and `version` naming the
  selected coordinate that owns it. The package-source build reuses the
  Workspace Semantic Graph's own per-module pass
  (`workspace_graph::build_package_scalar_sources` calling
  `retain_workspace_module`), so no protocol fact here is derived by a second,
  independent code path; see [Package Semantic Graph
  v1](PACKAGE-SEMANTIC-GRAPH-V1.md#declared-session-protocols-issue-297-follow-on).
- **Documentation (R21).** `semaprax doc` renders a `session_protocol` entry
  per declaration: the canonical declaration text as its signature (states,
  initial, terminals with cleanup, and every transition, in source order),
  and `States`/`Initial`/`Terminals`/`Authority` facts. Transitions have no
  persistent identity of their own in the AST, so unlike a static `protocol`'s
  methods they are not separately documented members.
- **Declaration Query (R21).** `semaprax query --kind session_protocol`
  selects the same `session_protocol` entries `semaprax doc` renders, for
  both a single checked module and an authenticated Project, since both reuse
  `crate::doc::document`.
- **Help shape catalog and agent quick reference (R21, issue #297
  follow-on).** `src/doc.rs`'s own `SECTIONS` (the order `semaprax doc`
  renders) already carried `("session_protocol", "Session protocols")`;
  `tests/projections/shapes_catalog.rs`'s separate `SECTIONS` (which
  `semaprax help shapes` and `docs/LANGUAGE-SHAPES-CATALOG.{md,json}` are
  generated from) now matches it. `examples/session_protocol.spx` is the
  committed, compiler-verified example that gives the catalog its one
  `session_protocol` entry. [The agent quick reference](AGENT-QUICK-REFERENCE.md#session-protocols)
  gains a `## Session protocols` section: the shape's grammar, its
  checked-and-erased/`via`/`requires capability` rules, and one compiler-
  checked example module (`tests/documentation.rs::agent_quick_reference`
  parses, verifies with zero diagnostics, and byte-compares it against
  `format::canonical`, exactly like every other unmarked block on the page).
- **Semantic-workspace rename, change, impact, and review (R21, issue #297
  follow-on).** A `via` clause binds by persistent `@id`, never by display
  name, so `src/semantic_workspace_operations.rs`'s rename derivation (which
  changes only a declaration's display name, addressed by its immutable
  `@id`) can never break a `via` binding by construction -- there is no
  operation in this codebase that reassigns a declaration's `@id`. What a
  rename or a general change *can* do is remove or alter the realizing
  function outright; every candidate source set either derivation builds is
  replayed through the same per-module pass that already binds `via` targets
  against checked HIR (`retain_workspace_module` /
  `session_protocol_decl::declaration_facts`), so a candidate that would
  orphan a `via` fails closed with the ordinary stable diagnostic
  (`SPX-K104`) rather than being silently admitted; pinned at the exact entry
  points rename and change use
  (`workspace_graph::build_owned_retaining_sources_for_operations` and
  `_for_change`) by
  `a_via_bound_functions_display_rename_is_admitted_by_operations_and_change_candidate_builds`
  and
  `removing_a_via_bound_function_is_refused_with_a_stable_diagnostic_by_operations_and_change_candidate_builds`
  in `src/workspace_graph/session_protocol_decl.rs`. Impact and review
  (`src/impact.rs`, wrapped unchanged by `src/review.rs`) gain a
  `session_protocols_affected` fact array: one entry per session protocol in
  the previewed module whose `via` names a declaration id the patch directly
  changes (a rename target or the owner of a changed call instance),
  restricted to the affected transitions, each carrying `protocol`,
  `protocol_name`, `authority: "none"`, and the matched `via` edges. Omitted
  entirely (not even `[]`) for a module with no session protocol or none
  affected, so every existing pinned Impact/Review report stays
  byte-identical; a session protocol's `via` is not a real call edge, so this
  reporting is computed independently of, and never feeds, the reverse-call
  closure (`reverse_closure`/`affected_functions`) that already exists for
  real callers.

### Bundled dependency pruning

The Workspace Semantic Graph's bundled-dependency pruning
(`src/workspace_graph/dependency_pruning.rs`) treats every `via` target as a
root, because `via` names its realizer by persistent id rather than display
name (`a_session_protocol_via_target_is_retained`).

### Non-claims

A session protocol declaration is projected by the per-source graph (v48),
the Workspace Semantic Graph (v2, R21), the Package Semantic Graph (v2, R21),
`context` (`--filters session_protocol`), Architecture Claims
(`protocol_realizers_bound`, Rust API only), single-file and Project Assurance
Manifest v1, `semaprax doc`, and `semaprax query --kind session_protocol`. The
following omit declarations entirely, and nothing here claims otherwise:

- the VS Code grammar (`editors/`);
- a CLI flag for `protocol_realizers_bound`;
- any runtime enforcement from a declaration: it is erased, and the two live
  lifecycles are enforced by the Rust kernel, not by their `.spx`
  declarations. A function's `follows` clause (below) is checked and erased
  the same way, and grants no runtime authority either;
- ordering attestation of any kind by `protocol_realizers_bound`;
- a `follows`/typestate fact in Architecture Claims or the VS Code grammar --
  see [Endpoint typestate non-claims](#endpoint-typestate-non-claims) for
  which of the eight outputs above do carry one;
- typestate checking of an endpoint *value*'s flow through locals, fields, or
  return positions (use-after-close, ownership) -- see [Endpoint typestate
  non-claims](#endpoint-typestate-non-claims). What issue #297 follow-on (R21)
  *does* check -- the call order of a function's own direct calls to a
  protocol's `via`-bound functions -- is [Endpoint typestate
  checking](#endpoint-typestate-checking-issue-297-follow-on-r21), below.

### The two canonical declarations and the drift gate

`src/session_protocol/tests/fixtures/project_agent_session.spx` and
`database_transaction.spx` declare `project-agent-session-v1` and
`database-transaction-v1` in source. They live under the session-protocol
tests rather than `std/` so no standard-library catalog changes.
`canonical_declarations_match_the_kernel_specs_field_for_field` parses and
verifies each one and compares it with `protocols::project_agent_session_protocol()`
and `protocols::database_transaction_protocol()` (the specs the live
subsystems run) through `source::drift_from_spec`: name, state set, initial,
terminal set, per-terminal cleanup inventory in canonical order, and every
transition in order including payload, capability, ownership and
continuation. `drift_gate_refuses_a_mutated_declaration_and_a_mutated_kernel_spec`
is the negative control. The runtime subsystems keep using `shared.rs`
unchanged; neither was migrated a second time.

### `std.db.transaction` is a different state machine

`std/db`'s `std.db.transaction.*` functions are **not** bound to
`database-transaction-v1`, and must not be. They model a *connection* that is
reused across transactions: `next_on_begin(COMMITTED)` returns `OPEN`, and
`next_on_connection_lost(NONE)` leaves `NONE` unchanged. The kernel spec
models one transaction's lifecycle: `Committed`, `RolledBack` and `Failed`
are terminal, and a `connection_lost` outside `Open` is refused. Binding one
to the other would project a false fact, so the two stay separate and this
difference is recorded here instead.

## Endpoint typestate checking (issue #297 follow-on, R21)

Declaring a protocol (above) is checked and erased, but until this slice
nothing checked that a program's *use* of a declared protocol's endpoints
follows the legal order it declares. This section is the design for the
smallest admissible static check that is sound for the admitted subset, and
records exactly what it does and does not check.

### Opt-in: `follows session protocol "<protocol-id>"`

A function opts in with a new, optional clause between `yields` and the
`requires`/`ensures` contracts, admitted only there so the canonical
formatter keeps one fixed spelling:

```text
fn name(params) -> T
    uses { ... }
    yields Request -> Response
    follows session protocol "<protocol-id>"
    requires ...
    ensures ...
{ body }
```

`<protocol-id>` is the persistent `@id` of a `session protocol` declaration in
the *same module* -- never its versioned display name, and never a
cross-module reference -- mirroring how a transition's own `via` names its
realizing function by persistent id rather than display name
([`crate::ast::SessionProtocolFollowsClause`]). A function that does not
write `follows` is entirely unaffected, byte for byte: parsing, canonical
formatting, verification, the semantic graph, and both backends produce
identical output whether or not this feature exists for that function.
Naming a protocol declared in another module, or one that does not exist, is
refused (`SPX-K107`); this keeps the check local to one already-checked
`Program`, exactly like the declaration's own `via` binding.

### What is checked, and how

Starting from the protocol's declared `initial` state (an explicit,
separately declared entry state is a natural future extension this slice does
not add -- see [Non-claims](#endpoint-typestate-non-claims)), the checker
walks the opted-in function's own body once, left to right -- the same
evaluation order [RFC 0001](RFC-0001.md) requires everywhere else -- tracking
the *set* of protocol states the function could be in at each point. A set,
not one state, because `if`/`else` genuinely can leave the function in either
of two different states depending on a runtime condition this static check
never evaluates: both branches are walked from the same incoming state(s),
and their resulting states are unioned for whatever follows the `if`. A
direct call to one of the protocol's `via`-bound functions advances every
state currently in the set along that function's realized transition; a call
the current state does not admit is refused (`SPX-K108`). Every path through
the function's body must end in a declared terminal state (`SPX-K108`) --
exactly "every exit path reaches a terminal state or a declared escape",
since `Cancel`/`Timeout`/`Fail` transitions already land on an ordinary
terminal state in this kernel, so no separate "escape" case is needed. The
state set can never grow without bound across arbitrarily nested `if`/`else`:
the number of distinct protocol states is already bounded at 64 (`SPX-K106`).

### What is refused outright, rather than approximated

This is a small, *sound* subset of possible programs, not an unsound
approximation of a larger one: anything the walk cannot resolve statically
and precisely is refused with a stable diagnostic (`SPX-K109`) rather than
silently admitted or silently skipped over:

- **Loops** (`while`, `for`, `for own`) whose condition/source or body
  reaches a `via`-bound call: the number of iterations is not known
  statically, so the call could execute any number of times, including zero
  or unboundedly many.
- **Recursion**: a followed function that calls itself anywhere in its own
  body, `via`-bound or not. A straight-line/if-else walk has no model for a
  re-entrant call, and the recursive invocation could revisit `via` calls an
  unbounded number of times, the same hazard as a loop.
- **Closures**: a `via`-bound call written inside a closure literal may run
  zero, one, or many times, at a point this walk cannot order against the
  rest of the function -- the same "escaping endpoint" hazard the issue
  names, generalized to any deferred/uncertain-arity invocation rather than a
  single "endpoint value" representation this slice does not add.
- **Indirect or ambiguous calls**: a call whose callee name is shadowed
  *anywhere* in the function by a parameter or `let` binding is never
  resolved as a direct call to a `via`-bound function, even where the
  shadowing does not lexically reach this exact call site (the check is
  deliberately whole-function and scope-insensitive here, which can only
  over-refuse, never under-refuse); likewise a `via`-bound function that
  itself realizes more than one transition from the same current state.
- **A `via` transition with a branching (`choice`) continuation**: which
  branch a call actually took is a runtime fact (for example, the remote
  peer's reply) this static check cannot observe. Admitting the union of
  every declared branch's target state would be *unsound*, not merely
  imprecise: a later call that is illegal on one branch but legal on another
  would then wrongly pass. Only a `via` transition with a `Then` continuation
  is checked; one with a `Choice` continuation is refused wherever a checked
  function actually calls it.
- Every other expression shape the walk does not specifically know how to
  step through (`match`, method calls, record construction/update, `project`,
  `try`, `yield`) is treated as an opaque, state-preserving expression *only
  when it contains no reachable `via`-bound call at all*; otherwise it is
  refused the same way, rather than silently skipped past a call it cannot
  order.

### Legal order is still not authority

Exactly like a declaration itself: passing this check proves only that the
checked function's own call sequence traces a legal path through the declared
graph. It adds no effect, capability, or resource authority of its own; the
ordinary effect checks (`SPX-E101`/`SPX-E102`/`SPX-E103`) and `SPX-K105`'s
"ordering metadata cannot mint a capability" rule are unchanged and
unaffected by whether a function opts in.

### Erasure

A `follows` clause has no HIR node, no native lowering, and no Wasm lowering.
`hir::resolve` never reads `Function::follows`, so it cannot influence
checked HIR, and both backends produce byte-identical output whether or not a
function opts in --
`session_protocol::typestate::tests::a_follows_clause_is_erased_from_native_and_wasm_output`
asserts this directly by comparing `codegen::emit_c` and `wasm::emit_module`
between two otherwise-identical programs that differ only in whether the
one function's `follows` clause is present.

### Projections

Each fact below is the function `@id` bound to the protocol `@id` it
follows, plus the fixed result `"typestate_checked"` -- exactly as
authority-free as passing typestate checking itself
(`crate::session_protocol::source::follows_json`). A program with no
`follows` clause is unaffected, byte for byte, in every one of these
projections, mirroring the declaration's own v-bump-only-for-a-declaring-
program discipline.

- **Per-source graph.** `graph::to_json` selects `semaprax.graph.v49` only
  when the program has at least one `follows` clause -- always a strict
  additional selection over `semaprax.graph.v48`, since a `follows` clause
  names a protocol declared in the same module (`SPX-K107` refuses anything
  else). The document is v48's own output (or the program's ordinary schema,
  for a protocol-free program -- unreachable here since a `follows` clause
  requires a declaration) with the v49 header and one trailing
  `session_protocol_follows` object: `base_schema` (the schema it extends),
  `authority: "none"`, and one binding fact per opted-in function (function,
  protocol, result, span). A program with no `follows` clause keeps its v48
  (or ordinary) schema and bytes (`src/graph/session_protocol_follows.rs`).
- **Context.** With `--filters session_protocol`, the envelope's
  `session_protocol_kernel` object gains a `follows` array of the same facts
  when, and only when, the queried program has at least one `follows`
  clause.
- **Workspace Semantic Graph.** `workspace_graph::snapshot` selects
  `semaprax.workspace-semantic-graph.v3` only when at least one managed
  module has a `follows` clause; a workspace with none keeps `.v2` (or `.v1`)
  and byte-identical output. A selecting workspace gains one trailing
  `session_protocol_follows` object: `base_schema` (the `.v2` schema it
  extends), `authority: "none"`, and one binding fact per opted-in function,
  each carrying `module` and `path` naming the exact managed source that
  owns it (`src/workspace_graph/session_protocol_decl.rs`).
- **Package Semantic Graph.** `PackageSemanticGraph::derive` selects
  `semaprax.package-semantic-graph.v3` only when at least one selected
  package has a `follows` clause; a package graph with none keeps `.v2` (or
  `.v1`) and byte-identical output. A selecting graph gains one trailing
  `session_protocol_follows` object: `base_schema`, `authority: "none"`, and
  one binding fact per opted-in function, each carrying `package` and
  `version` naming the selected coordinate that owns it. The `summary`
  projection copies this key generically, exactly like every other
  top-level fact (`src/package_semantic_graph.rs`).
- **Assurance.** One further `session_protocol` obligation per opted-in
  function (locator `protocol:call-order-typestate`, distinct from a
  declaration's own `protocol:static-validation`), `compiler_proved` for the
  static call-order check only (`SPX-K107`..`SPX-K109`); never
  `model_checked`, and explicitly not runtime authority
  (`src/assurance_manifest/session_protocol.rs::follows_obligations`).
- **`semaprax doc` and `semaprax query` (R21 follow-on's own extension).**
  `semaprax doc` renders the binding on both sides of it: a following
  function's entry gains `Follows` (the protocol `@id`), `Typestate`
  (`"typestate_checked"`), and `Authority` (`"none"`) facts, and the named
  protocol's own entry gains a `Following functions` fact (every opted-in
  function `@id`, in source order) -- both read from the one canonical fact
  every other projection shares (`follows_json`/`follows_facts_json`) rather
  than re-derived (`src/doc.rs`). Only a top-level function ever gains these
  facts: `session_protocol::source::check`/`bind_follows` and
  `session_protocol::typestate::check` only ever validate and erasure-check a
  `follows` clause on `Program::functions`, never on a class method, so a
  method's clause (admitted by the shared function grammar but never checked
  or bound) has no canonical fact to document. `semaprax query` needs no
  separate code: `query::run` and `query::run_project` already render every
  entry directly from `crate::doc::document`, so the new facts are queryable
  exactly the way existing session-protocol facts already are (`States`,
  `Initial`, `Terminals`, `Authority`, or a method's `Owner`) -- present on
  the matched `Entry`'s `facts` once selected by the ordinary
  `--kind`/`--name`/`--id` filters, with no dedicated `--follows` filter or
  JSON field of its own. A program with no `follows` clause is unaffected,
  byte for byte (`src/doc/tests.rs` and `tests/projections/doc_projection.rs`
  keep their exact pre-existing fact sets and the pinned
  `EFFECTS_MARKDOWN`/`declared.spx` goldens).
- **Not projected.** Architecture Claims (`protocol_realizers_bound`
  attests only `via` targets, an orthogonal declaration-side fact) is
  unaffected by this follow-on.

### Diagnostics (`SPX-K107`..`SPX-K109`)

| Code | Meaning |
| --- | --- |
| `SPX-K107` | A `follows` clause names no `session protocol` declaration in this module. |
| `SPX-K108` | A call to a `via`-bound function is not legal from the function's current state at that point, or some path through the function ends in a non-terminal state. |
| `SPX-K109` | The function contains a construct this subset does not admit reaching a `via`-bound call through: a loop, direct recursion, a closure, an indirect/ambiguous call, a `via` transition with a branching `choice` continuation, or any other unsupported expression shape. |

### Endpoint typestate non-claims

- **Projections (R21 follow-on).** The function `@id` bound to the protocol
  `@id` it follows, plus the fixed result `"typestate_checked"`, is projected
  into the per-source graph (`semaprax.graph.v49`), `context` (`--filters
  session_protocol`), the Workspace and Package Semantic Graphs (`.v3`), the
  Assurance Manifest, `semaprax doc`, and `semaprax query` (see
  [Projections](#projections) above). Architecture Claims'
  `protocol_realizers_bound` remains an orthogonal, declaration-side (`via`)
  fact this follow-on does not touch, and the VS Code grammar does not parse
  `follows` distinctly. Every projection keeps a program with no `follows`
  clause byte-for-byte unaffected, exactly like `semaprax.graph.v48` already
  does for the declaration itself.
- **No explicit entry state.** Every followed function starts from the
  protocol's declared `initial` state; a clause selecting a different,
  explicitly declared entry state is a natural extension this slice does not
  add.
- **No cross-module protocol reference**, mirroring `via`'s own restriction to
  ordinary functions of the same module.
- **No interprocedural analysis.** Only calls written directly in the
  checked function's own body are considered; a call to an ordinary
  (non-`via`) function that itself calls a `via`-bound function elsewhere is
  invisible to this check, exactly as issue #297's own request describes
  ("within one function body").
- **No compiler-checked ownership/use-after-close analysis of an endpoint
  *value***. This checks the *order of calls by name*, never the flow of an
  endpoint value through locals, fields, or return positions. Nothing here
  represents an `Endpoint` in checked HIR, so "does this local still denote a
  live, non-terminal endpoint" is not a question this check can ask or
  answer; only `session_protocol::engine::Endpoint`'s own Rust-level affine
  drop bomb (see above) answers that question, and only for the two Rust
  reference kernels, not for `.spx` source.

### Gate (endpoint typestate checking)

`cargo test --locked -p semaprax --lib session_protocol::typestate` covers
the parser, canonical formatter, `SPX-K107`..`SPX-K109`, canonical
round-trip, cache-codec round-trip, and native/Wasm erasure for `follows`.
It is exercised by the existing `cargo test --locked -p semaprax --lib
session_protocol` selector alongside every other test in this module.

The [Projections](#projections) above (R21) are covered by: `cargo test
--locked -p semaprax --lib graph::session_protocol_follows` (graph v49 and
`context`'s `follows` facet, including the byte-identical regression against
the declaration-only golden and the dangling-binding negative control);
`cargo test --locked -p semaprax --lib workspace_graph::session_protocol_decl`
(the same module as the declaration's own v2 gating, extended with the v3
`follows` gating, module-bound facts, and the dangling-binding negative
control); `cargo test --locked -p semaprax --test offline_package
semantic_graph::` (Package Semantic Graph's v3 gating and package-bound
facts); and `cargo test --locked -p semaprax --lib
assurance_manifest::session_protocol` (the `protocol:call-order-typestate`
obligation, its coexistence with the declaration's own obligation in one
generated envelope, and the dangling-binding negative control); `cargo test
--locked -p semaprax --lib doc::` (the `Follows`/`Typestate`/`Authority`
facts on a following function, the `Following functions` fact on its
protocol, their Markdown and JSON rendering, and the byte-identical
regression against the declaration-only fixture); and `cargo test --locked
-p semaprax --test projections doc_projection:: query_projection::` (the
pinned `EFFECTS_MARKDOWN` golden staying unchanged, and the same facts
surfacing through `semaprax query`'s ordinary `--kind`/`--name` filters with
no dedicated query code of their own).

## Scope boundary

Explicitly **not** done in this slice, and why:

- **Superseded by issue #297 for declarations, and by issue #297 follow-on
  (R21) for call-order typestate checking; still open for endpoint values.**
  A `session protocol` declaration now exists end to end (see [Declared
  session protocols](#declared-session-protocols-issue-297)), and a function
  may now opt in to a static call-order check over its own body (see
  [Endpoint typestate
  checking](#endpoint-typestate-checking-issue-297-follow-on-r21)). What
  remains open is typestate checking of an `.spx` endpoint *value*'s flow
  through locals, fields, and return positions (ownership, use-after-close),
  which -- unlike the call-order check, which is checked and erased with no
  HIR or backend representation of its own -- would carry runtime meaning if
  it modeled a real endpoint value, and so needs parser, HIR, verifier and
  both backends together. The original rationale follows. The repository's
  change protocol requires parser, canonical formatter, resolver/HIR,
  verifier, semantic graph, native backend, and Wasm backend to move
  together once syntax carries runtime meaning; landing a half-wired parser
  rule with no checked HIR consumer would violate that protocol rather than
  satisfy it. This is the same scope boundary `resumable_effects` and
  `live_invocation` already document for their own boundaries, and this
  document is the design a follow-up parser/HIR/graph tranche implements
  against. (A later
  session did add a `context` projection of this module's own fixed catalog
  -- `session_protocol_kernel`, declaration-independent reference data, not
  a projection of any `.spx`-declared protocol, since none exists -- plus a
  standalone `graph::session_protocol_kernel_json()` Rust API carrying the
  same catalog's full transition detail with no CLI verb of its own; see
  `src/graph/session_protocol_facet.rs` and the "Protocol facts" row of
  [Acceptance criteria](#acceptance-criteria-met-here-versus-open) for that
  decision and for the `architecture_claims`/`assurance_manifest`
  evaluation the same row records.)
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
- **No native/Wasm lowering, no *live* migration of an existing subsystem
  onto this mechanism.** `protocols::project_agent_session_protocol` is a
  transcription of the real `project_transport::session` state machine's
  states and topology, evidenced against exact file:line spans -- a
  meaningfully harder bar than the two invented fixtures, since a
  hand-written transcription can misreport the source it claims to model
  in a way a fixture designed for the kernel cannot. It is **not** the
  live migration: `project_transport::session`'s fields and dispatch
  methods stay private and it still performs its own hand-rolled checks,
  never calling into `SessionTable`. Rewiring that stdio transport's
  request loop through this kernel is downstream of a maintainer decision
  plus a live, heavily-tested transport's own regression surface, not a
  single bounded change here.
- **`model_check::check_bounded` explores the declared graph only, not a
  live `SessionTable`'s runtime behavior.** It is a design-time property
  check over `ProtocolSpec` (are all states reachable, can every reachable
  nonterminal state reach a terminal one within the bound), not a
  bounded-trace *runtime* fuzzer that also exercises `advance`'s
  payload/capability/ownership checks together with the graph shape; the
  runtime layer's own defects are covered by the `ProtocolError` tests
  above instead.

## Acceptance criteria: met here versus open

| Criterion (from issue #206) | Status |
| --- | --- |
| At least two real interaction lifecycles are checked by the general protocol type system | **Met, at this module's Rust layer.** Two real subsystems now run on the kernel rather than beside it. (1) `project_transport::session::Session` no longer has a `SessionState` field: it holds a `lifecycle::ProjectSessionLifecycle` owning a `SessionTable` over `project_agent_session_protocol`, every lifecycle gate calls `SessionTable::admits`, every state change is a `SessionTable::advance`, and the state reported on the wire is rendered back out of the live `Endpoint`. (2) `database_fixture::DatabaseFixture`'s transaction lifecycle is the same shape over `database_transaction_protocol`: `begin`/`commit`/`rollback`/`connection_lost` reach their outcome only through `advance`, the four `TransactionState::next_on_*` helpers that re-derived the rule by hand are deleted, and the terminal cleanup inventories really perform the snapshot work. `model_stream_protocol` and `resource_transaction_protocol` remain invented fixtures and are not counted toward this row. This is local, re-runnable Rust evidence; it is still not `.spx` syntax, and the language-level half stays open (see [Scope boundary](#scope-boundary)). |
| Invalid order is rejected before runtime | **Static declaration defects**: at spec-validation time (`SpecError`, before any session opens). **Message-order defects**: at the engine's own runtime check (`IllegalTransition` etc.) -- not before compilation, since the protocol is declared data in this slice, not `.spx` source the compiler itself parses. The one case genuinely caught by `rustc` at compile time is presenting an already-consumed `Endpoint` binding a second time, and presenting a `Grant` for the wrong capability marker type. |
| Ownership and authority are coupled to protocol state | **Met at the reference-kernel level**: `required_capability` and `OwnershipMove` are per-transition fields the engine checks alongside state/order, and are proven independently failing from state/order correctness (`missing_authority_is_refused_even_in_correct_order`). |
| Failure/cancellation/uncertainty remain explicit | **Met**: `Cancel`/`Timeout`/`Fail` are ordinary declared transitions with their own cleanup; `Timeout` is routed to a distinct `Uncertain` terminal in both applied protocols. |
| Protocol facts appear in context, graph, architecture, and assurance outputs | **Met for declared protocols (issue #297), locally evidenced.** A `.spx` `session protocol` declaration is bound to its `@id`, source span, and the checked HIR functions its `via` clauses name, and appears in these four outputs, and only these: `context` (`session_protocol_kernel.declared`, alongside the unchanged built-in catalog), the per-source graph (`semaprax.graph.v48`, selected only for a declaring program), Architecture Claims (`protocol_realizers_bound`, bound to the `project_revision` digest; it attests only that every `via` target is a checked call-graph node, not ordering), and Assurance Manifest v1 (`session_protocol` obligations, `compiler_proved` for static validation only; the `no_model_checker_invoked` nonclaim stands). The two real lifecycles have canonical `.spx` declarations gated field-for-field against the kernel specs they run on. Ordering metadata grants nothing (`SPX-K105`, and the ordinary effect checks still apply). An earlier session had evaluated `architecture_claims` and `assurance_manifest` as not applicable because no declaration existed to bind; that reasoning no longer holds and the narrowing was never accepted. See [Declared session protocols](#declared-session-protocols-issue-297). Not claimed: see [Non-claims](#non-claims). |
| Applied subsystem regressions and bounded model-checking integration (required tests/evidence) | **Bounded model-checking: met**, at the graph-shape level -- `model_check::check_bounded` (see [Bounded model-checking](#bounded-model-checking)), exercised against all three applied protocols including the real-subsystem transcription. **Applied subsystem regression: met.** `tests/applied_project_session.rs` still regresses the declared topology directly, and both applied subsystems' own existing suites (`src/project_transport/session/rename/tests.rs`, `tests/agent_transport*`, `database_fixture`'s transaction tests) now exercise the kernel on every run, because those subsystems have no other state machine left to exercise: breaking a `SessionTable` call in either one turns those suites red. `tests/admits.rs` additionally pins `SessionTable::admits` to `advance` across every state/label pair of both applied specs, so the read-only gate a migrated subsystem depends on cannot drift from the operation that commits. |

## Gate

`cargo test --locked -p semaprax --lib session_protocol` covers this module's
unit tests (including `tests::source_declarations`, the #297 parser,
formatter, `SPX-K1xx`, erasure, HIR-binding, cache-codec and drift-gate
tests), `graph::session_protocol_decl` and `graph::session_protocol_facet`
(graph v48 and `context`), and `assurance_manifest::session_protocol`.
`cargo test --locked -p semaprax --test workspace architecture_claims::`
covers `protocol_realizers_bound` over real compiled revisions.
`cargo test --locked -p semaprax --lib workspace_graph::session_protocol_decl`
covers the Workspace Semantic Graph's v2 gating, module-bound facts,
determinism, and the `via`-binding failure (R21); `cargo test --locked -p
semaprax --test projections doc_projection::` and `query_projection::` cover
`semaprax doc`'s `session_protocol` entry and `semaprax query --kind
session_protocol` (R21).
`cargo test --locked -p semaprax --doc session_protocol` (3 `compile_fail`
doctests: grant-for-wrong-capability, double-use of a consumed `Endpoint`,
and the capability module's own copy of the grant-for-wrong-capability
example) remains the doctest selector. The two live lifecycles are regressed
by `cargo test --locked -p semaprax --lib database_fixture::` and
`--lib project_transport::session`.
