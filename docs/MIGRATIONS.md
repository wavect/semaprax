# Protocol migrations

SEMAPRAX is pre-alpha, but agent-facing changes are still explicit. Consumers must inspect the declared schema field rather than assuming every JSON object has the latest shape.

## Persistent identities are NUL-free

Persistent semantic identities and logical import keys may not contain a literal NUL byte. Source validation reports the declaration-specific stable diagnostic before resolution or graph serialization; `\0` remains an unsupported source-string escape. Public consumers of transformed resolved HIR must likewise reject NUL in declaration IDs, types, expressions, places, call/record/field references, and attached cleanup inventory or plan metadata before code generation or serialization. Regenerate or rename any pre-alpha fixture that constructed such an identity directly.

## Semantic graph v1 to v2

Graph v2 adds:

- top-level declaration/expression identity policy;
- deterministic revision-scoped structural nodes for function bodies and local bindings;
- `requires_graph` and `ensures_graph` expression structures;
- contract calls in function call dependencies and bounded context traversal.

The existing textual `requires` and `ensures` arrays remain during this migration. A v1-only consumer should reject `semaprax.graph.v2` explicitly. A compatible consumer may continue reading declaration IDs, names, signatures, effects, contracts, and calls while progressively adopting the new structural fields.

## Semantic graph v2 to v3

Graph v3 is a breaking migration from parsed syntax to validated resolved HIR:

- public Rust APIs `graph::to_json` and `graph::context_json` are fallible and return verification/HIR diagnostics instead of producing an unchecked graph;
- top-level `entrypoint` is a declaration ID and `view` distinguishes complete module graphs from bounded context slices;
- context views report their declaration-ID `root`, requested `depth`, `truncated` state, and sorted dependency `frontier`;
- declaration nodes expose `identity_origin` and `persistent`, because only explicit `@id` identities survive renames;
- parameters, local bindings, places, and function results use resolved value IDs; calls and dependency arrays use resolved declaration IDs rather than display names;
- `calls` is a sorted, de-duplicated dependency set; individual call occurrences remain in contract and body expression trees;
- expressions expose resolved `type_id` and `ownership_mode`; `ownership_mode` is a boundary mode, not flow-sensitive availability state;
- `i64` literal `value` fields are decimal strings, preserving the full signed 64-bit domain in JavaScript/TypeScript consumers;
- `let` statements carry their binding's value ID only inside `binding`; statements do not claim a separate identity until a dedicated statement-ID domain exists;
- top-level `type_facts` entries are keyed by the resolved type identity and carry resolved type structure plus copy, resource containment, size, drop, and layout facts;
- context output includes only selected functions and their referenced nominal type declarations rather than every resource in the module;
- textual `requires` and `ensures` arrays are removed; `requires_graph` and `ensures_graph` are authoritative structured contracts;
- HIR spans are omitted because the canonical source revision is whitespace-insensitive while source positions are not.

The `revision` remains derived from the canonical human-readable source projection; graph wire-format changes alone do not alter it. A v2 consumer must reject `semaprax.graph.v3` until it supports the new type/value identity tables and fallible API. Exact v3 module fixtures live in `tests/snapshots/meaning.graph.json` and `tests/snapshots/control_flow.graph.json`.

## Semantic graph v3 to v4

Graph v4 adds the first algebraic-data declarations and expressions:

- record declarations are `record` nodes whose ordered `fields` array contains stable field declaration IDs;
- every field is a separate `field` node with its stable ID, display name, explicit/automatic identity origin, persistence flag, owner record ID, declaration-order `index`, and resolved `type_id`;
- record construction expressions use `kind: "construct_record"`, a stable record declaration ID, and source-ordered initializer entries containing stable field IDs and values;
- projection expressions and place projections use stable field IDs rather than display names;
- context slices close transitively over nominal record types referenced by selected functions and over the nominal types of their fields;
- the type-facts table includes every field type required by selected record declarations;
- validated-HIR and graph reference checks fail closed before an unresolved or foreign record/field reference can be serialized.

The SHA-256 revision contract is unchanged. A v3 consumer must reject `semaprax.graph.v4` until it understands record/field nodes and record expression kinds.

## Semantic graph v4 to v5 and explicit resource lifecycles

Graph v5 adds persistent resource-lifecycle, interface, and logical-import declarations. Resource nodes now reference a `resource_drop` node. Imported drop nodes reference an `import` node and target-neutral `import_key`; interface nodes expose their authority ceiling and import IDs; import nodes serialize parameter ownership, consumption on failure, unit-result publication rules, effects, required authority, and normalized failure contracts. Context slices close a referenced resource through its lifecycle, complete owning interface, import signatures, and their nominal types.

