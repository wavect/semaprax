# Project Interface Change v1

Status: Partial; implementation and regression tests authored, unrun.

Audience: agent builders, compiler contributors, and reviewers.

`implement_interface` appends one actual source `impl` declaration that binds a
local record to a local protocol's complete member table. The binding names
existing functions by persistent identity. The ordinary source checker validates
static conformance, and candidate application independently rebuilds the full
Project and its admitted target projections. This does not generate function
bodies, lower dynamic dispatch, or introduce a runtime witness table.

## Intention and discovery

```json
{
  "kind": "implement_interface",
  "target": "example.counter",
  "protocol": "example.readable",
  "id": "example.counter.readable",
  "members": [
    {"method": "example.readable.read", "implementation": "example.read"}
  ]
}
```

These are the exact outer and member object fields. `target` selects one
explicit monomorphic record. `protocol` selects an explicit protocol in that
record's module. Each `method` is an explicit required member, and each
`implementation` is an existing explicit ordinary non-`main`, monomorphic
function in the same module. Every required member must be selected exactly
once, and implementation functions must be distinct. The new implementation ID
must be globally fresh. Selectors use the source static-conformance grammar:
1–240 ASCII letters, digits, underscores, dots, colons, or hyphens. The new ID
cannot use the `auto:` or `semaprax.` prefixes or collide with prelude identities.

`ProjectCandidate::interface_catalog(expected_candidate, target)` returns
`semaprax.project-interface-change-catalog.v1`, bound to the exact candidate and
Project revision. It lists local protocols, required signatures and modes,
eligible function IDs for each member, and an existing implementation if present.
`complete_mapping_available` requires an actual one-to-one matching, not merely
one candidate per member. The ordinary change catalogue advertises
`implement_interface` when a complete new mapping is available. Discovery never
admits a proposed implementation or selects a preferred table automatically.

The compiler's shared `static_protocol::member_matches` owns signature
eligibility. The receiver position substitutes protocol `Self` or the protocol
name with the actual record type. Remaining types, parameter modes, arity, and
return type must match exactly. Implementation functions cannot add effects or
preconditions; ordinary postconditions are permitted. The ordinary verifier
still owns all body, ownership, cleanup, and backend admission. Discovery does
not designate an owning record as Copy or relax an unsupported ownership mode.

## Canonical source and persistent identities

The operation constructs a typed AST declaration, orders bindings by required
method ID, and leaves projection to the canonical formatter. The output has
ordinary source form:

```text
@id("example.counter.readable")
impl "example.readable" for "example.counter" {
    "example.readable.read" = "example.read";
}
```

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
or original files. Rebase of this operation rejects explicitly and requires
fresh discovery on the intended base; it does not silently remap protocol,
receiver, or member identities. Histories retained unchanged by merge remain
subject to ordinary exact replay.

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
source implementation inventory. `SPX-G275` rejects implicit implementation
rebasing. The source static-conformance `SPX-Q1xx`, Project, and ordinary
candidate stale/replay diagnostics remain authoritative where delegated.

[`tests/project_candidate_interface_v1.rs`](../tests/project_candidate_interface_v1.rs)
authors discovery, real source addition, selected delta verification, exact
replay/recovery, no-write behavior, incomplete/wrong/duplicate/colliding mapping
rejection, display rename preservation, precondition revalidation, and explicit
rebase rejection. A focused unit regression checks one-to-one discovery
matching. All are unrun at the user's request; no compiler, interpreter, target,
or local quality gate was executed for this change.
