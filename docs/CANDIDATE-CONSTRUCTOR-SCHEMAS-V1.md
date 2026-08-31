# Candidate Constructor Schemas v1

Status: compiler-owned structural documents with authored, unrun regression
coverage. No semantic acceptance or hosted-validation claim.

Audience: agent builders, compiler contributors, and reviewers.

`SemanticChange::constructor_schemas()` and the candidate-only protocol method
`protocol/constructor-schemas` return
`semaprax.candidate-constructor-schemas.v1`. The transport method requires the
current `image_revision` and existing held-source authentication. It creates no
candidate and changes no registry or filesystem state.

The result carries four JSON Schema draft 2020-12 documents identified by:

- `urn:semaprax.typed-expression.v1`
- `urn:semaprax.semantic-change-intent.v1`
- `urn:semaprax.semantic-change.v1`
- `urn:semaprax.project-candidate-recovery.v1`

Each document is self-contained. Recursive expressions use local `$defs`
references; validators need no network lookup. These IDs resolve the existing
constructor references exposed by Image Candidate Protocol v2 without changing
its previous request descriptors. Every constructor object has explicit
required fields and `additionalProperties: false`.

Expression alternatives cover typed `i64`, `i32`, `u8`, `usize`, and `bool`
literals; places; ordinary calls; compiler-owned byte calls; binary and unary
operators; conditional expressions;
immutable scoped `let` bindings;
identity-selected record/variant construction, stable-ID record-field projection,
exhaustive stable-ID variant matching, and typed stable-ID record updates.
The additive `field_place` shape is closed to `kind`, stable field `target`,
and lexical identifier `root`. Its exact root type and member identity are
compiler obligations, not schema assertions. It lowers without a staging
temporary and retains ordinary source ownership and borrowing admission; see
[Field Place Constructor](PROJECT-FIELD-PLACE-CONSTRUCTOR-V1.md).
Catalogue and hole discovery add optional `field_places` descriptors and the
`field_place` constructor kind when visible source-record fields are available.
The closed descriptor shapes retain ordinary source monomorphic/generic
projection metadata, change `base_evaluation` to
`direct_named_place_no_staging`, and require
`root_requirement: authenticated_lexical_nominal_binding`. Template presence
does not authenticate a supplied root or prove a borrow is admissible.
Literal bounds use the corresponding Rust integer limits, including the
target-neutral unsigned 64-bit input range for `usize`; target admission can
still reject a value. New names use the same bounded ordinary identifier shape
and excluded keyword set as candidate constructors. Call arguments recurse into
the same closed expression alternatives.

`builtin_call` selects one of seven stable byte-operation identities from the
compiler's operation table, with an exact argument count for each alternative.
Its recursive arguments still require complete source admission, including
view provenance and ownership. Target-specific `builtin_calls` descriptors
remain separate from ordinary function discovery; array parameters describe
the fixed byte-array family rather than the internal zero-length sentinel.
See [Builtin Call Constructor](PROJECT-BUILTIN-CALL-CONSTRUCTOR-V1.md).

The closed `let` shape requires `name`, recursive `value`, and recursive `body`.
Its initializer cannot see the introduced name; only its body can. Normal
compiler admission checks scope, inferred type, ownership and context legality.
See [Lexical Binding Constructor](PROJECT-LEXICAL-BINDING-CONSTRUCTOR-V1.md).

Ordered signature mappings additionally admit the closed
`{name, type, argument_expression}` form, with the existing five scalar type
choices or the same closed nominal type selector used by function declarations,
and the recursive expression definition. It is a separate alternative to
`{name, type, argument}` and retained `from` mappings; append remains literal-only.
Original-parameter scope and actual per-caller admission belong to
[Signature Argument Expressions](PROJECT-SIGNATURE-ARGUMENT-EXPRESSIONS-V1.md).
Nominal discovery identifies existing provider/caller type bindings and rebuilt
Copy admission; the schema alone cannot establish either property.

Aggregate expressions use exactly `{"kind":"record","target":record_id,
"fields":[{"target":field_id,"value":expression}]}` or the same shape with
`kind: variant` and a case identity as the target. Each field value recurses
through the complete expression grammar. Empty field arrays are structurally
valid for zero-field records/cases; actual coverage is checked by the compiler.
The maximum is 4,095 fields before the shared 4,096-node/64-depth budget further
constrains nested values. Duplicate, missing, foreign-owner, or unknown field
identities fail actual construction. Request field order is expression
evaluation order; a schema never sorts or repairs that order.