The initial source grammar deliberately uses an import's explicit `@id` as its v1 logical import key while resolved HIR stores `import_id` and `import_key` separately. This is a versioned source projection choice, not a permanent conflation of conceptual identity and target binding keys.

Rust API consumers must update exhaustive matches and construction code:

- `Program` adds `interfaces: Vec<InterfaceDeclaration>`;
- `TypeDeclarationKind::Resource` becomes `Resource { lifecycles: Vec<ResourceLifecycleDeclaration> }`;
- the parsed AST adds resource lifecycle kind/declaration, interface/import declaration, and import-failure types;
- `ResolvedProgram` adds resolved interfaces;
- resolved resource declarations carry `ResolvedResourceDrop` and its strategy;
- resolved HIR adds interface/import parameter, unit-result publication, authority, and normalized-failure structures;
- `DeclarationKind` adds `ResourceDrop`, `Interface`, and `Import`.

Legacy `resource Name;` still parses so `check` can report `SPX-O112`, but it is no longer a valid resource declaration. Migration requires an explicit, persistent lifecycle ID and an authored choice:

```semaprax
@id("token.type")
resource Token {
    @id("token.type.drop")
    drop trivial;
}
```

or a complete `drop import` plus interface/import contract. Formatting never invents `drop trivial`; it is an audited semantic assertion. Phase 1 by itself did not execute cleanup: native resource builds still retain `SPX-B104`, and Wasm retains `SPX-W111` for every shape outside the later, separately versioned narrow owned ABI.

The SHA-256 algorithm and domain separator are unchanged, but migrated canonical source receives a new revision as expected. A v4 consumer must reject `semaprax.graph.v5`. The former exact v5 fixtures were superseded by the Graph v6 cleanup-plan migration below.

## Rust HIR cleanup inventory

`ResolvedFunction` now carries a mandatory `cleanup: CleanupInventory`. Direct Rust consumers that construct or transform resolved HIR must preserve the exact inventory or rerun source resolution; `hir::validate`, native lowering, and Wasm lowering reject a missing or stale inventory with `SPX-H006` before any target feature gate.

The inventory schema is `semaprax.cleanup-inventory.v1`. It catalogs canonical storage candidates for owned non-copy parameters, droppable local bindings, owned-producing expression temporaries, and droppable provisional results. Recursive shapes retain declaration-ordered field IDs, and every resource leaf has an exact projected place, lifecycle ID, and distinct liveness-flag identity. Entry state lists only owned droppable parameters. `discovery_index` is deterministic structural discovery order, not runtime initialization or finalization order.

The inventory remains a structural Rust HIR boundary. It does not itself contain CFG edges, path-sensitive liveness, transfers, call commits, status sources, cleanup regions/exits, finalization order, result publication, or a backend trace. Those facts now live in the separately versioned plan below.

## Graph v5 to v6 and Rust HIR cleanup plans

`ResolvedFunction` now also carries mandatory `cleanup_plan: CleanupPlan` using schema `semaprax.cleanup-plan.v1`. Direct Rust consumers that construct or transform resolved HIR must rerun source resolution or preserve the exact canonical plan. Validation first checks core HIR, then rebuilds `CleanupInventory`, then rebuilds the plan without consulting the attached plan; any mismatch is `SPX-H006` before native or Wasm lowering.

Graph v6 embeds the complete plan under each selected function's `cleanup` member. It adds tagged storage/place, recursive liveness shapes, status sources and stable arithmetic codes, transitions, blocks, edges, regions, guarded finalizers, exits, and scalar/owned result commits. Arrays are already in canonical semantic order and consumers must not sort them. Context slices include complete plans for selected functions without unrelated functions.

The canonical source revision algorithm and domain separator are unchanged. The same source can therefore have the same revision in Graph v5 and v6 while the graph payload differs; caches and protocol negotiation must key by `(graph schema, revision)`. A v5 consumer must reject `semaprax.graph.v6`. Exact v6 scalar, control-flow, record, and lifecycle snapshots replace the v5 fixtures.

## Normalized status v1 and conformance trace v1

This release introduces two independent public protocol schemas:

- `semaprax.status.v1` is the target-neutral normalized failure record stored behind a context-local ABI token. It fixes the required `schema`, 1–255-byte UTF-8 `domain_id` without NUL, nonzero `code`, `class`, and boolean-or-`"unknown"` `retryable` fields; compiler-owned contract and arithmetic domains have exact versioned codes. The byte bound is normative for source, HIR, native, Wasm, and adapters. Token zero remains success and has no status record. A physical token, arena index, host exception, or opaque diagnostic detail is never part of status JSON.
- `semaprax.conformance-trace.v1` is the canonical semantic event envelope. It fixes resolved function/invocation identities, ordered cleanup places and projections, ownership transitions, atomic call commits, callable/finalizer import events, frame-local `select_failure`, guarded finalization, result commits, and the terminal result/status outcome. Callable import completion may contain success or normalized failure. Finalizer import completion is a distinct success-only Rust variant even though both project to wire kind `import_end` and are distinguished by `site.kind`.

The exact JSON contract, event field order, status tables, examples, excluded physical fields, and outstanding validation requirements are documented in [Conformance trace v1](CONFORMANCE-TRACE-V1.md).

These are first-version protocols, not an in-place extension of an unversioned wire format. Consumers must inspect `schema` before reading any other semantic field. A status consumer must reject any schema other than `semaprax.status.v1`, an unknown class/retryability representation, an empty domain, or code zero. A trace consumer must reject any schema other than `semaprax.conformance-trace.v1`, every unknown event/site/result/outcome kind, and every required v1 field it cannot validate. In particular, consumers may not ignore `select_failure`, reinterpret a finalizer `import_end` as a fallible callable import, sort event/projection/argument vectors, accept physical target fields, or downgrade an unknown future schema to v1. Producers requiring a new event meaning, field meaning, status mapping, or incompatible encoding must publish a new schema rather than silently changing v1.

Trace data must also be bound out of band to the exact validated program, Graph schema/revision, cleanup plan, and scenario. A source revision alone is insufficient as a cache key because Graph and trace schemas can change without changing canonical source. Cache and negotiation keys must include at least the status schema, trace schema, Graph schema/revision, and scenario identity. A consumer must reject a trace whose referenced semantic IDs do not belong to that bound program and invocation path.

Implementation status remains deliberately bounded. Public normalized-status
types, compiler-owned mappings, a context-local status arena, public trace
types, deterministic canonical JSON, independent inventory/HIR coverage and
path-state replay, and a scenario-driven single-frame reference executor exist.
The new `semaprax.semantic-event-dictionary.v1` projection assigns deterministic
nonzero ordinals to exact event shapes and fingerprints its complete canonical
JSON. Generated native cleanup C and the real narrow Wasm owned adapter now emit
actual executed ordinals for the same authoritative 14-case corpus; independent
materialization proves exact reference/native/Wasm traces, outcomes, and JSON.
Unknown or zero ordinals fail closed, and consumers may not infer or repair
events.

That equality does not make the native resource backend production reachable.
The native generated-C path remains a conformance harness disconnected from the
physical ownership host. Recursive callee execution, callable-import execution,
imported finalizers, aggregates, broader control flow, and the production
native callable-host boundary do not exist yet. Native resources, records, and
every Wasm resource shape outside the documented narrow slice remain fail
closed.

The internal native invocation context and first-slice trace storage are now one-shot objects that require canonical C zero initialization before their initialization functions are called, for example `struct spx_context context = {0};`. This replaces the earlier accepted-but-indeterminate stack declaration form. Generated entry wrappers and repository probes have migrated. Embedders using `SPX_NO_ENTRY_WRAPPER` must zero-initialize context, trace-buffer, and trace-event storage; reinitialization or storage aliasing is rejected to preserve invocation isolation. This runtime scaffold remains private and does not lift native resource execution.

## Rust AST resource declarations to nominal type declarations

The public pre-alpha Rust AST migration from the earlier graph v4 tranche represents both resources and records through `Program::types: Vec<TypeDeclaration>`. `Program::resources` is removed, and `Type::Resource(String)` becomes `Type::Named(String)` because a nominal reference may name either kind. Graph v5 further changes the resource variant as described above.

The lexer now tokenizes `.` separately so expression projection is unambiguous. Module names, capability/effect names, and named types still accept qualified identifiers through parser-specific `IDENT ("." IDENT)*` rules. Canonical formatting expands record initializer shorthand (`Point { x }` becomes `Point { x: x }`) and preserves initializer evaluation order.

This migration enables `check`, HIR, `graph`, and `context` for records. `build` fails closed with `SPX-B103` (native) or `SPX-W110` (Wasm) until aggregate layout and cleanup semantics land.

## Whole-record to prefix-aware ownership

