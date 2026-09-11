# Semantic Service Surface Consolidation Audit v1

Status: audit and one proven boundary; **local evidence only**, not a HOSTED
badge. This document records findings and does not itself implement a new
protocol, transport, or client.

Audience: coordinators and implementing agents working on the multi-surface
consolidation tracked by GitHub issue #200, and reviewers who need to know
which claim in that issue is already true, partially true, or not yet true.

## Why this document exists

Issue #200 asks to "consolidate CLI, LSP, MCP, and generated SDKs on one
versioned semantic service API." Its audit baseline
(`ae25c6a49dc09ec4613c7a6f52a27daa69dcd3f3`) asserts that "a persistent
incremental semantic service, stdio transport, MCP facade, universal
query/transaction operations, and multiple generated clients exist" and that
"several older Project Agent Transport and Image/Workspace protocols remain
for compatibility." Before any further consolidation work is spent, this audit
separates three different situations issue #200 could actually be in, because
the correct next step differs for each:

1. a shared service API already exists and the named surfaces already sit on
   it;
2. a shared core exists but a surface duplicates logic or schema on top of it;
3. no shared service API exists and each surface is independent.

**Finding: this repository is simultaneously in state 1 for one protocol
family and state 3 across protocol families, and the "LSP" surface named by
the issue does not exist at all.** The sections below give exact evidence for
each claim, one operation proven byte-equivalent across two real surfaces, and
a precise list of what remains.

## Fact: there is no LSP module in this repository

`ls src/ | grep -i lsp` and `rg -il '\blsp\b' src/` return no module that
implements the Language Server Protocol. The string `lsp` appears only inside
authority-nonclaim prose (for example
`src/semantic_service_transport.rs:336`'s
`"not_socket_mcp_lsp_or_shared_multiprocess_service"` and several passages of
[Architecture](ARCHITECTURE.md) disclaiming LSP authority) and inside
spec prose describing what has *not* been built. No `initialize`/`textDocument/*`
handling, no LSP crate dependency, and no LSP integration test exist. This was
independently confirmed by the worker auditing issue #199 while checking
whether repair plans are exposed over LSP, and is re-confirmed here.

This means acceptance criterion "CLI, LSP, MCP, and SDKs share one underlying
operation schema and semantics" cannot be met for LSP by any amount of
refactoring existing code: there is no LSP surface to bring into alignment.
Writing an LSP server from scratch is a separate, much larger effort (a
`textDocument/*` state machine, incremental document sync, editor-facing
diagnostics translation) and is explicitly out of scope for this bounded
worker assignment per its instructions.

## What the CLI actually exposes today

`src/cli/mod.rs` dispatches roughly thirty subcommands (`src/cli/*.rs`).  The
two relevant to semantic service consolidation are:

- `semaprax query <project> <declarations|symbol|context|impact|available-operations> ...`
  (`src/cli/query.rs`, `run_universal`) — a one-shot Universal Semantic Query
  v1 client.
- `semaprax service <project> [--mcp]` (`src/cli/service.rs`) — starts the
  persistent stdio JSON-RPC transport, or the MCP facade over the same
  transport, for the process lifetime.

Both of these **already delegate to the identical kernel call**. In
`src/cli/query.rs::run_universal`:

```rust
let service = SemanticWorkspaceService::open(snapshot.retain_revision())?;
...
Ok(service.query(query.to_json().as_bytes())?.to_json().to_owned())
```

and in `src/semantic_service_transport.rs`'s `"workspace/query"` dispatch arm,
which both the raw stdio transport and the MCP facade's `workspace__query`
tool route through:

```rust
let result = self.service.query(query.as_bytes())?;
```

