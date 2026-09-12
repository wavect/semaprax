# Agent Lifecycle Typed Carrier v1

Audience: runtime integrators wiring [Agent Interaction Schema v1](AGENT-INTERACTION-SCHEMA-V1.md)
values into checked Agent lifecycle stages and effect operations, and
compiler contributors maintaining the carrier.

Status: **LOCAL** bounded implementation with an executable reference and
focused regression corpus, implemented in `src/agent_lifecycle_typed_carrier/`.

This is issue #110 ("Carry rich typed Proposal and effect values through the
checked Agent lifecycle"). Its prerequisite, issue #109 (Agent Interaction
Schema v1: the derived rich schema and its strict decoder), is accepted and
unmodified by this work — this module reads it and the real checked
interpreter's retained-call seam, and adds nothing to either.

## Outcome

A value that enters as a schema-checked
[`DecodedInteractionValue`](AGENT-INTERACTION-SCHEMA-V1.md#the-decoded-value-document)
stays typed at every boundary this carrier drives, rather than degrading to
opaque bytes, an untyped blob, or a caller-authored selector trick. It is
carried through the real checked interpreter (`interpreter::retained_call`),
not a second home-grown execution model, and its ownership settles
deterministically on every tested path, including every required failure
edge.

## Why projection into `RetainedValue`, not a new value representation

The checked interpreter's real owned-call seam
(`crate::interpreter::retained_call`) already carries recursive, nominally
identified `Record`/`Variant` values with real ownership and cleanup
semantics — a strictly richer vocabulary than the flat, single-level scalar
projection `agent_lifecycle::iterative::effects`'s `EffectScalar`/
`EffectArgument`/`EffectResult` boundary uses today. That existing flat
boundary is untouched and remains fully available; this module is additive.

[`projection::to_retained`](../src/agent_lifecycle_typed_carrier/projection.rs)
recursively projects one admitted `DecodedInteractionValue` into
`RetainedValue`, walking the decoded value and its derivation `TypeGraph` in
lockstep — the decoded value alone does not retain a nested field's nominal
type identity, only the derivation graph does, so both are required to
build a `RetainedRecord`/`RetainedVariant` correctly keyed by persistent
stable identity. Every leaf and shape `retained_call` admits round-trips
exactly: `bool`/`i32`/`i64`/`u8`/`usize`, owned `Bytes`, and bounded acyclic
nested records/variants over exactly those leaves.

**One leaf kind is structurally excluded.** `retained_call` deliberately has
no owned-`String` carrier (owned UTF-8 stays on its own profile there); a
decoded value carrying a `string` field is refused explicitly
(`SPX-Z210`, `projection.string_unsupported`) rather than silently dropped,
truncated, or re-encoded as bytes. This is the module's concrete instance of
"a value that cannot be carried while preserving its type/ownership identity
must be refused, never silently degraded."

## Admission: `StageBinding`

[`binding::StageBinding`](../src/agent_lifecycle_typed_carrier/binding.rs)
is captured once from a real `CompiledInteractionSchema`, at the exact root
type and schema revision that schema derived, optionally narrowed to one
exact variant case. `StageBinding::admit` refuses, before any dispatch:

- `stage.wrong_nominal_type` — the value's root type disagrees.
- `stage.schema_mismatch` — the value's schema revision disagrees. The
  revision digest changes on any structural edit (an added/removed/retyped
  field, a changed case, a changed nested reference) but never on a display
  rename, so a binding rebuilt from a renamed-only schema still admits
  values decoded under the original schema, while a binding built from a
  structurally different schema refuses them — this is the module's proof
  that "source field identity persists through display renaming; stale
  structural schema use fails."
- `stage.wrong_variant` — this binding names an exact case and the value's
  case disagrees, or the value is not a variant at all.

Field-order mutation is refused one layer down, by
`agent_interaction_schema::decode`'s byte-exact canonical-replay check (a
reordered document fails to re-render byte-identical to itself and is
refused before a `DecodedInteractionValue` ever exists); this module relies
on that existing guarantee rather than re-implementing it, and its own test
(`field_order_mutation_is_refused_upstream_before_admission_ever_runs`)
exercises the composed path.

## The rich effect operation registry

