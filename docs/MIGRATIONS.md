# Protocol migrations

SEMAPRAX is pre-alpha, but agent-facing changes are still explicit. Consumers must inspect the declared schema field rather than assuming every JSON object has the latest shape.

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

The SHA-256 revision contract is unchanged. A v3 consumer must reject `semaprax.graph.v4` until it understands record/field nodes and record expression kinds. Exact v4 fixtures cover scalar programs in `tests/snapshots/meaning.graph.json` and `tests/snapshots/control_flow.graph.json`, and records in `tests/snapshots/records.graph.json`.

## Rust AST resource declarations to nominal type declarations

The public pre-alpha Rust AST now represents both resources and records through `Program::types: Vec<TypeDeclaration>`. `Program::resources` is removed, and `Type::Resource(String)` becomes `Type::Named(String)` because a nominal reference may name either kind. Rust library consumers must migrate exhaustive matches and use `TypeDeclarationKind::{Resource, Record}`.

The lexer now tokenizes `.` separately so expression projection is unambiguous. Module names, capability/effect names, and named types still accept qualified identifiers through parser-specific `IDENT ("." IDENT)*` rules. Canonical formatting expands record initializer shorthand (`Point { x }` becomes `Point { x: x }`) and preserves initializer evaluation order.

This migration enables `check`, HIR, `graph`, and `context` for records. `build` fails closed with `SPX-B103` (native) or `SPX-W110` (Wasm) until aggregate layout and cleanup semantics land.

## Whole-record to prefix-aware ownership

Resource-containing record projections now carry prefix-aware availability instead of conservatively moving the complete root. Moving one owned non-copy field leaves disjoint sibling fields available. Reusing that field or an enclosing parent reports `SPX-O109`; a place moved on only some control-flow paths reports `SPX-O110`. Existing whole-resource moves retain `SPX-O101` and `SPX-O107`. Borrowed or shared projections cannot cross an owned field or parameter boundary and report `SPX-O108`. Validated HIR independently replays the same rules, while Graph v4 continues to expose identities and ownership modes rather than flow-sensitive availability.

## Revision token FNV-1a64 to SHA-256

Graph v3 and later, semantic patch bases, CLI output, and `semaprax.web.v2` manifests use one algorithm-tagged token:

```text
sha256:<64 lowercase hexadecimal digits>
```

The digest input is exactly `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. The domain separator and canonical projection are part of the protocol. Paths, comments, and formatting-only differences do not affect the token; semantic source changes do. This is collision-resistant content addressing and stale-base detection, not source authentication.

Legacy `fnv1a64:` patch bases, graph caches, snapshots, and web manifest expectations are incompatible. Regenerate them from the current source. SEMAPRAX deliberately does not accept an FNV fallback: an old patch fails with `SPX-G409` before modifying its source. Web consumers must reject `semaprax.web.v1` when they require the SHA-256 revision contract.

There is not yet a stable compatibility guarantee. Before 1.0, every breaking public syntax, CLI, diagnostics JSON, graph, patch, web manifest, package, component, or ABI change must add a section here and update the changelog.