Resource-containing record projections now carry prefix-aware availability instead of conservatively moving the complete root. Moving one owned non-copy field leaves disjoint sibling fields available. Reusing that field or an enclosing parent reports `SPX-O109`; a place moved on only some control-flow paths reports `SPX-O110`. Existing whole-resource moves retain `SPX-O101` and `SPX-O107`. Borrowed or shared projections cannot cross an owned field or parameter boundary and report `SPX-O108`. Validated HIR independently replays the same rules; Graph v6 additionally exposes the resulting cleanup-plan places, flags, transfers, and guarded exits.

## Web manifest v2 to v3 and Wasm owned ABI v1

Browser packages now emit `semaprax.web.v3`. The existing `module`,
`graph_revision`, `wasm`, `entry`, and `capabilities` fields keep their v2
meanings and canonical order. Version 3 adds one required member:

```json
{"owned_abi":{"schema":"semaprax.wasm-owned.v1","functions":[]}}
```

`functions` is in declaration order. Each admitted entry fixes its persistent
function, resource, and lifecycle IDs; deterministic `semaprax_owned_N` export;
source-parameter ABI kinds; and exact result kind. Scalar-only packages still
use web manifest v3 with an empty array. Consumers must not infer an owned ABI
from Wasm signatures or export spelling, and must reject an unknown
`owned_abi.schema`, a missing field,
an unknown parameter/result kind, or a mapping that disagrees with the module.
A v2-only consumer must reject v3; migration consists of validating the new
object before instantiation, not silently treating it as optional metadata.

`semaprax.wasm-owned.v1` is narrower than RFC 0003 and the Component Model. It
admits one direct trivial-resource identity and a restricted direct body. Its
generated JavaScript facade binds invocation to the exact generated metadata,
uses branded one-shot trusted-adoption tickets, keeps ownership imports private,
authenticates the exact generated Wasm bytes with an embedded SHA-256 digest,
checks canonical argument encodings and aligned result ranges before commit,
and exposes normalized
`semaprax.status.v1` records with the canonical `domain_id` field. Unsupported
resource shapes retain `SPX-W111`. A same-realm `Symbol.for` allocator
coordinates runtime tags across separately evaluated copies of the generated
host. The surrounding realm and that reserved global binding are trusted v1
host state; hostile pre-poisoning, cross-realm, and worker identity isolation
remain outside v1.
The adapter now emits compiler-generated semantic event ordinals and the shared
14-case suite materializes them to exact reference/native/Wasm traces and
outcomes. The full [owned-resource vertical
contract](OWNED-RESOURCE-VERTICAL-V1.md), Components, imports/finalizers,
broader shapes, and the production native callable-host connection remain later
gates.

## Native adapter descriptor v1 to callable descriptor v2

Native adapter descriptor v1 remains descriptor-only and promises no callable
owner API. Callable admission uses a separate private `SPXNABI2` wire rather
than extending or reinterpreting v1. The staged v2 descriptor binds eleven
independently domain-separated fingerprints, exact getter and callable symbols,
the required `0x0f` call profile, request/response/event and dictionary bounds,
the complete ordered parameter signature, opaque-`u64` owned payload kind,
and the exact result mapping. The event dictionary itself is not embedded.

Private consumers must select a decoder from the eight-byte magic before
loading. They must never pass `SPXNABI1` to callable admission, infer v2 fields
from a v1 function-template hash, accept unknown obligation bits, or repair
noncanonical fields. The compiler's staged encoder and the unpublished host's
independent strict parser are cross-tested, including every-byte mutation,
truncation, and trailing data. The loader can bind the exact v2 getter and one
one-shot byte callable, but the physical ownership host does not yet use either
the v2 call request or response. Windows runtime and callable sanitizer evidence
also remain absent, so this migration does not change `SPX-B104`.

## Revision token FNV-1a64 to SHA-256

Graph v3 and later, semantic patch bases, CLI output, and `semaprax.web.v2`/`semaprax.web.v3` manifests use one algorithm-tagged token:

```text
sha256:<64 lowercase hexadecimal digits>
```

The digest input is exactly `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. The domain separator and canonical projection are part of the protocol. Paths, comments, and formatting-only differences do not affect the token; semantic source changes do. This is collision-resistant content addressing and stale-base detection, not source authentication.

Legacy `fnv1a64:` patch bases, graph caches, snapshots, and web manifest expectations are incompatible. Regenerate them from the current source. SEMAPRAX deliberately does not accept an FNV fallback: an old patch fails with `SPX-G409` before modifying its source. Web consumers must reject `semaprax.web.v1` when they require the SHA-256 revision contract.

There is not yet a stable compatibility guarantee. Before 1.0, every breaking public syntax, CLI, diagnostics JSON, graph, patch, web manifest, package, component, or ABI change must add a section here and update the changelog.