[`registry::TypedCarrierRegistry`](../src/agent_lifecycle_typed_carrier/registry.rs)
mirrors [Direct Agent Runtime v2](AGENT-RUNTIME-V2.md)'s own operation
registry discipline for the rich profile: operations are ordered and
selected by an exact checked `usize` selector, and `TypedCarrierRegistry::resolve`
requires the resolved slot's deployed operation identity to equal exactly
what the caller declares it expects — refusing an incorrect deployed
operation (a reordered registry, or a caller naming the wrong operation)
before a `TypedCarrierHandler` is ever called.

`registry::call_typed_operation` then: checks cancellation; resolves the
operation; admits and projects the argument; stages exactly one owned
temporary ([`ownership::OwnershipLedger`](../src/agent_lifecycle_typed_carrier/ownership.rs))
for the handler call; and, after the handler returns untrusted response
bytes, decodes and admits the result against the operation's declared result
binding. A malformed or wrongly-typed result (partial decode, wrong nominal
type/case, stale schema) is refused (`SPX-Z212`/`SPX-Z210`) *after* the
argument's ownership has already settled — never before.

## Ownership settlement

[`ownership::OwnershipLedger`](../src/agent_lifecycle_typed_carrier/ownership.rs)
counts owned temporaries currently staged across a boundary this module
drives. `OwnershipLedger::open` returns an `OwnedToken` whose `Drop`
unconditionally decrements the count exactly once, regardless of which path
out of the guarded block is taken — success, a language-level contract
failure, a capacity failure, or an early `?` return. This makes "cleanup
runs regardless of the selected status" (`AGENTS.md`) an observable
Rust-level invariant: a caller asserts `OwnershipLedger::live() == 0` after
any scenario, including every required failure edge, without threading
manual settlement calls through every branch.

Admission and projection both run *before* a token is ever opened, so a
refused admission or projection never constructs an owned temporary in the
first place — cancellation, a wrong nominal/variant/schema, and an
unsupported `string` leaf all leave the ledger at `0` by construction, not
by a separately tracked cleanup path.

`ownership::stage_and_evaluate` drives the real checked interpreter
(`interpreter::retained_call::{prepare_retained_call, evaluate_retained_call}`)
with one projected argument: an owned nested record with a genuine owned
`Bytes` leaf, executed through `prepare_retained_call`'s real admission and
`evaluate_retained_call`'s real evaluator, contract checking, and cleanup —
not a second execution model. Its focused test
(`owned_bytes_leaf_identity_survives_the_boundary_not_just_a_scalar`) proves
a nested owned `Bytes` field, not merely a scalar leaf, survives decode →
admit → project → real interpreter call → harvest intact, and its contract
failure test proves ownership settles on a postcondition violation without
the failure status being replaced.

## The rich value checkpoint codec

`semaprax.agent-lifecycle-typed-checkpoint.v1`
([`checkpoint`](../src/agent_lifecycle_typed_carrier/checkpoint.rs)) wraps
one canonical `agent_interaction_schema` decoded value with an explicit
`type_version` tag. It is wholly additive: it neither reads nor changes the
existing flat checkpoint codec
(`agent_runtime_v2::checkpoint::value::{encode,decode}`, closed
`RetainedValue` scalars/one-level records only), which keeps every one of
its own known answers unchanged.

`encode` performs no re-derivation — it trusts the already-admitted value
and only adds the versioned envelope, reusing
`agent_interaction_schema::decode::render_typed_value` verbatim, so the same
value always encodes to the same bytes (`checkpoint_encoding_is_deterministic_and_independently_reconstructible`
independently reconstructs the expected bytes without calling `encode`'s own
formula). `decode` is independent replay: given the exact schema the caller
currently has bound, it validates the envelope and the embedded revision
by exact string matching (never re-serializing the value payload, which
would risk losing byte-exactness), then delegates the value payload itself
to `CompiledInteractionSchema::decode` for full canonical, bounded,
byte-exact admission.

Rejected, each with the stable `SPX-Z212` diagnostic, never silently widened
or truncated:

- An unrecognized `type_version` (only `1` exists today).
- A schema revision that disagrees with the caller's current binding (a
  stale schema binding).
- A payload over `MAX_CHECKPOINT_BYTES` (131,072 bytes), checked before any
  parsing.
- A malformed envelope or a value payload that does not itself decode.

## Diagnostics

