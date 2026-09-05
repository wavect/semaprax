# Persistent Incremental Semantic Workspace Service v1

Status: additive process-resident core; focused local evidence passed.

Audience: compiler contributors, workspace-service hosts, agent-tool authors,
and reviewers of incremental semantic state.

Persistent Incremental Semantic Workspace Service v1 is the first
transport-neutral service core over an admitted immutable `ProjectRevision`.
It retains one indivisible semantic generation and one compiler-created
checked-module cache inside the caller's process. It supports revision-bound
read snapshots, bounded semantic query delegation, source-exact incremental
refresh, and authority-free Universal Semantic Transaction validation.

This badge is a library core, not a daemon or persistent public service route.
The separate [Universal Semantic Workflow CLI
v1](UNIVERSAL-SEMANTIC-WORKFLOW-CLI-V1.md) creates it for one authenticated
Project operation and immediately drops it; that adapter does not expose the
service lifecycle or make it shared. This core does not add a wire protocol,
MCP tool, LSP server, editor integration, filesystem watcher, socket, or
multi-process service. The embedding host owns process lifetime, input
admission, transport, concurrency, and any persistence.

## Public API

The implementation is owned by `src/project/semantic_service.rs` and exported
through `semaprax::project`:

```rust
pub const SEMANTIC_WORKSPACE_SERVICE_WORK_SCHEMA: &str =
    "semaprax.semantic-workspace-service-work.v1";
pub const SEMANTIC_WORKSPACE_SERVICE_REFRESH_SCHEMA: &str =
    "semaprax.semantic-workspace-service-refresh.v1";
pub const MAX_SEMANTIC_WORKSPACE_SERVICE_RECEIPT_BYTES: usize = 65_536;

pub struct SemanticWorkspaceService { /* opaque */ }
pub struct SemanticWorkspaceGeneration { /* opaque */ }
pub struct SemanticWorkspaceSnapshot { /* immutable, cloneable */ }
pub struct SemanticWorkspaceServiceWork { /* opaque */ }
pub struct SemanticWorkspaceServiceRefresh { /* opaque */ }
```

`SemanticWorkspaceService::open` accepts an already admitted
`Arc<ProjectRevision>` and creates a checked-module semantic cache.
`open_with_semantic_cache` instead accepts an opaque
`ProjectFrontendCache` that must already be in semantic-cache mode. A cache
restored from disk must have been authenticated by the separate explicit host
adapter before it enters this API. The service itself opens no cache root and
does not decode caller-supplied HIR.

Opening reconstructs the cache against the revision's exact retained source
bytes and requires the resulting Project revision, existing workspace
revision, canonical manifest, workspace manifest, complete semantic graph, and
ordered source facts to equal the admitted input. It then derives one current
generation containing:

- the admitted `Arc<ProjectRevision>`;
- its `SemanticWorkspaceRevision`; and
- its `Arc<ProjectSemanticImage>`.

`active_generation` and `semantic_cache` expose read-only references to the
current values. `open_work` returns a deterministic account of the complete
open. No returned value contains filesystem, process, network, execution,
commit, or publication authority.

## Immutable revision-bound snapshots

`snapshot(expected_workspace_revision)` requires the exact Canonical Semantic
Workspace Revision v1 composite digest of the active generation. Malformed
selectors fail as invalid; a well-formed non-current selector fails as stale.

The returned cloneable `SemanticWorkspaceSnapshot` retains an
`Arc<SemanticWorkspaceGeneration>`. A later service refresh therefore cannot
split or rewrite an earlier snapshot. Its `symbol`, `context`, and `impact`
methods delegate to the same generation's immutable `ProjectSemanticImage`
using that image's exact digest. Existing query schemas, traversal semantics,
limits, ordering, and diagnostics remain owned by Semantic Workspace Image v1
and Workspace Analysis v1. The service does not create a second graph or query
implementation.

