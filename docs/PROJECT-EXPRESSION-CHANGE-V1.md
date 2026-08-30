# Project Expression Change v1

Status: authored, unrun; the graph-operational programme remains Partial.

Audience: agent builders, compiler contributors, and reviewers.

This additive [Project Candidate](PROJECT-CANDIDATES-V1.md) operation replaces
one authored body expression selected by its actual retained HIR identity.
Canonical source remains authoritative. A caller supplies neither source text,
byte offsets, AST paths, nor trusted graph/HIR objects.

## Discovery and selection

`ProjectCandidate::expression_catalog(target)` discovers expressions belonging
to one explicitly identified, monomorphic, top-level source function. `main`
is admitted. Methods, templates, instantiated generic functions, synthetic
entrypoints, and missing or ambiguous function identities are not targetable.
The report schema is `semaprax.project-expression-catalog.v1`.

A report binds the current candidate digest, Project revision, function stable
ID, module, source path, source revision, and exact source digest. Expression
entries contain:

- the real HIR `expression_id`, source phase, and expression kind;
- `expected_type` as the stable HIR type identity key and resolved ownership;
- the authenticated source span for inspection, never as a mutation selector;
- a bounded typed lexical scope with names, value identities, types,
  ownership, and mutability; and
- `replaceable` and the applicable restriction or admission requirement.

The function's declared effect budget is reported separately. This is an
authority ceiling, not a claim that expression effects were inferred precisely.
Lexical visibility is also distinct from liveness: an owned binding may be
lexically visible after it was consumed. The actual replacement must pass the
ordinary ownership and effect verifier.

The scope includes parameters, preceding `let` bindings in the same enclosing
blocks, and the current match arm's typed binders. A `let` initializer cannot
see the binding being introduced or later declarations. Branch-local and match
arm bindings do not leak into siblings or enclosing scopes. Guard expressions
see their own arm's bindings. Contract entries are read-only; postcondition
entries additionally expose the authenticated result binding.

## Closed intention

The intention has exactly `kind`, `target`, `expression_id`, and `replacement`:

```json
{
  "kind": "replace_expression",
  "target": "calculator.add",
  "expression_id": "<identity returned by the current expression catalogue>",
  "replacement": {
    "kind": "binary",
    "op": "+",
    "left": {"kind": "place", "name": "subtotal"},
    "right": {"kind": "i64", "value": 2}
  }
}
```

The enclosing `SemanticChange.base_revision` and the apply call's expected
candidate digest bind the selection to the current revision. Expression IDs
are revision-scoped even when their serialized spelling is reused at the same
structural location in a later revision. Stale requests do not gain authority
from matching an ID string.

Replacement uses the existing closed constructors: five typed scalar literal
kinds, scoped `place`, stable-ID `call`, `binary`, `unary`, `if`, immutable scoped
[`let`](PROJECT-LEXICAL-BINDING-CONSTRUCTOR-V1.md), and the admitted aggregate
constructor forms. Calls must
already be locally bound or explicitly imported. The constructors cannot add
imports, parameters, top-level declarations, source fragments, arbitrary AST nodes, or
unresolved holes. Required source block categories are retained by wrapping a
constructed expression in an empty-statement block where necessary.

## Authenticated AST join and full replay

The selected HIR function comes from the retained, admitted Phase-A module,
including functions outside the entry/test closures. Its source revision and
digest must match the corresponding retained Project source. Canonical source
is parsed afresh and must identify the same explicit function, module, display
name, and span.

A selectable expression must have exactly one HIR occurrence at its exact
source span and exactly one compatible AST expression at that span in the
same source phase. The span must select a nonempty valid UTF-8 slice of the
authenticated source. Implicit upcasts, compiler-owned borrow/range forms,
special host/native import lowering, ambiguous spans, and expressions without
an exact authored AST counterpart are rejected. Source method/super-call sugar
that does not have the same ordinary HIR expression kind is likewise not
silently treated as a direct authored call. Its independently matched child
expressions may still be discoverable.

The compiler derives the selected AST node's child-index path internally and
constructs the replacement there. Complete candidate processing formats all
source, reparses it, independently rebuilds the Project, and revalidates
identity, effects, contracts, ownership, cleanup, profile, and admitted core
targets. A second expression check follows that rebuild: the compiler uses the
original AST parent path to locate the new source expression, authenticates
its new unique HIR counterpart, and requires the exact original resolved type
and ownership. It does **not** assume the original HIR ID or source span
survives formatting and replacement.

This extra check rejects an otherwise well-typed change such as replacing an
unused `i64` initializer with `bool`, where the surrounding function could
still compile but the selected expression's expected type would change.

Candidate rebase separately rejects competing body/signature/effect edits.
After that conflict check, expression selections are remapped through the
corresponding authenticated AST paths for each original and rebased
intermediate revision. Remapping alone is not evidence of merge safety.

## Limits and diagnostics

Discovery and admission inspect at most 4,096 expressions, depth 256, and
16,384 cumulative lexical binding facts. The canonical report is bounded to
1 MiB including its terminal LF. Function IDs are at most 4,096 bytes;
expression IDs are at most 16,384 bytes. Existing constructor and Semantic
Change bounds also remain active. These are structural/output bounds, not a
total heap-memory or execution-cost guarantee.

`SPX-G225` rejects invalid selectors, unsupported source origins, contract
replacement, unavailable lexical names, and changed expected type or ownership.
`SPX-G226` rejects capacity excess. Existing Project diagnostics handle source
and semantic admission; candidate stale/replay failures retain `SPX-G224`.
Failure discards only invocation-local candidate construction. Existing
candidates, sibling branches, and live files remain unchanged.

## Evidence and remaining scope

[Integration evidence](../tests/project_candidate_expression_v1.rs) is authored
but unrun at the user's request. It covers typed local-scope discovery, real
HIR ID selection, exact replay and stale rejection, unknown selectors,
initializer scope and inferred-type rejection, read-only contracts, `main`
and block replacement, match-arm scope, and sequential expression rebasing
across an unrelated display rename. No local compiler, test, target runtime,
or quality gate was run for this change.

This surface is not arbitrary graph editing, a contract-rewrite operation,
complete expression effect inference, a liveness proof, generic/class method
editing, migration of external consumers, or behavioral equivalence evidence.
The full graph-operational programme remains Partial.