Existing source-defined monomorphic and generic record/variant declarations
require one visible local or imported type binding. The compiler also recognizes
its own `Option` and `Result` declarations when their prelude bindings are
available. The optional `type_arguments` field selects ordered direct `i64` or
`bool` arguments; generic targets require their exact declared arity. This is
explicit instantiation, not inference, a raw type-expression parser, or a
conversion. Monomorphic constructors retain the form without type arguments.
No raw source or arbitrary nominal type argument enters this
grammar. `type_arguments` may contain at most 4,095 entries, and each entry
also consumes the shared expression-node budget. Omitting it is equivalent to
an empty array: monomorphic targets accept either form; a generic target rejects
missing, extra, or unsupported arguments. The schema describes the bounded
array but cannot determine a selected declaration's arity. Exact nominal
identity, substituted field types, ownership, and full
Project admission remain verifier obligations.

For a selected function's module, `change/catalog` and body/expression-hole
contexts expose optional `aggregate_constructors` when the compiler finds
eligible visible declarations. Each descriptor identifies kind, constructor
target, owner type, source display name, unique visible binding, declaration
path/module, a `generic` flag, and declaration-ordered fields carrying stable
`target`, display `name`, `index`, and checked `type_identity`. Descriptors state
`evidence_owner: retained_checked_hir` and
`requires_full_candidate_validation: true`. Their field order describes the
declaration; agents may choose a different request evaluation order. This
module-wide inventory is not a claim that every constructor matches the
selected hole or body's expected result type or ownership. A generic descriptor
describes one template, not all concrete instances. Its `type_parameters` retain
declaration order and identify the parameter name/index and allowed direct
scalar types. Generic field `type_identity` values describe the template's
owner/index-bound parameters; they are not already substituted concrete types.
Each parameter descriptor is closed to `name`, `index`, and
`allowed_types: ["i64", "bool"]`; no cartesian product of instances is returned.
Existing monomorphic descriptor objects remain unchanged.

Compiler-owned prelude cases use a distinct closed descriptor shape:
`identity_origin: compiler_owned`, null `path` and `module`, and
`compiler_prelude: {schema: "semaprax.prelude.v1", digest: ...}`. The digest binds
the actual compiler prelude. These are compiler definitions, not invented
filesystem declarations. Their field identities and generic parameter facts
come from the checked prelude index.
Prelude constructor and match descriptors use
`evidence_owner: compiler_checked_prelude`; authored source descriptors retain
`evidence_owner: retained_checked_hir`.

Available aggregate kinds are appended to the existing constructor-kind
inventories. Empty aggregate inventories are omitted. The newly discoverable
four `Option`/`Result` cases can make an otherwise scalar-only module's inventory
nonempty, so complete catalogue/hole-context bytes intentionally change in this
extension. Existing monomorphic entries, protocol envelopes, and authority do
not change. The v5 bundled change-catalogue schema
closes the optional descriptor objects; heterogeneous hole reports retain their
previous explicit unbundled-schema status. Discovery grants no source, repair,
test, build, or publication authority. Schema and end-to-end aggregate
regressions are authored and unrun.

Record-field projection uses the closed expression
`{"kind":"project","target":field_id,"base":expression}` with the same optional
direct-scalar `type_arguments` array. The field's checked owner determines the
required base type. A source-defined generic record requires its exact ordered
arguments; monomorphic owners accept omission or an empty array. Classes,
variant payloads, prelude fields, and implicit source identities remain outside
this route. The caller supplies neither an owner identity nor a display field
name to override the selected stable field.

The compiler evaluates the base once into a fresh, explicitly typed value
binding and projects through that binding. Its type annotation prevents an
unrelated record with an identically named field from satisfying the request.
The generated block, let, projection, and place remain in the original
expression position; naming is hygienic. The schema records three additional
constructor-budget nodes for the generated let statement, projection, and
place, and a conservative two-level depth increment for the base. Explicit
type arguments still consume the shared node budget. This is structural
construction accounting, not a runtime cost promise.

Ordinary value binding may copy a Copy base or transfer an owned base. It is
not a borrow-preserving operation, and the schema grants no permission to copy
an owner, bypass loans, or alter cleanup. The existing whole-candidate verifier
decides whether the staged base and selected field are admissible.

