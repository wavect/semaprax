# Class Inheritance v1

Audience: language users, tool authors, and compiler contributors.

Status: Bounded tranche — RFC-STRING-OO badge 4. This document plus
`tests/language/class_inheritance.rs` are the executable evidence for everything
claimed here. Nothing in this file claims completion-matrix promotion.

## Objective

Badge 3 admitted `class Name { fields, methods }` with static method calls.
This tranche adds single named inheritance while keeping every other OO
surface closed:

```text
class Child : Parent { members }     // one named parent, no interfaces
let alias: Parent = child;           // typed binding with implicit upcast
super.method(args)                   // parent implementation, same receiver
```

## Syntax and canonical form

- `class C : P { .. }` — the parent is written after the class name and any
  type parameters as a bare named type (`Type::Named`, no arguments). The
  canonical formatter renders exactly `class C : P {`; programs without
  `extends` format byte-for-byte identically to pre-feature output.
- Typed local bindings: `let name: T = value;` joins the existing
  `let`/`let mut` grammar. The declared type is optional; when present it is
  rendered between the name and `=`.
- `super.method(args)` — postfix syntax only. A bare `super` identifier
  remains an ordinary (unresolved) name; only `super.` followed by a call
  produces `ExprKind::SuperMethod`. There is no `super` outside a call, no
  constructor chaining, and no `super` field access.

## Layout

The child's effective member sequence is the ancestor chain's effective
sequence (root first) followed by the child's own declared fields. Effective
positions are renumbered canonically for the extending class, so:

- Every field of the standalone parent keeps its exact offset inside the
  child on both Native64 and Wasm32 (pinned through the emitted
  `_Static_assert(offsetof(..))` facts and the independent layout
  reconstruction gate).
- Aggregate layouts, semantic facts (`copy`/`needs_drop`/layout keys),
  projections, construction completeness, cleanup shapes, and backend struct
  emission all consume the same materialized prefix — no consumer recomputes
  inheritance independently.
- Field identity stays with the declaring class: inherited fields keep their
  original stable IDs, and graph field nodes always record the true declaring
  owner.

Construction remains a flat full-member literal: `Dog { legs: 4, bark: 2 }`
must supply every effective member exactly once (inherited names resolve
through the child). Redeclaring an ancestor member is a diagnostic.

## Value semantics and upcasts

A child value embeds its ancestors' prefixes. An upcast consumes a descendant
value and produces an ancestor-typed value by transferring the inherited
prefix leaves into the result — own-transfer semantics, exactly like moving
the value into any other binding:

- `let a: Animal = p;` accepts either the exact type or an ancestor.
- Inherited-method calls consume the receiver through the same guarded
  upcast: `d.describe()` where `describe` is declared on `Animal` resolves to
  `Animal::describe` with `d` consumed as an `Animal`.
- `super.method(args)` passes the enclosing override's own receiver through
  the same upcast to the declaring ancestor.
- After an upcast the source place is fully moved; using it again is the
  ordinary use-after-move diagnostic family.

**Cleanup-inert suffix rule.** Consuming a child transfers its inherited
leaves into the ancestor-typed result, so any owned state declared by the
child itself would silently leak. Upcasts are therefore admitted only when
every child-declared field (beyond the ancestor prefix) introduces no cleanup
leaves (`i64`, `bool`, and other Copy aggregates). Violations fail closed at
compile time rather than leaking at run time.

Because of this rule, cleanup plans stay schema-identical: an upcast lowers
to the ordinary construct/move machinery, contributes no new transition kinds,
and replay validation is unchanged. Copy-only corpora produce plans without a
single transfer or finalizer, byte-comparable to pre-feature structure.

## Dispatch

Method resolution is fully static; there is no vtable:

1. Start at the declared type of the receiver.
2. Walk the ancestor chain nearest-first; the first class declaring the name
   wins. An override therefore replaces the inherited symbol for receivers of
   its own class while unoverridden parents stay callable.
3. If the declaring class is not the receiver's own class, the receiver is
   consumed through the guarded prefix upcast before the static call.

