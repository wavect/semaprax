# Project Aggregate Expression Constructors v1

Status: authored, unrun; an additive constructor slice of the graph-operational
programme. No completion-matrix promotion or runtime evidence claim.

Audience: agent builders, compiler contributors, and reviewers.

## Stable-ID requests

Record and variant construction use these two closed recursive expression
shapes, each with an optional `type_arguments` array:

```json
{"kind":"record","target":"payments.money","fields":[{"target":"payments.money.amount","value":{"kind":"i64","value":7}}]}
```

```json
{"kind":"variant","target":"payments.status.ready","fields":[{"target":"payments.status.ready.value","value":{"kind":"i64","value":7}}]}
```

Record targets identify an explicitly identified source record. Variant targets
identify an explicit case belonging to an explicitly identified source variant.
Each field target identifies a member of that exact record or case. The compiler
derives all source names from authenticated declarations and the destination
module's existing local or imported type binding. Requests cannot supply source
declaration spellings, aliases, paths, spans, HIR, graph facts, or a new import.

Every field must occur exactly once. Missing, repeated, foreign, and unknown
field identities reject. The input array remains initializer evaluation order;
the compiler must not sort expressions into declaration order. Each value uses
the same recursive expression vocabulary and shared depth/node accounting.
Empty records and unit cases use an empty field array.

Generic records and variant cases require exactly the template's number of
explicit type arguments, each the compiler's admitted direct `i64` or `bool`
type. Order determines the concrete nominal identity, including parameters
unused by any field. The compiler does not infer arguments from field values,
and matching field layouts do not make two nominal instances interchangeable.
Monomorphic requests may omit `type_arguments` or supply an empty array.
Generic source types must have a module-local binding: the existing linker
still rejects cross-file generic type imports with `SPX-G172`. Imported
monomorphic recursive Copy types retain their existing admission rules.

```json
{"kind":"record","target":"payments.box","type_arguments":["i64"],"fields":[{"target":"payments.box.value","value":{"kind":"i64","value":7}}]}
```

Compiler-owned `Option` and `Result` cases use the same shape and their exact
persistent identities. For example:

```json
{"kind":"variant","target":"core.option.some","type_arguments":["bool"],"fields":[{"target":"core.option.some.value","value":{"kind":"bool","value":true}}]}
```

`core.option.none` has no fields and one type argument. `core.result.ok` uses
`core.result.ok.value`; `core.result.err` uses `core.result.err.error`. Both
Result cases require two arguments in success/error order, even when the
selected case carries only one of them. Explicit arguments materialize as
ordinary canonical `.spx`, such as `Option<bool>::Some { value: true }`.

Classes, resource creation, nested or named generic arguments and general
pattern synthesis remain outside this constructor grammar. The separate
[field place constructor](PROJECT-FIELD-PLACE-CONSTRUCTOR-V1.md) selects a
direct field of an authenticated named root without value staging; its use in
a byte view retains the source language's existing borrow profile.
A named field type does not waive ordinary profile, ownership, effect,
or backend admission.

## Record field value projection

The `project` constructor selects an explicit source-record field identity:

```json
{"kind":"project","target":"payments.money.amount","base":{"kind":"place","name":"payment"}}
```

An optional `type_arguments` array supplies the exact owning record's generic
arguments under the same direct `i64`/`bool` rules. The compiler authenticates
the field's owner and complete explicit member inventory, then selects that
record's unique existing local or imported type binding. Variant payloads,
prelude case fields, classes, and implicit field identities are not admitted.

Lowering deliberately creates an ordinary typed value binding, conceptually:

```text
{
    let spx_project_0: Money = payment;
    spx_project_0.amount
}
```

The generated name is deterministic and avoids the constructor's lexical scope,
function/import/type bindings, and earlier generated names. The base expression
occurs exactly once. Its explicit owner annotation forces ordinary source
admission to check the complete nominal type and ordered generic arguments.
A different record with an identically named, identically typed field cannot
satisfy the request merely because `.amount` would otherwise type-check.

This is value projection, not a promise to borrow the original place. The
temporary follows the source language's normal copying, transfer, loan and
cleanup rules. An owned base may transfer into this scope; reusing a consumed
base, escaping an invalid loan, or violating cleanup obligations must fail
ordinary candidate admission. The operation neither duplicates the base nor
claims behavioral equivalence with arbitrary existing field-access code.

Use the separate `field_place` constructor when the base is an existing lexical
root whose nominal type can be authenticated and the field must remain a direct
place. It does not change `project` lowering or admit general nested borrowing.

