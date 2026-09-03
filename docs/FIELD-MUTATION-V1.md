# Field Mutation v1

Audience: language users, tool authors, and compiler contributors.

Status: Partial — extends Explicit Mutation v1 with direct scalar-field
stores on `let mut` record/class locals; evidence lives in
`tests/language/field_mutation.rs` plus `examples/field_mutation.spx`.

## Objective

Records and classes are value-semantics aggregates. This tranche admits
whole-field replacement of one direct checked Copy scalar field of a `let mut`
record- or class-typed local:

```text
let mut point = Point { x: 1, y: 2, enabled: false };
point.x = point.x + 41;
```

Everything else stays closed: nested place chains (`a.b.c = ...`), aggregate
or String/resource fields, mutation through parameters or immutable bindings
(including method receivers), arrays (none exist), and any new expression
forms. Simple `<binding> = <expr>;` assignment keeps its exact previous
behavior, diagnostics, and serialization.

## Semantics

1. Evaluation order matches Explicit Mutation v1 exactly: the assigned value
   is evaluated completely before the store; checked-arithmetic failure
   statuses propagate exactly as from the same expression in an initializer
   position. The left-hand side is a pure place (one local binding plus one
   field offset), so RHS-first lowering is observationally identical to
   left-to-right evaluation.
2. The target binding must be a `let mut` local whose type is a nominal record
   or class; the named field must exist and be a scalar Copy type (`i64`,
   `i32`, `u8`, `char`, `f32`, `f64`, `bool`) after generic substitution.
   Whole-field replacement only; no partial or nested stores.
3. Types must match the field type exactly; there is no conversion. The store
   creates no new value identity: the statement reuses the original binding's
   [`ValueId`](RFC-0001.md) and names the targeted field's stable id.

## Diagnostics (family SPX-U1xx, continuing after SPX-U106)

| Code | Meaning |
| --- | --- |
| `SPX-U107` | Field assignment targets an immutable binding (declare it `let mut`). |
| `SPX-U108` | The target record/class has no field with the given name. |
| `SPX-U109` | The targeted field is not a direct scalar Copy field. |
| `SPX-U110` | Assigned value type does not exactly match the field type. |
| `SPX-U111` | Nested place chains (`a.b.c = ...`) are outside this slice (parse-time). |
| `SPX-U112` | Field mutation base is not a record/class value. |

Unknown assignment names keep the established unknown-value diagnostic
(`SPX-T202`); contract expressions still reject every assignment via
`SPX-U106`.

## Layer behavior

- **Parser/AST**: `Statement::Assign` carries `field: Option<FieldTarget>`;
  lookahead recognizes `<binding> (. <field>)+ =` so deeper chains enter the
  assignment path and fail with `SPX-U111`. Programs without the new syntax
  parse identically.
- **Formatter**: renders `{name}.{field} = {value};`; all pre-existing
  programs format byte-for-byte identically.
- **HIR/resolver**: `ResolvedStatement::Assign` gained
  `field: Option<DeclarationId>`; both the iterative resolver and its
  recursive oracle twin resolve the field before the assigned value and agree
  on U107–U112 fail-closed checks. Source-level verification implements the
  same checks for functions; method bodies remain covered by resolution
  itself.
- **Graph**: `statement_json` emits an additive `"field":"<stable-id>"`
  attribute on assign nodes only; programs without field mutation serialize
  byte-for-byte identically (pinned by the pre-feature digest in
  `tests/language/explicit_mutation.rs` and the `meaning.spx` revision pin).
- **Cleanup**: Copy-scalar field stores lower their RHS exactly like an
  initializer and never transfer into cleanup slots; straight-line field
  mutation produces CleanupPlan v2 output structurally identical to the
  initializer-only equivalent (asserted modulo function names in tests). Note
  that extending `ResolvedStatement` legitimately grows the deterministic
  `HIR_EXPR_FIXED_BUNDLE` budget-model term; affected workspace budget KATs
  were re-pinned accordingly in `tests/workspace/semantic_graph.rs`.
- **Native C11**: plain struct-member store `local.field = value;` after full
  RHS evaluation; O0/O2 identical including failure statuses.
- **Wasm**: aggregate lane computes the field pointer (binding frame slot +
  layout offset) and copies the fully evaluated scalar into it; i32 overflow
  traps/fails identically to initializer positions.

## Evidence

`tests/language/field_mutation.rs` pins canonical round-trips, all six new
U-family regressions, additive deterministic Graph JSON (schema stays v10),
CleanupPlan structural equality, native O0/O2 probes over records and classes
including branch-local mutation, Node/Wasm equivalence (134 corpus result,
overflow trap), and native failure-status selection. 
`examples/field_mutation.spx` exercises the feature under the example
check/format/run gates.

## Non-claims

No nested-place stores, no aggregate/String/resource field stores, no method
receiver mutation, no mutation through parameters, no arrays, no references,
no concurrency claims, and no change to simple-assignment behavior.
