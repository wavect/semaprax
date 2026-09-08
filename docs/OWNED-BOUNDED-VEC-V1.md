# Owned Bounded Vec v1

Audience: language users, standard-library authors, and compiler contributors.

Status: implementation tranche. This document owns one internal, explicitly
instantiated `Vec<T>` profile for Copy scalar elements. It defines no public
aggregate ABI and does not implement Iterator.
The additive [owned Bytes profile](OWNED-BOUNDED-VEC-V2.md) has a separate
contract and evidence boundary.

The separately versioned
[Owned Bounded Vec For Traversal v1](OWNED-BOUNDED-VEC-FOR-TRAVERSAL-V1.md)
adds one source `for item in values { body }` form over a simple immutable
binding of this exact vector profile. Its resolver lowering reuses the existing
len/get/while HIR and adds no operation, prelude version, backend primitive,
standard-library declaration, or ABI.

## Exact profile

`Vec<T>` is the compiler-owned nominal type with stable identity `core.vec`.
`T` is exactly one of `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, or
`bool`. Type inference, `Bytes`, `String`, authored aggregates, variants,
resources, nested vectors, and nonconcrete element types are rejected.
The additive [generic compiler collection profile](GENERIC-COMPILER-COLLECTIONS-V1.md)
admits a scoped type parameter inside private function templates only after
independently validating every concrete Copy substitution. It preserves this
runtime profile and public ABI boundary.

The compiler-owned operations are:

| Source | Stable identity | Signature |
| --- | --- | --- |
| `vec_with_capacity` | `core.vec.with-capacity` | `<T>(capacity: usize) -> Vec<T>` |
| `vec_push` | `core.vec.push` | `<T>(values: own Vec<T>, value: T) -> Vec<T>` |
| `vec_len` | `core.vec.len` | `<T>(values: borrow Vec<T>) -> usize` |
| `vec_capacity` | `core.vec.capacity` | `<T>(values: borrow Vec<T>) -> usize` |
| `vec_get` | `core.vec.get` | `<T>(values: borrow Vec<T>, index: usize) -> T` |
| `vec_reserve_exact` | `core.vec.reserve-exact` | `<T>(values: own Vec<T>, additional: usize) -> Vec<T>` |
| `vec_set` | `core.vec.set` | `<T>(values: own Vec<T>, index: usize, value: T) -> Vec<T>` |
| `vec_clear` | `core.vec.clear` | `<T>(values: own Vec<T>) -> Vec<T>` |

The frozen `semaprax.prelude.v2` contract remains the original five-operation
Vec surface byte for byte. A program selects additive
`semaprax.prelude.v3` only when it uses `vec_reserve_exact`, `vec_set`, or
`vec_clear` (directly or through an authenticated wrapper); v3 adds exactly
those declarations and their status facts. Programs using only the original
five operations and none of the three newly reserved intrinsic names or
identities retain their previous prelude binding bytes and digests.

Every call spells its one type argument explicitly. `vec_with_capacity` accepts
any `usize` expression. A literal greater than 8192 is rejected statically; a
dynamic value greater than 8192 selects sticky `semaprax.vec.v1` code 3. The
target-neutral capacity charge is eight bytes per element and therefore never
exceeds 65536 bytes. `vec_push`
requires `vec_len(values) < vec_capacity(values)`. `vec_reserve_exact` sets
capacity to `max(old_capacity, vec_len(values) + additional)` without changing
length or initialized elements. The resulting capacity remains bounded by
8192; overflow or a larger requested result selects code 3. `vec_get` and
`vec_set` require `index < vec_len(values)` and reuse code 2 on failure.
`vec_clear` sets length to zero and retains capacity. Target runtimes recheck
these invariants.

Dynamic failures are sticky in domain `semaprax.vec.v1`: code 1 is push at
full capacity, code 2 is get or set outside the initialized length, and code 3
is an observable construction or reserve allocation failure. Cleanup cannot
replace the selected status.

## Ownership and mutation

A vector is one non-Copy owner regardless of its element count. Its Copy
elements add no child finalizers. `vec_push`, `vec_reserve_exact`, `vec_set`,
and `vec_clear` consume that owner and return its next generation. Push adds one
initialized element, reserve may replace the backing allocation while
preserving initialized elements, set replaces one Copy element, and clear
forgets all initialized Copy elements while retaining capacity. None duplicates
the carrier.

The first loop-carried profile is exact: a mutable vector is initialized once
outside one bounded `while`, the body assigns that same binding exactly once
from `vec_push<T>(binding, value)`, and no borrow or other use of the vector
crosses the assignment. The right-hand side evaluates before publication. At
the call commit the old binding transfers to staged argument storage; success
transfers the returned owner into the next binding generation. Failure before
commit leaves the old owner live, and every exit after commit settles whichever
single generation is live. General owned assignment remains rejected.

`vec_len`, `vec_capacity`, and `vec_get` borrow synchronously. Their loans do
not escape the call. Binding assignment while any other lexical loan is live is
rejected.

## Standard-library surface

The alloc-tier `std.collections` package authors exactly eight authenticated
transparent aliases:

| Stable identity | Intrinsic |
| --- | --- |
| `std.collections.vec.with-capacity` | `core.vec.with-capacity` |
| `std.collections.vec.push` | `core.vec.push` |
| `std.collections.vec.len` | `core.vec.len` |
| `std.collections.vec.capacity` | `core.vec.capacity` |
| `std.collections.vec.get` | `core.vec.get` |
| `std.collections.vec.reserve-exact` | `core.vec.reserve-exact` |
| `std.collections.vec.set` | `core.vec.set` |
| `std.collections.vec.clear` | `core.vec.clear` |

Each wrapper forwards only its own explicitly supplied type parameter and
preserves the intrinsic operation and status identity in HIR and Graph. The
package conformance source instantiates all eight aliases for every admitted
Copy scalar. The package exports no public ABI; ordinary authored generic
functions still cannot stand in for these authenticated aliases.

## Current gate and promotion evidence

- canonical source and Graph projection plus exact HIR nominal, operation, type
  argument, ownership, and materialized-wrapper identities for all eight Copy
  scalars;
- stable rejection of inference, missing/surplus arguments, statically
  oversized literal capacity, unsupported element types, nested vectors, forged
  intrinsic identities, and broader owned assignment;
- independent cleanup replay for construction, push, loop-carried replacement,
  precondition failure, postcondition failure, and hostile transition/order/
  liveness mutations;
- repeated focused local interpreter, native C11 O0/O2, and Core-Wasm execution
  for empty, full, push/get/len/capacity, exact reserve, set, clear, loop-carried
  growth, and exact failures, with no shallow owner copy and exact observed
  capacities;
- focused local canonical-source/HIR evidence for bounded `for` traversal over
  every Copy scalar, plus empty, singleton, multi-element, full-capacity,
  repeated re-entry, and body-failure execution on the interpreter, native C11
  O0/O2, and Core-Wasm;
- frozen prelude-v1/v2 contract bytes and digests, plus native and Core-Wasm
  reachability checks proving legacy Vec source does not emit v3 helpers; and
- the `std.collections` manifest, scalar-result example, eight-scalar conformance
  source, bundled dependency entry, closed package metadata, focused local
  Project/package selectors, and byte-exact generated catalogs. This promotes
  only that exact package slice locally; the broader collection and
  hosted-support nonclaims below keep the module Partial; and
- the committed `examples/vector-stats-project` accumulate-and-filter example
  project, whose entry and conformance modules both return `0` on those same
  three engines and whose accumulating function is driven at seven element
  counts and three thresholds, so the loop-carried profile is exercised from
  committed source rather than from a hand-built plan.

## Nonclaims

There is no `pop`, insertion, removal, implicit or amortized growth, shrink,
owned element, general iterator, iterator object, closure adapter, escaping borrow, mutable reference,
public Project/FFI/WIT/Component representation, hosted promotion, or production
support. `std.iter` remains blocked on its independent interface,
associated-type, closure, and lifetime contracts. The separately versioned
[Owned Bounded Box v1](OWNED-BOUNDED-BOX-V1.md) owns the later, narrow
`std.mem` allocation slice; Vec traversal does not imply it.
