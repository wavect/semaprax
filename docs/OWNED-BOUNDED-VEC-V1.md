# Owned Bounded Vec v1

Audience: language users, standard-library authors, and compiler contributors.

Status: implementation tranche. This document owns one internal, explicitly
instantiated `Vec<T>` profile for Copy scalar elements. It defines no public
aggregate ABI and does not implement Iterator.

## Exact profile

`Vec<T>` is the compiler-owned nominal type with stable identity `core.vec`.
`T` is exactly one of `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, or
`bool`. Type inference, `Bytes`, `String`, authored aggregates, variants,
resources, nested vectors, and nonconcrete element types are rejected.

The compiler-owned operations are:

| Source | Stable identity | Signature |
| --- | --- | --- |
| `vec_with_capacity` | `core.vec.with-capacity` | `<T>(capacity: usize) -> Vec<T>` |
| `vec_push` | `core.vec.push` | `<T>(values: own Vec<T>, value: T) -> Vec<T>` |
| `vec_len` | `core.vec.len` | `<T>(values: borrow Vec<T>) -> usize` |
| `vec_capacity` | `core.vec.capacity` | `<T>(values: borrow Vec<T>) -> usize` |
| `vec_get` | `core.vec.get` | `<T>(values: borrow Vec<T>, index: usize) -> T` |

Every call spells its one type argument explicitly. `vec_with_capacity` accepts
any `usize` expression. A literal greater than 8192 is rejected statically; a
dynamic value greater than 8192 selects sticky `semaprax.vec.v1` code 3. The
target-neutral capacity charge is eight bytes per element and therefore never
exceeds 65536 bytes. `vec_push`
requires `vec_len(values) < vec_capacity(values)` and `vec_get` requires
`index < vec_len(values)`; target runtimes recheck both invariants.

Dynamic failures are sticky in domain `semaprax.vec.v1`: code 1 is push at
full capacity, code 2 is get outside the initialized length, and code 3 is an
observable allocation failure. Cleanup cannot replace the selected status.

## Ownership and mutation

A vector is one non-Copy owner regardless of its element count. Its Copy
elements add no child finalizers. `vec_push` consumes that owner and returns the
same logical allocation with one additional initialized element; it never
duplicates the carrier.

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

The alloc-tier `std.collections` package remains closed. Its future generic
wrappers must preserve the intrinsic identity in HIR and Graph and forward only
their own exact type parameter. That requires an authenticated transparent
wrapper profile; ordinary authored generic functions cannot stand in for it.

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
- repeated interpreter, native C11 O0/O2, and Core-Wasm execution for empty,
  full, push/get/len/capacity, loop-carried growth, and exact failures, with no
  shallow owner copy and exact/+1 allocator evidence; and
Promotion beyond this internal profile additionally requires the ordinary
`std.collections` manifest, example, conformance test, generated catalogs, and
Project check/test/run gates on all three listed targets.

## Nonclaims

There is no `set`, `pop`, insertion, removal, reserve/growth beyond the initial
capacity, owned element, iterator, closure adapter, escaping borrow, mutable
reference, public Project/FFI/WIT/Component representation, hosted promotion,
or production support. `std.iter` remains blocked on its independent interface,
associated-type, closure, and lifetime contracts. `std.mem` is not created by
this tranche.
