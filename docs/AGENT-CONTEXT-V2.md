# Agent Context v2

Audience: agent and tool authors, plus compiler contributors.

Status: implemented additive semantic-query contract. V1 remains the default
and retains its exact API, CLI behavior, and bytes. Supplying an explicit
direction selects v2:

```text
semaprax context <file|project> <symbol|stable-id>
  --direction forward|reverse|both
  [--depth N] [--max-bytes N] [--max-nodes N]
  [--filters contracts,ownership,effects,types,targets,diagnostics,tests]
```

Selecting a Project directory or its `semaprax.toml` authenticates the whole
declared source set and emits `semaprax.project-agent-context.v1`, a compact
projection of the existing `semaprax.project-semantic-context.v1` result from
its retained typed cross-file index. The exact Project and graph revisions are
present, and `context_revision` is the digest of that complete underlying
context. The public byte limit applies to the compact bytes actually returned.
The same direction, depth, byte, and node options apply; omitting direction
preserves v1's forward traversal default. Project context covers its six
structural edge families and therefore rejects the single-file `--filters`
option rather than pretending that filter changed the result.

To avoid repeating field names on every fact, the compact schema uses closed
positional rows:

- `target`: identity, declaration kind, source path, module;
- `query`: direction, depth, returned maximum bytes, maximum nodes, underlying
  authenticated-context maximum bytes (needed to replay `context_revision`);
- each `nodes` row: identity, node kind, declaration kind, source path, module,
  minimum depth, traversal provenance;
- each `edges` row: edge kind, caller, target, caller path, target path, site,
  expression identity;
- `budget`: used nodes, used edges, used depth.

`truncation` and `frontier` retain their named structures because agents must
inspect them before trusting closure. `authority` is always false. Malformed
compiler-owned input or an output that cannot fit the requested bound fails
closed with `SPX-G004`.

The admitted byte budget is `2048..=16777216`. The lower bound is large
enough for the smallest canonical v1/v2 envelope; longer identities or a
resumable frontier can still require the exact larger budget reported by
`SPX-G004` or by a frontier cursor.

The public Rust surface is `AgentContextDirection`,
`AgentContextV2Options`, and `agent_context_v2_json`. Direction names are
closed, exact, and case-sensitive. Unknown, missing, or duplicated direction
options reject before semantic output.

## Traversal

`forward` follows authenticated function and function-template call edges.
`reverse` follows an independently constructed caller index over the same
validated HIR. `both` follows their set union. Generic call-instance edges
target the persistent template; concrete instances remain metadata rather than
separately budgeted facts.

Traversal is breadth-first. The root is first and each depth is ordered by
stable declaration ID across both edge directions. A declaration reached by
multiple paths is emitted once at its minimum depth. Direction provenance is
retained only for paths at that minimum depth.

Every fact contains exact outgoing `calls` and exact `called_by`. The requested
direction alone controls traversal; the other relation is referential closure,
not hidden impact traversal.

## Budgets and two frontiers

V2 retains the v1 byte, node, depth, filter, used-budget, and fail-closed
limits. Its two frontiers have different meanings:

- `frontier` contains only selected-direction nodes omitted by `depth`,
  `max_nodes`, or `max_bytes`. Each entry records its traversal direction.
- `reference_frontier` contains only non-selected relation targets referenced
  by emitted facts. Each entry records `calls` or `called_by` provenance.

`omitted_known_nodes` and `deferred_known_nodes` count traversal omissions
only. `reference_closure.referenced_unselected_nodes` counts the separate
reference frontier. Reference closure is never reported as a truncation
reason.

The non-dangling invariants are separate: every emitted selected-direction edge
targets an emitted fact or `frontier`; every emitted non-selected relation
targets an emitted fact or `reference_frontier`. A target that is already an
omitted traversal node is never duplicated as reference-only.

Every resume entry binds the exact symbol, target, query direction, and a
sufficient byte budget. Depth, node, filter, and direction values remain bound
to the query. V2 validates that every advertised traversal or reference target
can emit its individual fact plus mandatory frontiers within the 16 MiB
contract maximum; permanently unavailable facts reject as `SPX-G004` rather
than producing a non-progressing cursor. Non-byte and reference cursors
conservatively advertise the contract maximum.

## Concrete generic instance ownership

When `types` or `ownership` is selected, a retained generic template fact also
contains `generic_instance_ownership`: the complete revision-bound Graph v34
facts for that template's admitted concrete instances, in the graph's identity
presentation order. The exact signatures, owned-leaf paths, selected cleanup
schema, inventory and plan, loans, and forwarding closure are replayed from
checked HIR. This field is absent when neither filter is selected.

These instances remain metadata inside one template fact rather than additional
traversal nodes. Their complete bytes count toward the ordinary fact budget;
a large instance closure may defer that entire fact or fail with `SPX-G004`.
No partial ownership inventory or cleanup plan is returned as a complete fact.
The same graph facts are available through ordinary Graph v34 and through
ProgramRoot's separately versioned generic-ownership node. The query grants no
execution, mutation, publication, or generic ABI authority.

## Compatibility and evidence boundary

`source_graph_schema` follows the selected graph contract, including additive
Graph v34 for programs with concrete generic function instances. Agent Context
v1 retains its previous source schema, facts, API, and bytes. The v2 envelope
and traversal rules are unchanged; selecting v34 changes the explicit source
schema and adds instance ownership under the selected filters. Programs without
concrete function instances retain their previous graph selection.

The earlier Graph-v14-backed forward, reverse, and both SHA-256 known answers
remain frozen historical projections:

- forward: `922404133444942ab86607772362098e0f5656add6bea607a890be2bcfe5b7c9`
- reverse: `9a2ebfe569926e67f436379cf2b5c96d510daadd11d0a295ed54903cb612627b`
- both: `4ec8a62a17551e87dc301d08f0a09c6159445757bca6dd9920a7db4e3790ce17`

The additive Graph-v34-backed projection of the same Effects-only fixture has
these forward, reverse, and both known answers. The gate also reconstructs the
historical source-schema projection and verifies all three prior hashes:

- forward: `928e1789312b7e2649b57ba0749440a31447c1a1a2269767592702a78d619f43`
- reverse: `2a91b46760ee702e3d0772dc17a5223767a6326a25cb92d7831fb31c494818df`
- both: `c99e0c8e1042e366abf15b1641ce378f1b94092c2ed3cf553fac4d171d90c5ae`

The earlier local v2 and legacy-v1 gates were 8/8 and 8/8. The full hosted matrix is green
in [run 31397881268, including Ubuntu job
93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327).

The executable gates cover deterministic JSON parsing, global per-depth order,
minimum-depth cycle handling, generic-template callers, direction-bound
traversal and reference replay, byte/node/depth truncation, permanent
unavailability, CLI confusion, v1 golden preservation, and every current Graph
schema selection. The Project CLI gate additionally proves directory/manifest
byte identity, raw-library fail-closed behavior, unsupported-filter rejection,
exact revision fields, canonical one-line JSON, and a calculator result below
2 KiB, 600 lexical units, and one sixth of the full authenticated Project graph.

V2 traverses call edges only; ownership metadata does not add reverse edges.
It does not claim reverse type, data, ownership,
effect, capability, cleanup, import, target, diagnostic, or test edges; impact
analysis; ranking; repository indexing; persistence; or a graph daemon.
The separate [Semantic Impact v1](SEMANTIC-IMPACT-V1.md) patch-preview contract
reuses the validated persistent-call index without changing this Context v2
schema, behavior, KATs, or nonclaims.
