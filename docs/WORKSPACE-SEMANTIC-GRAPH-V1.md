# Workspace Semantic Graph v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Workspace Semantic Graph v1 is a bounded, canonical, read-only projection of
one authenticated Semantic Workspace v1 generation. It validates the complete
managed set before selecting an entry closure. The projection contains the
entry module plus transitive provider modules reached only through explicit
direct `use function` and `use type` edges. It excludes reverse consumers,
disconnected modules, implicit imports, reexports, stubs, and synthetic main
declarations.

## Public API and command

```rust
pub fn snapshot(
    root: &std::path::Path,
    entry_module: &str,
) -> Result<WorkspaceSemanticGraph, Vec<Diagnostic>>
```

The public immutable types are exactly:

```text
WorkspaceSemanticGraph
WorkspaceSemanticGraphEntry
WorkspaceSemanticGraphModule
WorkspaceSemanticGraphDeclaration
WorkspaceSemanticGraphEdge
WorkspaceSemanticGraphLimits
WorkspaceSemanticGraphBudget
```

No fields or constructors are public. The graph exposes borrowed string
getters for schema, manifest schema, revision, digest, entry, modules,
declarations, edges, nonclaims, and canonical JSON. `limits()` and `budget()`
return their public `Clone + Copy + Debug + Eq + PartialEq` value types.
Modules expose path, module, source schema/revision/digest, dependency depth,
and permits. Declarations expose ID, wire kind, wire identity origin, and
nullable owner/path/module. Edges expose their ten wire fields. `to_json()`
borrows the exact compact JSON body.

```text
semaprax workspace-graph <root> <entry-module>
```

The API JSON has no terminal LF. CLI success writes the API JSON followed by
exactly one LF. Wrong arity exits 2 with
`workspace-graph requires exactly <root> <entry-module>` and no stdout.

## Entry and authenticated closure

`entry_module` is exact ASCII
`[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*`, is not normalized, is at
most 16,777,216 bytes, and must equal one authenticated module. It need not
define `main`. Malformed entry text is `SPX-G170`; an absent canonical module
is `SPX-G172`. The length check precedes grammar and never echoes an oversized
entry.

All 2–16 managed modules are parsed, resolved, cross-file checked, and bounded
before projection. Modules sort by UTF-8 path bytes; permits retain source
order. Declarations sort by stable ID and contain closure-owned source
declarations plus the compiler prelude exactly once. Compiler-owned prelude
declarations have null path/module. Synthetic declarations and stubs are never
emitted. Edges retain only closure-owned emitted facts and use the exact
Workspace edge tuple order.

## Canonical wire

The schema is `semaprax.workspace-semantic-graph.v1`; the manifest schema is
`semaprax.workspace-semantic-manifest.v1`. JSON is compact canonical UTF-8 with
no BOM, whitespace, CRLF, or terminal LF. Top-level key order is:

```text
schema,workspace_manifest_schema,workspace_revision,graph_digest,entry,
modules,declarations,edges,limits,budget,nonclaims
```

Nested key order is:

```text
entry:       module,path
module:      path,module,source_graph_schema,source_revision,source_digest,
             dependency_depth,permits
declaration: id,kind,identity_origin,owner,path,module
edge:        caller_path,caller,target_path,target,kind,site,expression,
             ast_path,alias,ordinal
```

Declaration kinds are `resource`, `resource_drop`, `record`, `field`,
`variant`, `variant_case`, `case_field`, `interface`, `import`, and `function`.
Identity origins are `explicit`, `automatic`, and `compiler_owned`. Edge kinds
are `call`, `capability_authority`, `effect_requirement`, `function_import`,
`type_import`, and `type_reference`. Sites are `module`, `type`, `requires`,
`body`, and `ensures`. `alias` is an empty string when absent, never null;
nullable declaration owner/path/module values are explicit JSON null.

## Digest

`graph_digest` is lowercase `sha256:` plus 64 hex digits. Its domain is the
exact bytes `semaprax.workspace-semantic-graph.artifact-digest.v1\0`. The hash
input is:

```text
domain || u64_le(payload_byte_length) || payload
```

`payload` is the exact compact top object with only `graph_digest` omitted and
with final fixed-point `used_output_bytes` already present. A fixed-width digest
placeholder establishes the final size; the implementation hashes once,
inserts the digest, rerenders, and exact-compares length and typed binding.

## Limits and budget

The `limits` object has this exact order and values:

| Key | Value |
| --- | ---: |
| `max_managed_files` | 16 |
| `max_reachable_modules` | 16 |
| `max_entry_module_bytes` | 16,777,216 |
| `max_total_source_bytes` | 16,777,216 |
| `max_declarations` | 4,096 |
| `max_callables` | 1,024 |
| `max_call_sites` | 65,536 |
| `max_uses` | 4,096 |
| `max_resolved_cross_file_edges` | 65,536 |
| `max_dependency_depth` | 16 |
| `max_builder_bytes` | 16,777,216 |
| `max_manifest_bytes` | 1,048,576 |
| `max_output_bytes` | 16,777,216 |
| `max_retained_generations` | 32 |
| `max_staging_attempts` | 32 |
| `max_unexpected_inventory_entries` | 0 |

