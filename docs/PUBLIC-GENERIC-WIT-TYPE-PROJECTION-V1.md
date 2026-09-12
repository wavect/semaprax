# Public Generic WIT Type Projection v1

Status: implemented bounded projection; local evidence only, no hosted CI run
recorded. This is a documentation-tracked slice of issue #176 ("Add a WIT and
WebAssembly Component Model projection for supported generic resources"),
which is itself gated behind PG-9 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
**Public generic ownership is not supported or published, and this document
does not change that.** No public generic export exists, no compiled `.wasm`
artifact implements the provider ABI (open issue #229), and no engine has ever
executed a component built from this projection. What exists is a
deterministic, refusal-total *type* projection from an already-checked
[Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)
admission to WIT `record`/`resource` text — nothing that calls it, nothing
that lowers it to component bytes, and nothing that publishes it.

Audience: ABI, WIT/Component, package, and evidence reviewers.

Implementation: [`src/public_generic_abi/wit_projection.rs`](../src/public_generic_abi/wit_projection.rs).

## Why this exists before PG-9

Issue #176 asks for a WIT projection of the "accepted public-generic scope."
At the audit baseline (`ae25c6a4`, 2026-09-11) that scope is, per the
milestone's own standing decision, **empty**: no public generic export is
admitted, generated, published, or supported. What the milestone *does* have,
hosted green, is the [Public Generic Type Grammar
v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) (PG-1/PG-2) and the [Public Generic
Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) classifier — a
read-only admission predicate over already-checked HIR that computes exactly
which record instances a candidate export's signature would reach, without
admitting a public surface.

This document and its module project *that* — a classifier admission's
already-computed record closure — into WIT text. It is useful evidence toward
issue #176 (the mapping-table and naming halves of its scope) without
overclaiming the calling-convention, component-binary, or support halves,
which remain blocked on PG-9 and on #229 respectively.

## Scope: the whole mapping table

The type grammar admits exactly three term shapes, and only three: a Copy
scalar, direct owned `Bytes`, and a fully concrete authored record instance.
Every other type the grammar could name — an unsubstituted type parameter,
`string`, `str`, `Slice<u8>`, `unit`, an inline byte array, a function type, a
compiler-owned nominal (`Option`, `Result`, `Vec`, `Box`, `Iter`, ...), or an
authored class/variant/resource — is refused by the grammar itself, with its
own closed reason, before this projection is ever reached. This module
therefore has exactly this mapping table, and no more:

| Grammar term | WIT projection | Notes |
| --- | --- | --- |
| `i64` | `s64` | |
| `i32` | `s32` | |
| `u8` | `u8` | |
| `char` | `char` | |
| `f32` | `f32` | |
| `f64` | `f64` | |
| `bool` | `bool` | |
| `usize` | refused (`SPX-PGWIT101`) | no WIT primitive has a portable pointer width; approximating it as `u32`/`u64` would silently pick a width the source type never specified |
| `bytes` (owned `Bytes`) | `own<spx-owned-bytes>` | a handle to one shared opaque `resource`, never `list<u8>` — see below |
| a concrete record instance | a WIT `record`, one field per substituted field, recursively projected | |

Issue #176's in-scope list also names variants, owned/borrowed strings,
`Option`, `Result`, and authored resources. None of those can reach this
module today, because the grammar itself does not admit them yet; widening
the grammar to admit any of them is its own new grammar version with its own
gates (stated already in [Public Generic Type Grammar
v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md#admitted-vocabulary)), and only then
would this projection gain a corresponding new mapping row. This document
does not invent a spelling for any of them in the meantime.

### Why `bytes` is a resource, not `list<u8>`

WIT's `list<u8>` is a by-value component type: the canonical ABI copies it
across the boundary and the receiving side owns an ordinary value, with no
notion of an explicit release. SEMAPRAX's owned `Bytes` is the opposite: a
single unique owner, sticky failure selection, and a canonical cleanup order
the compiler verifies and the [Public Generic Settlement
Obligations](PUBLIC-GENERIC-SETTLEMENT-V1.md) work derives field by field.
Projecting it as `list<u8>` would silently hand a foreign consumer a value
with GC/copy semantics and let the Component Model runtime believe it settled
ownership the compiler never delegated to it — exactly the assumption issue
#176 names as explicitly out of scope. Projecting it as an owned resource
handle (`own<spx-owned-bytes>`) keeps the explicit-close discipline visible in
the WIT surface itself, even though no physical resource destructor exists
behind it yet (that is calling-convention and adapter work, blocked on PG-9
and #229, not a type-projection concern).

## Naming

Every WIT identifier is `spx-` followed by the lowercase hexadecimal UTF-8
bytes of a persistent identity — the same convention
[`src/project/scalar_wit.rs`](../src/project/scalar_wit.rs) already uses for
the Project-v1 scalar WIT interface's exported function names:

- a record's name hexes its own canonical [Public Generic Type
  Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) term (e.g.
  `@23:mod.pair<@23:mod.leaf<>,i64>`), not its declaration id alone, so two
  different concrete instantiations of the same template are two different
  WIT records;
- a field's name hexes its own persistent field declaration id, not its
  display name.

Hex framing of an already-injective grammar term (injectivity is proved in
[Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md#injectivity))
is itself injective: two distinct byte strings hex to two distinct names,
independent of length, punctuation, or any grammar delimiter one of them might
contain. No hashing, truncation, or normalization step is in this path to hide
a collision. A display-only rename of a record, its type parameters, its
fields, or the exported function that reaches it leaves every WIT identity
here unchanged, because the grammar term and the stable field id it hexes
already exclude every display name.

## Determinism

[`project_admitted_subject`] takes its closure directly from
[`AdmittedSubject::record_closure`](../src/public_generic_abi/classifier.rs)
— a `BTreeMap<String, InstanceFacts>` keyed by canonical term the [Public
Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) classifier
already computes for candidate-delta and settlement use. Visiting a
`BTreeMap` is ordered by key; visiting a record's fields walks the
already-ordered `Vec<FieldFact>` the classifier produced. Nothing in this path
reads a `HashMap`, the process environment, the clock, or randomness, so the
same admitted subject renders byte-identical WIT text on every call — the
projection's own test asserts this directly by projecting the same subject
twice and comparing both the structured result and the rendered bytes.

Records are emitted in dependency order (a record's own dependencies are
always emitted, and reach the closure lookup, before the record itself),
computed by recursion over each record's fields rather than by the
closure map's own top-level key order — which is a distinct thing to prove,
demonstrated by picking declaration ids whose canonical terms sort in the
*opposite* order from their dependency (`aaa_pair` contains `zzz_leaf` but
sorts before it), so a projection that only followed the map's key order
would emit the dependent record first and be caught doing so.

## Lossless or explicitly refused — never approximated

Two refusal paths exist, and each carries a specific, checked reason rather
than an approximation:

- **`usize`** (`SPX-PGWIT101`): the message names the exact rejected scalar.
  The regression test compiles a real program whose classification succeeds
  (proving the refusal is this module's, not an earlier grammar/classifier
  check reused under a new name), then confirms only the projection step
  fails, with this code, and that the message contains `usize`.
- **a record instance absent from the supplied closure** (`SPX-PGWIT102`):
  the closure is the caller's explicit input, exactly like the "explicit
  closure, never auto-scanned" convention [PG-4's candidate
  delta](PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md) already established; a missing
  member is refused by name rather than silently treated as an empty/opaque
  type. A defensive third code (`SPX-PGWIT103`) additionally catches an
  internally inconsistent closure (two different facts under one canonical
  term, or a closure entry whose own term disagrees with its map key) — not
  reachable through the classifier's own construction, but refused rather
  than trusted if a future caller assembles a closure by hand.

Capacity (`SPX-PGWIT104`, the rendered WIT exceeding
[`MAX_WIT_PROJECTION_BYTES`] = 65,536 bytes, or the shared record-nesting
depth bound the type grammar already enforces) is a refusal, never a
truncation.

## Round-trip

[`parse_wit_projection`] is an independent, bounded profile parser for
exactly this module's own rendering — the same "independent bounded parser
accepts only that profile" convention the private WIT/Component harness in
[WIT-COMPONENT-BOUNDARY-V1](WIT-COMPONENT-BOUNDARY-V1.md) already uses, not a
general WIT-text parser, and not cross-checked against an external
`wit-parser`/`wasmparser` crate (adding one would touch `Cargo.toml`, out of
this work's lease). Every identifier and type-text token is scanned by its
exact legal character set rather than searched for with an unbounded
substring search, so a single corrupted delimiter cannot let parsing
resynchronize past it onto a later, unrelated line and silently accept the
wrong text as the field that was actually mutated — an early draft of this
parser had exactly that bug, and the hostile mutation test below is what
caught it.

The round-trip regression compares the **parsed** structure against the
**pre-render** `WitTypeProjectionV1` facts that produced the text — never
against a second call to the renderer — so a renderer/parser pair that agreed
with each other but silently drifted from what was actually intended to be
projected could not pass this test by construction.

Hostile cases: truncating the rendered text by a handful of trailing bytes,
corrupting one field's `: ` separator, and appending one trailing byte after
the closing `world` block are each rejected with `SPX-PGWIT105`, independently
of each other.

## Nonclaims

This document and its module admit no `.spx` syntax, define no calling
convention, emit no Component Model binary, generate no host or guest
adapter, execute nothing, and grant no filesystem, process, network,
execution, signing, or publication authority. They do not:

- export a callable WIT function — only the record/resource *type* shapes an
  eventual function signature would use;
- lower anything to Core Wasm or Component Model bytes;
- prove, or attempt to prove, that a public generic export is "callable
  through a real Wasm component" (issue #176's first acceptance criterion) —
  that requires a calling convention and a physical adapter, both blocked on
  PG-9's undecided support/publication decision and on #229's open compiled
  `.wasm` provider gap;
- widen the milestone's standing support decision, reinterpret any existing
  descriptor/carrier/package/prelude/graph/cleanup schema, or change the
  frozen `semaprax:project-scalar@1.0.0` WIT identity
  [Public Scalar WIT Interface v1](PUBLIC-SCALAR-WIT-INTERFACE-V1.md) owns —
  this projection's package (`semaprax:public-generic-types@0.1.0`), interface
  (`types`), and world (`public-generic-types-v1`) are a distinct, new
  identity precisely so neither can be confused with the other;
- cover a variant, an authored resource, an owned/borrowed string, `Option`,
  or `Result` — the grammar does not admit any of them yet, so none can reach
  this module; each stays a grammar/classifier-level refusal with its own
  existing closed reason until a future grammar version admits it and this
  projection gains its own new, separately gated mapping row.
