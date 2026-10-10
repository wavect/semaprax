# Nested owned collection records v1

Status: coherent source batch and owning regressions authored; executable
qualification and public delivery remain pending. This is a compiler prerequisite for generic
application JSON composition in #724, not completion of that request.

## Closed internal carrier

An explicitly identified, monomorphic, invariant-free record may contain
admitted Copy scalars, owned `string`/`Bytes`, further records of this shape,
and already admitted `Vec<T>` values. At least one transitive Vec field selects
this additive profile. Every record and field has an explicit stable identity.
The eight scalar types and the existing scalar, Bytes, flat Copy-record and
flat owned-leaf Vec element sets remain unchanged. In particular, this does
not admit a Vec of nested records or a Vec of Vecs.

A useful independent application shape is a report containing `Vec<Item>` and
a nested metrics record. Item can contain an owning String and scalar fields;
the metrics record can contain only Copy fields. Neither names, wire names,
declaration order nor generated-code origin establish admission.

Classification uses a bounded explicit worklist. The existing maximum depth
64, 256 owned leaves and 4096 visited record fields apply to each complete
root. Each stored Vec counts as one owning carrier leaf; its elements remain
subject to their own exact classifier and capacity. The existing element,
payload, live-owner, allocation, fuel and builder limits are not increased.
Repeated nested record occurrences count separately, while the active-path set
rejects cycles. Source and retained HIR derive shape independently, including
field identity, owner, order, declared type, and concrete Vec element identity.
Attached Copy/layout facts, cache origin and descriptors grant no authority.

Classes, generic record wrappers, recursive types, stored borrows, resources,
maps, callables, iterators, Option, arbitrary variants and invariants remain
outside this slice. A typed result whose success case wraps the whole record
is a separate follow-on: the existing direct collection-outcome contract is
unchanged.

## Construction, access and settlement

Constructors evaluate initializers in authored declaration order. A completed
Vec initializer is one owning leaf even when empty. A later failing initializer
settles only completed owning leaves in the canonical reverse completion
order, retaining the original failure. A trailing failing Copy initializer
cannot make an incomplete record publishable.

Whole record bindings, explicit own/borrow parameters, owning results and exact
owned destructuring use complete stable field paths. Owned arguments stage
left to right and transfer together at the ordinary call commit. Record shells
never retain a second live Vec owner. Result publication follows postconditions
and non-result cleanup. Vec movement authenticates the existing carrier and
renews it under its ordinary protocol; copying struct bytes or a Wasm token
does not create an owner.

Call-local borrowed Vec reads from a named record field are required. For
example an encoder must be able to inspect the length and clone an admitted
element while the report remains live. The loan retains the full root/field
path. Moving or replacing the root or overlapping field while that loan lives
is refused; independent siblings remain independent. A temporary root, an
escaping borrowed Vec result, implicit whole-vector clone and a stale field
path are refused. Borrowed destructuring may provide the same exact field
loan, but cannot synthesize ownership.

Immutable reconstruction must preserve the same prefix/retained-leaf rules
as existing nested record updates before it is admitted. Until that join is
qualified, an update involving a Vec-bearing record must fail explicitly; a
general record-admission predicate alone must not silently enable it.

## Physical and proof boundaries

Native aggregate layout uses the existing full `spx_vec_v1` carrier (40 bytes,
alignment 8 on Native64), not the eight-byte String/Map representation. The
Wasm aggregate stores the existing authenticated i64 Vec token (8 bytes,
alignment 8). Aggregate layout records a distinct owning Vec field kind and
validates it independently from source/HIR admission. Neither is a public ABI.

Canonical cleanup already represents a Vec lifecycle below a field path. The
native cleanup bridge must retain projected Vec leaves, including existing
case-qualified collection outcomes. Every native shell, parameter alias,
constructor, move and provisional-result route uses those canonical leaves.
Wasm and interpreter movement/validation preserve the corresponding exact
element identity, liveness and authority. An invalid runtime carrier remains
an invariant failure before access, not an ordinary JSON decode error.

Graph and cache replay bind the complete authored nominal closure, Vec element
identity and cleanup/loan paths. Existing renderers that cannot represent the
composition fail closed. Graph v72 carries a non-authoritative nested-collection marker and derives its
selection from the authenticated ordinary carrier; old graph bytes and evidence
scopes are not reinterpreted. No syntax or new Vec operation is necessary.

Project v27, v29 and v30 remain closed to this runtime shape, including unused
functions before reachability cropping. Ordinary internal source qualification
is exposed to the separately selected Project v31
`language-command-io.collection-record.v1` private command closure, whose roots
remain scalar and whose helper signatures are independently replayed. No source-generated schema,
codec name, origin marker or private backend success authorizes public exports
or a command provider. Public roots and capability/provider bounds remain
unchanged.

## Owning joins and required gate

- Source/HIR: new `declared_type/collection_record` and
  `hir/owned_collection_record` classifiers; declaration-only schema exclusion,
  ordinary parameters/results, constructor and exact-pattern admission,
  projected Vec loans, old-profile negative scans and hostile cache replay.
- Layout/cleanup: aggregate Vec field kind, target-size replay, complete nested
  leaf paths, canonical construction/transfer/cleanup plans and loan replay.
- Interpreter: nested record classification, exact Vec runtime value/uniqueness
  validation, construction, projected borrowed reads, move and settlement.
- Native: nested record shell/publication, typed Vec field aliases and scoped
  projected operands, plan slots, runtime selection even without constructors.
- Wasm: nested record classification, token loads/stores/movement, projected
  loan operands and private runtime selection for signature-only carriers.

The owning corpus must include a report with Vec and nested Copy metadata, a
second shape with different field identities/order and nested String/Bytes
siblings, and constructor-free own/borrow helpers. It must cover empty/full
collections, forward/return/consume, borrowed reads and owned destructuring;
initialization failure before/after every owning leaf and after a trailing
Copy field; callee/postcondition failure; repeated entry; exact/+1 structural
bounds; cycles and every excluded shape; forged type/origin/field/element/
layout/cleanup/loan facts; stale cache/graph replay; and old-profile refusal.

Promotion requires identical interpreter, native C11 O0/O2 and strict Core-Wasm
outcomes with physical allocation/owner settlement and no leaks or duplicate
drops. Source assertions or serialized plans alone do not qualify the feature.
After an explicit command successor is qualified, the unchanged Catalog23 and
ShiftSim15/LogLens49 application suites remain required. Manual source reduction
is a proxy; no live agent cost or token advantage follows from this slice.

Focused owning selectors authored with this batch:

- `--lib hir::owned_collection_record::tests::` (8 tests).
- `--test owned_data nested_collection_record::` (3 tests, two success corpora
  and seven failure positions including provisional-result postcondition).
- Existing `codegen::native_bytes::projected_vec_tests` and Project v31 routing
  owners remain additional prerequisites.

The wrapped-result variant, record updates, nested collection elements and
collection-record loop construction remain outside this tranche.
