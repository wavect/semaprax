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

Supported type/default pairs are `bool`, `i64`, `i32`, `u8`, and `usize`.
Integer literals must be exact JSON integers with a source-representable
magnitude: `-i64::MAX..=i64::MAX`, `-i32::MAX..=i32::MAX`, unsigned 8-bit,
or unsigned 64-bit `usize`, respectively. The frozen lexer parses the positive
magnitude before unary minus, so `i64::MIN` and `i32::MIN` are not admitted
literal defaults; the intention rejects them with `SPX-G225` before migration.
This does not narrow runtime integer values or widen source syntax.
The literal kind must equal the field type. Calls, places,
expressions, source strings, unknown keys, and implicit conversions are not
accepted defaults. The new stable ID must be globally unused and use one to
128 lowercase ASCII ID characters; the name must be a bounded ordinary field
identifier and must not already occur on the target record.

## Eligibility and identity

The target is an explicit, monomorphic, authored record from retained validated
HIR. Eligibility uses the compiler's checked type facts for sized,
resource-free records, including both Copy records and records with owned
cleanup. Empty records are allowed. Already-admitted String, Bytes, array and
nested record/variant storage need not match the flat owned-byte pattern
profile merely to receive an inert scalar field. The selected record's exact
dependency closure must still have compiler-derived type facts; display names
never establish Copy or ownership facts.

For an unused record, eligibility reconstructs only its selected nominal
dependency closure from the retained checked declarations and compiler prelude.
The ordinary HIR TypeFacts owner computes Copy, drop, resource, and layout facts;
the operation does not infer them from function use or duplicate those rules.
This temporary index is neither retained nor a new source of graph authority.

Generic target records, classes, variant targets, resources, borrowed storage,
and types without admitted bounded compiler facts remain excluded. A newly
appended field is always an inert Copy scalar, never another owned field,
allocation, resource, reference, or implicit ownership transfer. This broadens
the semantic intention, not the language's aggregate or backend admission.

Module source revision/digest facts and the target declaration's source origin
must match the retained Project. Local type bindings and imported aliases map
to persistent identities; imported aliases must name the authenticated owning
module. Thus an imported `Point` renamed locally to `Metric` still selects the
same record, while another record with a similar display name is untouched.
Alias migration applies only where Project already admits the import: Copy
record aliases are supported; owned-record type aliases and owned-argument
function imports remain rejected by `SPX-G172`. Owned-record migration evidence
uses local declarations without widening that cross-module boundary.
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
The default has no effects or allocation and cannot fail when evaluated;
negative defaults retain the ordinary checked unary-negation representation.

Exact record patterns are traversed recursively, including nested field
patterns. Each affected exact pattern must match the old field inventory and
receives the new field with `_`. Existing names, binding positions, stable field
references, and nested patterns remain unchanged. Whole-record binding and
wildcard patterns need no new binding. Record updates are not expanded: the
ordinary compiler copies the new field from the base unless explicitly changed
by later source, preserving existing update and projection semantics.

For an owned record, explicit `match own` and `match borrow` modes stay
unchanged. Every old droppable field retains its required binding; only the
new non-droppable scalar receives `_`. The migration never discards an owned
field or manufactures a second owner. Existing direct field loans retain their
root and persistent field identities. Full source admission still checks loan
overlap and last use after migration.

Target eligibility does not extend the language's pattern or borrowing
profiles. In particular, admitting an existing nested owned record for scalar
field addition does not admit a new nested owning match, projected loan or
import. A source that requires an unsupported operation still fails ordinary
Project verification before a candidate is returned.

The broader target evidence includes migration of checked String-bearing
constructor bodies outside the selected executable closure. Current aggregate
String target layout remains unsupported; retaining and changing those source
bodies does not establish native or Wasm execution of them. Likewise, nested
or mixed owned-Bytes storage remains subject to the existing `SPX-T268` source
gate, even if a hypothetical type-fact calculation would be resource-free.

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

Appending a non-droppable field adds no owned cleanup leaf. Cleanup inventories
and plans are rebuilt from the complete candidate, retaining the compiler's
canonical order; they are not copied, sorted, repaired, or declared byte-equal
merely because the old owned field identities remain unchanged.

After admission, the operation independently reconstructs the field declaration
and every migration from the prior immutable revision. All canonical candidate
sources must match this reconstruction exactly. It also compares the old
ordered checked field identities, names and types against the retained prefix
and confirms that the single added field has its requested identity and scalar
type. The selected record's Copy, drop, resource and sized flags must remain
unchanged; its layout is intentionally allowed to change. These checks do not
copy or reinterpret cleanup vectors. Existing candidate replay binds
the full change history and canonical evidence. Rebase checks record-shape
conflicts and new-ID collisions before replaying the migration in the new
revision; unrelated function display renames can coexist, while concurrent
changes to the same record shape fail closed.

The final target record has at most 64 fields. Selected type reconstruction has
at most 4,096 source declarations, 1,048,576 visits, depth 256, and a 16 MiB
charged-input budget; TypeFacts rendering has a separate 16 MiB output bound.
Existing global nominal guards remain unchanged. Expression and pattern
traversal depth is at most 256.
Expression migration and inserted items share a 1,048,576 item ceiling; pattern
traversal is separately bounded to that many items. Existing Project source and
candidate byte limits still apply. Invalid/unsupported operations use
`SPX-G225`; capacity failures use `SPX-G226`. Existing candidate stale/replay and
rebase diagnostics remain unchanged.

Successful and failed operations affect only private in-memory candidates.
They do not rewrite source files, persist images, publish `ACTIVE`, or commit
Git changes.

## Authored evidence and remaining scope

`tests/project_candidate/record_field.rs` contains authored, unrun cases for
cross-module aliases, constructor ordering, contract constructors, nested exact
patterns, unchanged updates, lazy failure placement, exact replay, recovery, and tampering,
stale requests, global ID/name collisions, default/type rejection, boolean
fields, generic rejection and unused owned-record admission, unchanged source bytes, unrelated rename
merges, and competing record-shape rejection.

`tests/project/owned_record_field_addition.rs` adds authored, unrun cases for
flat owned-byte records, initializer order, owning match bindings, live field
loans and cleanup order, imported aliases, broader checked Copy fields, and
scalar default ranges.

`tests/project/resource_free_record_evolution.rs` authors broader owned
target cases and their ordinary source-admission boundaries. These cases are
unrun; they do not establish new runtime, matching, borrowing or ABI support.

No local tests, compiler checks, or long gates were executed, as requested by the
user. These cases are not passing completion evidence. General record evolution,
field removal/reordering, generic record migration, new owning fields, arbitrary defaults,
public ABI compatibility, and the full graph-operational roadmap remain open.
