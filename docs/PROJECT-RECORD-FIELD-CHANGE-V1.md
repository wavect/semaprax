# Project Record Field Change v1

Status: Partial; implementation and regression evidence authored, unrun.

Audience: compiler contributors and agents editing immutable Project candidates.

`add_record_field` appends one explicitly identified scalar field to an existing
record and derives all required authored constructor and exact-pattern changes.
Canonical `.spx` remains authoritative. This operation introduces no syntax,
backend exception, arbitrary source edit, graph mutation, or publication power.

## Closed request

The existing Semantic Change envelope binds the current Project revision and
fixed candidate requirements. Its intention is exactly:

```json
{
  "kind": "add_record_field",
  "target": "geometry.point",
  "field": {
    "id": "geometry.point.visible",
    "name": "visible",
    "type": "bool",
    "default": { "kind": "bool", "value": false }
  }
}
```

The other supported type/default pair is `"i64"` with an exact signed 64-bit
JSON integer. The literal kind must equal the field type. Calls, places,
expressions, source strings, unknown keys, and implicit conversions are not
accepted defaults. The new stable ID must be globally unused and use one to
128 lowercase ASCII ID characters; the name must be a bounded ordinary field
identifier and must not already occur on the target record.

## Eligibility and identity

The target is an explicit, monomorphic, authored record from retained validated
HIR. Every existing field must be directly `i64`, `bool`, or another admitted
monomorphic Copy record whose fields recursively meet the same rule. Empty
records are allowed. Generic records, classes, variants, owned or borrowed data,
resources, and other scalar/aggregate field kinds are outside this version.
Eligibility uses memoized nominal-type traversal with a depth bound; it does not
infer Copy semantics from display names.

Module source revision/digest facts and the target declaration's source origin
must match the retained Project. Local type bindings and imported aliases map
to persistent identities; imported aliases must name the authenticated owning
module. Thus an imported `Point` renamed locally to `Metric` still selects the
same record, while another record with a similar display name is untouched.
The addition records the exact field ID, name, record owner ID, source path,
and module. All old identities and fields retain their existing order.

## Derived migration

The compiler traverses all authored function and class-method bodies, including
uninstantiated generic bodies, preconditions, postconditions, guards, loops,
unsafe blocks, nested constructors, and update expressions. An affected record
constructor must exactly match the old field-name inventory. Its old initializer
sequence remains unchanged and the inert default literal is appended last.
This preserves left-to-right evaluation of the old values and their checked
failure order; an initializer that was lazy remains at its original position.
The new literal has no effects, allocation, or checked-failure path.

Exact record patterns are traversed recursively, including nested field
patterns. Each affected exact pattern must match the old field inventory and
receives the new field with `_`. Existing names, binding positions, stable field
references, and nested patterns remain unchanged. Whole-record binding and
wildcard patterns need no new binding. Record updates are not expanded: the
ordinary compiler copies the new field from the base unless explicitly changed
by later source, preserving existing update and projection semantics.

The operation is append-only. It neither reorders old fields nor renames old
members. It intentionally changes record layout and adds a new field to values;
it does not claim identical target bytes, physical layout, ABI, memory use,
execution fuel, or performance. Public aggregate compatibility is not inferred
from unchanged export IDs.

## Independent admission and bounds

The candidate pipeline canonically formats, reparses, and independently rebuilds
the complete Project under its existing profile. Ordinary ownership, cleanup,
record-layout, and admitted target checks remain mandatory; the operation never
relaxes them. Native C and structurally validated Core Wasm target facts are
rederived where the base lane is admitted, without claiming target execution.

After admission, the operation independently reconstructs the field declaration
and every migration from the prior immutable revision. All canonical candidate
sources must match this reconstruction exactly. Existing candidate replay binds
the full change history and canonical evidence. Rebase checks record-shape
conflicts and new-ID collisions before replaying the migration in the new
revision; unrelated function display renames can coexist, while concurrent
changes to the same record shape fail closed.

The final target record has at most 64 fields. Existing nested records must also
meet that bound. Type, expression, and pattern traversal depth is at most 256.
Expression migration and inserted items share a 1,048,576 item ceiling; pattern
traversal is separately bounded to that many items. Existing Project source and
candidate byte limits still apply. Invalid/unsupported operations use
`SPX-G225`; capacity failures use `SPX-G226`. Existing candidate stale/replay and
rebase diagnostics remain unchanged.

Successful and failed operations affect only private in-memory candidates.
They do not rewrite source files, persist images, publish `ACTIVE`, or commit
Git changes.

## Authored evidence and remaining scope

`tests/project_candidate_record_field_v1.rs` contains authored, unrun cases for
cross-module aliases, constructor ordering, contract constructors, nested exact
patterns, unchanged updates, lazy failure placement, exact replay, recovery, and tampering,
stale requests, global ID/name collisions, default/type rejection, boolean
fields, generic/owned rejection, unchanged source bytes, unrelated rename
merges, and competing record-shape rejection.

No local tests, compiler checks, or long gates were executed, as requested by the
user. These cases are not passing completion evidence. General record evolution,
field removal/reordering, generic and owned record migration, arbitrary defaults,
public ABI compatibility, and the full graph-operational roadmap remain open.
