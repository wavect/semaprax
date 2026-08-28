# Semantic Workspace Patch Evidence v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Semantic Workspace Patch Evidence v1 is the bounded proof carrier for one
admitted [Semantic Workspace Transaction
v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md). It independently rebuilds the exact
workspace preview and, for every changed path, the existing Semantic Review v1
and Semantic Patch Evidence v1 facts. A separate apply route requires exact
typed and byte replay before any candidate generation or staging object is
created. Here, “proof” means only that closed deterministic replay contract; it
does not mean signature, provenance, approval, safety, compatibility, target
execution, or a general proof system.

## Commands and public API

The fixed-arity commands are:

```text
semaprax workspace-patch-evidence <root> <patch.wspatch>
semaprax verify-workspace-patch-evidence <root> <patch.wspatch> <evidence.json>
semaprax workspace-apply-with-evidence <root> <patch.wspatch> <evidence.json>
```

The public module is `semaprax::workspace_patch_evidence`. Its exact functions
are:

```rust
pub fn generate(root: &Path, workspace_patch_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn verify(root: &Path, workspace_patch_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>
pub fn apply(root: &Path, workspace_patch_path: &Path, evidence_path: &Path)
    -> Result<String, Vec<Diagnostic>>
```

`generate` and `verify` return the complete canonical artifact including
exactly one terminal LF. Their CLIs write those bytes unchanged. `apply`
returns only the candidate workspace revision. Its CLI success output is
exactly the following bytes, including the shown terminal `\n`:

```text
applied semantic workspace transaction with exact evidence replay; workspace is now <sha256-revision>\n
```

The ordinary `workspace-preview` and `workspace-apply` APIs and commands are
unchanged. Workspace Patch Evidence v1 is opt-in and changes no existing
Workspace, Patch, Impact, Review, Repair, Target Evidence, or Patch Evidence
v1/v2 artifact bytes, KATs, or behavior.

## Canonical capsule

The capsule is exactly one compact UTF-8 JSON line followed by one LF. BOM,
CR, additional lines, duplicate or unknown keys, noncanonical spelling or key
order, and nesting deeper than 8 reject. Its schema is exactly
`semaprax.semantic-workspace-patch-evidence.v1`. Top-level key order is:

```text
schema
workspace_manifest_schema
base_workspace_revision
candidate_workspace_revision
workspace_patch
workspace_preview
files
limits
budget
nonclaims
```

`workspace_manifest_schema` is exactly `semaprax.workspace-manifest.v1`.
Closed nested objects have exact key order:

```text
workspace_patch: schema, digest
workspace_preview: schema, digest

file: path, base_source_graph_schema, candidate_source_graph_schema,
      base_revision, candidate_revision, base_source, candidate_source,
      patch, review, assessments, supporting_evidence, patch_evidence

base_source: digest
candidate_source: digest
patch: schema, digest
review: schema, digest
assessments: behavior, api_identity, security_authority, memory_ownership,
             target_artifact, migration, unsafe
supporting_evidence: id, kind, schema, digest
patch_evidence: schema, digest
```

`workspace_patch.schema` is exactly
`semaprax.semantic-workspace-patch.v1`; `workspace_preview.schema` is exactly
`semaprax.semantic-workspace-preview.v1`; `review.schema` is exactly
`semaprax.semantic-review.v1`; and `patch_evidence.schema` is exactly
`semaprax.semantic-patch-evidence.v1`.

The `files` array contains exactly the workspace patch's 2–16 changed paths in
strict canonical lexicographic path order. Per-file Patch schema is one of the
unchanged `semaprax.semantic-patch.v1`, `.v2`, or `.v3` schemas. Each
assessment value is exactly one of `change_proven`,
`unchanged_within_admitted_domain`, `mixed`, `unknown`, or `not_applicable`.

For Patch v1/v2, `supporting_evidence` is exactly id `evidence:0`, kind
`semantic_impact_v1`, and schema `semaprax.semantic-impact.v1`. For the sole
canonical Patch v3 `assign-function-id`, it is exactly id `evidence:0`, kind
`identity_rebase_v1`, and schema `semaprax.identity-rebase.v1`. The kind,
schema, and child Patch schema are correlated and fail closed on substitution.

Each file binds the exact base/candidate Graph v10–v14 schema and revision,
base/candidate source digest, processed Patch schema/digest, whole Review
digest, seven scalar assessments, complete supporting-evidence digest, and
independently rebuilt child Patch Evidence v1 artifact digest. Child Evidence
v1 bytes are not embedded. Each child is built one at a time from the shared
path-keyed workspace preflight plan, then discarded after its digest and
accounting are fixed. The child capsule's exact `no_multi_file_transaction`
nonclaim remains true: it is still a single-file artifact nested only by
digest in a separate outer workspace proof carrier.

## Verification receipt