The additive [Universal Semantic Query v1](UNIVERSAL-SEMANTIC-QUERY-V1.md)
core also executes its five canonical revision-bound operations through
`SemanticWorkspaceSnapshot::query` or exact request bytes through
`SemanticWorkspaceService::query`. Those entry points retain this snapshot and
authority boundary. They are library calls, not a service wire route.
The one-shot workflow CLI invokes these library calls but does not retain
service state after the command.

The additive [Persistent Semantic Workspace Service Transport
v1](PERSISTENT-SEMANTIC-SERVICE-TRANSPORT-V1.md) now exposes one retained core
through a bounded single-client stdio process. That separately specified
adapter does not change this core or turn its in-memory state into durable,
shared, MCP, or LSP service state.

Each immutable generation also owns bounded retained indexes for tests that
statically reach a stable declaration and functions that statically reach a
named effect. Index derivation completes before refresh adopts the replacement
generation, so revision selection, semantic cache, image, and indexes change
together. Exact canonical query/result bytes support deterministic replay;
these are static HIR reachability facts, not runtime coverage or path
feasibility.

Snapshots are suitable for caller-coordinated concurrent read-only use. V1
does not provide a scheduler, worker pool, request cancellation, fairness,
transport ordering, or simultaneous refresh API.

## Incremental refresh and adoption

`refresh_owned_sources` accepts:

- a caller-owned `ProjectManifest`;
- a bounded ordered slice of caller-owned `ProjectFrontendSource` values; and
- the exact expected current canonical workspace revision.

The method requires exclusive `&mut` access to the service. Before changing
current state it forks the current compiler-created cache, performs complete
cached Project admission, derives the candidate Canonical Semantic Workspace
Revision and Semantic Workspace Image, computes the invalidation account, and
renders the complete bounded refresh receipt. Only after all those steps
succeed does it adopt the staged generation and staged cache together. This is
the v1 in-memory generation/cache compare-and-swap boundary.

A malformed or stale expected revision, parse/type/effect/ownership/link/profile
failure, cache disagreement, canonical-revision or image failure, or receipt
capacity failure returns without adopting either staged value. The previously
returned snapshots and the complete installed generation/cache remain valid.

Changed sources are exact source-byte differences over the union of old and
new declared paths. A manifest or source-inventory change invalidates the whole
union. Otherwise invalidation begins with changed sources and takes the
transitive union of old and new reverse `function_import` and `type_import`
edges. This is conservative module invalidation, not complete function-level
incremental verification or a claim that every semantic dependency family is
indexed.

If the newly derived canonical workspace revision equals the current one, the
existing generation `Arc` is reused. Equality of that composite digest is not
trusted alone: all retained Project facts must also match. The staged cache is
still adopted because its compiler work state may have advanced without a new
semantic generation.

## Transaction validation

`validate_transaction(transaction_bytes)` parses an exact canonical Universal
Semantic Transaction v1 envelope and validates it against the active immutable
Project revision. It returns the existing `SemanticTransactionArtifacts`.

Validation does not adopt the candidate revision, refresh the service, mutate
the cache, append transaction history, write source, commit a managed
generation, run a target, or publish anything. Universal Semantic Transaction
v1 still admits exactly one explicit monomorphic non-`main` function display
rename over comment-free canonical source. This service does not broaden that
operation algebra.

## Deterministic receipts

Open work uses schema `semaprax.semantic-workspace-service-work.v1`. Its closed
payload records authority `false`, Project and canonical workspace revisions,
image digest, actual frontend work, optional authenticated restored-work
metadata, the 65,536-byte receipt limit, and exact nonclaims. Its receipt digest
uses:

```text
semaprax.semantic-workspace-service.work.digest.v1\0
```

A successful refresh uses schema
`semaprax.semantic-workspace-service-refresh.v1`. It records old and new
Project, canonical workspace, and image identities; exact changed and
invalidated source sets; manifest and inventory changes; whether the existing
generation `Arc` was reused; actual frontend work; the invalidation basis; the
receipt limit; authority `false`; and exact nonclaims. Its receipt digest uses:

```text
semaprax.semantic-workspace-service.refresh.digest.v1\0
```

