# Project Signature Argument Expressions v1

Audience: agent authors, compiler contributors, and candidate reviewers.

Status: implementation and regression cases authored, **unrun**. No runtime,
cross-backend, performance, or complete signature-evolution claim.

The ordered `parameters` form of `change_function_signature` adds one explicit
alternative for a fresh scalar parameter:

```json
{
  "name": "derived",
  "type": "i64",
  "argument_expression": {
    "kind": "let",
    "name": "subtotal",
    "value": {
      "kind": "binary",
      "op": "+",
      "left": {"kind": "place", "name": "left"},
      "right": {"kind": "place", "name": "right"}
    },
    "body": {"kind": "place", "name": "subtotal"}
  }
}
```

The object is closed to `name`, `type`, and `argument_expression`. Types remain
`i64`, `i32`, `u8`, `usize`, or `bool`. The expression uses the existing typed
constructor grammar, including scoped bindings and admitted aggregate operands.
It is mutually exclusive with literal `argument` and retained `from` mappings.
The old literal and `append_parameters` routes retain their existing rules and
output; a nonliteral expression in their `argument` field still rejects.

## Scope and caller migration

Template places refer to the target's **original parameter names**, including
removed Copy parameters. They do not refer to a caller's coincidentally named
local, a renamed parameter's new spelling, or a newly added parameter. Local
`let` and match bindings retain the shared constructor's lexical scope rules.

The compiler first stages every original call argument once, left to right,
including arguments whose parameters will be removed. It then computes the
new argument expressions in their mapped-parameter order. Each result is stored
in a fresh immutable local explicitly annotated with the requested scalar type.
The final ordinary call receives retained staged values, unchanged literals,
and computed locals in the requested parameter order.

This keeps original effects and failures in their original relative order.
An original failure prevents later original and computed arguments. A computed
failure prevents later computed arguments and the final call. New expressions
can introduce checked failures or additional calls; this is an explicit semantic
change, not behavioral equivalence. The generated block stays inside the old
call's branch, lazy operand, loop, or contract position.

Template construction uses each affected caller's existing local/import
bindings. Calls and nominal operands must be available there; no imports,
effects, capabilities or profile authority are silently added. Original
parameter references are mapped to staging locals through the existing
scope-aware substitution machinery. Compiler staging names and constructor
locals remain distinct, including adversarial staging-like user names.

## Verification and limits

The provider receives constructor grammar/scope/binding preflight even if there
are no direct callers. Actual type, effect and ownership checks apply to the
expressions instantiated in migrated callers through full Project admission.
When no calls migrate, no default expression is materialized or executed, and
preflight is not standalone type-checking or proof that a future caller would
admit the template. The report's migrated-call count remains zero.

The exact type annotation prevents an otherwise unused new parameter from
hiding a wrongly typed computed value at an instantiated call. Existing
`own Bytes` parameters must still be retained exactly once. A computed
expression cannot consume such an owner and also transfer it again through the
final call; ordinary ownership and loan verification decides each use.

Canonical source is generated in memory and independently reparsed/rebuilt.
Stable identities, contracts, declared effects, module permits, target profiles
and existing core-target admission remain checked by the candidate pipeline.
Recovery and semantic rebase retain dependencies nested inside the template.
The intention and evidence carry no source publication authority.

Existing parameter, constructor, expression-walk, source, candidate and history
bounds remain in force. Computed expression growth is charged to the cumulative
migration budget. No nominal signature-fact limit is removed. Grammar/capacity
errors retain `SPX-G225`/`SPX-G226`; substitution, ownership, stale selection and
ordinary language diagnostics retain their existing codes.

The ordered mapping catalogue advertises `computed_parameter_fields` and its
scope/order/binding constraints. Constructor schemas include the closed
recursive form. This is discovery of a constructor requiring admission, not a
proof that every caller accepts an arbitrary expression.

Authored evidence is in `tests/project_signature_argument_expressions_v1.rs`
and the computed-signature cases in
`tests/project_candidate_lexical_binding_rebase_v1.rs`. They remain unrun.
General type conversions, new owning parameters, aggregate parameter defaults,
external consumer migration and measured signature-evolution performance remain
open.
