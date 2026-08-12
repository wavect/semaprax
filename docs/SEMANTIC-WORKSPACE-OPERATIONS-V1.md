# Semantic Workspace Operations v1

Semantic Workspace Operations v1 is a bounded, read-only stable-identity
compiler from one authenticated managed workspace pre-state to one canonical
existing Semantic Workspace Change v1 replacements proposal. It writes no
workspace state and grants no evidence, verification, application, approval,
signature, publication, or reusable authorization authority.

## Proposal

The schema is `semaprax.semantic-workspace-operations.v1`. The document is
compact UTF-8 JSON with exactly one terminal LF and top-level keys, in order,
`schema,base_workspace_revision,entry_module,operations`.

`rename_declaration` has keys
`kind,path,declaration_kind,target_id,from,to`. Its admitted declaration kinds,
in rank order, are `function`, `function_template`, `resource`, `record`,
`variant`, and `interface`. The target must be one explicit user-owned
pre-state declaration with the exact stable ID, path, kind, and current name.

`rename_import_alias` has keys
`kind,path,import_kind,target_id,target_module,from,to`. Its admitted import
kinds are `function` and `type`. The target must be one direct explicit
pre-state import binding to an explicit user-owned provider. Capability and
effect strings, automatic/compiler identities, path operations, and other
semantic edits are not admitted.

Operations are strictly sorted by the bytewise tuple
`(path,operation-rank,subject-kind-rank,target-id,target-module-or-empty,from,to)`.
Declaration operations precede import operations. Selector identity is
`(path,kind,subject-kind,target-id,target-module-or-empty)` and must be unique.
There are 2–64 operations over 2–16 distinct existing managed paths. `from`
and `to` differ; `to` is one canonical identifier token of 1–128 bytes.

Every selector is resolved against the same authenticated pre-state. The
compiler joins authenticated AST spans with retained HIR identity/binding
facts, proves the defining token and every admitted occurrence, rejects
missing, extra, substituted, overlapping, or out-of-range spans, and applies
all nonoverlapping edits simultaneously in reverse source order. Declaration
renames update their defining token and same-module identity-proven
occurrences; import-alias renames update the direct `use` alias and exact
consumer binding occurrences. Simultaneous swaps, chains, and cycles are
allowed only when edit ranges are disjoint and the final namespace is valid.

The proposal digest is lowercase
`sha256:` plus SHA-256 over
`semaprax.semantic-workspace-operations.proposal-digest.v1\0`, followed by the
little-endian `u64` payload length and the exact proposal bytes including LF.

## Authenticated derivation

The route acquires one shared semantic workspace lock first, opens, inspects,
bounded-reads, and UTF-8-decodes the proposal path exactly once, then strictly
parses the owned bytes. Under the same lock it authenticates one retained
Operations-capable base graph and AST/HIR sidecar, consumes that graph once,
plans all operations, builds one candidate unified graph, and independently
replays the exact allowed identity, edge, name, alias, source, manifest, and
budget delta. It derives one canonical
`semaprax.workspace-semantic-change.v1` proposal without changing that schema,
API, or its byte KATs. A resolver-free held-object, manifest, generation,
staging, permission, identity, and inventory recheck and checked unlock occur
before any output returns.

The candidate preserves the managed path set and every unchanged source byte.
Explicit stable IDs, modules, permits, effects, provider identities, and
semantic content remain exact except for the selected name/alias projections.
Automatic identities are not selectable. No source, generation, staging path,
or `ACTIVE` file is written.

## Derivation wrapper

The wrapper schema is
`semaprax.semantic-workspace-operations-derivation.v1`. Its exact top-level
order is:

`schema,workspace_manifest_schema,base_workspace_revision,candidate_workspace_revision,entry_module,operations_proposal,derived_workspace_change_proposal,limits,budget,nonclaims`.

`workspace_manifest_schema` is
`semaprax.workspace-semantic-manifest.v1`. Both proposal references have exact
keys `schema,digest,bytes`; their schemas are respectively the Operations-v1
and Change-v1 schemas, and byte counts include the terminal LF. The derived
Change digest reuses
`semaprax.workspace-semantic-change.proposal-digest.v1\0 || exact bytes`, with
no length frame. The wrapper digest is SHA-256 over
`semaprax.semantic-workspace-operations-derivation.artifact-digest.v1\0 || exact wrapper bytes`,
again with no length frame. The wrapper contains no inline graph, source, edit,
candidate, analysis, evidence, receipt, or self-digest object.

Limits, in exact order, are:

`max_managed_files=16,max_operations_proposal_bytes=1048576,max_operations=64,max_affected_paths=16,max_path_bytes=240,max_target_id_bytes=4096,max_target_module_bytes=240,max_entry_module_bytes=16777216,max_name_bytes=128,max_planned_edits=131072,max_edit_replacement_bytes=16777216,max_total_base_source_bytes=16777216,max_total_candidate_source_bytes=16777216,max_total_replacement_source_bytes=4194304,max_replacement_source_bytes_per_path=1048576,max_candidate_graph_builder_bytes=16777216,max_operations_builder_bytes=67108864,max_derived_change_proposal_bytes=33554432,max_derivation_bytes=33554432,max_total_derivation_bytes=67108864,max_json_depth=8`.

