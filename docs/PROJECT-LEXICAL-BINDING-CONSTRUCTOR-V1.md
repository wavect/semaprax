# Project Lexical Binding Constructor v1

Audience: agent authors, compiler contributors, and candidate reviewers.

Status: implementation and regression cases authored, **unrun**. No execution,
cross-backend completion, performance, or publication evidence is claimed.

The typed expression grammar adds one immutable scoped binding:

```json
{
  "kind": "let",
  "name": "subtotal",
  "value": {
    "kind": "call",
    "target": "payments.calculate",
    "arguments": [{"kind": "place", "name": "payment"}]
  },
  "body": {
    "kind": "binary",
    "op": "+",
    "left": {"kind": "place", "name": "subtotal"},
    "right": {"kind": "place", "name": "subtotal"}
  }
}
```

The object is closed to `kind`, `name`, `value`, and `body`. Both expressions
use the same recursive typed constructor grammar. The compiler builds an
ordinary block containing one immutable `let` statement and the body as its
tail expression. It infers the binding's type through normal verification.
No new source syntax, explicit type parser, mutable local, assignment, raw AST,
or unresolved-hole form is introduced.

The initializer occupies exactly one AST position before the body. Reusing a
Copy local avoids duplicating the initializer expression. This structural fact
does not permit duplicating an owned value: ordinary ownership admission still
rejects use after move, repeated consumption, or invalid borrow escape.

## Lexical scope and hygiene

The new name is unavailable in its initializer and visible only in its body.
Outer bindings remain visible. Nested bindings are permitted, and names may be
reused in disjoint sibling scopes, but a binding cannot shadow an active local,
parameter, match binder, call/import alias, type binding, or compiler-generated
temporary. Reserved identifiers are rejected. These checks prevent a local
from changing the meaning of a stable-ID call or nominal constructor.

The name is reserved before constructing its initializer so generated aggregate
staging temporaries cannot capture it. Reservation affects generated names,
not place lookup; it does not make the new local visible early or leak it into
another branch. The canonical source uses the admitted local name.

## Admission, replay and bounds

The constructor is available wherever the existing candidate pipeline invokes
the shared expression builder, including body/expression replacement, added
function bodies and typed-hole fills. Recognition is not admission: the full
Project verifier still decides whether a block is legal in the selected
context and whether its type, effects, contracts, ownership, cleanup and target
profile satisfy existing requirements. No effect or capability budget widens.

The enclosing Semantic Change and candidate selectors bind the exact base.
The compiler renders canonical source in memory, reparses and independently
rebuilds it, and retains ordinary candidate identity and expected-expression
checks. Recovery reconstructs the same typed intention from canonical source;
semantic rebase still checks stable-ID dependencies inside initializer and body.
Nothing writes source without separate publication authority.

The block uses the constructor's charged root node; its generated `let`
statement consumes one additional node. Initializer and body descend through
the corresponding AST structure under the existing shared 4,096-node and
64-depth limits. The enclosing JSON, change/history and Project bounds remain
unchanged. `SPX-G225` rejects malformed objects, invalid names and scope errors;
`SPX-G226` rejects structural capacity excess. Type, ownership and other
semantic failures retain their ordinary compiler diagnostics.

Schemas describe this closed recursive shape and discovery includes `let` in
constructor lists. Schema validation cannot prove lexical scope or ownership.
Existing protocol method sets and authority selections are unchanged.

Authored evidence is in `tests/project_candidate/lexical_binding.rs` and
`tests/project_candidate/lexical_binding_rebase.rs`, plus constructor/schema
module cases. They cover scope, evaluation structure, ownership rejection,
hole filling, source replay, callee renames and changed-signature conflicts.
All remain unrun. General imperative constructors, liveness-guided synthesis,
mutable/borrow-preserving bindings and arbitrary source rewriting remain open.
