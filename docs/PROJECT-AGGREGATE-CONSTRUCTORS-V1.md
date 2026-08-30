# Project Aggregate Expression Constructors v1

Status: authored, unrun; an additive constructor slice of the graph-operational
programme. No completion-matrix promotion or runtime evidence claim.

Audience: agent builders, compiler contributors, and reviewers.

## Stable-ID requests

The recursive typed expression vocabulary adds two closed shapes:

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
type names, aliases, paths, spans, HIR, graph facts, or a new import.

Every field must occur exactly once. Missing, repeated, foreign, and unknown
field identities reject. The input array remains initializer evaluation order;
the compiler must not sort expressions into declaration order. Each value uses
the same recursive expression vocabulary and shared depth/node accounting.
Empty records and unit cases use an empty field array.

This slice admits monomorphic source records and variants. Generic templates,
explicit type arguments, compiler-prelude constructors, classes, arbitrary
field projections, record updates and match synthesis remain outside this
constructor grammar. A named field type does not waive ordinary profile,
ownership, effect, or backend admission.

## Checked bindings and candidate admission

Construction resolves the requested identities through the retained checked
module HIR and authenticates the source declaration/member relationship. A
unique existing type binding is required in the expression's destination
module. A declaration that exists somewhere in the project is not implicitly
in scope. Ambiguous aliases reject rather than being selected arbitrarily.

The expression lowers to the existing record/variant AST forms. Whole-body
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
`evidence_owner` identifies retained checked HIR; full candidate validation is
still required. These are available lexical constructors, not expected-type
filtering or a guarantee that arbitrary field values are legal.

The self-contained [constructor schemas](CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md)
describe both recursive shapes. Schema validation alone cannot establish
membership, complete field coverage, unique aliases, lexical scope, types,
effects or ownership. Scalar-only contexts retain their prior shape when no
aggregate constructor is available.

## Rebase and limits

Before replaying each history step, semantic rebase compares referenced
aggregate descriptors between that step's original checked revision and its
current rebased revision. A missing target or changed member identity/order/type
rejects with `SPX-G235`, even when the intention's function target has only a
scalar signature. Descriptor names and source location can also produce
conservative conflicts. This is not general structural compatibility or a
transitive behavioral equivalence proof. Surviving intentions still undergo
complete candidate source admission.

Existing Semantic Change byte/JSON limits, recursive expression node/depth
limits, and catalogue/context rendering limits remain enforced. Identity and
shape errors use the constructor's `SPX-G225`; constructor capacity uses
`SPX-G226`. Source verification retains its own diagnostics.

An aggregate has at most 4,095 fields within the shared 4,096-expression-node
budget and depth limit of 64. Discovery bounds its combined constructor/member
inventory to 65,536 items and its aggregate descriptor encoding to 1 MiB;
individual descriptor construction conservatively charges string expansion.
The enclosing change catalogue retains its stricter 256 KiB report limit,
and hole contexts retain their 1 MiB limit. These are finite work/rendering
bounds, not measured proportional lookup costs or aggregate heap guarantees.

Focused aggregate constructor integration, schema/discovery, and semantic
rebase regressions are authored but intentionally unrun. Executed canonical
round-trip, graph, target and runtime evidence remains required before a
completion claim.