Budget keys, in exact order, are:

`used_managed_files,used_operations,used_affected_paths,used_planned_edits,used_edit_replacement_bytes,used_total_base_source_bytes,used_total_candidate_source_bytes,used_total_replacement_source_bytes,used_entry_module_bytes,used_operations_proposal_bytes,used_candidate_graph_builder_bytes,used_operations_builder_bytes,used_derived_changed_files,used_derived_change_proposal_bytes,used_derivation_bytes,used_total_derivation_bytes`.

The total is the checked sum of input proposal, derived Change proposal, and
wrapper bytes. Rendering uses a decreasing 64 MiB aggregate cap and a fixed
point for exact byte counters. No partial document escapes an overflow.

Ordered nonclaims are:

1. `not_signature_or_authenticated_provenance`
2. `not_human_approval_or_policy`
3. `not_safe_compatible_or_target_verified`
4. `no_reusable_authorization_token`
5. `no_test_or_target_execution`
6. `no_target_evidence_or_machine_code_claim`
7. `no_context_impact_review_or_evidence`
8. `no_operations_evidence_verification_receipt_or_apply_authority`
9. `no_commit_or_publication_authority_in_derivation`
10. `no_existing_change_v1_evidence_binding_to_operations_intent`
11. `no_raw_path_create_delete_move_or_write`
12. `no_path_set_change`
13. `no_automatic_or_compiler_identity_targeting`
14. `no_unmanaged_path_or_raw_tree_authority`
15. `no_raw_tree_git_or_editor_atomic_visibility`
16. `no_automatic_rollback_cleanup_or_gc`
17. `no_power_loss_durability_guarantee`
18. `no_network_distributed_nfs_or_overlay_guarantee`
19. `no_acl_xattr_ads_preservation`
20. `no_general_proof_system`
21. `no_persistence_or_incrementality`
22. `no_external_consumer_compatibility`
23. `no_new_language_graph_cleanup_backend_or_runtime_semantics`
24. `no_change_v1_schema_api_or_kat_modification`

Existing Change-v1 Evidence can bind the exact derived Change proposal bytes;
it does not bind, attest, replay, or authorize the Operations proposal or its
intent.

## Public read-only surface

`SemanticWorkspaceOperationsDerivation` is opaque: it has no public fields,
constructor, `Clone`, `Default`, or serialization implementation. Its only
getters are:

```rust
pub fn operations_proposal_digest(&self) -> &str
pub fn derived_change_proposal(&self) -> &str
pub fn derived_change_proposal_digest(&self) -> &str
pub fn derivation(&self) -> &str
pub fn derivation_digest(&self) -> &str
```

The only public functions are:

```rust
pub fn derive(root: &Path, proposal_path: &Path)
    -> Result<SemanticWorkspaceOperationsDerivation, Vec<Diagnostic>>
pub fn derived_change_proposal(root: &Path, proposal_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn derivation(root: &Path, proposal_path: &Path)
    -> Result<String, Vec<Diagnostic>>
```

Document strings include their one terminal LF. The CLI projections are:

```text
semaprax semantic-workspace-operations-derive <root> <proposal.json>
semaprax semantic-workspace-operations-change-proposal <root> <proposal.json>
```

Wrong arity prints, with one LF, the command token followed by
` requires exactly <root> <proposal.json>`, exits 2, and writes no stdout.
Domain failure exits 1 through the ordinary diagnostic printer and writes no
stdout. Success writes the selected exact API bytes without another LF.

## Diagnostics and evidence

`SPX-G196` owns canonical proposal grammar/schema/key/type/order/depth failures;
`SPX-G197` owns selector binding; `SPX-G198` owns operation conflicts and
incomplete occurrence proof; `SPX-G199` owns numeric admissions and output
caps; `SPX-G200` owns authenticated derivation replay disagreement. Proposal
I/O uses `SPX-I216` with the stable prefix
`could not read Semantic Workspace Operations proposal: ` and suffix
`open failed`, `metadata inspection failed`, `input is not a regular file`,
`read failed`, or `input is not UTF-8`. Paths and bytes are never echoed.
Underlying Workspace and Graph diagnostics retain their ownership; checked
unlock failure remains `SPX-I210`.

Executable digest KATs are:

- Operations proposal: `sha256:3c7bf340a5313907edcec41748063e8666793ee76b903bc4e691871a843544b5`
- derived Change-v1 proposal: `sha256:5c7a67d42ef76b3a241c0dc98f3d8919a799d3745bb6ae54a1d0289a51ee3e86`
- derivation wrapper: `sha256:80df18fea48a663e25cca66e90c0842fa8146ed35ab2ee30f2659728509dd2b7`

Local unit, hostile authority, public API/CLI, strict formatting, and
preservation gates are green. Exact-head Ubuntu, macOS, Windows, MSRV,
Component, and dependency-policy evidence is pending. This adds no completion
status promotion; the matrix remains 38 Partial / 18 Missing.
