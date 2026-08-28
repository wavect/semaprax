# Semantic Workspace Change v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Semantic Workspace Change v1 is a bounded replacements-only protocol for an
authenticated Semantic Workspace v1. One canonical proposal names 2–16
existing managed paths and supplies complete replacement source for each. The
implementation validates one complete base workspace and one complete
candidate workspace, derives full-graph delta Context, reverse Impact, Review,
and Evidence, and can publish the exact replayed candidate through one
exclusive `ACTIVE` pivot.

The protocol does not create, delete, move, or change the managed path set. It
does not treat submitted JSON, a verification receipt, or an old invocation as
authority.

## Public API

```rust
pub struct SemanticWorkspaceChangeArtifacts { /* opaque */ }

pub fn generate(
    root: &Path,
    proposal_path: &Path,
) -> Result<SemanticWorkspaceChangeArtifacts, Vec<Diagnostic>>;

pub fn preview(root: &Path, proposal_path: &Path)
    -> Result<String, Vec<Diagnostic>>;

pub fn evidence(root: &Path, proposal_path: &Path)
    -> Result<String, Vec<Diagnostic>>;

pub fn verify(root: &Path, proposal_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>;

pub fn apply(root: &Path, proposal_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>;
```

The opaque artifact bundle has no public fields, constructor, `Clone`,
`Default`, or serde implementation. Its only getters are:

```rust
pub fn proposal_digest(&self) -> &str;
pub fn candidate_manifest_digest(&self) -> &str;
pub fn preview(&self) -> &str;
pub fn preview_digest(&self) -> &str;
pub fn context(&self) -> &str;
pub fn context_digest(&self) -> &str;
pub fn impact(&self) -> &str;
pub fn impact_digest(&self) -> &str;
pub fn review(&self) -> &str;
pub fn review_digest(&self) -> &str;
pub fn evidence(&self) -> &str;
pub fn evidence_digest(&self) -> &str;
```

All document getters and returned strings include the artifact's one terminal
LF. `preview()` and `evidence()` are owned projections of the same complete
internal `generate()` result; they have no alternate renderer.

## Commands

```text
semaprax semantic-workspace-change-preview <root> <proposal.json>
semaprax semantic-workspace-change-evidence <root> <proposal.json>
semaprax verify-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>
semaprax apply-semantic-workspace-change-evidence <root> <proposal.json> <evidence.json>
```

Each command requires exactly the displayed positional arguments and accepts no
flags. Wrong arity exits 2, writes no stdout, and writes the corresponding
stderr line below with exactly one terminal LF:

```text
semantic-workspace-change-preview requires exactly <root> <proposal.json>
semantic-workspace-change-evidence requires exactly <root> <proposal.json>
verify-semantic-workspace-change-evidence requires exactly <root> <proposal.json> <evidence.json>
apply-semantic-workspace-change-evidence requires exactly <root> <proposal.json> <evidence.json>
```

Success writes the API bytes verbatim; the CLI adds no second LF. Domain
failure exits 1 with no stdout.

## Proposal

The proposal schema is `semaprax.workspace-semantic-change.v1`. It is compact
canonical UTF-8 JSON with exactly one terminal LF, depth at most 8, and top key
order:

```text
schema,base_workspace_revision,entry_module,changes
```

Each change has exact key order:

```text
path,base_source_graph_schema,base_source_revision,base_source_digest,
replacement_source
```

`changes` contains 2–16 strictly path-sorted unique existing managed files.
Each base schema/revision/digest tuple must exactly equal the authenticated base
manifest. Replacement source is the complete canonical source for the same
path, at most 1,048,576 bytes per file and 4,194,304 bytes in total. The
candidate has the same 2–16-file path set, remains at most 16,777,216 source
bytes, and still contains the exact canonical `entry_module`.

The proposal digest is lowercase `sha256:` plus 64 hex digits over:

```text
"semaprax.workspace-semantic-change.proposal-digest.v1\0" ||
exact_canonical_proposal_bytes_including_LF
```

## Typed delta

The base graph is built once from the authenticated generation; the candidate
graph is built once from the complete replacement overlay. Delta covers the
full managed graph, including disconnected changed modules, rather than only
the entry provider projection.