| Code | Meaning |
|---|---|
| `SPX-Z210` | An admission or projection refusal: a wrong nominal type/variant, a stale structural schema, or a leaf `retained_call` has no carrier for (`string`). |
| `SPX-Z211` | A rich effect operation registry refusal: an out-of-range selector, or a resolved slot whose deployed operation identity disagrees with what the caller expects. |
| `SPX-Z212` | A rich value checkpoint refusal, or a rich effect operation's malformed/wrongly-typed result. |
| `SPX-Z213` | Cancellation observed before dispatch. |

## Wiring into `agent_lifecycle` (landed)

[`agent_lifecycle::rich_stage`](../src/agent_lifecycle/rich_stage.rs) is the
thin adapter this document originally deferred: it validates a new
`authorize(state, proposal) -> Decision` / `reduce(state, proposal, outcome)
-> Transition` signature convention directly against persistent HIR facts,
derives a real `CompiledInteractionSchema` for the Proposal role, and
sequences decode (`CompiledInteractionSchema::decode`) -> admit
(`binding::StageBinding::admit`) -> project (`projection::to_retained`) ->
dispatch (`interpreter::retained_call::evaluate_retained_call`) for one
turn (`run_rich_turn`). It is additive: `stages::bind`'s scalar-exploded
convention, `iterative`'s whole multi-turn driver loop, and every lifecycle
already compiled through them are unchanged, and this module edits neither
`agent_lifecycle_typed_carrier` nor `agent_interaction_schema` — it only
calls their existing public API. Focused gate:

```sh
cargo test --locked -p semaprax --lib agent_lifecycle::rich_stage
```

**A concrete, confirmed boundary found while wiring this**: a genuinely
*nested* Proposal (a record field whose type is itself a further record)
does not clear the retained-call seam as a Value-mode parameter today.
`interpreter::retained_call`'s `resolved_data_parameter_is_admitted` admits
a non-flat nominal parameter at `OwnershipMode::Value` only when its
declaration is `DeclarationKind::Class`, but
`agent_interaction_schema::shape::derive` refuses a `class` root outright
(`type.kind`) as one of its own stated exclusions — so a type that is both a
valid Agent Interaction Schema v1 root and an admitted retained-call Value
parameter must, today, be a flat record. `rich_stage`'s own fixture is a
genuine multi-field flat Proposal crossing as one nominal argument (not
exploded into per-field scalar parameters); it is not a nested one.
Resolving this needs a change in `interpreter::retained_call` or
`agent_interaction_schema::shape`, both outside `agent_lifecycle`'s file
lease.

## Known limitations (this round)

- **`agent_lifecycle::rich_stage` does not yet drive
  `agent_lifecycle::iterative::CompiledIterativeLifecycle`'s existing
  multi-turn loop, and validates a fixed two-case Decision/Transition shape
  rather than the four-case `Continue/Complete/Suspend/Fail` Step grammar.**
  `run_rich_turn` runs one turn, standalone. See `rich_stage`'s own
  module documentation for the exact admitted shapes and the
  single-scalar-field State restriction.
- **Still no wiring into `execution_revision::typed` or
  `agent_runtime_v2::checkpoint::value`.** Those files remain outside every
  worker's file lease so far (other issues own them). This module's
  `checkpoint` codec is designed to make that wiring a thin adapter, not a
  redesign, but the adapter itself is not written here.
- **No wiring into `src/live_invocation/`'s `ProposalDecoder`/`TurnEffect`.**
  Those traits are outside this worker's file lease too; a real integration
  would bind them to `CompiledInteractionSchema::decode` and to
  `registry::call_typed_operation` respectively.
- **Variant case payloads remain flat scalars only, in this language,
  today**, independent of this module — `source_verify`'s "Copy Variants
  v1" rule (`SPX-T215`) admits only a direct Copy scalar (or `Bytes`) as a
  variant case field, so a nested record inside a case is not yet
  constructible in checked source, matching Agent Interaction Schema v1's
  own declared limitation.
- **No configurable bounds.** `MAX_CHECKPOINT_BYTES` is a fixed constant.

## Executable reference

`src/agent_lifecycle_typed_carrier/` (`binding.rs`, `projection.rs`,
`registry.rs`, `ownership.rs`, `checkpoint.rs`, `tests.rs`) is the complete
reference implementation this document describes. Focused gate:

```sh
cargo test --locked -p semaprax --lib agent_lifecycle_typed_carrier
```
