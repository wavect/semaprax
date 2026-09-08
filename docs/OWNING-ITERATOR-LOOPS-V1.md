# Owning Iterator Loops v1

Status: bounded implementation with focused local evidence; hosted promotion
is pending.

Audience: language users, compiler contributors, backend implementers, and
workspace-service authors.

This LANG-07 profile adds one consuming traversal form over the existing
private scalar iterator protocol:

```spx
for own item in iterator { body }
```

It is distinct from the frozen bounded-Vec traversal spelling `for item in
values { body }`. The older syntax, cache bytes, lowering, and contracts retain
their exact meaning.

## Admission and evaluation

The source expression must produce `own Iter<T>` and is evaluated once. It is
consumed at loop entry; the old source binding cannot be used after that
transfer. `T` is one of the eight Copy scalar types admitted by Owning
Iterators v1. `item` is an immutable per-iteration binding. The body may update
ordinary mutable scalar accumulators and use existing same-owner `Vec<T>`
assignment rules, but it cannot introduce a second owner for the iterator.
Conditional same-owner Vec renewal inside an authenticated `for own` body is
specified separately by [Owning Iterator Renewal v1](OWNING-ITERATOR-RENEWAL-V1.md).

The lowering has a hidden `IterStep<T>` slot. Each condition borrows that slot:
`Done` terminates and `Yield` enters the body. A `Yield` transfers its `rest`
iterator owner into the next hidden iteration state only after the body has
completed; the yielded item remains a Copy binding. The next iteration calls
`iter_next(rest)` exactly once. This preserves source order and requires exact
ownership equality at every loop boundary.

If the body fails, the selected status remains sticky and cleanup destroys the
remainder exactly once. A failing `iter_next` read occurs before its transfer
commit, so the staged iterator remains available to canonical failure cleanup.
No loop result is published on either failure path.

## Projections and compatibility

`CleanupPlan v11` is additive over v10 and records the hidden step/remainder
lifecycle required by this loop. `Graph v39` and ProgramRoot retain the checked
consuming-loop source shape, hidden ownership boundary, selected prelude, and
v11 cleanup facts. Earlier cleanup, graph, AST-cache, and canonical source
bytes remain frozen; the new AST cache carrier is additive and cannot reinterpret
the old `for` tag.

Graphs, caches, plans, and externally supplied lowering bytes carry no
authority. Source, HIR, cleanup replay, and each backend independently verify
the same ownership boundary before execution.

## Exclusions and required evidence

This v1 profile has no `break` or `continue`, owned payload elements, iterator
adapters, generic iterator implementations, or public iterator ABI. It does
not claim hosted support.

Focused evidence must cover all eight scalar types, empty and exhausted
iterators, ordered accumulation, source use-after-consume rejection, body and
`iter_next` failure settlement, repeated invocation, v11/Graph-v39 hostile
replay, and interpreter, C11 O0/O2, and Core Wasm equivalence before promotion.

The focused local iterator selector passes the eight-scalar interpreter,
C11 O0/O2, and Core-Wasm corpus, including captured generic callbacks,
vector accumulation, empty/multiple-yield loops, body-contract failure,
repeated settlement, and consumed-source rejection. The workspace selector
also verifies private generic loop instances and exact ProgramRoot/source
replay. These observations do not constitute hosted evidence.
