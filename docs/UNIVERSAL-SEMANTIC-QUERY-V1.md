# Universal Semantic Query v1

Status: additive transport-neutral core; focused local integration evidence passed.

Audience: compiler contributors, service hosts, agent-tool authors, and
reviewers of revision-bound semantic reads.

Universal Semantic Query v1 is the first closed query envelope over one
immutable [Persistent Incremental Semantic Workspace Service
v1](PERSISTENT-INCREMENTAL-SEMANTIC-SERVICE-V1.md) snapshot. It gives CLI, MCP,
LSP, and other adapters one canonical request/result/replay boundary
without making adapter behavior part of the core. The later read-only
[Universal Semantic Workflow CLI v1](UNIVERSAL-SEMANTIC-WORKFLOW-CLI-V1.md)
constructs these same typed operations and returns their exact results; it adds
no query schema or alternate execution path.

[Installed Agent Guidance v1](INSTALLED-AGENT-GUIDANCE-V1.md) separately exposes
installed operation metadata through `query --capabilities`.
That authority-free document is static installed-support metadata, not a
revision-bound query result or live service discovery, and it cannot enable an
operation.

The additive v1 operation set contains seven operations: `declarations`,
`symbol`, `context`, `impact`, `available_operations`,
`ownership_at_expression`, and `declaration_consumers`. The implementation
reuses the existing Project declaration query, Semantic Workspace Image symbol
lookup, Workspace Analysis context and impact, and Universal Semantic
Transaction eligibility classifier. It does not create a parallel semantic
index or a second operation-eligibility truth.

## Public API

The implementation is owned by `src/project/semantic_query.rs` and exported
through `semaprax::project`:

```rust
pub const SEMANTIC_QUERY_SCHEMA: &str = "semaprax.semantic-query.v1";
pub const SEMANTIC_QUERY_RESULT_SCHEMA: &str =
    "semaprax.semantic-query-result.v1";
pub const SEMANTIC_QUERY_DECLARATIONS_SCHEMA: &str =
    "semaprax.semantic-query-declarations.v1";
pub const SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA: &str =
    "semaprax.semantic-query-available-operations.v1";
pub const SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA: &str =
    "semaprax.semantic-query-ownership-at-expression.v1";
pub const SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA: &str =
    "semaprax.semantic-query-declaration-consumers.v1";
pub const MAX_SEMANTIC_QUERY_BYTES: usize = 65_536;
pub const MAX_SEMANTIC_QUERY_RESULT_BYTES: usize = 32 * 1024 * 1024;

pub struct SemanticQuery { /* opaque */ }
pub struct SemanticQueryResult { /* opaque */ }
```

`SemanticQuery` provides typed constructors named after all seven operations,
`from_json`, `to_json`, `query_digest`, `expected_workspace_revision`,
`execute`, and `replay`. `SemanticQueryResult` exposes `to_json`,
`result_digest`, `query_digest`, `payload`, `payload_digest`, and
`workspace_revision`.

`SemanticWorkspaceSnapshot::query` accepts a typed `SemanticQuery`.
`SemanticWorkspaceService::query` accepts exact canonical query bytes and
executes them against one snapshot of the active generation. Neither method
refreshes the service or mutates its semantic cache.

## Canonical request and result

The request schema is `semaprax.semantic-query.v1`. Its exact top-level keys
are `expected_workspace_revision,operation,schema`. The expected revision is
the composite Canonical Semantic Workspace Revision v1 digest. The operation
object is closed and operation-specific.

The result schema is `semaprax.semantic-query-result.v1`. It binds:

- the query digest and exact composite workspace revision;
- the admitted Project revision and Semantic Workspace Image digest;
- all four canonical semantic-workspace component digests;
- the operation name, operation-specific payload, and payload digest;
- the 65,536-byte request and 32-MiB result limits;
- `authority: false`; and
- fixed nonclaims identifying the payload as a derived read-only projection,
  not behavioral equivalence or complete repository analysis.

Request and result JSON is compact, recursively key-sorted, and terminated by
one LF. The request and complete result digests use, respectively:

