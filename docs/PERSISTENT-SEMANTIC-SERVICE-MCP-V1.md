# Persistent Semantic Workspace Service MCP v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: local MCP hosts, agent clients, compiler contributors, and reviewers
of persistent semantic-service authority boundaries.

This protocol gives an MCP client access to one already authenticated
Persistent Incremental Semantic Workspace Service v1. Run:

```text
semaprax service <project> --mcp
```

`<project>` is one explicit Project directory or `semaprax.toml`. Startup reads
and authenticates that Project exactly once. After startup, no MCP method or
tool can select or reopen a host path. The optional `--mcp` suffix is the only
new CLI grammar; `semaprax service <project>` retains its exact JSON-RPC service
transport.

## Framing and lifecycle

Each LF-delimited MCP frame holds one UTF-8 JSON-RPC 2.0 object. Requests are
limited to 64 MiB. Responses are limited to six times the existing 128 MiB
inner-service bound plus 4096 bytes for worst-case JSON escaping and MCP syntax.
The facade reserves full response capacity before dispatch, so a successful
refresh cannot overflow after mutation without a report. The limit bounds
bytes, not general heap use or allocator failure.

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
| `workspace__validate_transaction_v2` | `workspace/validate-transaction-v2` | `{transaction: string}` |
| `workspace__validate_transaction_v2_workflow` | `workspace/validate-transaction-v2-workflow` | `{steps: [string, ...]}` |
| `workspace__patch_receipt` | `workspace/patch-receipt` | `{transaction, candidate_digest}` |
| `workspace__verify_patch_receipt` | `workspace/verify-patch-receipt` | `{transaction, candidate_digest, receipt}` |
| `workspace__patch_receipt_refusal` | `workspace/patch-receipt-refusal` | `{transaction, requested_candidate_digest}` |
| `workspace__verify_patch_receipt_refusal` | `workspace/verify-patch-receipt-refusal` | `{transaction, requested_candidate_digest, receipt}` |
| `workspace__patch_receipt_evidence_summary` | `workspace/patch-receipt-evidence-summary` | `{transaction, candidate_digest}` |
| `workspace__patch_receipt_evidence_page` | `workspace/patch-receipt-evidence-page` | `{transaction, candidate_digest, evidence_id, handle, cursor, page_size, max_bytes}` |
| `workspace__compare_patch_receipts` | `workspace/compare-patch-receipts` | `{left_transaction, left_candidate_digest, left_receipt, right_transaction, right_candidate_digest, right_receipt}` |
| `workspace__compact_projection` | `workspace/compact-projection` | `{expected_workspace_revision: string, profile: closed compact-v1 profile, encoding: text|binary, source_path?: retained source label, root?: string, candidate_capsule?: canonical bytes, agent_id?: string}` |
| `workspace__refresh` | `workspace/refresh` | `{expected_workspace_revision: string, manifest: string, sources: [{path: string, source: string}]}` |

Each input schema is closed. The query and transaction strings retain their
existing exact-canonical-JSON requirements. Refresh retains canonical manifest,
Project source-count, source identity, source-byte, expected-revision, staged
validation, and atomic generation/cache/index replacement rules. Its `path`
members are Project-relative source identities interpreted by the core, not
filesystem selectors exposed to the MCP host.

Compact projection forwards unchanged through the same inner service
dispatcher. It binds the supplied workspace revision before projection work.
Its source label only selects exact bytes already retained in that
generation, never a host path; its candidate capsule is restored against the
same retained revision. It returns an authority-free compact envelope over
existing kernels, as text or binary hex.

Patch-receipt tools rebuild candidates only from the already retained active
generation and canonical v1 transaction bytes. Verification and comparison
perform the candidate layer's independent replay before comparing receipts.
Evidence paging exposes only compiler-selected retained families and requires
the derived handle and cursor; it never treats an argument as a host path or
external evidence locator. These read-only calls do not append service history.

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
Workspace harness, covers the lifecycle gate, exact eight-tool catalogue, retained
revision across protocol/status/query, unavailable tools, and a real
`semaprax service <project> --mcp` NDJSON subprocess.

```sh
CARGO_TARGET_DIR=target/persistent-semantic-service-mcp-v1 \
  cargo test --locked -p semaprax --test workspace \
  persistent_semantic_service_mcp --no-fail-fast
```

The two cases pass locally with no failures.