The candidate Workspace Semantic Graph is a hypothetical read-only projection
over candidate manifest and source facts. Its budget inherits the authenticated
base snapshot's `retained_generations` and `staging_attempts`; it does not
predict a future publication `+1`. Typed delta remains over the full managed
graph, while base and candidate graph digest references bind entry-provider
projections under Workspace Semantic Graph v1.

Persistent explicit declaration IDs correlate across states and are compared
by exact authenticated semantic fingerprint. Automatic declarations in changed
paths are removed and added rather than correlated. An unchanged-path
declaration must preserve origin, metadata, and semantic fingerprint exactly.
Compiler-owned declarations are excluded.

Module roots capture path, module, permit, and import changes. Capability roots
capture incidence changes. Edge delta is the exact symmetric difference of all
six Workspace edge families. Roots sort by
`state,kind,id,path,module,change,identity_origin`; edge facts sort by state,
change, and the Workspace edge tuple. States are `base` and `candidate`;
changes are `removed`, `added`, `modified_before`, and `modified_after`.

Context is the complete root plus changed-edge endpoint union. Reverse
multi-source Impact is a complete shortest-path closure with sorted unique root
provenance. Affected roles are `target`, `consumer`, `module_consumer`, and
`dependency`; reasons are unique causal edge families in this order:

```text
function_import,type_import,call,type_reference,effect_requirement,
capability_authority
```

No Context, Impact, provenance, or depth truncation is permitted.

## Artifact schemas and digests

All artifacts are compact canonical UTF-8 JSON with exactly one terminal LF.

| Artifact | Schema | Digest domain |
| --- | --- | --- |
| Preview | `semaprax.workspace-semantic-change-preview.v1` | `semaprax.workspace-semantic-change-preview.artifact-digest.v1\0` |
| Context | `semaprax.workspace-semantic-change-context.v1` | `semaprax.workspace-semantic-change-context.artifact-digest.v1\0` |
| Impact | `semaprax.workspace-semantic-change-impact.v1` | `semaprax.workspace-semantic-change-impact.artifact-digest.v1\0` |
| Review | `semaprax.workspace-semantic-change-review.v1` | `semaprax.workspace-semantic-change-review.artifact-digest.v1\0` |
| Evidence | `semaprax.workspace-semantic-change-evidence.v1` | `semaprax.workspace-semantic-change-evidence.artifact-digest.v1\0` |
| Verification receipt | `semaprax.workspace-semantic-change-evidence-verification.v1` | none |
| Application receipt | `semaprax.workspace-semantic-change-evidence-application.v1` | none |

Each artifact digest is `sha256(domain || exact_artifact_bytes_including_LF)`.
The Evidence digest appears only in a receipt; Evidence has no self-digest
field. Base/candidate graph references reuse exact Workspace Semantic Graph v1
digests. Candidate manifest digest is
`sha256("semaprax.workspace-semantic-change.candidate-manifest-digest.v1\0" ||
exact_manifest_bytes_including_LF)`.

Common reference key order is:

```text
proposal:                 schema,digest,bytes
candidate_manifest:       schema,digest,bytes
change_preview/context/
impact/review:             schema,digest,bytes
workspace_change_evidence:schema,digest,bytes
base/candidate graph:      schema,digest
```

### Preview

Top-level order:

```text
schema,workspace_manifest_schema,base_workspace_revision,
candidate_workspace_revision,entry_module,proposal,base_workspace_graph,
candidate_workspace_graph,candidate_manifest,files,delta,limits,budget,
nonclaims
```

File order is path ascending; keys are:

```text
path,base_source_graph_schema,candidate_source_graph_schema,
base_source_revision,candidate_source_revision,base_source_digest,
candidate_source_digest,base_bytes,candidate_bytes
```

`delta` keys are `roots,edges`. Root keys are
`state,kind,id,path,module,change,identity_origin`; nullable values are explicit
null. Delta-edge keys are `state,change,edge`. The nested edge order is
`caller_path,caller,target_path,target,kind,site,expression,ast_path,alias,ordinal`.

### Context and Impact

Context top-level order:

```text
schema,base_workspace_revision,candidate_workspace_revision,entry_module,
proposal,change_preview,nodes,limits,budget,nonclaims
```