The `budget` object uses the corresponding `used_` names in the same order.
Full authenticated-work counters are `used_managed_files`,
`used_total_source_bytes`, `used_declarations`, `used_callables`,
`used_call_sites`, `used_uses`, `used_resolved_cross_file_edges`,
`used_dependency_depth`, `used_builder_bytes`, and `used_manifest_bytes`.
`used_entry_module_bytes` is the submitted entry scalar;
`used_reachable_modules` is the emitted provider closure;
`used_output_bytes` is the complete no-LF artifact; and
`used_retained_generations`, `used_staging_attempts`, and
`used_unexpected_inventory_entries` are authenticated storage state.
`used_builder_bytes` is the maximum sequential builder-phase debit:
`max(canonical_phase_bytes, core_phase_consumed_bytes)`.

The core phase charges a resolver pre-bound before it links anything, so a
program that cannot fit is refused with `SPX-G171` before mutation. The
pre-bound is computed from the authored program alone and is deliberately an
upper estimate, composed of three terms: structural source bytes (the AST
node footprints) times 24, which covers the sixteen footprints the
compile-time bundle assertions establish plus eight for map and tree
bookkeeping; string-content bytes times 64, the at-most 48 retained copies of
a source string plus bookkeeping; and, for every declaration identity slot a
resolved expression of that shape can hold, the longest identity in scope
times 64. A scalar expression holds three slots, a call or construction four,
a variant construction five, and `Try` eight. Before the split, every source
byte and every expression were charged at the `Try` and string rates, and a
4.9 KiB module of twenty scalar functions exhausted the budget.

An imported declaration is charged for what the synthetic projection retains,
not for the provider's source. A function import becomes a stub: the rewritten
signature, no preconditions or postconditions, and the return type's default
expression as the body. Its pre-bound is therefore the signature at the
resolved-structure rates plus that default expression, and the provider's
contract and body are charged once, in the module that declares them. Building
the stub clones one provider function transiently before replacing its body, so
the largest single imported contract is charged once per module at the raw AST
rate, with no structural expansion: no node of it becomes HIR. A type import is
retained in full and keeps its full charge. Before this split, every importing
module re-charged each imported body as resolved structure, so a conformance
module that imports every function its library exports cost a second complete
copy of that library — measured at 142,043 of the 226,999 raw pre-bound bytes
of `std.data.json.tests`, expanded by 24, which is why two packages of about
4.5 KiB could not be linked together.

Output is written through a hard sink before allocation. Exactly 16,777,216
bytes succeeds; one more reports `SPX-G171` for `output_bytes` with no partial
JSON. Typed render/digest disagreement is `SPX-G173`.

## Ordered nonclaims

The exact `nonclaims` array is:

1. `no_exclusive_lock_stage_publish_apply_or_commit_authority`
2. `not_patch_evidence_signature_provenance_approval_or_reusable_authorization`
3. `not_general_formal_proof_or_behavioral_equivalence_certificate`
4. `no_target_codegen_artifact_project_test_or_external_execution`
5. `no_cross_file_impact_review_context_repair_or_patch_generation`
6. `no_cross_file_agent_context_embedding_or_search`
7. `no_generic_cross_file_composition`
8. `no_cross_file_resource_interface_ownership_borrowing_or_lifetime_composition`
9. `no_reexport_wildcard_implicit_or_ambiguous_imports`
10. `no_dynamic_linking_package_registry_network_or_dependency_fetch`
11. `no_raw_working_tree_git_editor_or_unmanaged_file_analysis`
12. `no_create_delete_move_or_flat_materialization`
13. `no_incremental_cache_persistence_or_repository_index`
14. `no_automatic_recovery_rollback_cleanup_or_gc`
15. `no_power_loss_network_nfs_or_overlay_guarantee`
16. `no_acl_xattr_ads_preservation`
17. `no_new_patch_source_graph_cleanup_backend_or_runtime_semantics`
18. `no_external_consumer_compatibility`

## Authority and evidence status

Snapshot holds one shared semantic-workspace lock across authenticated snapshot
acquisition, the one retained unified build, projection, rendering, final held
object/inventory recheck, and checked unlock. No raw graph or authority escapes.
The module exposes no parser, verifier, source constructor, write, stage,
`ACTIVE` pivot, backend, or runtime authority.

Local canonical-wire, digest, mutation, cap, API/CLI, and preservation gates are
present. The internal literal whole-document fixture pins raw SHA-256
`sha256:6639d985e25d4d33a72e37034c6e3f116940d3598bbf46162a6baaeb547da972`;
the distinct public managed-workspace fixture pins raw SHA-256
`sha256:64dddc0c2046766640ec93b7a7249214d099f683a2b6f26f43cdc22073764a6c`.
Exact-head hosted evidence remains pending; this document makes no status
promotion.

The additive Project-v7 linker admits one narrow exception to the former
value-only function boundary: a monomorphic imported function may accept exact
`borrow Slice<u8>` parameters only when its return is a non-borrowing scalar.
`Slice<u8>` is not globally added to signature-type admission, so borrowed or
owned byte returns, strings, arrays, named aggregates, resources, generics,
and other non-Value modes remain rejected. The one build that widens this is
the package-source workspace, which additionally admits a whole `own Bytes`
imported parameter under the same non-borrowing-scalar-result condition; see
[Multi-Package Source Capsule v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md).
That admission is scoped to that build and changes no Project, draft, or
candidate boundary. Linked HIR revalidates lexical
borrowing, exact symbolic root provenance, call closure, cycles, and capacity
before backend selection. This grants no persistence, mutation, lifetime
erasure, or general cross-file ownership composition.
The frozen Workspace Semantic Graph v1 `nonclaims` bytes above remain
unchanged: the Project-v7 linked-execution admission does not grant this
read-only carrier general borrowing or lifetime authority.
