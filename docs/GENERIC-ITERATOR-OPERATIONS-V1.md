# Generic Iterator Operations v1

Status: private implementation with focused local runtime and replay evidence;
hosted promotion remains pending.

Audience: compiler contributors, reviewers, and agent authors.

This profile supports private authored operations over the consuming scalar
iterator protocol from [Owning Iterators v1](OWNING-ITERATORS-V1.md). The
consuming loop syntax remains owned by [Owning Iterator Loops v1](OWNING-ITERATOR-LOOPS-V1.md).

## Checked signatures

Each helper has exactly one or two explicit type parameters. Every type
parameter is independently substituted from the eight admitted Copy scalars:
`i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, and `bool`. The materialized
Cartesian substitution space is bounded to at most 64 combinations. Type
parameter identities are scoped to the declaring helper and retain their
explicit declaration order.

A parameter may be an owning compiler `Box`, `Vec`, `Iter`, or `IterStep`
whose element names either declared type parameter, or a scalar/callback value.
The result may be a scalar or one of those owning carriers. Helper names are
ordinary declarations, and forwarding may preserve or explicitly reorder the
type vector. Three or more type parameters remain outside this profile.

Representative authored signatures are:

```text
map<T, U>(own Iter<T>, capacity: usize, fn(T) -> U) -> Vec<U>
filter<T>(own Iter<T>, capacity: usize, fn(T) -> bool) -> Vec<T>
fold<T, A>(own Iter<T>, initial: A, fn(A, T) -> A) -> A
```

The iterator owner is consumed exactly once at entry. `map` produces the one
same-owner `Vec` result under existing capacity and element rules. `filter`
uses the conditional same-owner renewal path described by [Owning Iterator
Renewal v1](OWNING-ITERATOR-RENEWAL-V1.md) when it appends an accepted item.
`fold` transfers each accumulator value in source order and publishes the
final scalar only after the iterator remainder has settled. Callback
parameters and results use the existing scalar function-value profile, with
its zero-through-eight scalar-leaf arity ceiling; no callback receives an
iterator owner.

## Ownership and boundaries

Evaluation and callback invocation are left to right. The exact scoped owner,
type-parameter, and substitution identities are authenticated independently in
source and HIR. A callback failure or iterator failure retains the selected
status and settles every staged owner once. The helpers do not copy `Iter<T>` or
`IterStep<T>` and do not admit owned payload elements.

This profile adds no public ABI, package export, lazy adapter, associated type,
or general authored iterator implementation. `map` and `fold` use existing
Prelude v7, CleanupPlan v11, and Graph v39 meanings. Conditional `filter`
renewal uses additive CleanupPlan v12 and Graph v40 facts as specified by the
renewal profile. Earlier versioned contracts remain authoritative and
unchanged. Public signatures, hosted support, and broader iterator adapters
remain outside this profile.

## Evidence boundary

`tests/owned_data/generic_owned_function_runtime/iterator_operations.rs`
executes all 64 input/output scalar pairs through the interpreter, C11 O0/O2,
and Core Wasm. It checks map order, conditional filtering, order-sensitive
folds, empty folds, captured callbacks, explicit parameter permutation,
repeated invocation, and zero live allocation entries after settlement.
Separate cases cover callback/contract failure, exhausted output capacity,
output allocation refusal, and failure after staging an owner for renewal.
The canonical runnable example is [iterator-operations.spx](../examples/iterator-operations.spx).

`src/graph/iterator_operations_tests.rs` authenticates ordered argument and
scoped callback identities, changed-source rejection, exact v11/v12 selection,
and missing or downgraded renewal proof rejection. The workspace iterator
selector also passes: SemanticProgram v5 retains ordered instances through
checked callable closures, and ProgramRoot replay rejects changed source.
The existing Linux step
`Require Owning Iterators v1 identity, replay, and backend settlement` selects
these tests with the `iterator` filter in the library, owned-data, and workspace
harnesses. Local evidence does not imply an exact-head hosted result or a public
package/ABI support claim.
