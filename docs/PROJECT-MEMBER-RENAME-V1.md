# Project Member Rename v1

Status: Partial; focused implementation and regression evidence pass locally.

Audience: compiler contributors, agent builders and semantic tool authors.

The existing `rename_declaration` intention selects source record fields,
variant cases and variant payload fields by stable identity. The compiler
derives their defining and referring tokens, including cross-file consumers,
and returns canonical source through the ordinary immutable candidate route.
No source syntax, public graph schema, request fields or authority are added.

```json
{"kind":"rename_declaration","target":"payments.amount.cents","name":"minor_units"}
```

## Selection and preservation

The selected member and its owning record or variant must have explicit source
identities. A payload field additionally requires an explicit parent case.
Compiler-owned prelude members, implicit identities and members of classes,
resources or protocols are not admitted. The ordinary identifier grammar and
128-byte name limit apply; the new spelling must differ from the old one.
Sibling names must remain unique in the selected record, variant or case.
An unrelated owner may retain a member with the same name.

There is no additional Copy-only or monomorphic restriction. Existing Project
admission and the authenticated AST/HIR collector determine admissible source
forms. Missing or ambiguous reference joins reject the whole operation.
Unsupported class/method, upcast, native-import and command-expression joins
remain outside the collector; their presence elsewhere in the source set can
therefore reject a member rename as well.

Record field renames update their declaration, constructor and update labels,
projections and record-pattern labels. Variant case renames update case
declarations, constructors and case-pattern labels. Payload field renames
update their declaration and authenticated constructor/pattern labels. The
selected identity, parent identities, member order, field types and every
unselected declaration identity remain unchanged. Same-spelling local values,
pattern binders and unrelated members are not renamed by spelling.

Unlike a nominal owner display rename, a member rename can change consumer
source: type import aliases remain intact, but labels and projections through
those aliases must use the new member name. The compiler processes the whole
authenticated source set, including disconnected modules. It never substitutes
a call-only dependency closure for a complete member reference inventory.

## Shared planning and independent source replay

The [nominal rename adapter](PROJECT-NOMINAL-RENAME-V1.md) also owns this
candidate entry point. Its source selection is reused by catalogue eligibility.
It delegates to the same crate-private Operations source planner; no second
reference index or direct AST member rewrite is introduced in Candidate.

Private sidecar facts retain member parent namespaces and source occurrence
paths. The planner checks old source tokens, derives nonoverlapping edits in
each affected file, and independently rebuilds and replays the resulting
source set. Unselected source bytes remain exact. Complete Project admission
then runs through the existing candidate pipeline, preserves explicit identity
and effect/contract/export inventories and admitted core targets, and compares
the final canonical sources with the exact invocation-local plan.

These checks do not authorize mutation of graph fields or repair of runtime
cleanup vectors. A candidate is still derived from human-readable source;
evidence and review do not confer source or publication authority.

The public [Operations v1](SEMANTIC-WORKSPACE-OPERATIONS-V1.md) proposal keeps
its existing declaration kinds, 2–64 operations and 2–16 paths. Member subjects
are private planner data, not new accepted public proposal variants. Existing
source, graph-builder, operation-work, edit-count and replacement-byte bounds
remain active. Limits fail without a partial candidate or truncated inventory.
Additional private member and path facts can increase accounted builder work
and therefore change derived budget/evidence digests. No known-answer digest
was re-pinned without executing its owning gate.

## Discovery, conflicts and test relevance

`change_catalog(target)` advertises the existing rename constructor with
`member_kind` equal to `record_field`, `variant_case` or `variant_field`.
Discovery confirms source eligibility; an arbitrary supplied spelling still
requires full namespace and reference validation during application. Existing
candidate-enabled transports and generated clients need no new RPC or grant.

Rebase binds the selected member name/kind, complete ordered owner shape,
source location and conservative local/imported type bindings. Concurrent
member renames, owner shape changes, removed identities or changed ancestry
conflict with `SPX-G235`. Unrelated function changes can replay through full
admission. Each history step uses its corresponding original and rebased
intermediate sources, including members introduced earlier in that history.
Same-member competing rename histories conflict even if one history restores
the original name. This is conservative conflict detection, not complete
semantic merge normalization or external ABI compatibility.

Static test planning reports `non_callable_member_display_change` as a
conservative fallback. Absence of a member in a call graph is not evidence
that no tests are relevant. Rename, discovery, rebase and test planning never
run tests or target programs.

## Diagnostics and evidence

Malformed or unsupported candidate intentions use `SPX-G225`, exact-base
candidate binding failures `SPX-G224`, and semantic rebase conflicts
`SPX-G235`. Shared Operations, Graph, parser and verifier failures preserve
their owning diagnostics. No failure writes authoritative source.

Library and v5 transport regression evidence lives in
[member rename cases](../tests/project_candidate/member_rename.rs). The evidence
must establish source migration, identity preservation, collision and stale
rejection, independent history replay and unchanged authority. The focused
member-rename module passes locally. Full quality, hosted,
interpreter/application and target execution were not run for this batch.
Broader declaration kinds, unsupported reference forms,
external consumer migration, general merge normalization and broader execution
evidence remain outstanding beyond this focused module.

The generic regression compares the complete ordered type-parameter name
inventory while deliberately excluding source spans: an earlier
length-changing rename legitimately shifts later token locations without
changing generic meaning. The suite separately retains exact canonical-source,
stable-identity, owner-shape, field-type and recovery comparisons. The
projected-owned-Bytes case uses a fixture without unrelated owned variants, so
it exercises the already admitted projected-loan composition without weakening
the fail-closed `SPX-G410` rejection for owned-variant Graph v22 masking.