`change/catalog` and both hole contexts expose `aggregate_projections` only
when projections are available, adding `project` to their constructor kinds.
Each closed descriptor identifies the field target/name/index, owner record,
checked field `type_identity`, visible record binding, source path/module,
generic flag, and `base_evaluation: once_into_typed_value_binding`. It retains
`evidence_owner: retained_checked_hir` and
`requires_full_candidate_validation: true`. Generic descriptors add the same
ordered `type_parameters`; their field identity is a template fact, not a
substituted type. Existing aggregate constructor entries are unchanged. Schema
regressions and `tests/project_candidate_record_projection_v1.rs` are authored
and unrun; discovery does not validate an arbitrary proposed base expression.

Exhaustive matching uses `{"kind":"match","target":variant_owner_id,
"value":expression,"arms":[{"target":case_id,"fields":[{"target":payload_id,
"name":"binder"}],"body":expression}]}` with the same optional direct-scalar
`type_arguments`. The target selects a variant owner rather than one case.
Every case and each case's payload field must appear exactly once. Guards,
wildcards, omitted payloads, record/class patterns, and borrowing-match modes
have no constructor fields and remain unsupported. Exact owner/arity and full
candidate admission remain compiler checks, not JSON Schema acceptance.

The value is evaluated once into a fresh typed binding before matching, at the
original expression position. The schema charges three generated nodes for
the let statement, match, and place; each arm pattern and payload binder also
consumes the shared 4,096-node budget. Scrutinee and arm bodies use a two-level
depth increment under the shared depth limit. Arm and per-arm field arrays are
individually bounded to 4,095 entries, with at most 4,095 payload binders across
the whole match before the tighter shared node budget applies. No case-product
enumeration or implicit default arm is synthesized.

Binder names use the existing bounded identifier grammar. They are unique in
each arm and may not capture the outer lexical scope, callable/type/import
bindings, or generated staging names. A binder is available only to its own
arm body; a sibling arm has an independent scope. Constructed matching is an
ordinary value operation: staging may copy or transfer the base, and the full
verifier still owns payload ownership, cleanup, effects, result typing, and
exhaustiveness. Discovery is not evidence of borrow preservation or runtime
execution.

The optional `aggregate_matches` inventory in change catalogues and both hole
contexts identifies visible source variant owners and authenticated `Option`
and `Result` owners. The `match` constructor kind appears only with a nonempty
inventory. Each descriptor includes the owner `target`, name, binding,
path/module, generic flag, checked evidence owner, full-validation requirement,
`base_evaluation: once_into_typed_value_binding`, and declaration-ordered
`cases`. Cases carry target/name/index and payload fields with
target/name/index/type_identity. Generic templates retain the same ordered
type-parameter guidance. Prelude entries use null path/module, compiler-owned
identity origin, and the exact prelude schema/digest object used by constructor
discovery. Earlier constructor/projection descriptor entries stay unchanged.
The closed response schema describes the source monomorphic, source generic,
and compiler-prelude alternatives separately. Matching schema regressions are
authored and unrun.

Record update uses `{"kind":"update","target":record_owner_id,
"base":expression,"fields":[{"target":field_id,"value":expression}]}` with
the same optional direct-scalar `type_arguments`. It selects an explicit checked
source record with one visible binding, including supported generic instances.
Each requested field must belong to that exact owner and appear at most once;
unmentioned fields follow the existing record-update semantics. An empty field
array is permitted and remains an ordinary update AST, subject to full
admission. Classes, variants, prelude types, and implicit source identities are
not update owners in this constructor.

The base is evaluated once into a fresh binding annotated with the exact owner
and type arguments. The existing `UpdateRecord` expression then evaluates
replacement expressions in request order after the base. The schema charges
three generated nodes for the let statement, update, and place; base and field
children use a two-level depth increment. Field arrays are bounded to 4,095
entries before the shared node/depth budget applies. The compiler rejects
foreign or duplicate field IDs and does not reorder the replacement array.
Typed staging may copy or transfer the base; it is not a borrow-preserving
operation or an exception to ordinary owned-update, cleanup, or target checks.

Change catalogues and both hole contexts expose optional `aggregate_updates`
when visible source-record updates are available, adding `update` to constructor
kinds. Each descriptor retains the complete checked record field inventory,
including target/owner identity, source binding/provenance, and generic template
parameters when present. It changes the descriptor kind to `update` and adds
`base_evaluation: once_into_typed_value_binding` and `field_coverage: subset`.
The field inventory describes available selections, not required replacements.
The response schema has separate closed source monomorphic and source generic
forms; no prelude alternative is accepted. Existing constructor, projection,
and match entries remain unchanged. These schema regressions are authored and
unrun; discovery confers no source or execution authority.