Bodies, authenticated expression replacements, contracts, declaration bodies
and hole fills share the existing constructor/admission path. Generated locals
are visible in the canonical source diff; they are not graph-only state.

## Immutable record updates

The `update` constructor selects an explicitly identified source record owner
and a subset of its fields to replace:

```json
{"kind":"update","target":"payments.money","base":{"kind":"place","name":"payment"},"fields":[{"target":"payments.money.amount","value":{"kind":"i64","value":7}}]}
```

Optional `type_arguments` supply the exact record template's ordered direct
`i64`/`bool` arguments. The complete owner and field inventory must have
explicit identities, and the destination must have one existing local or
imported binding for that record. Classes, variant payloads and compiler-prelude
owners do not enter this route. A field with the same name on another record
is not a substitute for the selected identity.

Replacement targets form a subset: omitted fields retain the base's values
under ordinary source update semantics. Repeated, foreign or unknown fields
reject. An empty `fields` array is accepted structurally and remains an actual
update expression; it does not erase base evaluation or ownership operations.
Full source/profile/target admission still applies to that empty update.

Lowering stages the base once into a hygienic, exact-owner typed local, then
emits the existing `UpdateRecord` AST with the requested replacements. The base
is evaluated before every replacement, and replacement expressions stay in
request order. The compiler does not reconstruct untouched fields through
separate projections or reorder replacements into declaration order. For
example, canonical source contains the equivalent structure:

```text
{
    let spx_project_0: Money = payment;
    spx_project_0 with { amount: 7 }
}
```

The annotation checks complete nominal identity and generic arguments; field
spelling, result type or matching layout cannot waive that check. Name generation
also respects active match payload bindings. Updates can compose with other
typed constructors in bodies, authenticated expression replacements and hole
fills through the same ordinary admission path.

This is an immutable value update, not a mutation of a source place. Existing
copy/transfer, replacement settlement, loan and cleanup rules remain owned by
the source verifier and backends. A non-Copy base or replacement does not gain
permission to copy, escape a loan, reuse moved storage or skip a finalizer.
The constructor introduces no new owned-record or public aggregate ABI support.

## Exhaustive variant matching

The `match` constructor selects the owning variant identity, unlike a `variant`
constructor whose target is one case. It describes an exhaustive value match
through explicit case and payload field identities:

```json
{
  "kind": "match",
  "target": "core.option",
  "type_arguments": ["i64"],
  "value": {"kind": "place", "name": "payment"},
  "arms": [
    {"target": "core.option.none", "fields": [], "body": {"kind": "i64", "value": 0}},
    {"target": "core.option.some", "fields": [{"target": "core.option.some.value", "name": "amount"}], "body": {"kind": "place", "name": "amount"}}
  ]
}
```

Source variants require an explicit owner, every case, and every payload field.
The separate prelude route authenticates the complete selected Option/Result
family. The same unique local/imported binding and exact direct-scalar generic
argument rules apply. Every case occurs once, with every field of that case
bound once; omitted, repeated, unknown, or foreign identities reject. Unit
cases have an empty `fields` array. Requested arm and payload-field order is
preserved rather than sorted. This constructor has no wildcard, guard,
record/scalar pattern, nested pattern, or borrowed/owned match mode.

The compiler stages `value` once in a hygienic local annotated with the exact
nominal owner and ordered generic arguments, then emits an ordinary value
match over that local. Pattern source names are compiler-derived. A same-layout
variant or a phantom generic argument mismatch cannot substitute for the
requested owner. Generated source contains the staging local and all patterns;
the graph and cache contain no additional meaning.

Each payload's `name` is an explicit bounded local identifier. Bindings are
unique within an arm and cannot capture an existing lexical name, a callable,
type/import binding, or a generated temporary. `_` is not a binding name.
Arm bodies may refer to their own bindings; sibling bodies and the scrutinee
cannot. Nested matches inherit only their actual outer lexical scope.

Existing source verification remains responsible for Copy match eligibility,
consistent admitted arm result types, effects, contracts, ownership, cleanup,
and target admission. In particular this does not extend the language's
variant payload, aggregate arm-result, or ownership-aware matching support.
Construction and discovery are not evidence that an arbitrary match is valid.

## Checked bindings and candidate admission

Construction resolves the requested identities through the retained checked
module HIR and authenticates the source declaration/member relationship. A
unique existing type binding is required in the expression's destination
module. A declaration that exists somewhere in the project is not implicitly
in scope. Ambiguous aliases reject rather than being selected arbitrarily.

