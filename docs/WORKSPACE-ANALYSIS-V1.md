# Workspace Analysis v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Workspace Analysis v1 defines deterministic, read-only Context, Impact, and
Review artifacts over one authenticated Workspace Semantic Graph v1. The three
artifacts operate only on the six admitted cross-file edge families and retain
typed module, declaration, and capability namespaces. A module bridge node is
never selectable and is never conflated with a declaration that has the same
string spelling.

## Public API

The closed public enums are:

```rust
pub enum WorkspaceAnalysisTargetKind { Declaration, Capability }
pub enum WorkspaceAnalysisDirection { Forward, Reverse, Both }
```

Options are immutable `Copy + Debug + Eq + PartialEq` values with no public
field getters:

```rust
WorkspaceContextOptions::new(direction, depth, max_bytes, max_nodes)
WorkspaceImpactOptions::new(depth, max_bytes, max_nodes)
```

Context defaults to `Both`, depth 4, 1,048,576 bytes, and 1,024 nodes. Impact
defaults to depth 16, 1,048,576 bytes, and 1,024 nodes.

```rust
pub fn context(
    root: &Path,
    entry_module: &str,
    target_kind: WorkspaceAnalysisTargetKind,
    target: &str,
    options: WorkspaceContextOptions,
) -> Result<String, Vec<Diagnostic>>;

pub fn impact(
    root: &Path,
    entry_module: &str,
    target_kind: WorkspaceAnalysisTargetKind,
    target: &str,
    options: WorkspaceImpactOptions,
) -> Result<String, Vec<Diagnostic>>;

pub fn review(
    root: &Path,
    entry_module: &str,
    target_kind: WorkspaceAnalysisTargetKind,
    target: &str,
) -> Result<String, Vec<Diagnostic>>;
```

The API strings are compact JSON without a terminal LF. The CLIs add exactly
one LF:

```text
semaprax workspace-context <root> <entry-module> <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N]
semaprax workspace-impact <root> <entry-module> <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N]
semaprax workspace-review <root> <entry-module> <declaration|capability> <target>
```

Unknown, duplicate, missing-value, or noncanonical numeric options exit 2 and
write no stdout. Domain diagnostics exit 1.

## Selection and traversal

Targets are 1–4,096 UTF-8 bytes and contain no NUL. A declaration target must
be explicit or automatic, must be in the authenticated entry provider closure,
and cannot be compiler-owned. A capability target must occur as an
`effect_requirement` or `capability_authority` target. Missing or out-of-closure
targets are `SPX-G177`; compiler-owned declaration targets use the same code
with the dedicated unsupported message.

The exact edge-family order is:

```text
function_import,type_import,call,type_reference,effect_requirement,
capability_authority
```

Typed endpoints are:

| Family | Source | Target |
| --- | --- | --- |
| `function_import` | module | declaration |
| `type_import` | module | declaration |
| `call` | declaration | declaration |
| `type_reference` | declaration | declaration |
| `effect_requirement` | declaration | capability |
| `capability_authority` | module | capability |

The implementation independently rebuilds and exact-compares these endpoints,
paths, namespaces, and adjacency indexes before BFS. Context uses minimum-depth
forward, reverse, or bidirectional BFS. `reached_by` is an array in canonical
`root,forward,reverse` enum order. Only the root carries `root`; tied forward
and reverse paths may emit both direction values.
Context emits each authenticated compatible edge once by Workspace edge index
when both endpoints are emitted.

Impact is reverse-only potential structural dependency closure. It emits only
minimum-path dependency edges. Affected roles are `target`, `consumer`,
`module_consumer`, and `dependency`; reasons are unique contributing edge
families in the frozen family order. These are not behavioral-change claims.

Nodes are selected and emitted in `(minimum_depth,node_key)` order. Node-key
order is module, then declaration ordered by declaration kind and ID, then
capability. When bounded, the frontier contains only the first omitted known
depth and is globally node-key sorted. `omitted_known_nodes` and
`deferred_known_nodes` count unique typed nodes, not edges.

## Schemas and common wire

The schemas and digest domains are:

| Artifact | Schema | Digest domain |
| --- | --- | --- |
| Context | `semaprax.workspace-semantic-context.v1` | `semaprax.workspace-semantic-context.artifact-digest.v1\0` |
| Impact | `semaprax.workspace-semantic-impact.v1` | `semaprax.workspace-semantic-impact.artifact-digest.v1\0` |
| Review | `semaprax.workspace-semantic-review.v1` | `semaprax.workspace-semantic-review.artifact-digest.v1\0` |

