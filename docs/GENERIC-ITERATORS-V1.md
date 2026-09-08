# Generic Iterator Helpers v1

Status: focused source/HIR, graph, ProgramRoot, and cross-engine runtime checks
pass locally; exact-commit hosted promotion remains pending.

Audience: compiler contributors, reviewers, and agent authors.

This profile composes [Owning Iterators v1](OWNING-ITERATORS-V1.md) with
[Generic Compiler Collections v1](GENERIC-COMPILER-COLLECTIONS-V1.md) and
[Generic Closures v2](CLOSURES-V2.md). It admits private effect-free generic
helpers over the existing consuming iterator protocol.

## Checked signatures and bodies

A helper declares one explicit type parameter. Its concrete substitutions are
exactly the eight Copy scalars: `i64`, `i32`, `u8`, `usize`, `char`, `f32`,
`f64`, and `bool`. `Vec<T>`, `Box<T>`, `Iter<T>`, and `IterStep<T>` parameters
are owning; scalar and admitted scalar callback parameters are by value.
The result is an admitted scalar or one of those owning carriers. The type
parameter identity belongs to the declaring template at index zero; an equally
named parameter from another declaration cannot substitute for it.

Helpers may forward the exact argument vector to another admitted helper,
construct an iterator from a vector, advance an iterator, and consume a step.
An owning `Yield` match binds a Copy item and an owning iterator remainder.
A helper may return that same step type by explicitly reconstructing `Done`
or `Yield`, retaining the item's value and transferring the remainder once.
Helpers may invoke scalar callbacks and construct scalar snapshot closures
under the existing closure rules. The ordinary contract and cleanup rules
apply to the helper, callback invocation, and every staged owner.

An iterator or step is never Copy. Callback environments cannot capture an
iterator or step. No generic helper gains permission to copy an owner because
its element is a type parameter. Owning element substitutions, nested iterator
payloads, unconstrained type arguments, consuming loop syntax, lazy adapter
carriers, and public generic signatures remain outside this profile.

## Independent projections

Source verification checks the admitted concrete substitutions. HIR validates
scoped symbolic parameters independently, then validates every materialized
body and its exact concrete signature. Generic instance identity, forwarding
edges, ownership modes, cleanup paths, and selected cleanup schema remain
bound to the same ordered substitution and defining semantic revision.

This profile uses the existing Prelude v7, CleanupPlan v10, and Graph v38:
no new carrier, lifecycle, or runtime operation is introduced. Symbolic types
are template facts; only fully materialized concrete carriers reach cleanup
execution, the interpreter, native C11, or Core Wasm. ProgramRoot replay must
retain the generic closure and exact prelude declarations across private
workspace linking. Existing scalar, Box, Vec, graph, and cleanup contracts
remain unchanged. Public Project/package/FFI admission remains separate.

## Executable evidence

The focused corpus must exercise all eight substitutions together, transitive
helper calls, step reconstruction, preserved item values, empty/exhausted
iteration, discarded remainders, scalar callbacks and captures, helper failure,
and repeated exact owner settlement on the interpreter, native C11 O0/O2,
and Core Wasm. Projection tests must check generic identities and cleanup
selection, reject forged substitution/scoped identities, and reject ProgramRoot
replay against changed retained source. Local execution and exact-commit
hosted evidence must be recorded separately.

Focused tests use the `generic_iterator` filter in the library, `owned_data`,
and `workspace` harnesses. The named Linux owning-iterator selector also runs
these cases through its broader `iterator` filters.
