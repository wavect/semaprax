# Agent Interaction Schema v1

Audience: agent and tool authors consuming interaction schemas, and compiler contributors maintaining the derivation.

Status: **LOCAL** bounded implementation with an executable reference and
focused regression corpus. This document specifies `semaprax.agent-
interaction-schema.v1` (the derived canonical schema) and `semaprax.agent-
interaction-value.v1` (one decoded value), implemented in
`src/agent_interaction_schema/`.

Audience: implementers of the fourteen open issues in the `agent-runtime`
lane that consume a rich interaction schema, and reviewers of this bounded
admission profile.

This is issue #109 ("Derive bounded rich Agent interaction schemas from
checked source"). It owns schema derivation and decoding only; runtime value
transfer into an executing program is a separate, later concern.

## Why a new module instead of extending `agent_proposal`/`agent_observation`

`agent_proposal` and `agent_observation` already derive a closed schema from
one checked record/variant declaration, but only over **flat** fields: every
field must be a direct scalar (`bool`/`i32`/`i64`/`u8`/`usize`/`string`); a
field naming another record/variant type is rejected. That existing profile
and its wire behavior are unchanged by this document — this module neither
reads nor edits their internals.

This document defines an independent, additional profile: a **bounded,
nonrecursive** interaction schema whose fields may also be bounded `Bytes`
or a reference to exactly one further monomorphic record/variant
declaration, up to a bounded type-graph size and nesting depth. Where the
two profiles overlap (direct scalar fields), this module uses the exact
same wire conventions (bare `bool`, decimal-string integers, plain JSON
strings) so a genuinely flat type produces a compatible value under either
profile.

## Scope

One checked module resolves one named root record or variant declaration
(`root_type_id`, a persistent stable `@id`). Its fields may be:

- a direct scalar: `bool`, `i32`, `i64`, `u8`, `usize`;
- bounded UTF-8 text (`string`), bounded to 4,096 UTF-8 bytes;
- bounded `Bytes`, bounded to 4,096 elements;
- a reference to exactly one further monomorphic (non-generic) record or
  variant declaration, itself subject to the same rules, recursively.

The complete type graph (the root plus every transitively referenced type)
must be a DAG: at most 64 distinct types, at most 16 levels deep, and never
cyclic. A generic declaration, a field typed with any generic argument
(including `Box<T>`-shaped indirection, since instantiation always carries a
generic argument in this language), a resource, a class, a raw view
(`Str`/`SliceU8`), a fixed-size `ArrayU8`, a floating-point field, a
function value, an empty variant, or a non-persistent identity anywhere in
the graph is refused with an explicit `SPX-Z202` diagnostic naming exactly
which rule failed — never silently approximated as opaque JSON.

**A variant case's fields go through the same rule as a record's fields**
in this module's derivation and decoder, so a case may in principle carry
`string`, `Bytes`, or a nested type reference. Today's `source_verify`
independently enforces a narrower "Copy Variants v1" rule (`SPX-T215`):
a case field must be a direct Copy scalar or, as of the additive Copy
Aggregate Variant Payload v1, a direct, monomorphic, drop-free nested
`record` (its own fields must recursively need no drop at all, so it can
never itself reach an owned `Bytes` or `string`). That is a base compiler
restriction this module does not relax or work around — it means a case's
field is *practically* limited to a direct scalar or a Copy-only nested
record until a future language revision admits `string` or an owned nested
payload, without this module needing to change when that happens.

**Non-goals**, matching the issue's bounded scope: resources, arbitrary
recursion, raw pointers, borrowed values escaping the call, callbacks, and
public ABI promotion. Runtime value transfer into an executing program
(binding a decoded value to a live call) is out of scope for this document.

## Why this cannot depend on the streaming extension (#178)

[`CompiledInteractionSchema::decode`](../src/agent_interaction_schema.rs)
takes one complete `&[u8]` response and returns one decoded value or one
refusal. There is no partial-decode state threaded through this module, and
nothing here reads from or requires a streaming transport. A streaming
provider transport can buffer its events into one complete response before
calling this exact boundary. This mirrors the same independence the [Live
Invocation Contract v1](LIVE-INVOCATION-CONTRACT-V1.md) already documents
between its `ProposalDecoder` seam and its streaming extension: `#109 (rich
schema) does not depend on #178 (streaming extension)`. A future streaming
decoder is free to be *derived from* this schema (the type graph and field
bounds are exactly what a streaming decoder needs to know when it may emit
a partial field), but this module does not need that decoder, or anything
about how bytes arrive, to exist.

