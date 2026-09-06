# Owned Bounded Vec For Traversal v1

Audience: language users, standard-library authors, and compiler contributors.

Status: bounded implementation tranche. This document owns one source-level
traversal form over the internal [Owned Bounded Vec v1](OWNED-BOUNDED-VEC-V1.md)
profile. It is not a general Iterator design and adds no standard-library
declaration or public aggregate ABI.

## Exact source profile

The admitted form is:

```spx
for item in values {
    body
}
```

`values` names one simple immutable binding whose exact type is `Vec<T>`, where
`T` is one of the existing Copy scalars: `i64`, `i32`, `u8`, `usize`, `char`,
`f32`, `f64`, or `bool`. The source expression is a binding name, not a call,
field projection, index, block, conditional, match, or other computed
expression. The item binding is immutable, fresh in the loop body, and has type
`T`.

The loop snapshots `vec_len<T>(values)` once before its first iteration, then
visits indices `0usize` through `len - 1usize` in ascending order, exactly once
each. The source vector stays frozen for the whole traversal: the body cannot
move, replace, mutate, or otherwise transfer it, and no loan from the loop may
escape. Each iteration evaluates the body for its effects and discards its
result. Empty vectors execute the body zero times.

## Lowering and compatibility

The resolver lowers the admitted source form to the already checked HIR
vocabulary: one length snapshot, an ordinary mutable `usize` counter, a
bounded `while`, and one `vec_get<T>` per iteration. This is a resolver-owned
desugaring, not a new HIR expression. It introduces no stable identity, graph
or cleanup schema, prelude declaration or version, status domain, backend
operation, standard-library declaration, package surface, or public
Project/FFI/WIT/Component ABI.

The synthetic operations preserve the existing `core.vec.len` and
`core.vec.get` identities and the existing `while` semantics. Source
formatting remains source preserving: canonical formatting retains the `for`
form rather than printing its lowering. Existing source, HIR, Graph, cleanup,
prelude, backend, package, and ABI bytes remain unchanged when the form is not
used.

## Focused local evidence

The current worktree passes:

```sh
cargo test --locked -p semaprax --test language vec_for
cargo test --locked -p semaprax --test owned_data for_each_copy_scalars_preserves_order_settlement_and_reentry_on_every_engine
```

Together those focused selectors establish:

- canonical parse/format round-trip of the `for` source form and exact
  resolver lowering to the existing length/get/while vocabulary;
- singleton ascending traversal for all eight Copy scalar instantiations plus
  empty, multi-element, and full-capacity traversal for a representative
  scalar, with the body result discarded;
- evaluation of the length snapshot once, source-vector liveness through every
  iteration, and one ordinary final settlement after the loop;
- interpreter, native C11 `-O0`/`-O2`, and internal Core-Wasm parity for
  representative success and body-failure cases; and
- stable rejection of mutable or non-simple sources, unsupported element
  types, attempts to consume or rebind the source, item reassignment, and
  nested traversal.

Hosted passage remains required. The completion rows therefore stay Partial
and this profile remains local and unhosted. Because the source form lowers
entirely into the existing HIR vocabulary and carries no origin marker, HIR
validation applies the ordinary checks for those existing nodes rather than a
new traversal-specific canonical-shape rule.

## Nonclaims

This profile does not provide an iterator object or interface, `IntoIterator`,
`next`, ranges, enumeration values, adapters, `map`, `filter`, `fold`,
`collect`, closures or first-class functions, associated types, lifetime
inference, mutable references, escaping borrows, consuming iteration, owned or
aggregate elements, mutation during traversal, arbitrary iterable expressions,
early `break`/`continue`, a public generic ABI, or hosted/production support.
It does not satisfy `std.iter`; that package remains Missing pending its own
interface, associated-type, closure, and lifetime contracts.