The receipt schema is exactly
`semaprax.semantic-workspace-patch-evidence-verification.v1`; `result` is
exactly `exact_replay`. It is also one canonical JSON line plus one LF. Its
top-level key order is:

```text
schema
result
workspace_manifest_schema
base_workspace_revision
candidate_workspace_revision
workspace_patch
workspace_preview
workspace_patch_evidence
files
limits
budget
nonclaims
```

`workspace_patch_evidence` has exact key order `schema, digest`, and its schema
is `semaprax.semantic-workspace-patch-evidence.v1`. The receipt inherits the
capsule's exact `workspace_patch`, `workspace_preview`, `file`, source, Patch,
Review, assessment, supporting-evidence, child-evidence, limits, and nonclaim
orders and values. Its `files` array is byte-for-byte the same closed array.
A receipt is output evidence only; it is not an accepted capsule and cannot be
substituted into `workspace-apply-with-evidence`.

## Digests

Every digest has wire form `sha256:<64 lowercase hexadecimal digits>` and uses:

```text
SHA-256(domain || little_endian_u64(byte_length) || exact_bytes)
```

The two new domains are:

- `semaprax.semantic-workspace-patch-evidence.preview-digest.v1\0` over the
  exact canonical `workspace::preview` API bytes without an LF;
- `semaprax.semantic-workspace-patch-evidence.artifact-digest.v1\0` over the
  exact whole LF-terminated submitted capsule.

All other bindings reuse existing domains unchanged:

- workspace Patch:
  `semaprax.semantic-workspace-patch.digest.v1\0`;
- workspace revision:
  `semaprax.workspace-revision.v1\0` over the canonical LF-terminated
  manifest;
- base/candidate source:
  `semaprax.semantic-review.source-digest.v1\0`;
- child Patch:
  `semaprax.semantic-review.patch-digest.v1\0`;
- whole child Review:
  `semaprax.semantic-patch-evidence.review-digest.v1\0`;
- Impact support:
  `semaprax.semantic-review.impact-digest.v1\0`;
- identity-rebase support:
  `semaprax.semantic-review.identity-rebase-digest.v1\0`;
- child Patch Evidence v1 artifact:
  `semaprax.semantic-patch-evidence.artifact-digest.v1\0`.

## Exact limits and budgets

The capsule and receipt carry this exact `limits` key order and values:

| Key | Value |
| --- | ---: |
| `max_managed_files` | 16 |
| `max_changed_files` | 16 |
| `max_total_base_source_bytes` | 16,777,216 |
| `max_total_candidate_source_bytes` | 16,777,216 |
| `max_workspace_patch_bytes` | 4,194,304 |
| `max_operations` | 4,096 |
| `max_declarations` | 4,096 |
| `max_callables` | 1,024 |
| `max_call_sites` | 65,536 |
| `max_manifest_bytes` | 1,048,576 |
| `max_workspace_preview_bytes` | 65,536 |
| `max_child_impact_depth` | 1,024 |
| `max_child_impact_nodes` | 1,024 |
| `max_total_impact_nodes` | 16,384 |
| `max_total_impact_bytes` | 16,777,216 |
| `max_total_review_bytes` | 33,554,432 |
| `max_child_patch_evidence_bytes` | 65,536 |
| `max_total_child_patch_evidence_bytes` | 1,048,576 |
| `max_workspace_evidence_bytes` | 65,536 |
| `max_workspace_receipt_bytes` | 65,536 |
| `max_json_depth` | 8 |
| `max_retained_generations` | 32 |
| `max_staging_attempts` | 32 |
| `max_unexpected_inventory_entries` | 0 |

Capsule `budget` key order is:

```text
used_managed_files
used_changed_files
used_total_base_source_bytes
used_total_candidate_source_bytes
used_workspace_patch_bytes
used_operations
used_declarations
used_callables
used_call_sites
used_manifest_bytes
used_workspace_preview_bytes
used_max_child_impact_depth
used_max_child_impact_nodes
used_total_impact_nodes
used_total_impact_bytes
used_total_review_bytes
used_total_child_patch_evidence_bytes
used_workspace_evidence_bytes
used_retained_generations
used_staging_attempts
used_unexpected_inventory_entries
```

Receipt `budget` uses the same order except
`used_workspace_evidence_bytes` occurs immediately after
`used_workspace_patch_bytes`, and `used_workspace_receipt_bytes` replaces the
capsule's later evidence-byte field immediately after
`used_total_child_patch_evidence_bytes`.

Evidence and receipt byte counts include their one terminal LF and are solved
as exact canonical fixed points. The Workspace aggregate source and parsed-AST
declaration/callable/call-site caps bound HIR and index work before child
construction. The remaining Impact-node/byte, Review-byte, and child-artifact
budgets then cap closure and serialization and debit before the next child;
they do not meter HIR work by remaining output bytes. All renderers use active
capped sinks. Operations, manifest, preview, retained-generation, staging, and
zero-foreign-inventory facts reuse the authenticated Workspace v1 plan and
accounting.