```text
semaprax.semantic-query.intent.digest.v1\0
semaprax.semantic-query.result.digest.v1\0
```

Each digest is lowercase `sha256:` over
`domain || u64le(byte_length) || exact_canonical_bytes`, including the terminal
LF. Payloads use the same construction with one operation-specific domain:

```text
semaprax.semantic-query.declarations.payload.digest.v1\0
semaprax.semantic-query.symbol.payload.digest.v1\0
semaprax.semantic-query.context.payload.digest.v1\0
semaprax.semantic-query.impact.payload.digest.v1\0
semaprax.semantic-query.available-operations.payload.digest.v1\0
semaprax.semantic-query.ownership-at-expression.payload.digest.v1\0
semaprax.semantic-query.declaration-consumers.payload.digest.v1\0
```

`SemanticQuery::replay` admits the exact canonical query and closed result
wires, verifies the caller's result digest, freshly executes against the
selected immutable snapshot, and exact-compares the complete result bytes and
digest. Malformed, reminted, cross-revision, or stale material fails closed.

## Operations

### `declarations`

This operation carries the existing `QueryFilters` fields `kinds`, `name`,
`id_prefix`, `effect`, `calls`, and `called_by`, plus `offset` and `limit`.
Kinds are normalized into the existing canonical declaration-kind order and
deduplicated. Text fields are at most 4,096 bytes and contain no NUL.

The offset range is `0..=16_384`; the limit range is `1..=128`. The result uses
schema `semaprax.semantic-query-declarations.v1`, retains the existing Project
query graph/Project revisions and canonical match ordering, and adds
`total_matches` plus `next_offset`. `next_offset` is the next integer or null;
v1 has no mutable cursor registry and does not promise snapshot-independent
continuation.

### `symbol`

This operation carries one nonempty stable identity of at most 4,096 bytes. It
delegates to the snapshot's existing Semantic Workspace Image symbol query.
`SemanticQueryResult::payload` retains the exact inner bytes and the payload
digest binds them; the outer result embeds the parsed value in its own
recursively key-sorted canonical JSON.

### `context` and `impact`

These operations carry a `target_kind` of `declaration` or `capability`, a
nonempty target of at most 4,096 bytes, and the existing bounded Workspace
Analysis options. Context additionally carries `direction` as `forward`,
`reverse`, or `both`. Existing Workspace Analysis validation owns the depth,
node, and byte limits; the query layer does not weaken or reinterpret them.
Both operations delegate to the snapshot's exact image revision and retain the
existing canonical payload.

### `available_operations`

This operation requires an actual retained declaration stable identity and
returns schema `semaprax.semantic-query-available-operations.v1`. V1 contains
four ordered catalogue entries: `rename_display_name`, `replace_block`,
`add_contract`, and `add_declaration`. Every entry carries:

- `available`, derived from the same classifier used by transaction
  validation;
- the comment-free canonical workspace, explicit identity, monomorphic, and
  non-`main` constraint outcomes;
- the exact `semaprax.semantic-transaction.v1` schema; and
- an operation-specific nonclaim that availability does not prove an arbitrary
  new name, body, or predicate will validate.

The rename entry carries the currently expected old display name. The block
entry carries the exact old canonical body block. The contract entry carries
the exact ordered `requires`/`ensures` predicate-source inventory, the two
admitted phases, and the contract-inventory capacity outcome. Missing or
ineligible function targets retain null old-state projections and report their
individual constraint outcomes.

The shared classifier is read-only. `available: true` means the target satisfies
the structural prerequisites for that Universal Semantic Transaction v1
operation at that revision. It is not a transaction, approval, reservation,
validation result, or authority grant. Actual transaction validation repeats
the same checks against its bound base and additionally checks the proposed
new value.

### `ownership_at_expression`

This operation selects one stable function or function-template identity and
one revision-scoped expression identity within it. The expression must occur
exactly once in every retained copy of that declaration and those copies must
yield identical facts. The result schema is
`semaprax.semantic-query-ownership-at-expression.v1`; it reports the checked
expression kind, type identity and ownership mode, an exact place for place
expressions, and every authenticated loan whose site is that expression. Loan
origin, parent, start, endpoints, edge indexes and cause preserve the validated
LoanPlan vector order without sorting, repairing or inferring facts.