The prelude route separately authenticates the selected compiler-owned family's
fixed declaration/case/field identities, their kinds and owners, reserved names,
parameter arity and payload parameter slots against checked compiler HIR.
Scalar-only linked closures can omit unused prelude types; discovery then uses
the same fixed prelude declaration builder as the workspace linker, without
inserting those types into the linked closure or widening backend admission.
It does not weaken the explicit-ID requirement for authored subjects. Prelude
names are compiler bindings rather than invented imports; their definitions
remain part of the existing compiler prelude contract, not hidden candidate
state or new canonical files.

The expression lowers to the existing aggregate, projection and match AST forms. Whole-body
replacement, authenticated expression replacement, contract construction and
function declaration bodies share the revision-aware constructor. Body and
expression hole fills reuse those ordinary intention paths. There is no new
runtime syntax or backend exemption.

The complete candidate is canonically formatted, reparsed, rebuilt and subjected
to existing identity, contract, effect, ownership, cleanup, profile and target
preservation checks. Successful construction is not an admission receipt. A
failure leaves the immutable candidate/draft and authoritative source unchanged.
No constructor or discovery report grants filesystem or publication authority.

## Discovery and schemas

Change catalogues and hole contexts expose an optional nonempty
`aggregate_constructors` inventory for visible eligible types. Each descriptor
binds the kind, target, owner identity, checked name, source path/module, unique
local binding, and ordered field identities with checked type identities.
`evidence_owner` identifies retained checked HIR for source subjects; full candidate validation is
still required. These are available lexical constructors, not expected-type
filtering or a guarantee that arbitrary field values are legal.

Generic descriptors add ordered `type_parameters` entries with name, index and
the allowed direct types. Their field type identities describe the checked
template, including owner/index identities for parameters; these are not
substituted concrete field facts or proof that an instantiation is admitted.
The compiler avoids enumerating all possible argument combinations. Replaying
an actual typed request remains responsible for concrete admission.

Prelude descriptors have null source path/module, `identity_origin` equal to
`compiler_owned`, `evidence_owner` equal to `compiler_checked_prelude`, and a
`compiler_prelude` schema/digest binding. They do not
invent source provenance. Existing monomorphic source descriptors retain their
shape. Eligible catalogues and hole contexts now include the four prelude cases,
so whole-report bytes can change even for a scalar-only source project.

The self-contained [constructor schemas](CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md)
describe the recursive shapes. Schema validation alone cannot establish
membership, complete field coverage, unique aliases, lexical scope, types,
effects or ownership. Adding a discoverable constructor does not widen the
session's method set or grant execution/publication authority.

An optional nonempty `aggregate_projections` inventory describes visible
record fields. Entries bind field and owner IDs, field name/index/template type,
source path/module, the existing type binding, optional generic parameter
metadata, and `base_evaluation: once_into_typed_value_binding`. They do not
claim that any supplied base matches the owner or that a value-binding operation
preserves a loan. `project` is listed among available constructor kinds only
when this inventory is nonempty. Existing aggregate constructor entries are
unchanged; projects with no eligible record fields gain no projection property.

An optional nonempty `aggregate_updates` inventory describes eligible visible
source records, including empty records. Rows retain the complete checked
record constructor descriptor, with `kind: update`,
`base_evaluation: once_into_typed_value_binding`, and `field_coverage: subset`.
The full field inventory is guidance for selecting replacements, not a request
to replace every field. Generic rows retain their template parameter metadata.
No compiler-prelude variant row is included. The `update` constructor kind is
listed only when the inventory is nonempty; report presence does not prove
that a submitted base or replacement is admitted.

An optional nonempty `aggregate_matches` inventory describes each visible
variant owner and its complete declaration-ordered case/payload inventory.
Descriptors carry existing bindings, template field types, optional generic
parameters, retained-HIR evidence ownership for source subjects, and once-only
typed value staging. Prelude owners carry compiler-checked prelude evidence
ownership, the existing compiler-owned schema/digest provenance and
null source path/module. The `match` constructor kind appears when this
inventory is nonempty. The two prelude families may make it nonempty even for
a source project without its own variants. These are available shapes, not
exhaustiveness or type proofs for a submitted request.

## Rebase and limits