`SemanticWorkspaceService::query` (`src/project/semantic_service.rs`) is the
same function in both call sites — not two implementations of the same idea,
the same compiled code path. CLI one-shot query and the persistent
transport/MCP facade were already consolidated on one kernel before this audit;
the difference between them is service lifetime (fresh service per CLI
invocation vs. one retained generation across a process's requests) and
framing (raw stdout vs. JSON-RPC/MCP envelope), which is exactly the kind of
difference [Persistent Incremental Semantic Workspace Service
v1](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md) says is the transport's job,
not the kernel's.

Other CLI commands — `context`, `graph`, `review` (legacy patch path), `fix`
— call lower-level Project/workspace-analysis functions directly
(`ProjectRevision::semantic_context`, `snapshot.semantic_graph()`, etc.)
rather than going through `SemanticWorkspaceService`. Where the same
underlying data is involved this is not silent drift: for example `context`'s
compact `semaprax.project-agent-context.v1` projection
(`src/cli/context.rs`) is a byte-budget-constrained re-encoding of the exact
same `semaprax.project-semantic-context.v1` payload that Universal Semantic
Query v1's `context` operation embeds verbatim (see
[Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md), "`context` and
`impact`"). It is a second wire *schema* for agent token economy, over the
same computed payload, documented as such — not a second computation.

## What MCP actually exposes today

`src/semantic_service_mcp.rs` (`SemanticWorkspaceMcpSession`) is a framing
adapter with a closed seven-tool catalogue
(`service__protocol`, `workspace__status`, `workspace__query`,
`workspace__index_query`, `workspace__history_query`,
`workspace__validate_transaction`, `workspace__refresh`). Every tool forwards
one inner JSON-RPC request, with id `0`, to the same
`SemanticWorkspaceStdioSession` dispatch table used by the raw stdio
transport (`src/semantic_service_transport.rs`). It performs no independent
query execution, result construction, or schema translation of its own; it is
strictly additive framing over the transport, which is strictly a session
wrapper over the same `SemanticWorkspaceService` the CLI uses. There is
exactly one MCP surface exposing this kernel — the older
`serve-workspace-mcp`/Project Agent Transport v5 MCP catalog is a separate,
frozen protocol generation (see below), not a second implementation of this
one.

## Is there a versioned semantic service API already?

Yes, for query/transaction/refresh: [Persistent Incremental Semantic
Workspace Service v1](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md)
(`src/project/semantic_service.rs`) plus [Universal Semantic Query
v1](UNIVERSAL-SEMANTIC-QUERY-V1.md) (`src/project/semantic_query.rs`) plus
[Universal Semantic Transaction v1](UNIVERSAL-SEMANTIC-TRANSACTION-V1.md) form
one closed, versioned, schema-identified operation surface
(`semaprax.semantic-workspace-service-work.v1`,
`semaprax.semantic-query.v1`, `semaprax.semantic-transaction.v1`, etc.), and
both the CLI one-shot path and the persistent stdio/MCP path are literally
built on it, as shown above. This is the "one documented public
semantic-service API" issue #200 asks for, for the operations it covers
(seven query operations, transaction validation, refresh). It does not yet
cover build/test/run, publication, or Git operations, all of which are
explicitly out of scope per issue #200's own "Explicitly out of scope"
section (combining read, write, execution, and publication authority into one
session is disallowed by design, so a single session token was never the
goal).

No, for the repository as a whole: at least a dozen older, independent
protocol generations exist alongside it and are not built on this kernel:

| Generation | Docs | Kernel relationship |
| --- | --- | --- |
| Project Agent Transport v5 / v6 | [PROJECT-AGENT-TRANSPORT-V5.md](PROJECT-AGENT-TRANSPORT-V5.md), [PROJECT-AGENT-TRANSPORT-V6.md](PROJECT-AGENT-TRANSPORT-V6.md) | Independent; predates the v1 semantic-service kernel; explicitly frozen for compatibility per those docs and [Persistent Semantic Workspace Service Transport v1](PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md)'s "Frozen Project Agent Transport v5... remain unchanged." |
| Image/Workspace protocol family (`IMAGE-*.md`, `WORKSPACE-*.md`, ~40 documents) | e.g. [IMAGE-WORKSPACE-PROTOCOL-V5.md](IMAGE-WORKSPACE-PROTOCOL-V5.md) | Independent; own schemas, own `image_transport.rs`/`agent_transport.rs` modules, own MCP adapter (`serve-workspace-mcp`), not delegating to `SemanticWorkspaceService`. |
| Generated TypeScript workflow SDK | [IMAGE-PACKAGED-TYPESCRIPT-WORKFLOW-SDK-V1.md](IMAGE-PACKAGED-TYPESCRIPT-WORKFLOW-SDK-V1.md), [PROJECT-AGENT-TRANSPORT-V6-SDK-V1.md](PROJECT-AGENT-TRANSPORT-V6-SDK-V1.md) | Generated from the **older** Agent Transport v6 / Image protocol schema, not from Universal Semantic Query/Transaction v1. |
| Generated native Rust SDK | `src/project/native_sdk.rs`, `tests/public_native_rust_sdk_v1.rs` | A different, native-ABI-facing surface (public generic ownership consumers), unrelated to the JSON semantic-service schema. |

So "generated SDKs" named by issue #200's starting points exist, but they are
generated from the protocol generation that predates the current kernel, not
from `semaprax.semantic-query.v1`/`semaprax.semantic-transaction.v1`. No
generated TypeScript/Python/Rust client exists yet for Universal Semantic
Query/Transaction v1 or the persistent-service transport/MCP schemas. This is
the real, unresolved instance of "schema and behavior drift" issue #200 warns
about: not CLI-vs-MCP-vs-LSP (LSP doesn't exist; CLI and MCP already agree),
but current-kernel-vs-generated-SDK.

## State classification, precisely

- **State 1** (shared core, surfaces already sit on it): CLI one-shot
  Universal Query v1 subcommands, the persistent stdio transport, and its MCP
  facade, for the seven query operations plus transaction validation and
  refresh. Proven below with a real cross-process test, not just by reading
  the source.
- **State 2** (shared core exists, one surface adds a deliberate, documented
  secondary schema over the same payload): the CLI's `context` command's
  compact `semaprax.project-agent-context.v1` projection over the same
  `semaprax.project-semantic-context.v1` payload Universal Query v1's
  `context` operation embeds. Not drift; a documented token-budget adapter.
- **State 3** (no shared service API; fully independent): Project Agent
  Transport v5/v6, the Image/Workspace protocol family, and every generated
  SDK, relative to the v1 semantic-service kernel. These are correctly
  described by their own docs as frozen/compatibility surfaces, but issue
  #200's "one documented public semantic-service API" does not yet name them
  in one place with an explicit support-status classification; that
  classification is scattered across a dozen per-protocol documents' own
  "Compatibility" sections instead of being addressed to a reader who does not
  yet know which of the ~60 `docs/*TRANSPORT*`, `docs/IMAGE-*`,
  `docs/PERSISTENT-*`, and `docs/WORKSPACE-*` documents is current.

## Cross-transport equivalence, proven for one operation

[Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md)'s own focused
evidence proves CLI-output-equals-direct-core-call
(`tests/workspace/universal_semantic_workflow_cli.rs`,
`all_five_query_modes_equal_the_exact_direct_core_results_and_write_nothing`)
and separately proves transport/MCP-output-equals-direct-core-call
(`tests/workspace/persistent_semantic_service_transport.rs`,
`tests/workspace/persistent_semantic_service_mcp.rs`). No existing test ties
two *surfaces* directly to each other by spawning both real processes for the
same request and comparing their answers, which is what issue #200's required
test "Cross-transport result equivalence for every supported operation" asks
for.

`tests/workspace/cli_mcp_query_cross_transport_v1.rs` (new, registered as a
module of the existing Workspace harness in `tests/workspace.rs`) closes that
gap for the `declarations` operation: it spawns the real `semaprax` CLI binary
(`semaprax query <project> declarations ...`) and, separately, a real
`semaprax service <project> --mcp` subprocess driven over its NDJSON protocol,
issues the same `SemanticQuery::declarations` request to both, and asserts the
parsed JSON payloads are structurally equal to each other and to the
in-process direct-core `SemanticWorkspaceService::query` result. This is one
operation, not all seven; extending the same pattern to `symbol`, `context`,
`impact`, `available_operations`, and transaction validation is the same
mechanical pattern repeated six more times and is listed under "What remains."

## Acceptance-criteria audit

| Criterion | Status | Evidence |
| --- | --- | --- |
| One documented public semantic-service API | Partially met | [Persistent Incremental Semantic Workspace Service v1](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md) + [Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md) + [Universal Semantic Transaction v1](UNIVERSAL-SEMANTIC-TRANSACTION-V1.md) are that API for query/transaction/refresh. Not met for build/test/run/publication (explicitly out of scope) or for a single index naming every other protocol's support status in one place (this document is a first step, not that index). |
| CLI, LSP, MCP, and SDKs share one underlying operation schema and semantics | Not met as stated; partially met for what exists | LSP does not exist (nothing to share with). CLI and MCP already share the kernel (proven above and by the new cross-transport test). Generated SDKs do not share it — they are generated from an older protocol generation. |
| Authority classes remain distinct and explicit | Met for the v1 kernel | Every response schema carries `authority: false`; `SemanticWorkspaceService::refresh_owned_sources` requires caller-owned bytes, not filesystem authority; MCP/stdio grant no new authority per [PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md](PERSISTENT-SEMANTIC-SERVICE-MCP-V1.md) "Authority and compatibility". Not audited here for the older Image/Workspace family. |
| Older transports have clear frozen/deprecation/support status | Partially met | Individual docs (e.g. Persistent Semantic Workspace Service Transport v1's "Frozen Project Agent Transport v5... remain unchanged") state this per-protocol. No single classification table existed before this document's "State classification" and generation table above. |
| Integration no longer requires understanding every internal protocol generation | Not met | A new integrator still needs to read the individual docs this audit's table summarizes to find the current kernel among ~60 protocol documents. |

## What remains (honest scope boundary)

- Writing an LSP server is not attempted here; it is a separate, larger
  effort and explicitly out of this worker's assignment.
- Cross-transport equivalence is proven for one operation
  (`declarations`) as a concrete pattern; `symbol`, `context`, `impact`,
  `available_operations`, `validate-transaction`, and `refresh` need the same
  treatment to close the "every supported operation" bar completely.
- No TypeScript/Python/Rust client has been generated from
  `semaprax.semantic-query.v1`/`semaprax.semantic-transaction.v1`/the
  transport-v1 method table. Issue #200's "generate all transports/clients
  from one schema" step is unimplemented; existing generated SDKs remain
  bound to the older Agent Transport v6/Image protocol generation.
- No single top-level document classifies every one of the ~60
  `docs/*TRANSPORT*`, `docs/IMAGE-*`, `docs/PERSISTENT-*`, and
  `docs/WORKSPACE-*` documents by support status; this audit's generation
  table is a starting classification, not that complete index, and
  `docs/COMPLETION-MATRIX.md` (out of this worker's file lease) is the
  correct owner for any status-row change that follows from it.
- Session-authority-matrix and confused-deputy tests, schema/client
  regeneration determinism, and paging/cancellation/capacity tests named in
  issue #200's "Required tests and evidence" are not attempted here; they
  require the not-yet-built generated-client step to have a subject.
