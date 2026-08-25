# Bounded While-Loops v1

Status: Partial — v1 adds statement-level `while` loops over an explicitly
admitted Copy-scalar profile. Indexed Byte Loop v2 additively admits one exact
cleanup-inert `byte_get`/`Option<u8>` read profile. The broader control-flow
row of [COMPLETION-MATRIX.md](COMPLETION-MATRIX.md) remains Partial.

## Objective

SEMAPRAX previously expressed iteration only through recursion. This tranche
adds the smallest end-to-end loop slice that keeps every cleanup, ownership,
and backend invariant intact:

```text
while <condition> { <body> }
```

`while` is a **statement** form inside blocks, exactly like `let`, assignment,
and `unsafe` boundaries. It produces no value; statements never do.

## Syntax and canonical form

The condition excludes record literals exactly like `if` conditions. The body
is an ordinary block, and because every block in SEMAPRAX requires a final
value expression (`SPX-P203` otherwise), an admitted while body ends with a
value expression whose value the loop discards:

```text
while remaining > 0 {
    total = total + remaining % 10;
    remaining = remaining / 10;
    remaining > 0
}
```

The canonical formatter renders the header inline (`while <cond> {`) and the
multi-line body with standard indentation; nested single-line projections use
the same spelling. Programs without while syntax format byte-for-byte
identically to pre-feature output.

## Semantics

1. The condition must be exactly `bool` (`SPX-T251`); it re-evaluates before
   every iteration.
2. The body executes zero or more times. Evaluation stays strictly
   left-to-right; lazy `&&`/`||` operands still evaluate only when required.
3. Checked-arithmetic and propagated-call failures inside the condition or
   body select their exact normalized status at the failing expression and
   propagate out of the enclosing function immediately — identically to the
   same expression unrolled. Failure selection stays sticky and
   poison-preserving; no loop construct can replace or clear a status.
4. There are no `break` or `continue` forms in v1, and termination is NOT
   statically proven. Compiled programs loop natively (C11 `for (;;)`,
   Wasm `block`/`loop`/`br_if`); interpreted programs run under the existing
   fuel budget and fail closed with the existing exhausted-budget outcome.

## Admitted profile (v1 only)

Loop conditions and bodies may contain only Copy-scalar operations:

- scalar literals and names (`i64`, `i32`, `u8`, `char`, `f32`, `f64`, `bool`);
- checked scalar arithmetic, comparisons, and lazy boolean operators;
- nested `if`s over scalars;
- blocks with scalar statements, scalar `let` bindings (including `let mut`),
  and existing simple local assignment;
- monomorphic calls to functions whose parameters are all by-value scalars
  and whose result is a scalar;
- nested `while` statements obeying the same rules.

Everything else is rejected at compile time with `SPX-T252` rather than
approximated: record construction/update/projection, variant construction,
match expressions, postfix `?` propagation, string literals, method calls,
generic calls, calls with non-scalar signatures or ownership modes, unsafe
boundary statements, and any ownership change to an outer binding inside the
loop. Records whose fields are all Copy scalar are also rejected in v1 even
though their construction adds no cleanup edges today; rejecting is the
conservative choice until aggregate lowering inside loops is separately
evidenced.

This restriction means admitted loops contribute **zero** new cleanup slots,
transitions, or finalizers: the CleanupPlan v2/v3 schema set, the plan
builder, the independent replay gate, and every serialized plan for programs
without while syntax are byte-identical to pre-feature output.

## Indexed Byte Loop v2

The additive v2 profile admits immutable indexed byte inspection without
opening general aggregate execution inside loops. A loop condition or body may
call only the compiler-owned `byte_len` and `byte_get` byte operations in
addition to the v1 forms. The slice argument must already be an authenticated
non-escaping `Slice<u8>` place; constructing a view, copying `Bytes`, or
performing any allocation in the loop remains rejected.

The sole admitted aggregate expression is an exhaustive, guard-free match over
the direct result of compiler-owned `byte_get` with the exact authenticated
compiler-prelude `Option<u8>` instance. It has exactly the `Option::Some {
value: <u8 binding> }` and `Option::None {}` cases, and both arm results must
remain Copy scalars admitted by the surrounding loop profile. Out-of-range
indices are ordinary total reads selecting `None`; they are not diagnostics or
target traps. General variant matches, aliases of authored `Option<u8>`,
guards, nested aggregate patterns/results, `?`, owned data, imports, effects,
and calls that can allocate or alter cleanup liveness remain closed.

The indexed match itself adds no storage leaf, cleanup action, failure source,
or plan back-edge. Source resolution and hostile-HIR validation independently
authenticate the byte-operation identity, exact carrier/member identities,
field binding type, arm inventory, result type, and recursively admitted arm
expressions before existing interpreter, native, or Wasm match lowering runs.

