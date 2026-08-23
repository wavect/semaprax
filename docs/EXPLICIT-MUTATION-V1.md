# Explicit Mutation v1

Status: Partial — the Immutable-by-default values and explicit mutation row of
[COMPLETION-MATRIX.md](COMPLETION-MATRIX.md) moves from Missing to Partial on
the strength of this document plus `tests/explicit_mutation_v1.rs`.

## Objective

SEMAPRAX is immutable by default. This tranche adds the smallest end-to-end
language slice for explicit mutation of local values while keeping every
other mutation surface closed:

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

Immutable by default remains the rule: every parameter, match binding,
contract binding, and plain `let` is immutable, and assigning to one is a
compile-time diagnostic. Assignment semantics are deliberately narrow:

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
Everything else is rejected at compile time rather than approximated:

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
| `SPX-U105` | Target or value is outside the v1 slice (non-scalar, non-Copy, or non-value ownership). |
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
  failure statuses.
- **Wasm**: the core scalar lane stores with `local.set` after full RHS
  evaluation; i32 overflow detection traps identically for initializer and
  assignment positions. Aggregate lanes reuse existing slots and reject
  anything outside the scalar slice.

## Evidence

`tests/explicit_mutation_v1.rs` pins canonical round-trips, all six U-family
diagnostics plus statement-only grammar regressions, deterministic Graph JSON
with a byte-exact non-mutation digest pin, CleanupPlan structural equality,
native C11 O0/O2 probes (success values and assigned-overflow failure
statuses), and Node/Wasm equivalence including overflow trapping.
`examples/explicit_mutation.spx` exercises the feature under the example
check/fmt gates.

## Non-claims

This tranche does not claim field/aggregate mutation, collection mutation,
reference or mutable-borrow semantics, concurrency or memory-model rules,
cross-task mutation, closures, or any hosted-CI promotion beyond the gates
listed above.
