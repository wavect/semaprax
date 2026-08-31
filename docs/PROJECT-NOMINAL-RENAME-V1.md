# Project Nominal Declaration Rename v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors, agent builders and semantic tool authors.

The existing `rename_declaration` intention now selects explicit source record
and variant identities as well as its existing function targets. It changes a
display name and the references proven to use that declaration, then returns
an immutable source-replayed candidate. It grants no filesystem or publication
authority and introduces no new source syntax, graph schema or request fields.

```json
{"kind":"rename_declaration","target":"payments.amount","name":"Money"}
```

## Selection and source meaning

`target` is the declaration's persistent ID; `name` is one ordinary identifier
of at most 128 bytes and must differ from the current display name. Only an
explicit record or variant owner is admitted by the nominal route. The additive
[Member Rename v1](PROJECT-MEMBER-RENAME-V1.md) route selects explicit source
fields and cases through the same adapter and planner. Compiler prelude
identities, resources, classes, protocols and implicit declarations remain
unsupported. Existing function rename behavior is unchanged.

This route does not impose a new Copy-only or monomorphic type restriction.
Existing Project admission and the shared authenticated occurrence collector
determine which source shapes can be proven. Unsupported AST/HIR pairs still
reject; they never become an empty successful reference inventory. General
language, target or generic-import support is not widened by display renaming.

The collector operates over the complete source set. Unsupported class/method,
upcast, native-import or command-expression joins can therefore reject a rename
even when they occur outside its selected declaration. No omitted-reference
fallback is used to make those projects appear supported.

The compiler derives the declaration's module, path, kind and old spelling
from authenticated source/HIR facts. Same-module type annotations, constructors,
updates and patterns change only at proven occurrences of that stable identity.
Unrelated local values or shadowing type parameters retain their names. Field,
case and payload names/identities are unchanged. Consumer imports already name
the provider by stable ID, so their aliases and source bytes stay unchanged.

No source span, string replacement rule, trusted reference inventory or
graph/HIR object is accepted from the agent. Ordinary Git review still sees
the generated canonical source diff, not a second graph source of truth.

## One shared reference collector and replay engine

`candidate/type_rename.rs` delegates to a crate-private owned-source entry point
in `semantic_workspace_operations`. The entry point reuses the existing
Operations AST/HIR occurrence collector, namespace checks, source-token planner,
canonical rendering and independent candidate replay. It infers one nominal
declaration operation from the retained source and returns canonical sources.
It does not call managed-workspace publication or open paths and locks.

The collector joins checked type and expression facts instead of guessing by
spelling. Additional pairings cover built-in scalar/byte/borrow types and
literals, typed local annotations, guards and refutable patterns, and byte
view/range lowering, retaining failure on missing or ambiguous joins. Full replay checks stable IDs,
normalized declaration meaning, source occurrence ownership and cross-file
edges. No downstream code sorts, repairs or reinterprets cleanup vectors.

The public [Operations v1](SEMANTIC-WORKSPACE-OPERATIONS-V1.md) proposal remains
unchanged: it still requires 2–64 operations over 2–16 paths. The private
single-declaration entry point has no proposal parser, authority or public
minimum-count exemption. It uses the same bounded planner and replay checks;
no public operation can use it to bypass its own grammar or publication gate.

Candidate application then parses the planned sources into fresh ASTs, performs
its existing complete Project rebuild and independent source replay, checks
identity/effect/contract/export/profile invariants and previously admitted core
targets, and compares the final sources against the exact private rename plan.
The plan is invocation-local compiler data, not an imported proof or authority.

## Discovery, conflicts and tests

`change_catalog(target)` advertises the existing rename constructor for an
eligible source nominal owner. Payload-dependent namespace/reference validity
still requires application. Existing transport methods and generated request
shapes can submit the intention; no new permission, RPC method or disk cache
is introduced.

Semantic rebase separately tracks nominal display name, shape and origin. A
concurrent target rename, shape change, removal, reidentification or relocation
conflicts rather than silently retargeting the operation. The fingerprint also
binds local and imported type names/identities conservatively. Unrelated function
edits can replay, and a type introduced earlier in a candidate's history remains
subject to that history's identity checks. Final meaning must still pass full
candidate admission.

Existing function and aggregate fingerprints remain conservative: type display
changes, including unrelated type binding edits in the same module, can alter
source spellings used by another intention and cause a
conflict even when a human could safely merge them. This addition does not
claim complete semantic merge normalization or external ABI compatibility.

Test planning uses an explicit conservative fallback for nominal renames; a
type identity absent from a call-only test graph must not be mistaken for an
unaffected program. No test, interpreter or target is run by rename, discovery,
rebase or static test planning.

## Bounds and evidence

Existing Project, source, history and target bounds remain active. The reused
Operations engine caps its graph builder, aggregate builder work, occurrences
and rendered source using its existing limits; the new entry point does not
raise those caps. Capacity failures return diagnostics rather than truncating
the reference inventory. Candidate request errors retain `SPX-G225`, stale
candidate/Project bindings `SPX-G224`, and rebase conflicts `SPX-G235`; lower
Operations, Graph, parser and verifier diagnostics propagate unchanged.

Authored, unrun evidence is in `tests/project_candidate_nominal_rename_v1.rs`
and `tests/image_nominal_rename_transport_v5.rs`. It covers proven local uses,
stable identities and consumer aliases, generic/owned admitted source shapes,
collisions, immutable failure, recovery and conservative rebase. No compiler,
test, interpreter, application executable or long local quality gate was run.
Field/case renaming has a separate authored [member contract](PROJECT-MEMBER-RENAME-V1.md).
Broader declaration kinds, general merge normalization,
external consumer migration and executed evidence remain outstanding.