Intent alternatives cover declaration rename, both append and ordered-mapping
signature forms, whole-body replacement, revision-scoped expression replacement,
closed function declarations, compiler-derived function extraction, declaration
moves selected by destination anchor identity, scalar record-field additions,
and added `requires`/`ensures` contracts. New signature parameters constrain
their `argument` literal kind to match the selected scalar `type`. The complete
change-envelope schema fixes the version and compiler-owned ordered requirement
list. Unknown fields, mixed signature forms, or extra constructor keys are not
part of the described structural grammar.

Function declarations preserve the existing scalar and boundary-type strings
in parameter `type` and `return_type`. They additionally accept the closed
object `{"kind":"nominal","target":type_owner_id,"type_arguments":["i64","bool"]}`.
The argument array is required, including `[]` for monomorphic types; its
maximum is 4,095 direct scalar arguments and actual arity must match the selected
declaration. Nominal parameters use `mode: value` for checked Copy types or
`mode: own` for checked owning types. Copy parameters require no drop; owners
require non-Copy storage that needs drop. Both must be sized and resource-free
in the rebuilt signature, and the requested mode must agree with checked HIR.
Owning nominal parameters currently select monomorphic owners. Named returns
may be checked Copy or owning data. Lowercase `string` is an additional
parameter/return token: its source parameter mode is `value`, but checked HIR
must carry the language's implicit String ownership. `own string` remains
outside this constructor. Structural schema acceptance or a template's
presence never establishes those facts.
Existing `own Bytes` and borrowed string/slice parameter alternatives remain
unchanged; nominal borrowing and owned resource signatures are not added.

The change catalogue's optional `nominal_types` array supplies stable owner
identities and unique visible bindings for source records/variants and the
authenticated compiler-owned `Option`/`Result` owners. The `add_declaration`
operation identifies that inventory with `nominal_type_selector: nominal_types`.
Its separate `nominal_owning_admission: checked_candidate_owning_signature`
marker identifies the owning path without changing the meaning of Copy rows.
Closed descriptors carry `kind`, `target`, `binding`, `generic`,
`declaration_kind`, `path`, `module`, `evidence_owner`,
`requires_full_candidate_validation: true`, and
`copy_admission: checked_candidate_signature`. Generic templates add the same
ordered `type_parameters` guidance used by aggregate constructors. Prelude
rows use null source locations and the separately authenticated compiler
provenance described above. These are candidate type selections, not a list
of types already approved for a proposed signature. The v5 response schema
closes all three source-monomorphic, source-generic, and prelude forms.
Focused structural regressions are authored but unrun.

The `add_declaration` payload also accepts two closed type-declaration forms:
`{"kind":"record","id":owner_id,"name":name,"fields":[field]}` and
`{"kind":"variant","id":owner_id,"name":name,"cases":[case]}`.
A field is exactly `{"id":field_id,"name":name,"type":field_type}`.
`field_type` is one of `i64`, `bool`, `i32`, `u8`, `usize`, `string`, or `Bytes`,
or the closed nominal selector above with exact direct-scalar generic arguments.
The lowercase `string` spelling denotes the owned String type. Direct view or
array requests, resources, new self references and arbitrary source type
spellings are excluded. An existing nominal dependency may retain its ordinary
source-admitted array storage; the request cannot construct a new array type.
A case is exactly
`{"id":case_id,"name":name,"fields":[field]}`. Records and cases may have
zero fields. Variants require at least one case. Each field list and case list
is bounded to 64 entries, with at most 4,096 combined owner, case, and field
identities. The ordinary change JSON byte, node, and depth limits still apply.
`x-max-combined-identities` describes the compiler-enforced aggregate bound;
standard JSON Schema validation alone does not count this cross-list total.
New type declarations have no generic parameters, methods, effects, defaults,
or resource fields. Existing nominal fields must already have an authenticated
visible binding in the anchor module. Full rebuilding checks the new owner and
its selected dependency closure with the ordinary bounded type-facts engine;
the resulting type must be sized and resource-free. Field types need not be
Copy. Function signatures have separate checked Copy and owning admission;
field selection does not determine a parameter's mode.
The field vocabulary describes request structure, not aggregate profile
eligibility: non-Bytes variants retain `SPX-T215` restrictions, and nested
generic record fields retain `SPX-T223`. Complete source/target admission can
reject other structurally valid selections.
Their names and all identities must pass ordinary
freshness and namespace admission; structural acceptance is not that proof.

