# Project Interface Change v1

Status: Partial; implementation and regression tests authored, unrun.

Audience: agent builders, compiler contributors, and reviewers.

`implement_interface` appends one actual source `impl` declaration that binds an
explicit Project record to an explicit Project protocol's complete member table.
The binding names existing Project functions by persistent identity. The
declaration defaults to the receiver module; optional `destination` selects one
other exact declared Project module. The compiler plans canonical dependency
imports before mutation. Source and Project checkers validate the closed static
sidecar before candidate application rebuilds the full Project. This does not
generate function bodies, lower dynamic dispatch, introduce a runtime witness
table, or add a runtime Graph edge for the protocol dependency.

## Intention and discovery

```json
{
  "kind": "implement_interface",
  "target": "example.counter",
  "protocol": "example.readable",
  "id": "example.counter.readable",
  "destination": "example.bindings",
  "members": [
    {"method": "example.readable.read", "implementation": "example.read"}
  ]
}
```

The five-field form without `destination` remains exact and places the sidecar
in the receiver module. The six-field form shown above is also closed. `target`
selects one explicit monomorphic Project record. `protocol` selects one explicit
Project protocol. Each `method` is an explicit required member, and each
`implementation` is an existing explicit ordinary non-`main`, monomorphic
Project function. Every required member must be selected exactly
once, and implementation functions must be distinct. The new implementation ID
must be globally fresh. Selectors use the source static-conformance grammar:
1–240 ASCII letters, digits, underscores, dots, colons, or hyphens. The new ID
cannot use the `auto:` or `semaprax.` prefixes or collide with prelude identities.

`ProjectCandidate::interface_catalog(expected_candidate, target)` returns
`semaprax.project-interface-change-catalog.v1`, bound to the exact candidate and
Project revision. It lists Project protocols and provider modules, required
signatures and modes, eligible Project function IDs, declared destination
modules, and an existing implementation if present.
`complete_mapping_available` requires an actual one-to-one matching, not merely
one candidate per member. The ordinary change catalogue advertises
`implement_interface` when a complete new mapping is available. Discovery never
admits a proposed implementation or selects a preferred table automatically.

The compiler's static protocol owner resolves nominal parameter identities in
each declaration's own module before comparison. The receiver position
substitutes protocol `Self` or the protocol name with the record's exact stable
identity. Remaining resolved type identities, parameter modes, arity, and return
type must match exactly. Implementation functions cannot add effects or
preconditions; ordinary postconditions are permitted. Retained checked HIR must
contain the exact receiver record and every selected function before mutation.
The ordinary verifier still owns body, ownership, cleanup, and backend admission.

## Canonical source and persistent identities

The operation constructs a typed AST declaration, orders bindings by required
method ID, and leaves projection to the canonical formatter. Cross-module
dependencies are sorted by family and stable identity. Existing exact imports
are reused; fresh aliases prefer the provider display name and then use the
bounded `_spx_impl_<n>` namespace. A conflicting identity, kind, or provider
rejects before mutation. The output has ordinary source form:

```text
@id("example.counter.readable")
impl "example.readable" for "example.counter" {
    "example.readable.read" = "example.read";
}
```

A cross-module destination additionally carries exact `use protocol`, `use
type`, and `use function` declarations. `use protocol` is a static sidecar
dependency: Project validation authenticates it, while runtime HIR and runtime
Graph projection deliberately omit it.

Before mutation, candidate admission reparses each selected authenticated source
module and requires exact Program equality. The receiver's retained record HIR
must match its source name, span, monomorphic shape, and every ordered field's
stable identity, name, span, index, and recursively resolved type. Every
selected function must match retained HIR in name and span, each parameter's
name, span, ownership and recursively resolved type, return type, effects, and
requires/ensures inventory. Bare source `string` parameters normalize only to
the compiler's existing owning `String` mode. Nominal resolution is bounded to
64 levels and 65,536 authentication work items.

The source checker validates this complete table before candidate admission.
Every subsequent candidate operation preserves the existing source-owned
implementation identity, owner/module, protocol, and member-ID mapping. These
facts are checked before formatting and again after source replay. Global
source identity checks include protocol and implementation declarations even
where the runtime graph does not index them. Renaming a function's display name
can retain conformance because its ID is unchanged; an edit introducing a
forbidden precondition or incompatible signature fails the normal source check.

