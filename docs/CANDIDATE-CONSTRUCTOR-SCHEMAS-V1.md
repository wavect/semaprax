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
literals; places; calls; binary and unary operators; conditional expressions;
and identity-selected record/variant construction.
Literal bounds use the corresponding Rust integer limits, including the
target-neutral unsigned 64-bit input range for `usize`; target admission can
still reject a value. New names use the same bounded ordinary identifier shape
and excluded keyword set as candidate constructors. Call arguments recurse into
the same closed expression alternatives.

Aggregate expressions use exactly `{"kind":"record","target":record_id,
"fields":[{"target":field_id,"value":expression}]}` or the same shape with
`kind: variant` and a case identity as the target. Each field value recurses
through the complete expression grammar. Empty field arrays are structurally
valid for zero-field records/cases; actual coverage is checked by the compiler.
The maximum is 4,095 fields before the shared 4,096-node/64-depth budget further
constrains nested values. Duplicate, missing, foreign-owner, or unknown field
identities fail actual construction. Request field order is expression
evaluation order; a schema never sorts or repairs that order.

Only existing source-defined monomorphic record/variant declarations with one
visible local or imported type binding are eligible. The compiler selects the
source spelling through that binding; no raw type names, type arguments, source,
field projection, or guessed conversions enter this new request grammar.
Nominal identity and every field's expected type remain verifier obligations.

For a selected function's module, `change/catalog` and body/expression-hole
contexts expose optional `aggregate_constructors` when the compiler finds
eligible visible declarations. Each descriptor identifies kind, constructor
target, owner type, source display name, unique visible binding, declaration
path/module, `generic: false`, and declaration-ordered fields carrying stable
`target`, display `name`, `index`, and checked `type_identity`. Descriptors state
`evidence_owner: retained_checked_hir` and
`requires_full_candidate_validation: true`. Their field order describes the
declaration; agents may choose a different request evaluation order. This
module-wide inventory is not a claim that every constructor matches the
selected hole or body's expected result type or ownership.

Available aggregate kinds are appended to the existing constructor-kind
inventories. Empty aggregate inventories are omitted, preserving previous
scalar-only catalogue/context bytes. The v5 bundled change-catalogue schema
closes the optional descriptor objects; heterogeneous hole reports retain their
previous explicit unbundled-schema status. Discovery grants no source, repair,
test, build, or publication authority. Schema and end-to-end aggregate
regressions are authored and unrun.

Intent alternatives cover declaration rename, both append and ordered-mapping
signature forms, whole-body replacement, revision-scoped expression replacement,
closed function declarations, compiler-derived function extraction, declaration
moves selected by destination anchor identity, scalar record-field additions,
and added `requires`/`ensures` contracts. New signature parameters constrain
their `argument` literal kind to match the selected scalar `type`. The complete
change-envelope schema fixes the version and compiler-owned ordered requirement
list. Unknown fields, mixed signature forms, or extra constructor keys are not
part of the described structural grammar.

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
bounded scalar/Bytes/str/Slice<u8> declaration grammar; extraction accepts only
an expression identity and new declaration identity/name. Neither accepts raw
source, HIR, source spans, or arbitrary filesystem paths.
Record fields require an `i64` or `bool` default literal matching the requested
field type; constructor/pattern migration is owned by the compiler. Move
destinations select existing stable identities rather than paths or source text.
