# Protocol migrations

SEMAPRAX is pre-alpha, but agent-facing changes are still explicit. Consumers must inspect the declared schema field rather than assuming every JSON object has the latest shape.

## Semantic graph v1 to v2

Graph v2 adds:

- top-level declaration/expression identity policy;
- deterministic revision-scoped structural nodes for function bodies and local bindings;
- `requires_graph` and `ensures_graph` expression structures;
- contract calls in function call dependencies and bounded context traversal.

The existing textual `requires` and `ensures` arrays remain during this migration. A v1-only consumer should reject `semaprax.graph.v2` explicitly. A compatible consumer may continue reading declaration IDs, names, signatures, effects, contracts, and calls while progressively adopting the new structural fields.

There is not yet a stable compatibility guarantee. Before 1.0, every breaking public syntax, CLI, diagnostics JSON, graph, patch, web manifest, package, component, or ABI change must add a section here and update the changelog.