An existing explicit monomorphic function, including `main`, selects the module.
Record and variant declarations append to its source type inventory; canonical
formatting still groups types before functions. The existing function payload
is the unchanged first declaration alternative, with no `kind` field added.
Its discovery `placement: append_function_in_anchor_module` and function
constraints retain their prior meaning. The additive `type_declaration_forms`
inventory describes record and variant placement separately and supplies the
corresponding list/identity bounds, the direct `field_types`,
`nominal_type_selector: nominal_types`,
`field_type_admission: checked_resource_free_field_type`, and
`requires_full_candidate_validation: true`. The v5 descriptor schema accepts
that exact optional inventory. The shared nominal rows' `copy_admission`
metadata still describes Copy function signatures; it does not impose Copy on fields
or prove a new type's admission. Newly admitted types become ordinary stable-ID
nominal and aggregate discovery subjects after full candidate rebuilding;
discovery itself creates no source or publication authority. Structural
regressions for the declaration alternatives and discovery forms are authored
but unrun.

Ordered signature mapping retains its existing closed `from` / optional `name`
constructor. Selecting an existing named Copy record or variant does not add a
type spelling, conversion, or aggregate literal to the request grammar. The
compiler checks eligibility against retained checked HIR, including concrete
generic instances when already admitted by the Project profile. Fresh
parameters remain restricted to the five scalar literal kinds above. See
[Project Signature Evolution v1](PROJECT-SIGNATURE-EVOLUTION-V1.md) for exact
staging, ownership, and complete candidate admission requirements.

Change-catalogue parameter entries preserve `name`, display `type`, and source
`mode`. Named parameters of an eligible ordered-mapping signature additionally expose `type_identity` and
`type_provenance`, derived from retained checked HIR rather than display-name
lookup. These descriptive fields do not become valid request fields. Scalar
catalogue entries retain their prior shape. The v5 discovery bundle describes
both closed response alternatives; neither shape is a payload admission proof.
The provenance closes the declaration stable ID, ordered argument identity keys,
`ownership: copy`, `evidence_owner: retained_checked_hir`, `copy: true`,
`sized: true`, `contains_resource: false`, and `needs_drop: false`. An unsupported
signature does not acquire eligibility merely because its source type has a
name. `tests/project_signature_catalog_v1.rs` authors nominal/generic identity,
import-alias identity equivalence, unchanged scalar/Bytes shapes, and rejection
of owned-record or borrowed-view ordered mapping. These tests remain unrun.

Constructor limits are drawn from the implementation's shared limit constants.
Depth, aggregate node counts, implicit conditional block nodes, UTF-8 byte
limits, JSON canonicality, duplicate-key rejection, and lexical integer forms
are recorded as extension metadata or nonclaims where standard JSON Schema
alone cannot enforce them. Existing constructor validation remains unchanged;
these schemas do not pre-validate requests or alter legacy diagnostic behavior.

Passing a schema does not establish that a place is in scope, a function is
accessible, a call has the correct arity, an expression has the expected type,
effects are allowed, ownership and cleanup are valid, contracts hold, or the
Project profile/targets admit the resulting source. The actual compiler checks
all those conditions through candidate construction and independent source
replay. `result` is usable as a contract place only in the admitted `ensures`
context. The schema does not certify that contextual rule.

These are constructor documents, not complete response/HIR schemas, installed
SDK packages, source authority, or a behavioral-equivalence proof.

The recovery document closes the complete capsule envelope and embeds the same
change and expression definitions. Compiler compatibility is an exact constant;
content hashes, canonical bytes, original-base agreement, and actual replay are
checked by the recovery API, not JSON Schema. Addition schemas describe the
separate bounded function-signature and data-field grammars above; extraction accepts only
an expression identity and new declaration identity/name. Neither accepts raw
source, HIR, source spans, or arbitrary filesystem paths.
The distinct `add_record_field` operation requires a matching inert literal
from its direct scalar vocabulary; constructor/pattern migration is owned by the compiler. Move
destinations select existing stable identities rather than paths or source text.
