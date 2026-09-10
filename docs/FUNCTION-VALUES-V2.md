# Function Values v2: generic collection callbacks

Status: implemented private callback profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).

Audience: language users, compiler contributors, collection-adapter authors,
and backend implementers.

This additive prerequisite advances reusable collection adapters. It does not
introduce iterator objects, consuming `next`, captures, or a public callable ABI.
Those implemented iterator and scalar-closure additions retain their separate
contracts rather than changing this callback profile.

## Scoped callable signature

The existing private, single-parameter compiler collection profile may receive
value parameters of type `fn(T) -> T`, `fn(T) -> bool`, or another signature of
zero through eight scalar leaves. Each leaf is an existing Copy scalar or the
same declaration's sole type parameter. The function must otherwise satisfy the
existing `Box<T>`/`Vec<T>` collection profile. Callable return slots and callable
generic argument substitutions remain closed.

Every concrete substitution independently checks the exact scalar callable
signature from Function Values v1. References still identify monomorphic,
effect-free local declarations; generic target references remain unsupported.
Callables carry no environment or owner in this profile. Invocation evaluates
arguments once, left to right, and propagates the selected target's ordinary
checked failures.

## Reusable adapters

A private `map<T>(own Vec<T>, fn(T)->T)->Vec<T>` can snapshot length, allocate one
bounded output vector, and call the callback once per element in ascending index
order. `filter<T>(own Vec<T>, fn(T)->bool)->Vec<T>` preserves accepted order.
`fold<T>(own Vec<T>, T, fn(T,T)->T)->T` threads an accumulator from left to right.
These are ordinary source functions assembled from existing vector operations,
not new intrinsic names or authenticated standard-library aliases.

The collection template profile additionally preserves mutable local assignment
and bounded `while` statements through generic materialization. Template replay
checks exact scoped types, statement paths and target identities. Each concrete
instance then undergoes ordinary HIR, loop, ownership, cleanup and backend checks.
A failed callback retains its status and settles input/output owners using the
existing canonical cleanup plan. Empty input executes no callback.

## Projections and boundaries

Canonical source retains authored function types and generic calls. Materialized
HIR retains genuine invocation operands and exact concrete signatures; Graph v36
includes the generic instance and compatible callable target facts together.
Unmaterialized callable templates also select Graph v36. The runtime target
universe scans ordinary functions and materialized instance bodies, returning
only monomorphic reference targets; a template is not an additional executable
instance. Candidate cycle replay uses exact instance identities.
Native uses typed context/status/output function pointers. The Wasm aggregate
lane uses its existing status/output-pointer ABI in the indirect-call table and
snapshots the callable and each argument before evaluating the next operand.
No new prelude operation, host import, vector layout, public descriptor,
or workspace export is introduced. Function Values v1 programs retain their
existing representation.

Focused language evidence lives in `function_values_generic`; all-scalar adapter
execution and ownership settlement belong to the owned-data generic collection
harness. Success, empty input, callback failure and hostile template mutation
have hosted-green release evidence. Native allocation accounting and Wasm
generation tracking require zero live owners after repeated success and callback
failure. Earlier local executions retain their original scope.

Public package use and general iterator interfaces remain unfinished. Scalar
captures and generic/loop closure construction are implemented in
[Closures v1](CLOSURES-V1.md) and [v2](CLOSURES-V2.md); consuming traversal
and generic iterator operations likewise have their own versioned profiles.

SemanticProgram v4 also retains checked source graphs for unused generic
callback templates omitted from executable reachability. Its focused workspace
check covers exact replay and comment-only semantic stability; the
[canonical workspace contract](CANONICAL-SEMANTIC-WORKSPACE-REVISION-V1.md) owns
that additive projection. It grants no runtime reachability or authority.
