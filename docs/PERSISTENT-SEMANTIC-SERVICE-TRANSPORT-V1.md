# Persistent Semantic Workspace Service Transport v1

Status: additive single-client stdio transport; four focused integration cases
pass locally.

Audience: compiler contributors, local tool hosts, agent clients, and reviewers
of process-resident semantic service boundaries.

This transport exposes one Persistent Incremental Semantic Workspace Service
v1 through repeated JSON-RPC 2.0 lines in one `semaprax` process. One startup
Project is authenticated and retained for the complete process lifetime. The
session delegates query, transaction validation, and refresh to that one
service instead of rebuilding a one-shot service per request.

It is a local single-client stdio adapter, not MCP, LSP, a socket, daemon,
shared multiprocess service, editor protocol, or durable database.

## Command and framing

The exact command is:

```text
semaprax service <project>
```

`<project>` is one explicit Project directory or `semaprax.toml`. Startup uses
the existing authenticated Project loader once. After startup, stdin accepts
one UTF-8 JSON-RPC 2.0 request per LF-delimited frame and stdout returns at
most one LF-delimited response per call. EOF and the `shutdown` method end the
process. Notifications return no response; a shutdown notification also ends
the session.

The command has no policy path, output path, cache path, host grants, source
commit flag, network endpoint, or background mode. Invalid CLI grammar exits
with status 2 before starting the service.

## Protocol and lifecycle

`src/semantic_service_transport.rs` owns
`SemanticWorkspaceStdioSession`, `serve_semantic_workspace_stdio`, and:

```text
semaprax.semantic-workspace-service-transport.v1
semaprax.semantic-workspace-service-transport-result.v1
semaprax.semantic-workspace-service-transport-error.v1
```

The closed method order is:

```text
service/protocol
workspace/open
workspace/status
workspace/query
workspace/index-query
workspace/validate-transaction
workspace/refresh
shutdown
```

`service/protocol` reports that order, `authority: false`, no host grants,
limits, and explicit single-process/single-client nonclaims. `workspace/open`
marks the already constructed service ready and returns its exact open-work
receipt. Query, transaction validation, and refresh require that successful
open. `workspace/status` reports whether open occurred and the retained active
generation. A shutdown call returns one final success before termination.

Every successful semantic response wraps the current service generation's
Project revision, canonical workspace revision, image digest, `authority:
false`, transport/result schemas, and a method-specific payload. The JSON-RPC
request ID is returned by the existing shared codec.

## Exact delegation

`workspace/query` accepts exactly one `query` string containing canonical
Universal Semantic Query v1 JSON. It returns the exact core result value and
its query, payload, and result digests.

`workspace/index-query` accepts exactly one `query` string containing a
canonical retained-index query. It returns the exact bounded core result for
tests covering a stable declaration or functions that can reach a named
effect, plus the query and result digests. Refresh derives the replacement
indexes before the generation/cache/index CAS, so old snapshots retain their
old indexes and active queries reject stale revisions.

`workspace/validate-transaction` accepts exactly one `transaction` string
containing canonical Universal Semantic Transaction v1 JSON. It returns the
exact core impact, review, result, and evidence values and their existing
digests plus the candidate revision. Validation does not adopt the candidate
or change the service generation.

`workspace/refresh` accepts exactly:

- `expected_workspace_revision`;
- exact canonical manifest TOML in `manifest`; and
- an ordered `sources` array of closed `{path, source}` objects.

These are caller-owned bytes, not transport-selected filesystem paths. The
service applies its existing exclusive staged refresh and adopts generation
and semantic cache together only after complete admission and receipt
rendering. The response returns the exact refresh receipt, receipt digest, old
revision, and generation-reuse flag. Stale or invalid refresh leaves the
complete active generation/cache unchanged.

## Bounds and failure behavior

A request frame is at most 64 MiB and a response at most 128 MiB. Manifests are
also bounded to 65,536 bytes, source count retains the Project limit, and at
most 64 diagnostics are included in transport error data. Overflow is
rejection, not truncation or partial execution.

Malformed JSON-RPC uses the shared codec's standard protocol errors. Request
overflow uses `-32001`. Application failures use the closed transport-error
schema with `authority: false` and bounded existing Diagnostic JSON:

- `SPX-G548` owns invalid lifecycle, parameters, method, embedded core value,
  and transport service failure;
- `SPX-G549` owns transport manifest/source/document capacity; and
- `SPX-G528` through `SPX-G533`, query, transaction, Project, parser,
  verifier, and other core diagnostics retain their existing ownership and
  precedence.

Unknown fields are rejected. Query and transaction inputs remain exact
canonical strings, and refresh accepts no omitted or additional members.

## Authority, persistence, and compatibility

The transport core accepts an already admitted immutable Project and
caller-owned refresh bytes. It has no filesystem, process, network, secret,
key, test, execution, cache-store, commit, Git, publication, or deployment API.
The CLI performs only the explicitly requested startup Project reads; transport
requests cannot select further paths. No successful or failed request rewrites
those files.

State persistence means only that one process retains one in-memory service
generation across requests. There is no durable restart, crash recovery,
multi-client concurrency, locking protocol, scheduling, cancellation, watcher,
automatic refresh, history ledger, or cross-process visibility.

The feature is additive. Frozen Project Agent Transport v5,
`serve-workspace`, `serve-workspace-mcp`, MCP lifecycle/tool schemas, one-shot
Universal Semantic Workflow CLI, and all Project/image/query/transaction/core
service bytes remain unchanged. This transport is not an alias or successor to
those protocols and adds no authority to them.

## Focused evidence

`tests/workspace/persistent_semantic_service_transport.rs`, registered only in
the existing Workspace harness, covers one retained generation across repeated
open/status/query/transaction calls; exact direct-core query and transaction
parity; unchanged and changed refresh with cold equivalence; stale/failed
refresh rollback and old-query staleness; closed protocol/lifecycle,
malformed/unknown/oversized rejection and shutdown; a real long-running
`semaprax service` subprocess; bounded responses; complete unchanged fixture
inventory; and continued frozen v5 protocol identity.

```sh
CARGO_TARGET_DIR=target/persistent-semantic-service-transport-v1 \
  cargo test --locked -p semaprax --test workspace \
  persistent_semantic_service_transport --no-fail-fast
```

The four cases pass on the current checkout.