## Cleanup-plan contract for loops

The cleanup CFG remains acyclic (the independent replay gate authenticates
acyclicity). An admitted while statement therefore lowers to one linearized
iteration: the condition's Boolean result branches into one body pass or the
loop continuation, and the builder fails closed if that pass could change
owned liveness. Under the Copy-scalar admission profile this representation
is observationally exact for every cleanup event — loop entry liveness equals
body liveness equals loop exit liveness, so every failure exit finalizes
exactly what was live on loop entry regardless of iteration count. General
loops with owned storage require the RFC 0003 phase 7 extension before any
plan may contain a back-edge.

## Graph projection

Graph serialization gains one additive `"kind":"while"` node per loop
(`"condition"` plus `"body"`), and program-level schema selection gains
**Graph v15**, selected above the whole v10–v14 lattice exactly when an
authenticated while node exists in some function body; wildcard-free
programs without while syntax keep their exact previous schema and bytes
(pinned digest evidence). Module, Agent Context, and bounded-context
projections inherit the schema string unchanged.

Patch/evidence flows stay fail-closed: their verifiers authenticate only
`semaprax.graph.v10`–`v14`, so generation refuses v15 sources up front
(`SPX-G410`) rather than emitting capsules that replay would reject.

## Diagnostics (family SPX-T2xx)

| Code | Meaning |
| --- | --- |
| `SPX-T251` | `while` condition is not exactly `bool`. |
| `SPX-T252` | A construct appears inside a while condition/body outside the v1 admission profile, or ownership of an outer binding changes inside a loop. |
| `SPX-T253` | A `while` statement appears inside a contract expression (`requires`/`ensures`). |
| `SPX-G410` | Generation of patch/workspace-evidence artifacts from a Graph-v15 (while-loop) source; those flows admit only v10–v14. |

Unknown names inside loops reuse the established diagnostics
(`SPX-T202`/`SPX-H002`); immutable-target and type-mismatch assignment errors
remain `SPX-U101`–`SPX-U106`.

## Layer behavior

- **Parser/AST**: statement recognition of `while`; `Statement::While`
  carries condition and body plus child traversal helpers so every walker
  sees both evaluated expressions.
- **Canonical formatter**: header-inline rendering with the shared block
  machinery; budgets account for both children; non-while bytes unchanged.
- **Resolver/HIR**: `ResolvedStatement::While { condition, body }` with
  canonical `.s<N>.condition` / `.s<N>.body` identity paths; the iterative
  resolver and its recursive oracle twin perform identical admission scans
  and typing checks. Generic templates reject loops outright.
- **Verifier**: the iterative verifier and its recursive oracle emit
  T251/T252/T253 in the same order; ownership drift across a loop body is
  rejected fail-closed.
- **HIR validation**: hostile-HIR re-checks the admission profile
  structurally (`SPX-H006` on violation), requires an exact `bool`
  condition, and requires body-exit liveness to equal loop-entry liveness.
- **Cleanup inventory/plans**: loops add no droppable bindings; the builder
  linearizes one admitted iteration with a state-equality guard; the
  independent replay census counts the branch/skip multiplicity explicitly.
- **Native C11**: `for (;;) { <cond>; if (!(cond)) break; <body> }` keeps
  per-iteration condition re-evaluation; checked operations jump to the
  shared epilogue on failure exactly as straight-line code does. O0/O2
  outputs are observably identical.
- **Wasm**: `block $exit { loop $top { <cond> i32.eqz br_if 1 <body> drop
  br 0 } }`; failures use the same host imports/traps as straight-line code.
- **Interpreter**: per-node fuel charging bounds every iteration; exhaustion
  reports the existing fail-closed exhausted outcome. No admission widening
  was required beyond accepting the statement itself.

## Evidence

`tests/while_loops_v1.rs` pins canonical round-trips, deterministic Graph-v15
serialization plus the untouched non-while byte pin, CleanupPlan structural
equality against unrolled equivalents, stable regressions for every new
diagnostic, native C11 O0/O2 success and condition-dependent
division-by-zero status parity, Node/Wasm equivalence including the same
normalized failure, interpreter agreement, and a fuel-exhaustion fail-closed
case. `examples/while_loops.spx` exercises digit-summing and factorial loops
under the example check/fmt gates.

## Non-claims

This tranche does not claim `break`/`continue`, static termination proofs,
loops over aggregates/resources or any non-Copy content inside loops, field
mutation, cleanup-plan back-edges, generic-template loops, patch/evidence
admission for while-bearing programs, property-test generation inside loops,
or any hosted-CI promotion beyond the gates listed above.
