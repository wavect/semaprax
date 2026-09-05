# Persistent Semantic Workspace Service MCP v1

Status: additive bounded single-client stdio facade; focused tests passed locally.

Audience: local MCP hosts, agent clients, compiler contributors, and reviewers
of persistent semantic-service authority boundaries.

This protocol exposes one already authenticated Persistent Incremental
Semantic Workspace Service v1 to an MCP client. The exact command is:

```text
semaprax service <project> --mcp
```

`<project>` is one explicit Project directory or `semaprax.toml`. Startup reads
and authenticates that Project exactly once. After startup, no MCP method or
tool can select or reopen a host path. The optional `--mcp` suffix is the only
new CLI grammar; `semaprax service <project>` retains its exact JSON-RPC service
transport.

## Framing and lifecycle

The facade uses one UTF-8 JSON-RPC 2.0 object per LF-delimited MCP frame. The
request bound is 64 MiB. The response bound is six times the existing 128 MiB
inner service-response bound plus 4096 bytes, accounting for worst-case JSON
string escaping and fixed MCP syntax. The complete response capacity is
reserved before a tool dispatch, so a successful refresh cannot become an
unreportable post-mutation overflow. This is a byte bound, not a general heap or
allocator-failure claim.

The schema identity is:

```text
semaprax.semantic-workspace-service-mcp.v1
```

The lifecycle is closed:

```text
New -> initialize -> AwaitingInitialized
AwaitingInitialized -> notifications/initialized -> Ready
Ready -> tools/list | tools/call
EOF -> terminated
```

`ping` is accepted before or after initialization. Notifications other than a
valid `notifications/initialized` transition have no effect and never invoke a
tool. Repeated initialization, early tool calls, unknown methods, unknown
members, non-object params, over-depth/over-work JSON, noncanonical IDs, raw
newlines, and oversized frames fail closed. Initialization returns MCP version
`2025-11-25`, a fixed server identity, `tools.listChanged: false`, and explicit
authority-free instructions.

## Closed tool inventory

`tools/list` has one page and returns exactly these tools in this order:

| MCP tool | Existing service method | Exact arguments |
| --- | --- | --- |
| `service__protocol` | `service/protocol` | `{}` |
| `workspace__status` | `workspace/status` | `{}` |
| `workspace__query` | `workspace/query` | `{query: string}` |
| `workspace__index_query` | `workspace/index-query` | `{query: string}` |
| `workspace__history_query` | `workspace/history-query` | `{query: string}` |
| `workspace__validate_transaction` | `workspace/validate-transaction` | `{transaction: string}` |
| `workspace__refresh` | `workspace/refresh` | `{expected_workspace_revision: string, manifest: string, sources: [{path: string, source: string}]}` |

Each input schema is closed. The query and transaction strings retain their
existing exact-canonical-JSON requirements. Refresh retains canonical manifest,
Project source-count, source identity, source-byte, expected-revision, staged
validation, and atomic generation/cache/index replacement rules. Its `path`
members are Project-relative source identities interpreted by the core, not
filesystem selectors exposed to the MCP host.

`tools/call` forwards one private inner request with ID zero. Its MCP result has
one text content item containing the complete existing service JSON-RPC
response and `isError` reflecting the inner response. This deliberately does
not translate, weaken, or fork the service's application diagnostics, result
schemas, digests, revision checks, or refresh rollback behavior.

## Authority and compatibility

The facade creates no filesystem, process, network, home, secret, key, Git,
commit, publication, deployment, socket, listener, watcher, scheduling,
cancellation, multi-client, or durable-state authority. Caller-owned refresh
bytes can replace only the in-memory retained generation after full core
validation; no original source file is rewritten. EOF discards process-local
state.

This surface is additive. It neither imports nor modifies frozen Project Agent
Transport v5 bytes, its MCP catalog, `serve-workspace-mcp`, or any Project/image
protocol schema. It reuses only the independent Persistent Semantic Workspace
Service Transport v1 method boundary.

## Focused evidence

`tests/workspace/persistent_semantic_service_mcp.rs`, registered in the existing
Workspace harness, covers the lifecycle gate, exact seven-tool catalogue, retained
revision across protocol/status/query, unavailable tools, and a real
`semaprax service <project> --mcp` NDJSON subprocess.

```sh
CARGO_TARGET_DIR=target/persistent-semantic-service-mcp-v1 \
  cargo test --locked -p semaprax --test workspace \
  persistent_semantic_service_mcp --no-fail-fast
```

The two cases pass locally with no failures.
