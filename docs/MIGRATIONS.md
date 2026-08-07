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

## Revision token FNV-1a64 to SHA-256

Graph v3, semantic patch bases, CLI output, and `semaprax.web.v2` manifests use one algorithm-tagged token:

```text
sha256:<64 lowercase hexadecimal digits>
```

The digest input is exactly `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. The domain separator and canonical projection are part of the protocol. Paths, comments, and formatting-only differences do not affect the token; semantic source changes do. This is collision-resistant content addressing and stale-base detection, not source authentication.

Legacy `fnv1a64:` patch bases, graph caches, snapshots, and web manifest expectations are incompatible. Regenerate them from the current source. SEMAPRAX deliberately does not accept an FNV fallback: an old patch fails with `SPX-G409` before modifying its source. Web consumers must reject `semaprax.web.v1` when they require the SHA-256 revision contract.

There is not yet a stable compatibility guarantee. Before 1.0, every breaking public syntax, CLI, diagnostics JSON, graph, patch, web manifest, package, component, or ABI change must add a section here and update the changelog.