Node keys are
`state,kind,declaration_kind,identity_origin,id,path,module`, with explicit
nulls.

Impact top-level order:

```text
schema,base_workspace_revision,candidate_workspace_revision,entry_module,
proposal,change_preview,context,affected,dependency_edges,limits,budget,
nonclaims
```

Affected keys are
`state,kind,declaration_kind,identity_origin,id,path,module,minimum_depth,role,reasons,root_provenance`.
Provenance contains ascending unique zero-based indices into
`Preview.delta.roots`. Dependency-edge keys are `state,edge`.

### Review

Top-level order:

```text
schema,base_workspace_revision,candidate_workspace_revision,entry_module,
proposal,change_preview,context,impact,sections,evidence,limits,budget,
nonclaims
```

`sections` is an object ordered `behavior`, `api_identity`,
`security_authority`, `memory_ownership`, `target_artifact`, `migration`,
`unsafe`. Each section has keys `assessment,findings`; each findings array has
exactly one object with keys `code,statement,disposition,evidence`.

Assessments are: behavior `change_proven`; API identity `change_proven` iff a
declaration root exists, otherwise `unchanged_within_admitted_domain`; security
authority `change_proven` iff a capability root or effect/authority delta edge
exists, otherwise `unchanged_within_admitted_domain`; memory ownership, target
artifact, and unsafe `unknown`; migration `change_proven`.

The exact finding code, statement, and disposition triples are:

| Section | Code | Statement | Disposition |
| --- | --- | --- | --- |
| behavior | `SWC-BEHAVIOR-DELTA` | `Authenticated behavior delta and reverse impact are represented by the indexed evidence.` | `review_required` |
| api_identity | `SWC-API-IDENTITY-DELTA` | `Authenticated declaration identity changes are represented by the indexed preview roots.` | `review_required` |
| security_authority | `SWC-SECURITY-AUTHORITY-DELTA` | `Authenticated capability and effect-authority changes are represented by the indexed evidence.` | `review_required` |
| memory_ownership | `SWC-MEMORY-OWNERSHIP-UNASSESSED` | `No general cross-file memory-ownership compatibility claim is established.` | `no_claim` |
| target_artifact | `SWC-TARGET-ARTIFACT-UNASSESSED` | `No target artifact is emitted, executed, or verified.` | `no_claim` |
| migration | `SWC-MIGRATION-REPLACEMENTS` | `The proposal is a replacements-only managed semantic-workspace migration.` | `review_required` |
| unsafe | `SWC-UNSAFE-UNASSESSED` | `No general unsafe, ABI, or foreign-code analysis is established.` | `no_claim` |

The complete Review evidence array contains, once each and in this group order,
all Preview delta roots, Preview delta edges, Context nodes, Impact affected
facts, and Impact dependency edges. Entry keys are `artifact,index,relation`.
Relations are `delta_root`, `delta_edge`, `context_node`, `affected`, and
`dependency_edge`. Finding indices are the exact frozen subsets: behavior uses
delta edges, Context nodes, affected facts, and dependency edges; API identity
uses declaration roots; security authority uses capability roots and
effect/authority delta edges; migration uses every root and affected fact; the
three unassessed sections use empty evidence.

### Evidence and receipts

Evidence top-level order:

```text
schema,workspace_manifest_schema,base_workspace_revision,
candidate_workspace_revision,entry_module,proposal,base_workspace_graph,
candidate_workspace_graph,candidate_manifest,change_preview,context,impact,
review,files,limits,budget,nonclaims
```

Evidence contains references, not inline child documents, and repeats the exact
Preview file array. Verification strictly parses an owned submitted capsule,
regenerates every typed child from one base and candidate build, exact-byte
compares the complete Evidence, and only then renders a receipt.

Both receipt variants use top-level order:

```text
schema,result,workspace_manifest_schema,base_workspace_revision,
candidate_workspace_revision,entry_module,proposal,base_workspace_graph,
candidate_workspace_graph,candidate_manifest,change_preview,context,impact,
review,workspace_change_evidence,files,limits,budget,nonclaims
```

Verification uses result `exact_replay`; application uses result `applied`.
Neither receipt has a digest, is accepted as an input, or grants reusable
authorization.

## Limits and budget

