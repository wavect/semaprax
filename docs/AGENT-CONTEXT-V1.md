# Agent context v1

Status: implemented, additive semantic-query contract. It bounds deterministic
UTF-8 JSON bytes and function facts; it does not claim an exact model-token
budget, relevance ranking, repository-wide impact analysis, or facts absent
from the current Graph v10-v14 lattice.

`semaprax.agent-context.v1` is the exact default CLI projection for:

```text
semaprax context <file> <symbol|stable-id>
  [--depth N] [--max-bytes N] [--max-nodes N]
  [--filters contracts,ownership,effects,types,targets,diagnostics,tests]
```

Supplying an explicit `--direction forward|reverse|both` selects the additive
[`semaprax.agent-context.v2`](AGENT-CONTEXT-V2.md) contract; omitting it retains
the v1 API, CLI behavior, and bytes.

Options are closed, single-use, and require canonical decimal integers.
Unknown options and filters, duplicates, missing values, leading-zero numbers,
empty filters, `max_nodes = 0`, and values outside the published bounds fail
before semantic output. Defaults are depth 1, 64 KiB, 256 function facts, and
the four supported filters shared across Graph v10-v14.

## Envelope and budgets

Every result contains schema/source schema, canonical source revision, module,
exact root ID, normalized query, filter support, used budget, truncation and
omission evidence, a stable-ID frontier, and ordered function facts.

`used_bytes` is exactly the returned JSON's UTF-8 byte length and excludes the
CLI newline. `used_nodes` counts emitted function facts. `max_bytes` covers the
complete envelope, frontier, and facts. If the canonical envelope and required
frontier cannot fit, the query fails closed as `SPX-G004`; it never emits an
oversized partial document.

Reasons are `depth`, `max_nodes`, `max_bytes`, and `unavailable_filters`.
`omitted_known_nodes` counts every known omitted function, including a
byte-budget suffix that is deliberately paginated. `deferred_known_nodes`
counts omitted functions not listed in the current frontier;
`omitted_fact_bytes` counts canonical fact payload bytes omitted by budgets.
The first byte-omitted function is always the progress cursor. Every other
byte-omitted direct callee referenced by an emitted fact is also listed, so an
emitted call edge never dangles behind `deferred_known_nodes`. A byte resume
re-roots at the omitted stable ID, retains the depth, node limit, and filters,
and supplies a sufficient `min_bytes`; replaying it emits that individual fact
and exposes its later cursor. This permits an aggregate prefix larger than 16
MiB to paginate while every individual fact page remains bounded. If the
individual fact plus its mandatory direct-callee frontier cannot fit even the
16 MiB contract maximum, the query fails closed as permanently unavailable
instead of emitting a capped, non-progressing cursor. Depth and node frontier
items likewise replay as their stable-ID root. The top-level `resume_contract`
binds these requirements, so a consumer cannot silently change filters or
limits while claiming continuation.

The core `calls` array is intentionally limited to SEMAPRAX function
dependencies, each of which is present as a fact or frontier ID. Import calls
are not exposed as dangling IDs in this contract; their semantic subgraph is a
future facet.

For `semaprax.graph.v14` sources, function facts additionally distinguish
`function_template` from monomorphic `function`. Template facts carry
owner/index parameters plus the exact explicitly referenced concrete instances;
an unused template has an empty instance list. Monomorphic caller facts carry
exact `call_instances` with expression, template, derived instance, and ordered
`i64`/`bool` arguments. These additions do not fabricate a separately budgeted
function fact for each concrete instance. Every result, including a legacy
root, reports the program-wide v14 `source_graph_schema`.

## Filters and limits

Graph v10-v14 support exact compact `contracts`, parameter/result `ownership`,
`effects`, and `types` facets. Each emitted function carries a reference index
for contract value roots and referenced nominal declarations; resource drop
meaning is embedded as a closed strategy rather than an unresolved lifecycle
or import reference. Cleanup, lifecycle, and import subgraphs are intentionally
not claimed by this v1 projection. The current lattice has no target,
diagnostic, or test nodes.
Those names are closed and accepted, but are listed under
`filter_support.unavailable`; no facts are inferred from filenames, CI, or
source text.

The legacy Rust `graph::context_json` depth-only slice remains compatible. New
consumers use `graph::agent_context_json` with validated
`AgentContextOptions`. Neither API persists an index; both validate the current
resolved HIR on every query.