Before replaying each history step, semantic rebase compares referenced
aggregate descriptors between that step's original checked revision and its
current rebased revision. A missing target or changed member identity/order/type
rejects with `SPX-G235`, even when the intention's function target has only a
scalar signature. Generic parameter inventories, including phantom parameters,
and compiler-prelude provenance are included in dependency fingerprints.
The additive rebase normal form excludes known descriptor display names while
retaining identities, ordered shapes and generic parameter names. Source
location and other provenance can still produce conservative conflicts.
This is not general structural compatibility or a
transitive behavioral equivalence proof. Surviving intentions still undergo
complete candidate source admission.

Projection dependencies bind the selected field plus the complete checked
owning-record descriptor at each original/rebased intermediate revision.
Deleting or reidentifying a field, moving it to another owner, or changing the
owner's field/type-parameter inventory conflicts with `SPX-G235` before replay.
No same-spelling field fallback is used during rebase.

Update dependencies bind the whole checked record descriptor, including
untouched fields, before each history step is replayed. Reidentifying an omitted
field or changing any checked field/type-parameter inventory conflicts with
`SPX-G235`; preserving a remainder still depends on that exact record shape.
No empty-update shortcut bypasses this dependency check.

Match dependencies bind the complete checked variant owner, ordered cases,
payload identities/types, type parameters and any compiler-prelude provenance
at each original/rebased intermediate revision. Reidentifying even a unit case
or payload field conflicts with `SPX-G235` despite unchanged names and nominal
owner identity. Adding or removing a case cannot silently change an intended
exhaustive match. Accepted dependencies still undergo complete source replay.

Existing Semantic Change byte/JSON limits, recursive expression node/depth
limits, and catalogue/context rendering limits remain enforced. Identity and
shape errors use the constructor's `SPX-G225`; constructor capacity uses
`SPX-G226`. Source verification retains its own diagnostics.

An aggregate has at most 4,095 fields within the shared 4,096-expression-node
budget and depth limit of 64. Explicit type arguments also consume that shared
node budget, with an individual array maximum of 4,095; these maxima cannot all
be reached in the same expression. Exact template arity still applies.
Projection lowering charges three additional generated nodes beyond its wire
node and recurses into the base at depth plus two. Its generic arguments also
consume the shared node budget. Temporary-name search is bounded by occupied
names and the constructor-node bound.
Discovery bounds its combined constructor/member/type-parameter
inventory to 65,536 items and its aggregate descriptor encoding to 1 MiB;
individual descriptor construction conservatively charges string expansion.
The enclosing change catalogue retains its stricter 256 KiB report limit,
and hole contexts retain their 1 MiB limit. These are finite work/rendering
bounds, not measured proportional lookup costs or aggregate heap guarantees.
Projection discovery separately bounds its repeated field/template metadata to
65,536 items and 1 MiB, while retaining the enclosing catalogue/context limits.

Update lowering also charges three generated nodes beyond its wire node for
the let, update and base variable. Base and replacement expressions recurse at
depth plus two, with type arguments and replacement values charged to the same
4,096-node budget. Update discovery separately retains the 65,536-item and
1 MiB descriptor bounds, plus the enclosing catalogue/context limits.

Match lowering uses the same shared expression budget: the wire node becomes
a block, with three additional generated nodes for its let, match and variable.
Arm patterns, payload bindings, explicit type arguments and recursive bodies
also consume the budget. Child expressions account for the generated nesting;
no nested match resets the global node count. Match discovery remains bounded
by the aggregate inventory and rendering limits and the enclosing report cap.

Focused aggregate constructor integration, schema/discovery, and semantic
rebase regressions are authored but intentionally unrun. Executed canonical
round-trip, graph, target and runtime evidence remains required before a
completion claim.

[Generic constructor regressions](../tests/project_candidate/generic_aggregate_expressions.rs)
cover ordered arguments, module-local bindings, rejected generic imports, phantom nominal identity, named
generic variants, all four prelude cases, typed hole recovery, malformed inputs
and capacity rejection. [Rebase regressions](../tests/project_candidate/rebase.rs)
include a checked generic field-type change while the nominal instance and
function identities remain unchanged. These files are authored evidence only.

[Match regressions](../tests/project_candidate/match_expressions.rs) are
authored and unrun. They cover stable-ID selection, typed staging, lexical
bindings, discovery, recovery and rejection. The rebase regressions also cover
case/payload identity changes with an unchanged variant owner and signature.

[Record update regressions](../tests/project_candidate/record_update.rs)
are authored and unrun. They exercise typed staging, replacement subsets and
order, generic/local/imported owners, discovery, holes, recovery and rejection.
The rebase fixture also changes an untouched field identity to ensure a
partial update cannot silently adopt a different record remainder.
