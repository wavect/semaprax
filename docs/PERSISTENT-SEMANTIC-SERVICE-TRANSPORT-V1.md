# Persistent Semantic Workspace Service Transport v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

Audience: compiler contributors, local tool hosts, agent clients, and reviewers
of process-resident semantic service boundaries.

This transport serves one Persistent Incremental Semantic Workspace Service
v1 through JSON-RPC 2.0 lines in one `semaprax` process. It authenticates one
Project at startup and retains it for the process lifetime. Queries,
transaction validation, and refresh reuse that service instead of creating a
new one per request.

It is a local single-client stdio adapter, not LSP, a socket, daemon, shared
multiprocess service, editor protocol, or durable database. The separately
versioned [MCP facade](PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md) is an additive
framing adapter over this exact session; it does not change this protocol.

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
workspace/history-query
workspace/validate-transaction
workspace/validate-transaction-v2
workspace/validate-transaction-v2-workflow
workspace/patch-receipt
workspace/verify-patch-receipt
workspace/patch-receipt-refusal
workspace/verify-patch-receipt-refusal
workspace/patch-receipt-evidence-summary
workspace/patch-receipt-evidence-page
workspace/compare-patch-receipts
workspace/compact-projection
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

`workspace/history-query` accepts exactly one `query` string containing a
canonical, revision-bound service-history query. It returns a bounded page of
successful transaction-validation and refresh outcomes in mutex-serialized
observed-call order. Failed or stale attempts do not append entries; the order
is not a claim about deterministic scheduling between concurrent callers.

`workspace/validate-transaction` accepts exactly one `transaction` string
containing canonical Universal Semantic Transaction v1 JSON. It returns the
exact core impact, review, result, and evidence values and their existing
digests plus the candidate revision. Validation does not adopt the candidate
or change the service generation.

`workspace/validate-transaction-v2` likewise accepts exactly one canonical
Universal Semantic Transaction v2 `ReplaceExpression` string. It returns the
exact v2 impact, review, result, and evidence values with their existing
digests plus the candidate revision. `workspace/validate-transaction-v2-workflow`
accepts the existing bounded ordered v2 step array. Both are validation-only:
they retain a bounded history item but neither adopts, executes, writes, or
publishes the candidate.

The patch-receipt methods accept a canonical v1 transaction string and rebuild
its candidate from the exact active immutable generation before delegating to
the compiler-owned receipt APIs. Receipt verification and comparison replay the
candidate independently before exact byte comparison. A stale transaction
workspace revision fails before candidate derivation. The refusal pair renders
and verifies only an explicit stale-selector refusal. Evidence summary and page
accept only the four compiler selectors and their derived handles/cursors;
they do not accept a path, URL, JSON pointer, or caller-supplied evidence
document. These receipt adapters do not append service history.

`workspace/compact-projection` accepts `expected_workspace_revision`, one
closed Compact Semantic Projection v1 profile, and `encoding` (`text` or
`binary`). The only optional selectors are a retained `source_path` label and
`root` for source-derived profiles, canonical candidate recovery bytes for the
candidate-delta profile, and an admitted `agent_id` for the Agent Definition
profile. The exact expected revision is selected before parsing or recovery.
Source labels match only exact bytes already retained in that Project generation;
they never select host paths. Full graph without a source label compacts the
retained Project semantic graph. The route delegates to existing graph,
task-context, Project API, candidate-delta, and Agent Definition producers,
then returns text or hexadecimal binary with exact metadata. It adds no
semantic producer or authority.

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
multi-client concurrency, external locking protocol, scheduling, cancellation,
watcher, automatic refresh, durable history ledger, or cross-process visibility.

The feature is additive. Frozen Project Agent Transport v5,
`serve-workspace`, `serve-workspace-mcp`, their MCP lifecycle/tool schemas, one-shot
Universal Semantic Workflow CLI, and all Project/image/query/transaction/core
service bytes remain unchanged. This transport and its separately versioned MCP
facade are not aliases or successors to those protocols and add no authority to
them.

## Focused evidence

`tests/workspace/persistent_semantic_service_transport.rs` and
`tests/workspace/persistent_semantic_service_compact_projection.rs`, registered
only in the existing Workspace harness, cover one retained generation across repeated
open/status/query/transaction calls; exact direct-core query and transaction
parity; unchanged and changed refresh with cold equivalence; stale/failed
refresh rollback and old-query staleness; closed protocol/lifecycle,
malformed/unknown/oversized rejection and shutdown; retained Project API compact
projection parity; stale/unknown compact projection refusal; a real long-running
`semaprax service` subprocess; bounded responses; complete unchanged fixture
inventory; and continued frozen v5 protocol identity.

```sh
CARGO_TARGET_DIR=target/persistent-semantic-service-transport-v1 \
  cargo test --locked -p semaprax --test workspace \
  persistent_semantic_service_transport --no-fail-fast
```

The four cases pass on the current checkout.
