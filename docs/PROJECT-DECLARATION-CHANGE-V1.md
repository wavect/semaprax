# Typed Project Declaration Change v1

Audience: compiler contributors and agents constructing Project candidates.

Status: authored, unrun implementation and regression evidence. Local compiler,
test, and long quality gates were deliberately skipped at the user's request;
this document makes no verified completion or target-execution claim.

The additive `add_declaration` intention creates one explicit, monomorphic
function, record, or variant in an existing Project module. Functions cannot
be named `main`; record/variant fields use the closed data-type vocabulary
below. The intention travels through
the existing [Semantic Change and Candidate](PROJECT-CANDIDATES-V1.md) envelope,
revision checks, full source reconstruction, verifier, Project profile admission,
target projection, and exact replay. It does not edit canonical Git source or
change the Project manifest, imports, exports, capabilities, or module permits.
Existing intentions and image-v1 serialization are unchanged.

## Function constructor

Every illustrated object key is required. There are no implicit parameters,
contracts, effects, source paths, or source-text defaults.

```json
{
  "kind": "add_declaration",
  "target": "calculator.add",
  "declaration": {
    "id": "calculator.increment",
    "name": "increment",
    "parameters": [{"name": "value", "type": "i64", "mode": "value"}],
    "return_type": "i64",
    "effects": [],
    "requires": [],
    "ensures": [{"kind": "bool", "value": true}],
    "body": {
      "kind": "call",
      "target": "calculator.add",
      "arguments": [{"kind": "place", "name": "value"}, {"kind": "i64", "value": 1}]
    }
  }
}
```

`target` authenticates one existing explicit monomorphic top-level anchor. A
`main` anchor is allowed because its signature and body remain unchanged. The
new function is appended after the module's existing function declarations;
the anchor does not grant a filesystem path or arbitrary insertion position.

The new ID is 1–128 ASCII bytes from `[a-z0-9._-]`. It must be unused in the
complete retained declaration graph and module bindings, including IDs from
other modules and nested declaration kinds. The display name is one bounded
ordinary identifier, cannot be `main`, and cannot collide with a function,
type, interface, protocol, or imported alias in the destination module.

| Parameter type | Required mode | Allowed result |
| --- | --- | --- |
| `i64`, `i32`, `u8`, `usize`, `bool` | `value` | Yes |
| `Bytes` | `own` | Yes |
| `string` (owned String) | `value` in source, checked as `own` | Yes |
| `str`, `Slice<u8>` | `borrow` | No |
| Stable-ID nominal object for a checked Copy record/variant | `value` | Yes |
| Stable-ID nominal object for a checked resource-free owning record/variant | `own` | Yes |

Existing string types retain their wire representation. A named parameter or
return type uses the closed object below, selecting a type owner rather than a
case, field, source spelling, or new declaration:

```json
{"kind":"nominal","target":"calculator.box","type_arguments":["i64"]}
```

All three keys are required. Monomorphic types use `type_arguments: []`;
generic instances require exact declared arity and only direct `i64` or `bool`
arguments, with at most 4,095 arguments. Source types require an explicit owner
and complete explicit field/case/payload identities plus one existing visible
local or imported binding. Generic source types remain module-local; this
change does not admit generic type imports. Compiler-owned Option/Result use
the separately authenticated fixed prelude inventory. Nested type arguments,
classes, resources, inferred arguments and newly introduced imports are closed.

Selection is provisional. After full candidate rebuilding and identity replay,
every added function, including compiler-derived extraction, passes an exact
checked-signature gate before a candidate is exposed. Every nominal parameter
and return must resolve to a sized, resource-free record or variant. A `value`
nominal parameter requires checked value ownership, `copy` and no `needs_drop`.
An `own` nominal parameter requires checked owning mode, no `copy`, and
`needs_drop`. Nominal returns may belong to either checked data category;
borrowing and resources remain excluded. An owning type requested in `value`
mode and a Copy type requested in `own` mode still reject.