The new implementation remains in exact Semantic Change history, replay, and
recovery. A stale or failed application cannot change the existing candidate
or original files. Conservative rebase and merge compare a closed
compiler-owned dependency fingerprint at each original and destination
intermediate revision before replay. It binds the exact receiver, protocol, and
destination identities and shapes, relevant import bindings, the ordered
required-member signatures, the normalized
method-to-function mapping, each selected function's conformance-relevant
signature/effects/precondition facts, the vacant receiver/protocol pair, and
the globally absent new implementation ID. Mapping input order is immaterial
because the admitted table is normalized by method ID.
Named receiver-field and selected-function signature types also bind retained
checked-HIR identities, and protocol method source order remains part of the
protocol shape. Thus unchanged type spelling or an import alias cannot hide a
nominal identity substitution.

Selected-function body, postcondition, and display-name edits do not change
static conformance and are excluded from this fingerprint. They may therefore
survive conflict selection, as can an unrelated source edit, but the whole
intention is still reconstructed and passed through ordinary Project candidate
admission on the exact destination. Receiver or protocol display/shape drift,
required-member drift, selected-function signature/effect/precondition drift,
an occupied pair, and any new source identity collision reject with `SPX-G235`.
There is no selector guessing, same-spelling recovery, behavioral implication,
dynamic-dispatch compatibility, or dependency remapping. Dependencies created
only by another not-yet-replayed history remain outside this conservative
route.

## Evidence and boundaries

The existing runtime graph does not gain a fabricated declaration or call edge.
Candidate summaries carry the exact new source implementation fact. Its impact
entry explicitly reports `source_static_conformance_only` and unavailable
cross-file runtime impact. Semantic deltas can select source implementation,
protocol, and protocol-method IDs using authenticated authored source spans.
Related conformance facts also attach to receiver and implementing-function
deltas. Existing ordinary targets without related conformance do not gain an
empty new facet. These are recomputable static facts, not runtime dispatch,
behavioral equivalence, dynamic impact, or test coverage evidence.

The intention admits at most 64 members, below the broader source checker
limit. Discovery has at most 65,536 member/candidate entries and a 1 MiB output
cap; matching paths have at most 64 member steps. Source identity and
implementation inventories are bounded to 65,536 entries. Existing Semantic
Change, candidate history, source, static-protocol, and Project limits remain
active. These are structural/output bounds, not a total heap or latency promise.

`SPX-G272` rejects malformed mappings, unavailable subjects, duplicate or
incomplete selections, incompatible member signatures, and identity collisions.
`SPX-G273` reports candidate interface capacity. `SPX-G274` rejects a changed
source implementation inventory. `SPX-G497` rejects an absent or ambiguous
destination, `SPX-G498` rejects import conflicts or alias exhaustion, and
`SPX-G499` rejects missing or ambiguous retained source/HIR bindings.
`SPX-G235` owns conservative rebase/merge
dependency, pair-occupancy, identity, and unsupported-history conflicts. The
source static-conformance `SPX-Q1xx`, Project, and ordinary candidate
stale/replay diagnostics remain authoritative where delegated.

[`tests/project_candidate/interface.rs`](../tests/project_candidate/interface.rs)
authors discovery, same-module and cross-module source additions, canonical
dependency imports, selected delta verification, exact replay/recovery, no-write
behavior, absent-destination and incompatible-function rejection, display rename
preservation, precondition revalidation, and absence of the static protocol and
implementation identities from the real candidate Semantic Graph.
[`tests/project_candidate/interface_rebase.rs`](../tests/project_candidate/interface_rebase.rs)
authors conservative rebase/merge success across unrelated body and selected
display edits, exact replay equivalence, unchanged parents/files, the absence
of a fabricated runtime-graph declaration, and fail-closed receiver, protocol,
selected-function, pair, and implementation-identity conflicts. A focused unit
regression checks one-to-one discovery matching. All are unrun at the user's
request; no compiler, interpreter, target, or local quality gate was executed
for this change.
A focused authored Workspace Graph unit regression independently checks that
`use protocol`, the protocol and member identities, and the source
implementation identity are absent from both runtime Graph declarations/edges
and the operation-sidecar declaration/import inventories. Ordinary type and
function imports remain present in the sidecar.
