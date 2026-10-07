# Ordinary Vec Loop Renewal v1

Audience: language, ownership, cleanup, and backend contributors.

Status: implemented additive private profile with focused local interpreter, native O0/O2, Core Wasm and hostile-replay evidence.

Audience: compiler contributors maintaining vector ownership and loop cleanup plans.

This profile admits same-cell renewal of a mutable concrete scalar `Vec<T>`
inside an ordinary `while` body, including updates inside its `if` branches.
An untouched second owner no longer changes whether that exact update is
admitted. It uses the existing Vec operations and status domains; it adds no
source syntax, prelude operations, ABI, allocation authority, or capacity.

## Exact admission

`T` is exactly `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, or `bool`.
The destination is a whole local declared with `let mut`; its type and owned
mode equal the right-hand side. The RHS is a direct compiler-owned
`vec_push<T>`, `vec_set<T>`, `vec_clear<T>`, or `vec_reserve_exact<T>` call with
exact arity and explicit concrete type argument. Argument zero is the same
whole owned binding, without projections. Other arguments have the operation's
exact Copy types. User calls, wrapper instances, different source owners,
owned payloads, arbitrary RHS blocks, and projected assignments do not select
this profile. Existing source/HIR type, effect, loan and loop checks still apply.

```spx
module ordinary_vec_renewal;
@id("ordinary.main") fn main()->i64 {
    let mut values = vec_with_capacity<i64>(2usize);
    let mut unused = vec_with_capacity<i64>(1usize);
    let mut index = 0;
    while index < 2 {
        values = vec_push<i64>(values, index);
        index = index + 1;
        0
    }
    if vec_len<i64>(values) == 2usize && vec_len<i64>(unused) == 0usize { 7 } else { 0 }
}
```

The authenticated consuming `for own` lowering is excluded. Its conditional
[Owning Iterator Renewal v1](OWNING-ITERATOR-RENEWAL-V1.md) selects its frozen
CleanupPlan v12/Graph v40 protocol. An ordinary nested `while` has its own
context; consuming traversal itself does not gain unconditional renewal.

## Canonical proof

An affected function selects `semaprax.cleanup-plan.v15`. Before evaluating the
RHS, `ReserveRenewal(at, binding)` reserves its one live Vec leaf's position in
canonical initialization history. Arguments stage left to right and transfer
together at the existing commit boundary. `Renew(at, source, destination)`
transfers the successful replacement back to that position. Unrelated live
owners must retain their exact history; neither constructor nor replay sorts
or repairs a vector. A branch that skips the update retains the same history.
Zero iterations execute no update.

Failure cancels the reservation without resurrecting the old owner. Existing
staged/committed ownership and failure cleanup settle the actual live values
once, preserving the selected Vec status (1 capacity, 2 index, 3 allocation).
Postconditions, non-result cleanup, and result publication retain their existing
order. Reservations and history materialization use the existing replay budget;
that budget measures charged replay units, not peak heap allocation. Existing
source, loan, call-depth, Vec capacity and runtime authority limits remain fixed.

Construction and independent replay each derive sites from checked HIR,
including the mutable declaration and exact intrinsic argument place.
Reservations, successful transfers, destination identity and schema selection
are independently authenticated. Missing reservation, ordinary-transfer
substitution, wrong source/destination or schema downgrade fails `SPX-H006`.

## Graph and backend boundary

Affected modules select Graph v66. The final wrapper preserves every preceding
admitted projection and appends `vec_loop_renewal`, with schema
`semaprax.vec-loop-renewal.v1` and ordered `updates` entries containing
`function`, RHS expression `at`, and destination `binding`. These are structural
site metadata; cleanup-plan vectors keep canonical runtime order. Earlier graph
composition refusals remain closed. Programs without an ordinary renewal keep
their previously selected schemas and bytes. Prelude and LoanPlan versions do
not change. Trace-path certificate v1 refuses the new loop cleanup schema.

Interpreter, native C11 and Core Wasm consume the already validated renewal
transitions; this profile grants no backend exception. The owning executable
gates are `owned_data::vec_loop_renewal` (source/canonical graph, interpreter,
native O0/O2 and repeated Wasm settlement) and library
`cleanup_plan::replay::renewal::tests` (v15 hostile proofs and frozen v12 control).
They cover untouched versus updated secondary owners, conditional and skipped
updates, all four operations and failure settlement. Those focused gates pass locally; this evidence does not claim hosted cross-target execution.