Every artifact carries this exact ordered `limits` object:

| Key | Value |
| --- | ---: |
| `max_managed_files` | 16 |
| `max_changed_files` | 16 |
| `max_source_bytes_per_change` | 1,048,576 |
| `max_total_base_source_bytes` | 16,777,216 |
| `max_total_candidate_source_bytes` | 16,777,216 |
| `max_total_replacement_source_bytes` | 4,194,304 |
| `max_entry_module_bytes` | 16,777,216 |
| `max_proposal_bytes` | 33,554,432 |
| `max_candidate_manifest_bytes` | 1,048,576 |
| `max_delta_roots` | 8,192 |
| `max_delta_edges` | 131,072 |
| `max_context_nodes` | 16,384 |
| `max_impact_nodes` | 16,384 |
| `max_impact_provenance` | 65,536 |
| `max_impact_depth` | 1,024 |
| `max_analysis_builder_bytes` | 33,554,432 |
| `max_change_preview_bytes` | 33,554,432 |
| `max_context_bytes` | 16,777,216 |
| `max_impact_bytes` | 33,554,432 |
| `max_review_bytes` | 16,777,216 |
| `max_evidence_bytes` | 1,048,576 |
| `max_receipt_bytes` | 65,536 |
| `max_total_artifact_bytes` | 100,663,296 |
| `max_json_depth` | 8 |
| `max_retained_generations` | 32 |
| `max_staging_attempts` | 32 |
| `max_unexpected_inventory_entries` | 0 |

The exact `budget` order is:

```text
used_managed_files,used_changed_files,used_total_base_source_bytes,
used_total_candidate_source_bytes,used_total_replacement_source_bytes,
used_entry_module_bytes,used_proposal_bytes,used_candidate_manifest_bytes,
used_delta_roots,used_delta_edges,used_context_nodes,used_impact_nodes,
used_impact_provenance,used_impact_depth,used_analysis_builder_bytes,
used_change_preview_bytes,used_context_bytes,used_impact_bytes,
used_review_bytes,used_evidence_bytes,used_receipt_bytes,
used_total_artifact_bytes,used_retained_generations,used_staging_attempts,
used_unexpected_inventory_entries
```

All byte counts include terminal LF. Child artifacts are all rendered before
any artifact is finalized with the common exact usage. Capsule/children use
zero receipt bytes; receipts use their fixed-point exact length. Aggregate
rendering uses a decreasing 96 MiB allowance and never retains two complete
artifact bundles during convergence. No partial artifact is returned.

## Ordered nonclaims

Every artifact contains this exact array:

1. `not_signature_or_authenticated_provenance`
2. `not_human_approval_or_policy`
3. `not_safe_compatible_or_target_verified`
4. `no_reusable_authorization_token`
5. `no_test_or_target_execution`
6. `no_target_evidence_or_machine_code_claim`
7. `no_current_state_context_impact_or_review_reuse`
8. `no_create_delete_move_or_path_set_change`
9. `no_unmanaged_path_or_raw_tree_authority`
10. `no_raw_tree_git_or_editor_atomic_visibility`
11. `no_commit_authority_in_preview_context_impact_review_or_evidence`
12. `no_automatic_rollback_cleanup_or_gc`
13. `no_power_loss_durability_guarantee`
14. `no_network_distributed_nfs_or_overlay_guarantee`
15. `no_acl_xattr_ads_preservation`
16. `no_general_proof_system`
17. `no_persistence_or_incrementality`
18. `no_external_consumer_compatibility`
19. `no_new_language_graph_cleanup_backend_or_runtime_semantics`

## Authority and state-relative replay

Read-only generation acquires one shared Semantic Change lock, then opens,
inspects, bounded-reads, owns, and parses the proposal exactly once before
authenticating and consuming the retained base graph.

Verification ordering is exact: acquire one shared Semantic Change lock with no
snapshot yet; open, inspect, bounded-read, and own proposal bytes once without
parsing them; open, inspect, bounded-read, own, and strict-parse Evidence once;
strict-parse the already-owned proposal; authenticate and take the retained base
graph; perform one candidate build; render and exact-replay; render the
verification receipt; perform the final held-object/inventory recheck; and
checked-unlock.

