# Semantic Image HIR Relationships v1

Audience: compiler contributors and agents querying checked HIR relationships.

Status: implemented bounded projection; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Target execution and broad
unsafe-program admission are not implied.

The existing `image/facet` interface and `ProjectSemanticImage::expand_facet`
accept two additional names, `data-access` and `unsafe-boundaries`. Function
summaries append the corresponding digest-bound handles after the existing
seven entries. Previous facet payloads and handle/cursor formulas are unchanged;
summary and method-catalog choice arrays gain the new entries. Image v1 bytes,
source, Graph, cleanup plans, and ownership semantics are unchanged.

Every new item uses schema `semaprax.image-hir-relationship.v1` and binds image,
Project, and source revisions; source digest, path, module, function stable ID,
expression ID, source span, requires/body/ensures phase, edge kind, reason,
evidence owner, and evidence class. Expression and value IDs are revision-scoped;
field/case/declaration IDs retain their actual HIR identities. No identities or
spans are manufactured from text. `evidence_owner` is
`retained_validated_module_hir`, `evidence_class` is `structural_source_fact`,
and `runtime_execution` is false.

## Data access

Reverse declaration queries are separately exposed by
[Declaration Dependencies v1](SEMANTIC-IMAGE-DEPENDENCIES-V1.md). That immutable
index shares the candidate-delta collector; it does not change these function
facet payloads or infer ownership from an unclassified field use.

| Edge kind | Exact structural fact |
| --- | --- |
| `place_read` | HIR Place references its actual root ValueId and ordered field/case projection path. Ownership and use context remain explicit; this is not a claim that an unclassified use cannot consume. |
| `place_move` | An Own-mode HIR Place occurs in an explicit consuming context derived from a binding, retained declared-function parameter, result, owned match, aggregate initialization/update, propagation, or upcast. |
| `place_borrow` | A declared Borrow argument context or compiler-owned BorrowPlace; BorrowPlace additionally carries its exact operation identity. |
| `binding_initialize` | A Let introduces its actual binding identity, type, ownership, mutability, initializer expression, containing block, and statement index. |
| `binding_write` | Assign stores into the existing ValueId, optionally its exact direct field ID; no new binding identity is inferred. |
| `field_projection` | HIR Project selects an exact field from a base expression. |
| `field_initialize` | A constructed or immutably updated aggregate initializes an exact result field from its initializer expression; this is not an in-place store into the base. |

Own ownership alone does not establish a move. Call contexts use retained
resolved declaration or generic-instance parameter modes. Calls lacking such a
declaration, including intrinsic calls not in that inventory, retain
`use_context: unclassified`; the facet does not invent transfer semantics.
Place projections preserve field and variant-case order. Temporary expression
projections retain their base expression identity without inventing a root
ValueId. Pattern-bound values can appear as actual Place roots; this tranche
does not claim a complete pattern-binding def/use or alias graph.

## Unsafe boundaries and import calls

The HIR walker projects audited Unsafe statements with their verbatim audit,
actual body/block expression identities, statement index/span, enclosing unsafe
body identity, and module permit fact. Nested boundaries link to their immediate
parent body; boundaries execute ordinary checked source and do not authorize raw
memory. NativeRustImportCall nodes retain their actual import and expression
identities, argument expression/type/ownership facts, and the same-module import
descriptor where available: interface/import identities, declared effects,
required authority, parameter consumption-on-failure, and result ownership
provenance. A missing descriptor is explicit, not synthesized.

Admission remains owned by the selected Project and source profile. The focused
calculator Project regression still rejects a module `unsafe` permit with
`SPX-G172`; advertising an unsafe facet does not admit that source. The later
Graph v25/image path retains Native Rust import declarations as described by
[Analysis Coverage v1](SEMANTIC-IMAGE-ANALYSIS-COVERAGE-V1.md), so the old blanket
statement that every native import is rejected by `SPX-G218` no longer describes
the current code. Declaration retention, an admitted call node, unsafe-region
admission and physical provider execution remain distinct. The facet projects
only actual retained checked nodes; it creates none of those admissions itself.

Neither audit text nor required-authority facts grant host authority, establish
review approval, prove safety, or identify transitive runtime effects. The
facet does not treat ordinary capability calls as native Rust imports, infer
external callees, or run code.

## Order, bounds, and failure

Traversal is iterative, deterministic source-structural preorder: requires in
source order, then body, then ensures. It includes blocks/statements, loops,
conditionals, match scrutinees/guards/arms, calls, aggregate initializers, and
nested audited regions. Statement/aggregate relation rows precede child
expressions; this order is not an execution or destruction trace. Both lazy
branches and all match arms appear without any reachability assertion.

A request admits at most 65,536 traversal tasks and 65,536 retained call-target
index entries, depth 256, 65,536 relation items, and 16 MiB of cumulative rendered
item bytes before pagination. Existing page limits remain 1–128 items and
1,024–1,048,576 bytes. The full inventory is derived before slicing; page size
is not a proportional-work or total-memory guarantee. Crossing a limit returns
`SPX-G228` without partial JSON. Existing exact image/handle/cursor rejection
remains unchanged, including facet-bound and page-size-bound cursor checks.

[Focused regressions](../tests/image_protocol/hir_relationships_v1.rs) cover
contract/body regions, nested stores and shared ValueIds, field identities,
explicit owned call/result consumption, page concatenation, source provenance,
facet-handle mismatch, and the calculator profile's unsafe-permit rejection.
The implemented corpus has hosted-green v0.4.0 release evidence; this does not
prove general unsafe source support or physical native-import execution.