Every artifact is compact canonical UTF-8 JSON without a terminal LF. Its
`artifact_digest` is lowercase `sha256:` plus 64 hex digits over
`domain || u64_le(payload.len) || payload`, where `payload` is the exact top
object with only `artifact_digest` omitted and final fixed-point output usage
already present.

Context top-level order is:

```text
schema,workspace_manifest_schema,workspace_revision,workspace_graph_digest,
artifact_digest,entry,target,query,limits,budget,truncation,frontier,nodes,
edges,nonclaims
```

Impact replaces the final node/edge fields with `affected,dependency_edges`.
Review top-level order is:

```text
schema,workspace_manifest_schema,workspace_revision,workspace_graph_digest,
artifact_digest,entry,target,context,impact,sections,limits,budget,nonclaims
```

Nested key order is:

```text
entry:      module,path
target:     kind,id,declaration_kind,identity_origin,path,module
query:      direction,depth,max_bytes,max_nodes,edge_kinds
truncation: truncated,reasons,omitted_known_nodes,deferred_known_nodes
frontier:   kind,id,minimum_depth,reached_by
node:       kind,declaration_kind,identity_origin,id,path,module,
            minimum_depth,reached_by
affected:   kind,declaration_kind,identity_origin,id,path,module,
            minimum_depth,impact_role,reasons
edge:       caller_path,caller,target_path,target,kind,site,expression,
            ast_path,alias,ordinal
```

Truncation reasons are `max_depth`, `max_nodes`, and `max_bytes`. Context and
Impact may return complete bounded prefixes with explicit truncation facts.
Review uses fixed maximum children and rejects any child truncation, omitted or
deferred node, or frontier as `SPX-G180`:
`Workspace Semantic Review requires complete Context and Impact evidence`.

Review embeds the exact complete Context and Impact JSON objects from the same
typed analysis; it does not parse child JSON as authority. `sections` is an
object ordered `behavior`, `api_identity`, `security_authority`,
`memory_ownership`, `target_artifact`, `migration`, `unsafe`. Each value is an
array containing exactly one finding with keys
`code,disposition,statement,evidence`. Evidence references use keys
`artifact,relation,index`, point only to `context/edges`, `impact/affected`, or
`impact/dependency_edges`, and sort Context before Impact, affected before
dependency edges, then by index.

The exact findings are:

| Section/code | Evidence present | Evidence empty |
| --- | --- | --- |
| behavior / `workspace_behavior_dependencies` | `review_required`: `Authenticated workspace call dependencies require review.` | `informational`: `No authenticated workspace call dependencies are present in the selected closure.` |
| api_identity / `workspace_api_identity_dependencies` | `review_required`: `Authenticated workspace API identity dependencies require review.` | `informational`: `No authenticated workspace API identity dependencies are present in the selected closure.` |
| security_authority / `workspace_security_authority_dependencies` | `review_required`: `Authenticated workspace security-authority dependencies require review.` | `informational`: `No authenticated workspace security-authority dependencies are present in the selected closure.` |
| migration / `workspace_migration_dependencies` | `review_required`: `Authenticated workspace consumer dependencies require migration review.` | `informational`: `No authenticated workspace consumer dependencies are present in the selected impact closure.` |

The three fixed unsupported findings are
`workspace_memory_ownership_not_analyzed` / `not_analyzed` /
`Workspace memory-ownership effects are not analyzed by this version.`,
`workspace_target_artifact_not_analyzed` / `not_analyzed` /
`Workspace target-artifact effects are not analyzed by this version.`, and
`workspace_unsafe_not_analyzed` / `not_analyzed` /
`Workspace unsafe-code effects are not analyzed by this version.`. Each has
empty evidence.

Behavior evidence selects Context edges and Impact dependency edges whose kind
is `call`. API evidence selects those whose kind is `function_import`,
`type_import`, or `type_reference`. Security evidence selects
`effect_requirement` or `capability_authority`. Migration selects every
non-root Impact affected entry. The only allowed artifact/relation pairs are
`context/edges`, `impact/affected`, and `impact/dependency_edges`; references
are unique and sort by artifact, relation, then numeric index. Review is
dependency review, not approval, policy, a security audit, or general semantic
review.

## Limits and usage

All routes embed the exact Workspace Semantic Graph `limits` and `budget`
objects under `limits.workspace` and `budget.workspace`.

Context and Impact analysis limits, in exact order, are:

```text
max_target_bytes=4096
max_traversal_depth=1024
max_traversal_nodes=8208
max_analysis_builder_bytes=16777216
max_output_bytes=16777216
```

