# Universal Semantic Query v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, device/simulator, or separately provisioned evidence
below retains its narrower scope; public promotion, registry publication and
broader product completion remain separately gated.

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

The additive v1 operation set contains eight operations: `declarations`,
`symbol`, `context`, `impact`, `available_operations`,
`ownership_at_expression`, `declaration_consumers`, and `next_constructs`. The
implementation reuses the existing Project declaration query, Semantic
Workspace Image symbol lookup, Workspace Analysis context and impact, and
Universal Semantic Transaction eligibility classifier. It does not create a
parallel semantic index or a second operation-eligibility truth. The first
seven operations carry **HOSTED GREEN** evidence under the v0.4.0 release
baseline named above; `next_constructs` is a later additive operation with
local evidence only (see its own section below), and adding it does not
reclaim hosted status for itself.

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
pub const SEMANTIC_QUERY_NEXT_CONSTRUCTS_SCHEMA: &str =
    "semaprax.semantic-query-next-constructs.v1";
pub const MAX_SEMANTIC_QUERY_BYTES: usize = 65_536;
pub const MAX_SEMANTIC_QUERY_RESULT_BYTES: usize = 32 * 1024 * 1024;

pub struct SemanticQuery { /* opaque */ }
pub struct SemanticQueryResult { /* opaque */ }
```

`SemanticQuery` provides typed constructors named after all seven operations,
`from_json`, `to_json`, `query_digest`, `expected_workspace_revision`,
`execute`, `replay`, and the additive in-memory `execute_exact` and
`replay_exact` selectors. `SemanticQueryResult` exposes `to_json`,
`result_digest`, `query_digest`, `payload`, `payload_digest`, and
`workspace_revision`; an exact result additionally exposes its retained
`ProgramRootV2` through a Rust accessor without serializing it.

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
semaprax.semantic-query.next-constructs.payload.digest.v1\0
```

`SemanticQuery::replay` admits the exact canonical query and closed result
wires, verifies the caller's result digest, freshly executes against the
selected immutable snapshot, and exact-compares the complete result bytes and
digest. Malformed, reminted, cross-revision, or stale material fails closed.

`SemanticQuery::replay_exact` first requires an exact-context snapshot and both
its enriched workspace revision and ProgramRoot-v2 digest, then performs that
same fresh v1 replay. The returned typed result retains the selected
`ProgramRootV2`; query and result schemas, bytes, and digests remain exactly
the frozen v1 values. Neither a valid v1 result nor one selector alone can be
used to infer or recover the v2 association.

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

### `next_constructs`

This operation selects one retained function or function-template stable
identity and one revision-scoped expression identity within it, with the same
uniqueness and cross-copy consistency requirement as `ownership_at_expression`.
The result schema is `semaprax.semantic-query-next-constructs.v1`. It reports
the expression's own checked expected type identity and ownership mode
(`value`, `own`, `borrow`, or `shared`), then a closed, bounded, checked
construct vocabulary of what may soundly stand in for it: `literal`,
`parameter_reference`, and `call`. Every `admitted` entry is derived only from
facts the compiler already verified for the selected revision; nothing is
inferred. `admitted` and `excluded` are each sorted by construct kind and then
by name/stable-ID bytes and independently capped at 128 entries with a
`totals` object recording the true count and whether that list was truncated,
so truncation is deterministic rather than order-dependent.

This is intentionally the narrowest slice of the "next construct" idea this
codebase can currently back with a checked guarantee, and it says exactly
where that guarantee stops:

- A `literal` is admitted only when the expected type is one of Explicit
  Mutation v1's own closed Copy-scalar kinds (`i64`, `i32`, `u8`, `usize`,
  `char`, `f32`, `f64`, `bool`); this operation invents no value, only the
  admissibility of the construct kind.
- A `parameter_reference` is admitted only for one of the enclosing
  declaration's own parameters, only when its declared ownership mode exactly
  equals the mode already required at the target position, and never when
  that parameter is declared `own` — an `own` parameter of matching type is
  reported in `excluded` with reason `flow_sensitive_availability_not_computed`
  instead of a guess, because proving it has not already been moved away
  needs flow-sensitive tracking across branches and loops that this v1 does
  not perform. Local `let` bindings, match bindings, and closure captures are
  not covered by v1.
- A `call` is offered only at a `value` or `own` position (never at a
  `borrow`/`shared` position, matching the documented rule that a borrowed
  argument must name an existing binding, not a call result) and only when
  the callee's declared `uses { .. }` effect set is already a subset of the
  target declaration's own declared effects — the same containment
  `SPX-E102` itself enforces on a direct call. A callee whose effects are not
  already covered is reported in `excluded` with reason
  `effect_not_available` and the exact missing effect names, rather than
  silently omitted.
- No entry, admitted or excluded, proves that a completed replacement program
  will verify: argument shapes, contracts, and nested effects of a `call`
  candidate are not checked here, and only full validation after generation
  can make that claim.

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

## Source Agent visibility

`AgentDefinitionsQuery` is an additive typed, in-memory query over one exact
workspace snapshot. Its result retains the selected ProgramRoot v1 and the
existing canonical `AgentDefinitions` node, including the exact compiler-made
AgentDefinition, AgentGraph, and Runtime Profile bytes for source-owned Agents.
For source-owned Agents it also exposes the retained typed Proposal and
Observation contract-fact bundle; the canonical node contains the exact replayed
schema bytes and digests.
It intentionally has no JSON parser or renderer and does not extend the closed
Universal Semantic Query v1 wire operation grammar.

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

`next_constructs` has its own module, `tests/workspace/next_construct_query.rs`,
covering: literal admission for a scalar position; parameter-reference
admission at a matching mode; an `own` parameter of matching type excluded
with `flow_sensitive_availability_not_computed`; a call candidate excluded
with `effect_not_available` and its exact missing effect, alongside a second
call candidate whose effects are covered and is admitted; a `borrow`-mode
position offering no `call` candidate; byte-identical repeat execution against
one unchanged revision (determinism); and stale-revision, unknown-expression,
and unknown-declaration rejection. See its own module doc comment for the
exact focused command and current pass count.

## Additive exact selection

`execute_exact` requires an exact-context snapshot plus matching enriched
workspace and ProgramRoot-v2 selectors. The in-memory result exposes that v2
root; its v1 JSON, payload, and digest remain byte-identical to ordinary
execution. `replay_exact`, including the service-owned adapter, applies the
same dual selection before freshly replaying and exact-comparing the unchanged
v1 query/result pair. No v1 replay request may infer or select a v2 context by
workspace revision alone; malformed, stale, reminted, or cross-paired
selectors fail closed.

`execute_exact_v2` and `replay_exact_v2` are additive typed entry points over
[Exact Program Context v2](EXACT-PROGRAM-CONTEXT-V2.md). They require the
enriched workspace and ProgramRoot-v3 selectors before query parsing or
execution, retain the exact ProgramRoot v2 and v3 only on the in-memory result,
and serialize the same query/result v1 bytes and digests. They do not add an
operation or a ProgramRoot field to either frozen wire.