The owned String request uses lowercase `string` and `mode: value`, matching
the language's bare String parameter syntax. Its actual HIR mode must be `own`,
and ordinary type facts must establish sized, non-Copy, resource-free storage
that needs cleanup. This does not add `own string`, borrowed nominal parameters,
new imports or a wider callable/target profile. The extraction planner retains
its separate immutable Copy capture/result rules; sharing append validation
does not authorize owned capture or result extraction. The independently
guarded [nested-block extraction lane](PROJECT-EXTRACTION-V1.md#nested-blocks-with-internal-owned-data)
can retain internal owners without passing them across the new call boundary.

Template shapes never prove those properties. The retained facts include return-only instances and
share the existing per-module limit of 4,096 distinct nominal types and
builder-byte budget with checked body-value facts used by extraction. Fresh generic instances need not have appeared in an
earlier function signature, but must pass this rebuilt admission.

Catalogue `nominal_types` rows describe available selectors and provenance,
with `copy_admission: "checked_candidate_signature"` and
`requires_full_candidate_validation: true`. They do not claim every selected
template or concrete argument combination is Copy. The inventory is bounded
to 65,536 entries/parameter items and 1 MiB before the enclosing 256 KiB
catalogue limit.

The operation's separate
`nominal_owning_admission: "checked_candidate_owning_signature"` marker describes
the owning path; it does not turn template discovery into proof of eligibility.
Owning nominal parameters use monomorphic owners with no type arguments.

Parameters must have distinct names and cannot be named `result`. There are at
most 64 parameters, 64 effects, 64 preconditions, and 64 postconditions. Effects
are sorted and unique, and every effect must already occur in both the anchor's
declared effect budget and the destination module's permits. Creation cannot
widen either budget. These list limits supplement existing bounded change
bytes, aggregate JSON nodes, constructor depth, and complete Project limits;
they are not a claim that arbitrary HIR memory is bounded by report size.

Bodies and contracts use the existing closed typed expression constructors:
bounded scalar literals, parameter places, stable-ID calls, unary/binary
operators, conditionals, and the admitted
[aggregate constructors](PROJECT-AGGREGATE-CONSTRUCTORS-V1.md). Requires predicates see parameters; ensures
predicates additionally see `result`. Calls resolve only through the
destination module's admitted local and imported function bindings. Initial
construction cannot call the function being created, and cannot introduce a
new import or directly name an inaccessible function. Actual types, argument
modes, borrow escape, ownership, cleanup, effects, contracts, and target support
remain the real compiler's responsibility. A syntactically valid expression
does not establish that its contract is mathematically true.

The ordinary candidate path formats and reparses these compiler-owned ASTs,
rebuilds all held Project sources, and checks exact preserved facts. The only
new declaration identity permitted is the planned explicit function at its
authenticated path/module. All prior explicit identities and their ownership,
all prior effect/contract inventory facts, module permits, and the manifest
must remain unchanged. Failed construction exposes no candidate and leaves
both the original candidate and source files unchanged.

## Record and variant constructors

The function object above retains its original wire representation without a
`kind` field. Two additive closed declaration objects create types:

```json
{
  "kind": "add_declaration",
  "target": "calculator.add",
  "declaration": {
    "kind": "record",
    "id": "calculator.configuration",
    "name": "Configuration",
    "fields": [
      {"id":"calculator.configuration.amount","name":"amount","type":"i64"},
      {"id":"calculator.configuration.enabled","name":"enabled","type":"bool"}
    ]
  }
}
```

```json
{
  "kind": "add_declaration",
  "target": "calculator.add",
  "declaration": {
    "kind": "variant",
    "id": "calculator.decision",
    "name": "Decision",
    "cases": [
      {"id":"calculator.decision.accept","name":"Accept","fields":[
        {"id":"calculator.decision.accept.value","name":"value","type":"i64"}
      ]},
      {"id":"calculator.decision.reject","name":"Reject","fields":[]}
    ]
  }
}
```

All keys are required. The anchor remains one existing explicit monomorphic
function, including `main`; the new type appends to that module's type list.
The type, each case, and every field require their own globally fresh explicit
IDs under the same ID grammar as new functions. This includes conflicts with
existing functions, types, members, import bindings and source-only interface
identities. Type display names must not collide in the module namespace; case
names are unique within a variant and field names within their record or case.
Array order determines canonical field/case order and is never sorted away.

The field request vocabulary accepts `i64`, `bool`, `i32`, `u8`, `usize`, `string`, and `Bytes`,
or the same closed stable-ID nominal selector shown above. A nominal field
selects an already admitted visible record or variant, with exact generic arity
and direct `i64`/`bool` arguments. It cannot select the type currently being
created, invent a source spelling, or introduce an import. Selection remains
provisional until complete source rebuilding and checked data-type admission.
The owned String type uses lowercase `string` in this request and canonical
source; `String` is not an alias in the constructor vocabulary. Fixed arrays
are not part of this field constructor.
Owned fields do not gain borrowed storage, resource authority or a wider
Project/target profile. The function-signature constructor separately checks
its requested ownership mode and rebuilt type facts. A newly created owned
type can subsequently be selected by a local owning helper, but creating it
does not grant cross-module owning imports or waive function/target admission.

This vocabulary does not widen aggregate source profiles. In particular, a
variant without a direct `Bytes` payload retains the existing `SPX-T215`
restriction on payload types beyond `i64`/`bool`; putting `string`, `i32`, `u8`
or `usize` in such a variant still rejects. Nesting a generic record instance
in a record field retains `SPX-T223`. Other source, HIR and selected-target
restrictions also remain authoritative. These are explicit rejection cases,
not successful type-creation evidence.

The post-rebuild gate computes ordinary type facts for the new monomorphic
owner and its selected dependency closure even when unused. It requires
`sized` and no resources, rejects class/resource dependencies and preserves
the helper's existing limits: 4,096 declarations, 1,048,576 visits, depth 256
and 16 MiB input/output accounting. No global retained type-facts limit changes.

Empty records and payload-free cases
are allowed subject to ordinary Project admission; variants require at least
one case. Each record/case has at most 64 fields, a variant has at most 64
cases, and the complete planned identity inventory has at most 4,096 entries,
including its owner. Existing JSON, source, graph and candidate limits still
apply and can reject smaller requests. Generic declaration parameters, borrowed
views, resources, methods, custom layout, imports and manifest exports are not
constructor fields.

The compiler constructs source AST declarations, renders canonical source, and
performs full Project rebuilding and replay. Identity admission permits only
the exact planned owner/member facts, including declaration kind, explicit
origin, owner, source path and module. All prior explicit identities must remain
unchanged. A separate reconstruction pass derives the same append from the
original revision and compares every resulting canonical source. Neither the
request nor the identity inventory can inject graph-only meaning.

Subsequent intentions can construct values of the new type, use it through a
stable-ID nominal function signature, or extend a new record through the
existing field-change route when its own eligibility checks hold. Every step
re-enters ordinary source admission.
The catalogue's additive `type_declaration_forms` describes the two placements
and bounds, lists direct `field_types`, and points `nominal_type_selector` to
the existing provisional `nominal_types` inventory. Its field admission marker
is `checked_resource_free_field_type`. The shared nominal rows' function Copy
metadata does not impose Copy on fields or prove field eligibility. Existing
function placement and the separate checked signature rules still apply.

## Reports, composition, and remaining boundaries

Only new-operation summaries add `new_declaration` with `id`, `name`, `path`,
and `module`. Type additions also carry `kind` and a complete `identities`
inventory of exact owner/member facts. Impact includes new identities with a null base-side report;
null denotes absence from the original source revision, not missing proof for
an existing function. Later candidate intentions can address this stable ID.
Semantic rebase and merge replay creation and subsequent changes in order,
check collisions on the new base, and retain the original comparison base and
parent bindings. Exact candidate replay reconstructs the declarations from
the retained source base and typed intentions; serialized AST/HIR never gains
admission authority.

Type creation contributes every nested member ID to semantic rebase collision
checks. An independently created function occupying a new case or field ID is
a conflict before replay, even when the new type owner itself is unused.
Records introduced by an earlier history step can be extended later in that
history; their shape is checked through intermediate candidate admission.

Nominal type objects, including those in new record and variant fields,
additionally bind the complete checked owner inventory
at each original/rebased intermediate revision. Reidentifying an untouched
field or unit case conflicts even when the added body only forwards a parameter
and contains no aggregate expression constructor. This is conservative shape
dependency checking, not transitive semantic equivalence.

`SPX-G225` reports malformed or unsupported declaration constructors, ID/name
collisions, inaccessible scope, and disallowed effect/mode requests.
`SPX-G226` reports constructor list capacity. Full source verification and
Project admission retain their ordinary diagnostics; stale candidate/change
digests retain the existing candidate diagnostics.

`src/project/candidate/declaration.rs` owns construction and the shared internal
`append_function` helper used by compiler-derived extraction. Parent candidate
dispatch and invariant/identity checks live in `candidate/mod.rs`; semantic
composition lives in `candidate/rebase.rs`. Authored regressions in
`tests/project_candidate_declaration_v1.rs` cover canonical replay without
source writes, creation followed by rename/body change and merge, a `main`
placement anchor with existing imports, ID/name collisions, unauthorized
effects, invalid ownership modes, result scope, raw-source fields, malformed
bodies, list bounds, and borrowed-byte forwarding to an owned-byte result.
These regressions have not been run.

`candidate/aggregate_nominal.rs` owns nominal selector authentication and
template discovery; `declaration.rs` owns requested-mode preflight and the
post-build checked signature gate. Copy and owning modes remain distinct.
`tests/project_candidate_nominal_declarations_v1.rs` adds authored, unrun cases
for unused generic instances, return-only records/variants, monomorphic aliases,
Option/Result, malformed selectors, non-Copy signatures, recovery and no writes.
`tests/project_candidate_rebase_v1.rs` adds type-only dependency conflicts.

`tests/project_candidate_owned_declarations_v1.rs` adds authored, unrun
composition of data-type creation and local owning helpers, String forwarding,
checked ownership/cleanup evidence, exact replay, mode rejection and unchanged
import/source-profile limits. These are not physical execution or allocation
conformance results.

`candidate/type_declaration.rs` owns record/variant construction, exact planned
identity inventories and independent source reconstruction.
`tests/project_candidate_type_declarations_v1.rs` adds authored, unrun type
creation, downstream use, identity rejection, bounds and recovery cases. Rebase
regressions cover creation followed by record evolution and nominal use, plus
nested identity collisions against independently admitted candidates.
The former scalar-only field exclusions have explicit positive replay cases
for `Bytes`, `i32` and existing nominal records; borrowed and self-reference
inputs retain negative coverage. Additional data-field and ownership cases in
`tests/project_candidate_data_type_declarations_v1.rs` are authored, unrun.

Creating generic types, classes, resources, interfaces,
protocols, methods, generic functions, modules, public exports, new imports,
arbitrary structured types, or package entries remains outside this slice.
General recursive creation, new authority, independently verified target
execution, and full programme completion require additional evidence.