Overrides must match the overridden non-self signature exactly: parameter
count, ownership modes, types, order, and return type. The `self` parameter
necessarily changes to the overriding class; that difference is expected, not
a mismatch. `super.m(args)` skips the enclosing class and resolves from the
parent upward, so it reaches grandparent implementations through intermediate
overrides.

## Admitted slice / closed surfaces

- Single inheritance over non-generic classes. Generic parents, generic
  children, and interface conformance remain closed (protocol_check is
  untouched).
- Class members carrying `string` — directly or transitively — are rejected.
  Rationale: the string runtime deliberately keeps strings out of the
  resource-lifecycle inventory (backends free top-level strings inline), so
  an aggregate carrying one has no finalizer representation yet. Classes fail
  closed at verification instead of failing confusingly at cleanup-plan
  construction. Strings continue to work as locals, parameters, and returns;
  methods returning `string` compose freely through inheritance on native.
  Note the pre-existing boundary: the Wasm aggregate lane rejects
  string-typed expressions wholesale (badge-3 gap, unchanged here), so the
  Wasm corpus uses copy-only classes.
- Resources cannot appear as class members in this slice for the same
  lifecycle-inventory reason, and resources have no in-source constructors.
- Assignment statements still accept only checked Copy scalars; there is no
  `p = q` aggregate assignment, downcast, or `is`/`as` dynamic check.

## Diagnostics (new SPX-T codes)

| Code | Meaning |
| --- | --- |
| `SPX-T227` | Unknown, non-class, or generic `extends` target. |
| `SPX-T228` | Inheritance cycle (including self-extension). |
| `SPX-T229` | Member (field or method) redeclares an ancestor member name. |
| `SPX-T230` | Override does not exactly match the overridden signature. |
| `SPX-T231` | `super` misuse: outside a class-method override, in a parentless class, unknown super method, or argument mismatch. |
| `SPX-T232` | Declared binding type accepts neither the value's exact type nor an ancestor. |
| `SPX-T233` | Upcast would discard owned child-declared state (defense in depth behind `SPX-T234`). |
| `SPX-T234` | Class member carries `string` (closed surface, see above). |

Structural checks (T227-T230, T234) run in source verification; expression
checks (T231-T233) run during resolution. Both layers fail closed
independently.

## Layer behavior

- **AST**: `TypeDeclaration.extends: Option<Type>`,
  `Statement::Let.declared: Option<Type>`, and `ExprKind::SuperMethod`.
- **Parser**: colon-suffix parsing for classes and lets; `super.` interception
  in the dot-postfix path. Previously rejected inheritance now parses.
- **Formatter**: renders all three forms; pre-feature sources are unchanged.
- **HIR**: `DeclarationIndex.class_parents` plus a materialization pass that
  prepends ancestor prefixes (renumbering effective positions) before facts,
  types, layouts, or functions are built. The resolver adds
  `ResolvedExprKind::Upcast { source }` whose source re-resolves at the
  canonical `.source` identity below the slot it occupies; both the iterative
  resolver and validators re-derive the admissibility contract independently.
- **Graph**: additive `"extends"` key on class nodes, `"kind":"upcast"`
  expression nodes, and true-owner field nodes under each rendering class.
  Non-inheritance graphs are byte-identical (pinned revision digest).
- **Native/Wasm**: backends lower an upcast by copying the ancestor prefix
  field-by-field after validating prefix offsets against both canonical
  layouts; no other backend logic changed.

## Evidence

`tests/language/class_inheritance.rs`: canonical round-trip and prefix-sequence
pins; generated-C `_Static_assert` offset pins for two- and three-level
chains; clang O0/O2 execution; Node/Wasm equivalence on the copy-only corpus;
graph determinism plus pre-feature byte-identity pins; cleanup-plan schema,
transfer, and finalizer pins; every diagnostic above exercised with its exact
code. `examples/inheritance.spx` exercises chains, overrides, `super`,
string-returning methods, and upcast dispatch under the example gates.
