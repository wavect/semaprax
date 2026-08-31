# Project draft expression catalogue v1

Status: implementation and regression evidence authored, unrun.

Audience: agent builders, editor integrators and compiler contributors.

An unfinished draft can discover expression selections from its current
last-valid candidate without exposing that candidate as the completed draft.
This supports planning another hole after a successful fill changes checked
expression identities or introduces new lexical bindings.

## Library and report

`ProjectCandidateDraft::expression_catalog(expected_draft, target)` selects
body expressions. `contract_expression_catalog(expected_draft, target)` selects
existing precondition and postcondition expressions. Both first authenticate
the exact draft digest, then reuse the corresponding existing candidate
catalogue over the private last-valid state. The body report contains only
`body` rows; the contract report contains only `requires` and `ensures` rows.

Both methods return `semaprax.project-draft-expression-catalog.v1`. The report
binds `draft_revision`, `target` and `region` (`body` or `contract`) and names
the prior valid facts explicitly as `last_valid_revision` and
`last_valid_candidate_digest`. It preserves the owning catalogue's source
provenance, declared effect budget, expression identities, expected types,
ownership modes, source spans, lexical scopes and limits. Source paths and
spans are descriptive metadata, never client-supplied mutation coordinates.

Every report says:

```json
{
  "materializable": false,
  "source_authority": false,
  "validation": "pending_fill_full_source_replay",
  "evidence_class": "last_valid_expression_inventory_not_draft_validation",
  "selection_admission": "requires_hole_open_validation"
}
```

There is no ordinary `candidate_revision` or `candidate_digest` handle, source
text, source diff, candidate-release accessor, or implicit completion. A query
does not register the private last-valid candidate in a transport registry.
It does not compile placeholders or infer new facts about unfilled expressions.

Catalogue `replaceable` facts describe authenticated source-expression origins.
They do not establish that a new hole is disjoint from every pending selection,
that the sixteen-hole capacity remains available, or that a proposed fill will
pass type, effect, ownership, contract, cleanup or target checks. The ordinary
hole-opening and fill routes retain all those checks. Rejected selections and
fills preserve the immutable draft and canonical source.

The complete response is bounded to 1 MiB. Existing catalogue limits remain
4,096 expressions, depth 256 and 16,384 cumulative scope facts. Additional draft
bindings consume the same response budget; excess output fails closed rather
than silently truncating rows. This is a bounded full catalogue, not a paging
or performance improvement claim.

## Protocol and freshness

Candidate-enabled v5 sessions expose the pure `hole/expression-catalog` query
with exact `image_revision`, `draft_revision`, `target`, and `region` fields.
The region is a closed `body`/`contract` choice. The handler and detached read
batches use the same library projection over an authenticated retained draft.
V5 discovery publishes the closed response schema, so selected generated
clients and MCP tool discovery can describe the same method and facts.

Every live call or read batch retains ordinary held-source authentication.
The host must have selected candidate preparation at startup. The query grants
no execution, source commit, archive restoration or broader authority; legacy
protocol profiles and their existing candidate catalogues remain unchanged.

Use the returned expression identity with an existing hole-opening method,
the same current draft revision and the draft's original source-candidate
handle. Filling, adding, rebasing or merging a draft requires a fresh catalogue
for that new draft. Old immutable drafts may remain independently queryable
while retained, but their selections must not be substituted for current ones.

Stale draft selectors retain `SPX-G232`; catalogue target, grammar and capacity
diagnostics keep their existing owners. Source/image authentication remains a
separate transport check. No failure rewrites source or publishes a candidate.

## Editor integration and evidence

The saved-source editor uses ordinary candidate catalogues before opening its
first hole, and this draft-bound query whenever a draft exists. It can therefore
add body, expression or contract holes after earlier fills. It retains only the
latest expression-choice inventory and binds it to the exact draft revision;
every successful open or fill invalidates that inventory. Missing host support
fails explicitly rather than falling back to the original candidate's stale
expression catalogue.

Draft completion remains explicit and rejects pending holes. Source review,
retirement of superseded in-memory draft handles, scratch freshness and source
epoch checks retain their existing editor owners.

Library, transport and editor regression cases are authored but unrun. No
compiler, interpreter, generated client, Node runner, editor host or quality
gate was executed. The graph-operational programme remains partial, and no
completion-matrix row is promoted.