## Read, replay, and authority order

`generate` holds the Workspace shared lock while it owns the exact bounded
workspace Patch bytes, builds one path-keyed semantic plan, reuses its child
preflights for Review/Evidence construction, renders the capsule, and performs
the final workspace recheck. It writes no managed or raw source state.

`verify` acquires the shared lock, reads the evidence file exactly once into
owned bounded bytes, requires its canonical closed schema, owns the workspace
Patch exactly once, rebuilds the same plan and all child facts, requires exact
typed and byte replay, emits the receipt, and performs the final workspace
recheck. Those immutable submitted bytes remain untrusted until exact typed
and byte replay admits them; later path replacement cannot change the owned
bytes and replay supplies neither provenance nor general trust.

`apply` acquires the exclusive permanent Workspace lock first. While holding
it, it owns the workspace Patch exactly once, then owns the evidence exactly
once, parses the capsule, builds the shared plan and child facts from the held
authenticated workspace, and requires exact typed and byte replay. Only exact
replay can create or reuse a candidate generation or create an `ACTIVE` staging
entry. The capsule, receipt, and child artifacts have no authority; the live
invocation owns the existing Workspace commit authority.

After replay, apply enters the unchanged sealed Workspace apply core. It keeps
the existing candidate-generation authentication, two final pre-pivot
authority/input/inventory/candidate/staged-pointer checks, permission
preservation, immutable publication, and sole `ACTIVE` replacement. Original
raw sources remain byte-exact. A stale second application fails `SPX-G152`.
Rejected evidence can acquire and release the lock, but creates no candidate or
staging entry and performs no source write. Failures after replay inherit
Workspace v1's bounded owned-residue and foreign-replacement rules. `SPX-I212`
begins only after a successful `ACTIVE` replacement; it is post-pivot ambiguity,
not rollback.

Apply diagnostic precedence is exact: lock failure `SPX-I210` precedes all
input reads; missing/unreadable workspace Patch `SPX-I209` precedes evidence
I/O or parsing; readable malformed evidence reaches `SPX-G160` before semantic
workspace Patch parsing; valid evidence plus a readable malformed workspace
Patch reaches the existing `SPX-G150` family.

## Diagnostics

The outer layer adds:

| Code | Meaning |
| --- | --- |
| `SPX-G160` | noncanonical format, JSON, schema/key order, child correlation, or capsule/receipt confusion |
| `SPX-G161` | exact input, child, aggregate work, JSON-depth, or output bound exceeded |
| `SPX-G162` | submitted capsule differs from independent canonical replay |
| `SPX-G163` | typed evidence binding disagrees with the sealed workspace build |
| `SPX-I213` | evidence-file open, metadata, regular-file, read, or UTF-8 failure |

The exact outer diagnostic wire text is:

```text
SPX-G160 lead: Semantic Workspace Patch Evidence must be one canonical JSON line with one terminal LF
SPX-G160 detail: <lead>: <closed-format-or-schema-detail>
SPX-G161: Semantic Workspace Patch Evidence `<field>` exceeds <maximum>
SPX-G162: submitted Semantic Workspace Patch Evidence differs from independent canonical replay
SPX-G163: typed Semantic Workspace Patch Evidence bindings disagree with the sealed workspace build
SPX-I213 prefix: cannot read Semantic Workspace Patch Evidence
```

`SPX-I213` appends the bounded path/I/O detail, or the exact nonregular/UTF-8
detail, after that prefix. The `SPX-G161` `<maximum>` is the frozen base-10
ASCII integer without thousands separators (for example `16777216`); commas in
the limits table above are readability-only.

Existing Workspace `SPX-G150`–`SPX-G153` and `SPX-I209`–`SPX-I212`, Patch,
Impact, Review, Repair, and child Patch Evidence diagnostics remain in force.
Nested child bound failures normalize to the exact outer `SPX-G161` field;
impossible nested typed disagreements normalize to `SPX-G163`. No nested
Target Evidence or Patch Evidence v2 diagnostic can escape because neither is
admitted by this schema.

## Frozen KATs and local evidence

These are raw SHA-256 checks over each complete LF-terminated artifact, without
the `sha256:` wire prefix or domain framing:

| Child Patch composition | Capsule SHA-256 | Receipt SHA-256 |
| --- | --- | --- |
| homogeneous v1 | `d0f0ec9abde015cd84745d8d71b260736874b7cff8f172194d04e8ebe489c197` | `ee310a2f848dd034c20f727f011f30db46dfe478bbc1169467dec0d57c266ae1` |
| homogeneous v2 | `95b054e188a4721e03c08b94afe0963394fc0af16be42ef3bdec0990218eb9f6` | `da2440da67c87ec0ab873599c911fc78e816d02fcd12195532ce93817a15df0b` |
| homogeneous v3 | `3fc5dc57a01ce2a9d1110dfd66ec96e9def90b8bfd3e5d2328aa9d4a81da19e4` | `b05b0516508c7850b409b1b81dedfc51c708bfbe6e73c94db77a1aadce35f757` |
| mixed v1/v2/v3 | `de764637af59c533feaba15dca373408cb50972f81afd3fde903f463550fde27` | `3538b97acc1626972b0242085c87059c51b64c2ba7412172bbc2c5118f2f63c1` |

Local evidence is public generation/verification 6/6, evidence-gated apply
5/5, hostile wire/substitution/Graph-v10–v14 2/2, and module hook/limit units
8/8. The shared Workspace core is 39/39 and Workspace integration remains
12/12. The root library is 496/496; the focused preservation sweep is 107/107.
Full workspace/all-target/all-feature, release, examples, host 11/11 and loader
26/26 doctests, rustdoc `-D warnings`, strict Clippy, formatting, diff, and
independent security gates are locally green.

The exact `cda4892ee74100fd11c5161ad857d469ec5e5421` matrix is hosted green in
[run 31491573287](https://github.com/wavect/semaprax/actions/runs/31491573287),
with all 12 jobs passing: [Dependency
policy](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116816),
[Ubuntu](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117078),
[macOS](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116941),
[Windows](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117130),
[MSRV](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116811),
and [Component](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779116886).
The intermediate exact `658b2f4dc6d69974cef553dbd4e6eaecafacdd63`
documentation/count head [run
31490049153](https://github.com/wavect/semaprax/actions/runs/31490049153)
was nonqualifying and cancelled: its macOS early-error precedence test observed
`SPX-I210` instead of the expected `SPX-G150`; Windows was
concurrency-cancelled after that failure and reported no product failure.
The exact `3e41b3a0318730fec41e7d75438414e93dafa313` predecessor [run
31486578192](https://github.com/wavect/semaprax/actions/runs/31486578192)
was nonqualifying at 10/12: its macOS test observed `SPX-I210` instead of the
expected stale `SPX-G152` during snapshot-lock handoff, and its Windows
lock-precedence fixture hit OS error 33 while reopening the locked `LOCK`
file. The corrective head makes the owned-snapshot lock release explicit and
avoids that fixture-only reopen without changing the frozen wire contract.
Earlier hosted Workspace, Patch Evidence, Review, Impact, Repair, or Patch runs
prove only preservation of their frozen contracts; none replaces this exact
outer-workspace-evidence matrix.

Focused commands are:

```sh
cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1_apply
cargo test --locked -p semaprax --all-features --test semantic_workspace_patch_evidence_v1_hostile
cargo test --locked -p semaprax --all-features --lib workspace_patch_evidence::tests
```

## Exact nonclaims

Capsule and receipt carry this ordered array verbatim:

```text
not_signature_or_authenticated_provenance
not_human_approval_or_policy
not_safe_compatible_or_target_verified
no_commit_authority
no_reusable_authorization_token
no_test_or_target_execution
no_target_evidence_or_evidence_v2_aggregation
no_agent_context_or_repository_analysis
no_cross_file_module_type_call_capability_or_identity_resolution
no_cross_file_impact_or_review_reasoning
no_general_multi_file_repair
no_create_delete_move_or_raw_tree_materialization
no_atomic_visibility_for_raw_files_git_or_editors
no_automatic_rollback_cleanup_or_gc
no_network_distributed_nfs_or_overlay_guarantee
no_power_loss_durability_guarantee
no_acl_xattr_ads_preservation
no_general_proof_system
no_persistence_or_incrementality
no_external_consumer_compatibility
no_new_patch_repair_graph_cleanup_backend_or_runtime_semantics
```

The outer artifact aggregates exact per-file single-file facts but performs no
cross-file module, type, call, capability, ownership, identity, Impact, or
Review reasoning. It executes no test or target, includes no Target Evidence
v1 or Patch Evidence v2, and proves no safety, compatibility, ABI, machine
code, Wasm runtime, or external consumer property. It is not a signature, MAC,
authenticated provenance, human approval or policy, authorization token,
repository/Agent Context analysis, general proof system, general multi-file
repair, persistence, or incrementality.

It does not create/delete/move or raw-materialize sources, provide atomic
visibility for raw files/Git/editors, automatically roll back/clean up/garbage
collect, preserve ACL/xattr/ADS, or guarantee network/NFS/overlay or power-loss
behavior. It adds no Patch/Repair/Graph/CleanupPlan/backend/runtime semantics.
The broader unified multi-file Graph and semantic resolution tranche remains
open. Completion status stays exactly 38 Partial and 18 Missing.