The ownership mode is a boundary classification, not flow-sensitive value
availability. Loan facts are static proof, not runtime liveness or permission;
mutable and escaping borrows remain outside this query.

### `declaration_consumers`

This operation selects one retained declaration stable identity plus an offset
in `0..=16_384` and limit in `1..=128`. The result schema is
`semaprax.semantic-query-declaration-consumers.v1`. It reports direct uses in
retained checked HIR, ordered by consumer stable-ID bytes, with module names and
use kinds in canonical byte order. Supported facts include calls, nominal
types, record/variant construction, field initialization/projection/update/
assignment/matching, compiler-authenticated borrow/range operations, imports,
and `try` declarations.

`visibility: exported` means only that the consumer identity is directly
selected by the manifest's `web_exports`; it is not a language-level public
visibility claim. Test-module consumers are `test`, and all others are
`local`. The query makes no transitive, dynamic-dispatch, runtime path,
cross-project, or unloaded-source claim. A global 65,536-expression walk bound,
page bound, existing request bound, and existing result-byte bound fail closed.

## Diagnostics and precedence

| Code | Meaning |
| --- | --- |
| `SPX-G531` | Invalid, unsupported, noncanonical, or malformed query/result structure. |
| `SPX-G532` | Query or result capacity exceeded. |
| `SPX-G533` | Stale workspace/result identity or exact replay mismatch. |

Underlying Project query, Semantic Workspace Image, Workspace Analysis, and
transaction-classifier diagnostics retain their existing meanings and
precedence when those owners reject the delegated operation.

## Authority, compatibility, and nonclaims

Execution retains the snapshot's exact `ProgramRoot` in the in-memory result
and exposes it through an additive Rust accessor. The legacy query envelope and
result still serialize the same workspace/component fields and bytes; the
workspace selector is resolved through that root's canonical-workspace binding.

The core reads one retained immutable snapshot. It owns no filesystem, process,
network, execution, cache persistence, source mutation, commit, approval,
deployment, signing, or publication authority. Query results are evidence, not
authority.

This badge is additive. It does not change canonical `.spx` formatting,
Project or managed Workspace revisions, Canonical Semantic Workspace Revision
v1, Semantic Workspace Image v1, Universal Semantic Transaction v1, or their
bytes and digest algorithms. In particular it does not add methods, fields, or
schemas to the frozen Project Agent Transport v5 protocol.

The separate Universal Semantic Workflow CLI v1 badge adds five Project-only
one-shot `semaprax query` subcommands while preserving this revision binding,
canonical envelope, limits, diagnostic truth, exact result bytes, and
authority-free boundary. Neither badge adds a daemon wire route, Project Agent
Transport vNext, MCP, LSP, editor integration, generated clients, streaming,
subscriptions, a durable cursor, a repository-wide multi-workspace index, or a
general semantic query algebra.

## Focused evidence

The integration evidence lives in
`tests/workspace/universal_semantic_query.rs` as a module of the existing
Workspace harness. Existing passed evidence covers the original five typed
constructors, exact JSON parsing
and determinism; bounded declaration paging; direct symbol, context, and impact
parity; truthful rename, block-replacement, contract-addition, and declaration-addition availability
paired with known-good transactions;
unavailable main, generic, automatic-identity, nonfunction, and comment-bearing
subjects; stale active-service rejection with an old immutable snapshot still
usable; malformed, noncanonical, reminted, and oversized replay rejection; and
absence of filesystem writes or service mutation. The checked-fact case covers
ownership replay, direct consumer ordering and paging, and fail-closed unknown
expressions.

The following focused command passed locally with seven tests and no failures:

```sh
CARGO_TARGET_DIR=target/universal-semantic-query-v1 \
  cargo test --locked -p semaprax --test workspace \
  universal_semantic_query --no-fail-fast
```