Public `max_bytes` must be 4,096–16,777,216, `max_nodes` must be 1–8,208, and
depth must be 0–1,024. Review adds
`max_context_bytes=16777216,max_impact_bytes=16777216` before
`max_output_bytes=33554432` and uses one cumulative 16 MiB analysis-builder
budget across the shared graph build and both child analyses. Embedded child
artifacts remain byte-identical to their standalone canonical forms.

Context/Impact analysis budget order is:

```text
used_target_bytes,used_traversal_depth,used_traversal_nodes,
used_analysis_builder_bytes,used_output_bytes
```

Review inserts `used_context_bytes,used_impact_bytes` before output. Rendering
uses hard output sinks and reserve-first envelope accounting. Option grammar is
`SPX-G176`; target absence/compiler rejection is `SPX-G177`; limits are
artifact-specific `SPX-G178`; typed replay/digest disagreement is
artifact-specific `SPX-G179`; incomplete Review evidence is `SPX-G180`.

For Review, `used_traversal_depth` is the maximum emitted child depth,
`used_traversal_nodes` is the maximum child node count,
`used_analysis_builder_bytes` is the cumulative shared analysis debit, and
`used_context_bytes` and `used_impact_bytes` are the exact canonical child
lengths.

## Ordered nonclaims

Context begins with:

1. `no_patch_candidate_change_or_semantic_delta`
2. `no_impact_or_review_claim`
3. `only_six_workspace_graph_edge_families`
4. `no_embedding_search_ranking_or_answer_quality`

Impact begins with:

1. `potential_structural_dependency_impact_not_patch_candidate_or_behavioral_delta`
2. `no_source_consumer_span_or_authored_operation_provenance`
3. `only_reverse_closure_over_six_workspace_graph_edge_families`
4. `no_repair_review_ranking_or_commit_authority`

Review begins with:

1. `dependency_review_not_patch_change_or_general_semantic_review`
2. `not_human_approval_policy_or_security_audit`
3. `context_and_impact_are_current_state_read_only_projections`
4. `memory_ownership_target_artifact_and_unsafe_sections_are_not_analyzed`

Each then appends the same exact ordered eleven strings:

5. `no_generic_cross_file_composition`
6. `automatic_target_identity_is_revision_scoped_not_persistent_patch_address`
7. `no_cross_file_resource_interface_ownership_borrowing_or_lifetime_composition`
8. `no_reexport_wildcard_implicit_or_ambiguous_imports`
9. `no_target_codegen_artifact_project_test_or_execution`
10. `no_exclusive_lock_stage_publish_apply_or_commit_authority`
11. `not_proof_signature_provenance_approval_or_reusable_authorization`
12. `no_raw_working_tree_git_editor_or_unmanaged_file_analysis`
13. `no_incremental_cache_persistence_or_repository_index`
14. `no_recovery_rollback_cleanup_gc_or_durability_guarantee`
15. `no_external_consumer_compatibility`

## Authority and evidence status

Each operation validates scalar grammar before locking, holds one shared
semantic-workspace authority through the retained graph build, traversal,
canonical render, final held-object/inventory check, and checked unlock, and
returns only owned JSON. Raw analysis and authority cannot escape. There is no
write, stage, publish, apply, backend, runtime, parser, or verifier authority.

Local traversal, wire, digest, cap, mutation, API/CLI, and preservation gates
are present. The frozen whole-document raw SHA-256 KATs are:

| Artifact | SHA-256 wire value |
| --- | --- |
| Context forward | `sha256:35f39e8220a9fcd2e952e361ed70c8e47c290eda55422d7b772348c97d97668a` |
| Context reverse | `sha256:c93bfff8347892750ad1f0a3e87ed5dff32ede26f3793acfb904d703996becc2` |
| Context both | `sha256:8d9a68b005d8f6e147c954e32d8437cc2e7a86c771cded64f2a2e2db58d3d1f7` |
| Context capability reverse | `sha256:991a5a4f3e4339bd801918d50f15ffd40acb7d683162bd69d291b47b5808702c` |
| Impact declaration | `sha256:70259a820e24e9110874b645ec96d3c1350dcd0441d58c3562298937bc871af5` |
| Impact capability | `sha256:20c4f1d72f10d75852580da4ad5a1e43e9c69677e26d88c9c8212bf57531727e` |
| Review | `sha256:ff8dd7f60be9c8fc0ff06a9216c864e502ec5cca6d577ee460f338b6e6a12cf9` |

Exact-head hosted evidence remains pending; this document makes no status
promotion.