Apply follows that same owned-input order under one exclusive Semantic Change
lock. It prerenders the application receipt before any write and then consumes
one unforgeable invocation-local commit authority. It creates or
deeply reuses the exact candidate generation without clobbering, stages a new
`ACTIVE`, performs two complete final checks with an immediate check before the
sole rename, sets the pivot bit immediately after rename, structurally
reauthenticates the published candidate and `ACTIVE` without a second resolver,
performs terminal held identity/permission/inventory checks, checked-unlocks,
and only then releases the prebuilt receipt. There is no cleanup, retry,
rollback, or deletion fallback.

External proposal and Evidence paths are not authenticated by the workspace
lock; post-read path replacement cannot change the owned invocation.

Exact replay is state-relative. Candidate publication residue changes the
authenticated retained-generation/staging budget. Evidence generated before
that state change must fail `SPX-G187`, even if all candidate source bytes are
unchanged. Regenerating Evidence against the new authenticated base state may
then reuse the exact physical candidate without rewriting it. No wire field
reveals create versus reuse strategy.

## Diagnostics

- `SPX-G181`: proposal grammar/canonicality.
- `SPX-G182`: stale base revision, stale base file tuple, unmanaged path, or
  absent candidate entry.
- `SPX-G183`: frozen numeric limits, naming the exact field without `max_`.
- `SPX-G184`: typed proposal/delta/inventory replay disagreement.
- `SPX-G185`: canonical artifact/capsule/receipt grammar, including schema
  confusion and JSON depth.
- `SPX-G186`: incomplete Context, Impact, provenance, depth, or Review evidence.
- `SPX-G187`: canonical submitted Evidence does not exactly replay the
  authenticated proposal and candidate. It deliberately does not reveal which
  substituted field differed.
- `SPX-I214`: stable non-path proposal/Evidence open, metadata, nonregular,
  read, or UTF-8 failures. Only byte overage is `SPX-G183`.
- Existing Workspace `SPX-G150`–`SPX-G153` and `SPX-I209`–`SPX-I212` retain
  their ownership. Before the `ACTIVE` rename, publication failures are
  `SPX-I211`; no `SPX-I212` is possible until rename succeeds; every later
  uncertainty, including unlock, is `SPX-I212`.

No diagnostic echoes proposal/Evidence paths or bytes.

## Process termination and evidence status

Deterministic subprocess tests terminate the real apply process while the
exclusive lock is held at the private `BeforeActiveReplace` and
`AfterActiveReplace` boundaries. On the tested local filesystems they observe
respectively the authenticated old or candidate `ACTIVE`, exact generation and
staging inventories, unchanged original source paths, and immediate lock
reacquisition after process death. This is process-termination evidence only.
It is not a power-loss, storage-device flush, network filesystem, NFS, overlay,
or durability guarantee.

Local canonical proposal/artifact/receipt, replay, hostile filesystem,
create/reuse, no-clobber, reader, permission, and process-termination gates are
present. The frozen whole-document SHA-256 KAT ledger is:

| Document | SHA-256 wire value |
| --- | --- |
| Preview | `sha256:fbfba16e8c3a822b65e59b2a16e2f28393b6d9d9552bcc95fa1363e2599ff8fc` |
| Context | `sha256:18a7990f5b3e1d6a7b06586930684f24787119b99c1e3981c83d92f46d2db117` |
| Impact | `sha256:07c556a41f0ed1d6c25d48743f9550cb6a90eb6d1d8fe26c3ab274feac19284b` |
| Review | `sha256:86ef97e76b6e4ae55d43995a3f537aa5f55b4326cf51a1cfe7fc4127d5054662` |
| Evidence | `sha256:0c5393cb128adc8223a82b7181229cb2c18cb495d714949ccc2dfba07b4402b0` |
| Verification receipt | `sha256:564bdc6b50e475b68321787997aab2b4e96ad23397212e0efefe45b8895561c0` |
| Application receipt | `sha256:2aeb79acfa7420fd57f82d8afa436658c265bf5c02808d13bd7b6acaa6957636` |

The local public C3 suite is 10/10 and the private C3 authority suite is 11/11.
Exact-head Ubuntu, macOS, and Windows hosted evidence remains pending. This
document makes no completion status promotion.
