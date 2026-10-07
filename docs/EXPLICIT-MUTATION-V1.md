# Explicit Mutation v1

Audience: language users, tool authors, and compiler contributors.

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

## Objective

SEMAPRAX remains immutable by default. This profile adds a restricted,
end-to-end way to mutate local values explicitly; other mutation forms remain
closed:

- Local bindings may declare mutability with a `mut` modifier directly after
  `let`: `let mut total = 0;`. Plain `let` stays immutable.
- A new statement form `<binding> = <expr>;` stores the fully evaluated value
  of `<expr>` into an existing mutable local.

## Syntax and canonical form

```text
let mut x = expr;      // mutable local binding
x = expr;              // assignment statement
```

The canonical formatter renders both forms exactly as written above; inside a
block each statement ends with `;`, and single-line projections use the same
spelling (`{ let mut x = 1; x = x + 1; x }`). There are no compound
assignment operators (`+=` and friends do not exist), no assignment
expression, no destructuring assignment, and no `mut` parameters. Because the
expression grammar admits no `=`, `(x = 2)` fails to parse
(`SPX-P106`) and `x = y = 3;` cannot chain.

## Semantics

Parameters, match bindings, contract bindings and plain `let` bindings remain
immutable. Assigning to any of them produces a compile-time diagnostic. The
supported assignment semantics are restricted:

1. The assigned value is evaluated completely before the store; checked
   arithmetic failure statuses propagate exactly as they would from the same
   expression in an initializer position, and execution order stays strictly
   left-to-right.
2. Types must match the binding type exactly; there is no implicit
   conversion between scalar widths or between scalars and aggregates.
3. The target reuses the original `let` binding's stable [`ValueId`](RFC-0001.md);
   assignments create no new value identity, and re-declaration through
   shadowing rules is unchanged.

## Admitted slice (Explicit Mutation v1 only)

Assignment targets and assigned values must be checked Copy scalar values:
`i64`, `i32`, `u8`, `char`, `f32`, `f64`, or `bool`, with value ownership.
Other values are rejected at compile time, not given approximate semantics:

- No field mutation (`p.x = ...`), no record/variant replacement-in-place;
  `with { .. }` stays a pure copy-producing update.
- No collection mutation (no collections exist yet).
- No reference/mutable-borrow semantics and no escaping-store effects; there
  is no shared or aliasing model to reason about.
- No cross-task or concurrency/memory-model claims of any kind.
- No mutation inside contract expressions (`requires`/`ensures` stay pure).

## Diagnostics (family SPX-U1xx)

| Code | Meaning |
| --- | --- |
| `SPX-U101` | Assignment targets an immutable binding (declare it `let mut`). |
| `SPX-U102` | Assigned value type does not exactly match the binding type. |
| `SPX-U103` | `mut` appears outside a local `let` (parameters are immutable). |
| `SPX-U104` | Duplicate `mut` modifier (`let mut mut x`). |
| `SPX-U105` | Target or value is outside the v1 slice (non-scalar, non-Copy, or non-value ownership), other than the admitted same-owner reopens (`vec_push`, `bytes_set`, the [Owned String Loops v1](OWNED-STRING-LOOPS-V1.md) append `text = string_concat(text, more)`, and the [String Collections v1](STRING-COLLECTIONS-V1.md) map updates `counts = map_add(counts, key, n)` / `map_set`). |
| `SPX-U106` | Assignment statement inside a contract expression (`requires`/`ensures`). |

Unknown assignment names reuse the established unknown-value diagnostic
(`SPX-T202`); unresolved names during resolution report `SPX-H002`.

## Layer behavior

- **Parser/AST**: statement-level recognition of assignments via a two-token
  lookahead (`Ident` followed by `=`); `Statement::Assign` joins
  `Statement::Let { mutable }`.
- **Canonical formatter**: renders `let mut` and bare-target assignments with
  exact byte budgets; programs without mutation syntax format byte-for-byte
  identically to pre-feature output.
- **HIR**: `ResolvedStatement::Assign` carries the target's original
  `ResolvedBinding`; resolver scopes track per-binding mutability and enforce
  U101/U102/U105 fail-closed before any backend runs. Both the iterative
  resolver and its recursive oracle twin implement identical checks and agree
  on diagnostics.
- **Graph**: `statement_json` emits `"mutable":true` only on `let mut`
  bindings and a new additive `"kind":"assign"` node naming its reused target
  id. Graph schema selection (v10-v14) ignores mutation-only programs, and
  every pinned graph digest for non-mutation programs is unchanged.
- **Cleanup**: straight-line scalar mutation lowers its RHS exactly like an
  initializer and adds no cleanup structure; CleanupPlan v2 output for a
  mutation function equals the initializer-only equivalent structurally
  (asserted modulo function-name prefixes in tests).
- **Native C11**: plain local variables and plain C11 store statements; O0
  and O2 produce identical observable results including checked-arithmetic
  failure statuses. A read of a `let mut` binding whose type is a Copy scalar,
  or a record or class made only of such values, is copied into a fresh
  temporary at its own evaluation point. An earlier binary or comparison
  operand, or an earlier call argument, therefore keeps the value it read
  when a later operand's nested block assigns to the same binding. Reads of
  immutable bindings stay plain aliases, and their emitted C is unchanged.
- **Wasm**: the core scalar lane stores with `local.set` after full RHS
  evaluation; i32 overflow detection traps identically for initializer and
  assignment positions. Aggregate lanes reuse existing slots and reject
  anything outside the scalar slice.

## Evidence

`tests/language/explicit_mutation.rs` pins canonical round-trips, all six U-family
diagnostics plus statement-only grammar regressions, deterministic Graph JSON
with a byte-exact non-mutation digest pin, CleanupPlan structural equality,
native C11 O0/O2 probes (success values and assigned-overflow failure
statuses), and Node/Wasm equivalence including overflow trapping. Its
`read_order` submodule (`tests/language/explicit_mutation/read_order.rs`)
compares the interpreter, native C11 at O0 and O2, and Core Wasm when a later
operand or argument assigns to a binding that an earlier one read. It covers
every admitted Copy scalar type, a mutated record field, a whole Copy record
argument, lazy `&&` and `||`, and addition overflow selected by the values
that were read, both when the overflow is real and when only a later store
would have caused it.
`examples/explicit_mutation.spx` exercises the feature under the example
check/fmt gates.

## Non-claims

This tranche does not claim field/aggregate mutation, collection mutation,
reference or mutable-borrow semantics, concurrency or memory-model rules,
cross-task mutation, closures, or any hosted-CI promotion beyond the gates
listed above.