`CompiledInteractionSchema` is not wired into `src/live_invocation/`'s
`ProposalDecoder` trait in this tranche (that module is out of this
worker's file lease), but its shape is deliberately compatible: a real
integration wraps one `CompiledInteractionSchema`, exposes `schema_digest()`
as `self.schema().digest()`, and forwards `decode(turn, response)` to
`CompiledInteractionSchema::decode(response)`, mapping `Ok` to
`ProposalOutcome::Admitted` and `Err` to `ProposalOutcome::Refused`.

## The canonical schema document

`compile_agent_interaction_schema(source_path, root_type_id)` derives:

```json
{
  "schema": "semaprax.agent-interaction-schema.v1",
  "root_type_id": "outer.type",
  "root_type_revision": "sha256:...",
  "types": [
    {
      "stable_id": "inner.type",
      "kind": "record",
      "fields": [
        {"stable_id": "inner.x", "type": {"kind": "scalar", "representation": "i64", "minimum": "-9223372036854775808", "maximum": "9223372036854775807"}},
        {"stable_id": "inner.y", "type": {"kind": "scalar", "representation": "string", "max_bytes": 4096}}
      ]
    },
    {
      "stable_id": "outer.type",
      "kind": "record",
      "fields": [
        {"stable_id": "outer.inner", "type": {"kind": "nested", "stable_id": "inner.type"}}
      ]
    }
  ],
  "wire": {
    "closed_objects": true,
    "key_order": "declaration_order",
    "exact_integer_encoding": "decimal_string",
    "bytes_encoding": "byte_array_u8",
    "max_document_bytes": 65536,
    "max_string_field_bytes": 4096,
    "max_bytes_field_bytes": 4096,
    "max_types": 64,
    "max_depth": 16
  },
  "nonclaims": ["..."]
}
```

`types` is listed in dependency-first order (a valid deterministic
topological order of the DAG); `root_type_id` — not position — names the
entry point. Every identity in `types` is a persistent stable `@id`; display
names never appear anywhere in this document.

**Identity.** `root_type_revision` is a domain-separated SHA-256 digest of
exactly `{"root_type_id":...,"types":[...]}` — nothing else. A display
rename (of the root type, a nested type, a field, or a case) never appears
in that body, so it leaves `root_type_revision` (and therefore every
decoded value's binding) unchanged. Any structural change — an added or
removed field, a changed representation, a changed case, a changed nested
type reference — changes `root_type_revision`. `schema().digest()` is a
further domain-separated digest over the complete rendered document
(including `wire`/`nonclaims`), used to bind a decoded value to this exact
schema (see below).

## Source binding

`compile_agent_interaction_schema` binds to exact source bytes using the
same shared mechanism `capability_manifest::generate`,
`region_report::generate` and `assurance_manifest::generate` already use:
`patch::canonical_source_path` resolves and locks the path,
`patch::read_source_snapshot` takes an exact snapshot, and
`patch::validate_source_unchanged` re-checks the snapshot immediately before
returning success. A concurrent write between snapshot and return fails the
derivation closed rather than returning a schema for source that no longer
matches — this module invents no second source-binding mechanism.
`verify_agent_interaction_schema_bundle` independently rederives a schema
from source and requires a supplied document to equal it byte for byte
(`SPX-Z204` on drift).

## The decoded value document

```json
{
  "schema": "semaprax.agent-interaction-value.v1",
  "root_type_id": "outer.type",
  "schema_digest": "sha256:...",
  "value": {"fields": {"outer.inner": {"fields": {"inner.x": "7", "inner.y": "ok"}}}}
}
```

A record value is `{"fields": {<stable_id>: <value>, ...}}`; a variant
value is `{"case": "<stable_id>", "fields": {...}}`. Leaf scalar wire forms:

| Representation | Wire form |
|---|---|
| `bool` | JSON `true`/`false` |
| `i32`/`i64`/`u8`/`u64` (`usize`) | a canonical decimal-string integer (no `+`, no leading zero except `"0"`, no exponent, no fraction) |
| `string` | a plain JSON string, at most 4,096 UTF-8 bytes |
| `bytes` | a JSON array of integers `0..=255`, at most 4,096 elements |

Exact integers travel as decimal strings, never as JSON numbers, so every
consumer preserves values outside the range an IEEE-754 `f64` (and
therefore a JavaScript `Number`) can represent exactly. This is the same
convention `agent_proposal` already uses for its own five admitted host
scalars; this document keeps it for the two representations it adds.

**Decoding is total, bounded, whole-value validation of untrusted bytes.**
`CompiledInteractionSchema::decode` takes `&[u8]` — not yet known to be
UTF-8 — and:

1. Refuses a document over 65,536 bytes before any parsing work
   (`SPX-Z206`, `document_bytes`).
2. Refuses non-UTF-8 bytes explicitly (`SPX-Z206`, `utf8`) rather than
   panicking or lossily replacing invalid sequences.
3. Requires exactly one line ending in `\n`, no BOM, and valid JSON
   (`SPX-Z205` otherwise).
4. Requires the top-level envelope's `schema`, `root_type_id` and
   `schema_digest` to match exactly (`SPX-Z205`/`SPX-Z206`).
5. Walks the value against the derived type graph: every declared field
   must be present exactly once (`SPX-Z206`, `value.fields.missing`); no
   undeclared field may appear (`SPX-Z206`, `value.fields.unknown`); a
   variant's `case` must name one of the declared cases (`SPX-Z206`,
   `value.case`); every scalar must parse and fit its declared
   representation and bound (`SPX-Z206`, `value.representation` /
   `value.integer_range` / `value.string_bytes` / `value.bytes_length` /
   `value.bytes_element`); nesting may not exceed the declared depth bound
   (`SPX-Z206`, `value.max_depth`).
6. Re-renders the decoded value in the one canonical form (declaration-
   ordered keys, closed objects) and requires it to equal the input byte
   for byte (`SPX-Z205` on mismatch).

Step 6 is also this module's duplicate-key defense: canonical rendering can
only ever emit one occurrence of each declared key, so a source document
that repeats any key anywhere — declared or undeclared — is strictly longer
than its canonical replay and is therefore refused. A generic map that
silently kept "whichever occurrence came last" never becomes the accepted
reading, because that reading's own canonical replay still cannot match the
longer, duplicate-carrying source.

A decoded value carries no authority: it constructs no `Authorized<T>`, no
publication token, and no capability, and decoding performs no provider,
tool, filesystem, process, network, or approval effect.

## Provider presentation projections

`CompiledInteractionSchema::provider_json_schema` (`src/agent_interaction_
schema/provider.rs`) renders one self-contained JSON Schema draft 2020-12
document describing exactly the shape `decode` admits, following the same
local-`$defs` convention as [Candidate Constructor Schemas
v1](CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md): every nested type reference is a
`$ref` into `$defs`, no validator needs a network lookup. Exact integers are
presented as `{"type":"string","pattern":"^-?[0-9]+$","x-representation":...,
"x-minimum":...,"x-maximum":...}` — never `"type":"integer"` — so that no
client generated from this projection can round an out-of-range value
through a native number type merely by trusting the presentation schema's
declared JSON type. `Bytes` is presented as the same bounded
byte-integer-array wire form the decoder accepts. Every object closes
`additionalProperties: false` with an explicit `required` list; a variant
is `oneOf` one alternative per case, each pinning `"case":{"const":...}`.

**This projection cannot broaden runtime admission.** `decode` never reads
`provider_json_schema`'s output — the two are independent pure functions of
the same canonical `TypeGraph`, computed by disjoint code paths. If a
provider's schema dialect cannot express a constraint this projection
carries (for example the `x-minimum`/`x-maximum` vendor extension, or the
decimal-string integer pattern), the *provider* may accept something this
projection describes as invalid; the canonical decoder still enforces the
real bound after the response arrives. A provider limitation can therefore
only ever cause a *false accept* to be caught by post-response decoding —
never a *true accept* by the compiler's own admission.

Per-provider adapters (for example an OpenAI structured-output profile or
an Anthropic tool-use schema) are downstream transformations of this one
generic draft-2020-12 document; this tranche ships the one canonical
projection they would each start from, not every named-provider variant.

## Diagnostics

| Code | Meaning |
|---|---|
| `SPX-Z202` | A derivation-time structural admission failure: unresolved/non-persistent identity, generic declaration or field-type argument, empty variant, cyclic type reference, an over-budget type graph, or any other unsupported source type. |
| `SPX-Z203` | The derived schema exceeded its output byte budget. Fails closed; never truncated. |
| `SPX-Z204` | A supplied schema document is not the exact independent replay of its checked source. |
| `SPX-Z205` | A decoded document is not canonical JSON: invalid JSON, a malformed envelope shape, or a canonical-replay mismatch (which also covers every duplicate key). |
| `SPX-Z206` | A decode-time admission rule failed: oversized/non-UTF-8 input, a schema/root-type binding mismatch, an unknown/missing field, a wrong variant tag, an out-of-bound scalar, or excess nesting depth. |

## Known limitations (this round)

- **No wiring into `src/live_invocation/`'s `ProposalDecoder`.** That module
  is outside this worker's file lease; the shape above is designed to make
  that wiring a thin adapter, not a redesign, but the adapter itself is not
  written here.
- **No generated, compiled client bundles.** `agent_proposal::clients`
  verifies real TypeScript consumer bundles against its flat schema; this
  document does not extend that machinery to the rich profile. The
  decimal-string integer convention (never a native JSON number) is the
  structural property that keeps any conformant future client from
  rounding a large integer, independent of whether a bundle is generated
  and compiled in this repository.
- **Variant case payloads admit a Copy-only nested record, never `string`,
  in this language, today.** `source_verify`'s "Copy Variants v1" rule
  (`SPX-T215`) admits a direct Copy scalar (`bool`/`i32`/`i64`/`u8`/`usize`/
  `f32`/`f64`/`char`), an in-scope variant type parameter, or — as of the
  additive Copy Aggregate Variant Payload v1 — a direct, monomorphic,
  drop-free nested `record`; it still never admits `string` or a `record`
  that itself reaches an owned `Bytes`/`string`, independent of this module.
  This module's derivation and decoder both still implement the general
  nested-field rule for variant cases (a case's `FieldRow`s go through
  exactly the same `classify`/`decode_type` path a record's fields do), so
  a future language revision admitting `string` or an owned nested payload
  needs no change here — and a compiling `.spx` fixture can now exercise a
  Copy-only nested variant-case field; `string` and an owned nested payload
  remain unreachable.
- **The `type.field.generic_argument` and `type.recursive` guards are
  defense in depth, not exercised by a compiling fixture.** The base
  compiler already forecloses every nested generic-instantiated record
  field universally (`SPX-T223`, unconditional, independent of this
  module) before `hir::resolve` ever succeeds, and this language's records
  are sized values with no unboxed indirection — so a genuinely cyclic
  monomorphic record graph is not constructible in checked source either,
  since generic instantiation (already foreclosed) is the only route
  indirection could otherwise take. `tests::generic_argument_nested_
  field_is_refused_explicitly` proves the end-to-end contract (such a
  field is refused, not silently approximated) but observes `SPX-T223`
  from the base compiler, not this module's own `SPX-Z202`
  `type.field.generic_argument`; the `visiting`-stack cycle guard in
  `shape.rs` has no compiling regression at all. Both guards are kept as
  the profile's own second, independent line of defense.
- **No configurable bounds.** `max_types`/`max_depth`/`max_*_bytes` are
  fixed constants (matching `agent_proposal`'s own fixed constants), not a
  caller-supplied `Options` struct.

## Executable reference

`src/agent_interaction_schema/` (`shape.rs`, `decode.rs`, `render.rs`,
`provider.rs`, `tests.rs`) is the complete reference implementation this
document describes. Focused gate:

```sh
cargo test --locked -p semaprax --lib agent_interaction_schema
```
