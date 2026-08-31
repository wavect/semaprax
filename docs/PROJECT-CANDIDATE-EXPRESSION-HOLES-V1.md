# Project Candidate Expression Holes v1

Status: implementation and regression cases authored, unrun; programme Partial.

Audience: agent builders, embedding hosts and compiler contributors.

`ProjectCandidateDraft::with_expression_hole(expected_draft, target,
expression_id, hole_id)` extends ephemeral drafts with an authored expression
selection. The function stable ID, actual revision-scoped HIR expression ID and
exact draft digest select meaning. Source spans and AST paths are not request
inputs. The existing expression catalogue supplies eligible selections.

The selection must have a unique authenticated canonical AST origin in a
monomorphic top-level function body, including `main`. Contract regions remain
read-only. The compiler derives structural paths only to reject overlapping
ancestor/descendant holes and to preserve disjoint selections across fills.
Body and expression holes cannot overlap the same function. Multiple disjoint
expression holes can coexist within one function, alongside body holes in other
functions. All holes share the existing limit of sixteen and unique hole IDs.

The separate [Contract Expression Holes](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md)
route selects existing pre/postcondition subtrees without widening this API.
Those disjoint regions can coexist with a body hole in the same function and
share the same sixteen-hole budget. This body-expression route still rejects
contract selections.

## Context and validation

The additive [compact navigation API](PROJECT-HOLE-NAVIGATION-V1.md) exposes
expected type/ownership, effect policy and selected lexical/call/obligation pages
without changing the original context or expression-selection rules.

`hole_context` returns the existing body schema for body holes and
`semaprax.project-candidate-expression-hole-context.v1` for expression holes.
Expression context binds the draft, source revision/digest, actual selection,
expected type and ownership mode. Its lexical scope includes authenticated
parameters, preceding locals and current match-arm binders. Initializers cannot
see their own newly introduced binding or later locals. Visibility does not
establish whether an owned value remains live.

The context also supplies enclosing effects/permits, accessible calls, contracts
and prior-body loan/cleanup plans. These are last-valid facts, not fabricated
proofs for an unresolved expression. No placeholder source, invalid HIR, runtime
execution or source publication authority is introduced.

`fill_hole` constructs the existing typed `replace_expression` intention and
uses complete candidate admission, canonical source replay, type/ownership
preservation, effects, contracts, cleanup and target projection gates. Surviving
disjoint expression selections are independently rejoined to their canonical
AST paths in the new checked source. Their expected type/ownership must remain
equal; context is then regenerated from the new retained revision. Failure at
any stage preserves the original immutable draft and every sibling.

`complete` rejects until both body and expression inventories are empty. Draft
reports contain last-valid bindings and unresolved selections but expose no
candidate source or materialization accessor. Existing body-only report bytes,
handles and behavior remain unchanged. The 1 MiB draft/context, 16 MiB bounded
proof rendering and expression catalogue bounds remain in force. Diagnostics
reuse SPX-G230 grammar/overlap, SPX-G231 capacity and SPX-G232 stale/unresolved;
ordinary expression selection and candidate-admission diagnostics propagate.

## Protocol and evidence

The additive v4 `hole/open-expression` takes `image_revision`,
`candidate_revision`, `target`, `expression_id`, `hole_id`, and optional
`draft_revision`. It returns an immutable draft handle. Existing `hole/query`,
`hole/fill`, `hole/complete` and `hole/discard` operate on that draft. V4 discovery
advertises the body/expression context schema alternatives. V1–V3 retain their
existing method sets; no request elevates authority.

`tests/project_candidate_expression_holes_v1.rs` covers local visibility,
type rejection, disjoint selection remapping, mixed body/expression drafts,
overlap rejection, stale selectors, unresolved materialization rejection and
unchanged files. `tests/image_diagnostic_transport_v4.rs` adds the protocol
lifecycle and legacy rejection scenario. These cases were authored but not run;
no compiler, interpreter or long quality gate was executed in this work.
