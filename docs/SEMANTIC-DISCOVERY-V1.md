# Semantic Discovery v1

Audience: coding-agent and tool authors who need to learn what semantic
operations this compiler exposes without reading the whole repository, and
compiler contributors extending the shared `SEMANTIC-DISCOVERY` package.

Status: **LOCAL** bounded implementation with an executable reference and
focused regression corpus, implemented in `src/semantic_discovery.rs` and
`src/semantic_discovery/`. This is issue #125
("Expose compact capability discovery and revision-bound context deltas for
coding agents"). It composes existing owners rather than restating their
content, and is the shared foundation two related issues (#196, #197) build
on: see [Composing this module](#composing-this-module).

## Why this module instead of another static graph

The repository already has bounded, source-bound facts: `semaprax context`
(`semaprax.agent-context.v1`/`.v2`), `semaprax query --capabilities`
(`semaprax.installed-query-capabilities.v1`), the installed diagnostic
catalog, `capability-manifest`, `region-report`, `assurance_manifest` and
`agent_interaction_schema`. An agent orienting itself in this repository does
not need a *second* description of what any one of those does — it needs one
small index that says these exist, at which schema, through which surface,
and one way to ask "what changed" without re-fetching everything.
`semantic_discovery` is exactly that index plus that one delta operation. It
never re-derives another owner's facts; where it needs to be concrete (the
installed diagnostic catalog's code count, the installed query-capabilities
digest) it calls that owner's existing function and reports its live output,
so this catalog cannot drift out of sync with what it describes.

## Compact capability discovery

```rust
let options = semaprax::semantic_discovery::DiscoveryOptions::default(); // 8 KiB
let envelope = semaprax::semantic_discovery::generate_discovery_manifest(path, &options)?;
```

Read-only and source-bound: `generate_discovery_manifest` follows the same
`patch::canonical_source_path` / `read_source_snapshot` /
`validate_source_unchanged` pattern as `capability_manifest::generate`,
`region_report::generate`, `assurance_manifest::generate` and
`agent_interaction_schema::compile_agent_interaction_schema`. Source bytes
must remain unchanged between the snapshot and the final check or generation
fails closed.

The envelope is `semaprax.semantic-discovery.v1`:

```json
{
  "schema": "semaprax.semantic-discovery.v1",
  "digest": "sha256:...",
  "bytes": 1234,
  "payload": {
    "schema": "semaprax.semantic-discovery.v1",
    "selected_target": {"path": "example.spx", "revision": "..."},
    "tool_classes": ["read_only_delta", "read_only_help", "read_only_query", "read_only_report", "read_only_schema"],
    "operations": [
      {"name": "agent_context", "surface": "cli", "tool_class": "read_only_query", "payload_schemas": ["semaprax.agent-context.v1", "semaprax.agent-context.v2"]},
      {"name": "installed_query_capabilities", "surface": "cli", "tool_class": "read_only_help", "payload_schemas": ["semaprax.installed-query-capabilities.v1"], "digest": "sha256:..."},
      "... nine entries total, sorted by name ..."
    ],
    "known_limitations": ["..."]
  }
}
```

**Measured compactness.** On the smallest fixture module the crate's own
regression suite uses, the whole envelope is **2,426 bytes** — a few
kilobytes, not a full graph dump (`AGENTS.md` notes the full `graph` output
runs roughly forty times source size). `docs/AGENT-TASK-ECONOMICS-V1.md`
governs any further productivity/token-cost claim beyond this measured byte
count; this document only claims what it measured.

`tool_classes` is a closed, sorted set (`read_only_delta`, `read_only_help`,
`read_only_query`, `read_only_report`, `read_only_schema`); every listed
operation is read-only. `operations` is a static, sorted-by-name catalog;
`known_limitations` states explicitly that this is a capability description,
not a live authority grant, and that an operation listed here must still be
reconfirmed against the current revision before use — see the next
paragraph.

**Capability information is not authority, and staleness is checked, not
assumed.** `verify_discovery_manifest_against_source(envelope, source_path)`
independently rebinds a cached envelope's declared source revision to the
CURRENT bytes of `source_path`, replaying its digest and byte count exactly
like `capability_manifest::verify_envelope`. A caller holding a cached
discovery manifest MUST call this before acting on it: source drift after
generation means the manifest may describe operations or a target that no
longer resolve the same way, and stale capability information must never be
used to justify executing an operation. `SPX-Z305` reports every consistency
failure, including drift.

## Revision-bound context deltas

```rust
let request = semaprax::semantic_discovery::ContextDeltaRequest {
    symbol: "app.main",
    options: &v2_options,       // the exact graph::AgentContextV2Options used before
    claimed_base_revision: &base_revision,
    base_document: &base_document, // the exact prior semaprax.agent-context.v2 JSON
};
let delta = semaprax::semantic_discovery::compute_context_delta(path, &request)?;
```

`compute_context_delta` recomputes the exact same
[Agent Context v2](AGENT-CONTEXT-V2.md) query against the CURRENT source
(via `graph::agent_context_v2_json`, unmodified) and compares it against one
client-held prior response. It does not maintain a revision history: the
client supplies its own last-acknowledged document, and the server replays
truth from the live source, exactly the "client-side delta composition over
authenticated existing responses" the issue calls for. This module never
opens a new schema; `semaprax.agent-context.v2` remains the sole source of
declaration facts.

The response is `semaprax.semantic-discovery.context-delta.v1` with one of
three `outcome` values:

- **`"unchanged"`** — `claimed_base_revision` equals the current revision.
  (If the two documents' facts still disagreed, that would be a compiler
  determinism bug, not a client error: `compute_context_delta` reports it as
  a hard `SPX-Z304` error rather than silently trusting either side.)
- **`"delta"`** — the base was well-formed, exactly the same query and
  target, not itself truncated, and at a different revision. `diff.added`
  and `diff.changed` list the full current fact JSON for each affected
  declaration keyed by its persistent `@id`; `diff.removed` lists only the
  removed ids; `diff.unchanged_count` is the count of facts identical to the
  base. The current query's own `truncation` section is passed through
  unchanged, so a delta computed against a since-truncated current view
  still says so.
- **`"resync_required"`** — the base could not be safely diffed. The
  response embeds one bounded fresh `current_document` (the exact same
  `semaprax.agent-context.v2` JSON `compute_context_delta` itself produced)
  instead of an incomplete diff, and `reason` is exactly one of:

  | `reason` | Meaning |
  |---|---|
  | `malformed_base` | `base_document` is not parseable as one v2 context document with the required members. |
  | `schema_mismatch` | The base's `schema` is not the current query's schema (only v2-vs-v2 is diffed). |
  | `base_revision_mismatch` | The base document's own `revision` field disagrees with `claimed_base_revision`: the client's claim about its own document is internally inconsistent. |
  | `target_mismatch` | The base's `module`/`root` names a different declaration than the current query. |
  | `query_mismatch` | The base's `query` object (direction, depth, filters, max_bytes, max_nodes) differs from the current query. Widening or narrowing a query is a fresh fetch, not a delta. |
  | `base_truncated` | The base document itself reports `truncation.truncated: true`. A diff against an admittedly incomplete base cannot be trusted to be complete either, so this module refuses to compute one rather than mistake a partial base for full caller/ownership coverage. |
  | `delta_exceeds_max_bytes` | The honest delta (added + changed + removed + the current query's own truncation section) would exceed the query's own `max_bytes`. This module does not truncate a delta piecewise; it resynchronizes instead. |

**Identity across a rename.** A function's persistent `@id` is stable
independent of its display `name` (the same distinction `capability_manifest`
and every other stable-ID-keyed report rely on). Renaming a declaration
without touching its body changes that one declaration's fact (its `name`
field) and nothing else — its callers' own facts (their `calls` lists
reference the id, not the name) are unaffected and reported unchanged. A
body, effect, or type change to a declaration updates exactly the facts that
actually differ: if a callee gains a new effect its typed-effects contract
requires callers to declare too, both the callee's and caller's facts
legitimately change and the diff reports exactly those two, not more, not
fewer.

**Determinism.** `src/semantic_discovery/tests.rs` includes a golden test
(`compute_context_delta_is_deterministic_for_the_same_revision_pair`) that
calls `compute_context_delta` twice with the identical revision pair and
asserts byte-identical output.

## Composing this module

This module is the shared foundation of the `SEMANTIC-DISCOVERY` package
(issues #125, #196, #197, #200). It exposes:

- **An operation inventory** (`SEMANTIC_DISCOVERY_OPERATIONS`, rendered
  through `generate_discovery_manifest`) that a consumer extends by adding a
  catalog entry, not by building a parallel list. #196 (the version-matched
  Agent Skill bundle) had not landed an operation inventory of its own as of
  this writing; a future skill-bundle entry belongs in this same catalog
  rather than as a second one.
- **A service kernel** (`compute_context_delta`, `ContextDeltaRequest`,
  `parse_context_document`'s validation rules) over the existing
  `semaprax.agent-context.v2` schema, reusable by any transport (CLI, MCP,
  a future service route) that already has a way to hand the client's prior
  document back to the server.

Neither function performs a host effect, opens a socket, or grants a
capability; both are read-only projections of already-checked source.

## Diagnostics

| Code | Meaning |
|---|---|
| `SPX-Z301` | Invalid `DiscoveryOptions` (`max_bytes` out of `graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES`). |
| `SPX-Z302` | The discovery manifest or an `"unchanged"`/`"delta"` context-delta document exceeded its bounded output budget. Fails closed; never truncated. |
| `SPX-Z303` | `compute_context_delta`'s `symbol` does not resolve to any declaration reachable from the current source. |
| `SPX-Z304` | Two documents claim the same revision but disagree on their facts — a compiler determinism invariant failure, not a client error. |
| `SPX-Z305` | A discovery manifest envelope failed independent structural/digest replay, including source-revision drift detected by `verify_discovery_manifest_against_source`. |

## Non-goals

- This is not a second static graph and does not duplicate any other
  module's semantic facts in prose; where it needs to be concrete about
  another owner's output it calls that owner and reports a schema/digest.
- It does not add a workspace/Project-scoped operation inventory;
  `installed_query_capabilities` already owns that surface, and this
  module's `installed_query_capabilities` catalog entry points to it by
  digest rather than restating it.
- It does not implement `docs/AGENT-TASK-ECONOMICS-V1.md`'s comparative
  trial evidence. That evidence depends on issue #105 (the 18-trial paired
  pilot execution), which was still open at the time this module shipped;
  see the worker report for issue #125 for the exact blocking rationale.
