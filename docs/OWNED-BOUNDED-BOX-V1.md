# Owned Bounded Box v1

Audience: language users, standard-library authors, and compiler contributors.

Status: locally exercised bounded implementation tranche; hosted promotion and
the broader allocation model remain pending.
This document owns one compiler-provided uniquely owned allocation for Copy
scalar payloads and the corresponding authenticated `std.mem` surface. It
does not define a public aggregate ABI, allocator interface, region, arena, or
shared-ownership model.

## Exact profile

`Box<T>` is the compiler-owned nominal type with stable identity `core.box`.
`T` is exactly one of `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, or
`bool`; every operation spells that type argument explicitly.
[Generic Compiler Collections v1](GENERIC-COMPILER-COLLECTIONS-V1.md) adds private
function composition over these exact carriers and all eight substitutions;
it does not change allocation or public ABI contracts.

| Source | Stable identity | Signature |
| --- | --- | --- |
| `box_new` | `core.box.new` | `<T>(value: T) -> Box<T>` |
| `box_get` | `core.box.get` | `<T>(value: borrow Box<T>) -> T` |
| `box_into_inner` | `core.box.into-inner` | `<T>(value: own Box<T>) -> T` |

`box_new` creates one uniquely owned logical allocation containing the Copy
value. A function may create at most 4,096 live Box allocations. Allocation
refusal selects sticky `semaprax.box.v1` code 1. `box_get` borrows
synchronously and returns a Copy of the payload without changing ownership.
`box_into_inner` consumes the sole owner, returns the payload, and settles the
allocation exactly once. An owner not consumed by `box_into_inner` is settled
exactly once by ordinary lexical cleanup.

The carrier is non-Copy regardless of `T`. Its allocation identity, address,
layout, and target storage strategy are not observable language values.

## Prelude and compatibility

Box use selects additive `semaprax.prelude.v4`. Frozen prelude v1, v2, and v3
contract bytes and digests remain unchanged. Programs that do not use a Box
intrinsic keep their prior prelude selection, including programs that declare
an authored generic record named `Box<T>`; such records remain ordinary inline
nominal storage rather than this compiler-owned allocation. A program cannot
mix that authored lookalike with compiler-owned Box operations.

Prelude v4 adds only `core.box`, the three operations above, their eight-scalar
element rule, the 4,096-live-allocation bound, and `semaprax.box.v1` code 1.
It does not change Vec operations or the source/HIR/Graph/cleanup meaning of an
older program.

## Standard-library surface

The alloc-tier `std.mem` package contains exactly three authenticated
transparent aliases:

| Stable identity | Intrinsic |
| --- | --- |
| `std.mem.box.new` | `core.box.new` |
| `std.mem.box.get` | `core.box.get` |
| `std.mem.box.into-inner` | `core.box.into-inner` |

The package conformance source instantiates all three aliases for every
admitted Copy scalar. Its example and tests expose scalar results only. The
manifest has an empty `web_exports` list, so the package creates no public API
descriptor or stable generic ABI. Exact manifest and source authentication is
required before Project admission; a lookalike module, identity, profile,
source, example, test, or nonempty export list fails closed.

## Required focused evidence

Promotion requires focused source/HIR, cleanup, package, and runtime selectors
covering:

- exact explicit instantiation and stable intrinsic/wrapper identity for all
  eight Copy scalars;
- allocation, synchronous get, consuming extraction, lexical drop, repeated
  entry, and allocation refusal on the interpreter, native C11 `-O0`/`-O2`,
  and internal Core-Wasm;
- exact one-owner liveness and finalization under success, contract failure,
  and hostile cleanup/HIR mutation;
- frozen prelude-v1/v2/v3 known answers and selection of v4 only by Box use;
- exact authenticated `std.mem` Project check/test/run, empty public descriptor,
  3-by-8 conformance, bundled dependency resolution, and generated catalogs;
  and
- rejection of inference, missing or surplus type arguments, unsupported or
  owned payloads, forged intrinsic/wrapper identities, authored-Box collision,
  owner reuse, and escaping loans.

The focused source/HIR, cleanup replay, package/catalog, interpreter, native
C11 `-O0`/`-O2`, internal Core-Wasm, allocation-refusal, lexical-drop, and
hostile carrier/identity selectors pass locally at this revision. Evidence is
still unhosted, and contract-failure cleanup plus the broader allocation model
remain promotion work. All affected completion rows remain Partial.

## Nonclaims

There is no owned, aggregate, `Bytes`, `String`, Vec, variant, resource, or
nested Box payload; mutable Box borrow; replacement; pinning; raw pointer;
custom allocator; allocator transfer; placement allocation; allocator identity
or layout guarantee; region or arena syntax; bulk release; ARC, shared or weak
ownership; cross-thread sharing; public Project/FFI/WIT/Component Box ABI;
Iterator integration; hosted promotion; or production support. `std.mem`
advances only this exact Box slice; its broader ownership helpers, regions,
arenas, and shared immutable values remain missing.