Both digests are lowercase `sha256:` values over
`domain || u64le(byte_length) || exact_canonical_bytes`. Receipt JSON is compact,
recursively key-sorted, and terminated by one LF. Sets serialize in byte-sorted
order. Each complete receipt, including the LF, is capped at 65,536 bytes.

The receipts are deterministic accountability data. They are not cache
capsules, source freshness proofs, authorization grants, timing measurements,
peak-memory measurements, semantic-equivalence proofs, or persistence handles.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-G528` | Invalid service configuration, selector, retained work, or receipt rendering. |
| `SPX-G529` | A service receipt exceeds its fixed capacity. |
| `SPX-G530` | The expected canonical revision is stale, cache priming disagrees with the admitted revision, or equal canonical identities conceal different retained Project facts. |

Parser, verifier, Project, image, incremental-cache, workspace-analysis, and
transaction diagnostics retain their existing codes and precedence when those
owners reject before the service-specific boundary.

## Authority and persistence boundary

The core receives already owned semantic inputs. It stores no path, file
descriptor, directory handle, lock, cache key, secret, socket, subprocess,
credential, approval, Git target, output directory, or publication handle. It
does not read current raw files and therefore cannot assert filesystem
freshness. An embedding host that obtains sources from a filesystem must apply
the existing Project held-input rules outside this module.

The process-resident cache is not durable service state. The separate
Authenticated Semantic Cache Store v1 may persist or authenticate an opaque
compiler-created cache under explicit host filesystem authority. This core
does not initialize, discover, load, persist, evict, garbage-collect, or choose
that store. It never makes serialized cache or graph bytes a second mutable
truth.

## Compatibility and nonclaims

This additive core does not change canonical source formatting, Project or
managed Workspace revisions, Canonical Semantic Workspace Revision v1,
Semantic Workspace Image v1, Universal Semantic Transaction v1, Universal
Semantic Query v1, semantic-cache
formats, graph schemas, query bytes, diagnostics, candidate behavior, transport
method sets, MCP discovery, LSP behavior, editor behavior, or publication
routes. Universal Semantic Workflow CLI v1 separately adds only a one-shot
adapter over the unchanged core.

It is not yet:

- a wire, persistent CLI service, MCP, LSP, editor, CI, generated-SDK, or
  autonomous-agent service;
- a filesystem watcher, automatic refresh loop, repository index, or
  multi-workspace database;
- a disk-persistent service, crash-recovery protocol, cache eviction policy,
  transaction-history ledger, or multi-process coordinator;
- full incremental semantic verification, incremental target emission, a
  performance result, or a memory-use result;
- a mutable graph store, source commit path, build/test/run route, authority
  broker, or publication service; or
- a universal semantic edit algebra beyond the single operation independently
  admitted by Universal Semantic Transaction v1.

## Focused evidence

The focused integration evidence lives in
`tests/workspace/persistent_incremental_semantic_service.rs` as a module of the
existing Workspace harness. The current cases cover deterministic cold and
explicit caller-supplied semantic-cache open work accounting;
immutable revision-bound snapshots; exact symbol, context, and impact delegation;
same-revision generation reuse; one source-exact refresh with three checked-HIR
cache hits and one resolution; cold-equivalent Project, graph, and canonical
workspace identities; stale and failed refresh rollback; a subsequent complete
warm hit; direct transaction-artifact parity; transaction staleness after
refresh; and unchanged fixture bytes across every service operation.

The authored cases do not yet independently recompute the two receipt digests,
inject a cache loaded by the persistent store, exercise manifest/inventory or
reverse-import invalidation, compare legacy Image or candidate bytes, or expose
the core through any transport. Those remain part of the larger service gate,
not evidence claimed by this badge.

The following focused command passed locally with three tests and no failures;
the completion matrix remains Partial because transport and broader semantic
coverage remain open:

```sh
CARGO_TARGET_DIR=target/persistent-incremental-semantic-service-v1 \
  cargo test --locked -p semaprax --test workspace \
  persistent_incremental_semantic_service \
  -- --test-threads=1
```
