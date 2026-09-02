# Semantic Workspace Transaction v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Semantic Workspace Transaction v1 is a bounded multi-file publication
protocol for cooperating SEMAPRAX readers and writers. It manages immutable
source generations under one authenticated workspace control directory and
publishes one complete generation by replacing only the canonical `ACTIVE`
pointer. It is a real 2–16-file transaction for the admitted per-file Semantic
Patch v1/v2 operations and the sole canonical Patch v3 operation. It is not a
raw working-tree transaction, a repository graph, or general cross-file
semantics.

The architectural decision and rejected sequential-rename model are recorded
in [ADR 0002](decisions/0002-managed-workspace-generations.md).

## Commands and public API

The fixed-arity commands are:

```text
semaprax workspace-init <root> <path-set.json>
semaprax workspace-snapshot <root>
semaprax workspace-preview <root> <patch.wspatch>
semaprax workspace-apply <root> <patch.wspatch>
```

Initialization and apply print exactly one terminal LF:

```text
initialized semantic workspace; workspace is <sha256-revision>\n
applied semantic workspace transaction; workspace is now <sha256-revision>\n
```

Snapshot and preview print their API JSON followed by exactly one CLI LF. The
public Rust surface is:

```rust
workspace::initialize(
    root: &Path,
    path_set_path: &Path,
) -> Result<String, Vec<Diagnostic>>

workspace::snapshot(
    root: &Path,
) -> Result<WorkspaceSnapshot, Vec<Diagnostic>>

workspace::preview(
    root: &Path,
    patch_path: &Path,
) -> Result<String, Vec<Diagnostic>>

workspace::apply(
    root: &Path,
    patch_path: &Path,
) -> Result<String, Vec<Diagnostic>>
```

`WorkspaceSnapshot` exposes `workspace_revision()`, `files()`, and
`to_json()`. Each `WorkspaceFileSnapshot` exposes `path()`,
`source_graph_schema()`, `source_revision()`, `source_digest()`, and
`source()`. `to_json()` and `preview()` return the canonical JSON without an
LF; the string returned by `initialize()` or `apply()` is the canonical
`sha256:<64 lowercase hex>` workspace revision.

## Managed publication model

The root must be a real, non-aliased directory. Initialization creates and
authenticates this exact control tree without modifying the original sources:

```text
.semaprax-workspace/
  LOCK
  ACTIVE
  generations/
    <64-lowercase-hex-workspace-revision>/
      manifest.json
      files/
        <managed logical paths>
  staging/
    <decimal ordinal 0..31>
```

`LOCK` is a permanent zero-byte regular file opened read/write on one held
handle. Snapshot and preview take a shared lock. Initialization and apply take
an exclusive lock. Contention fails immediately as `SPX-I210`; process exit or
termination releases the operating-system lock without deleting `LOCK`.

Initialization authenticates the external path-set file and every original
source, requires canonical formatter bytes, builds generation zero, publishes
that immutable generation without replacement, and only then publishes the
initial `ACTIVE`. This initialization authority is distinct from ordinary
apply authority.

Apply holds the exclusive lock from base authentication through return. It
owns the exact workspace-patch bytes, independently preflights every changed
file, constructs or deeply authenticates the complete candidate generation,
publishes that generation without replacement, stages a replacement `ACTIVE`,
and performs two final authority, inventory, input, candidate, and staged-
pointer rechecks. It then atomically replaces only `ACTIVE`, authenticates the
published pointer and selected snapshot, and returns the candidate workspace
revision. A second application of the same stale patch fails as `SPX-G152`.

Before the `ACTIVE` replacement, rejection leaves the selected generation
unchanged. Owned retained generation/staging residue is authenticated and
bounded; a foreign replacement is preserved rather than deleted and causes
fail-closed inventory/authentication. After the replacement, an error is
`SPX-I212` post-pivot ambiguity: the new generation can already be selected and
no rollback is claimed.

A cooperating reader takes the shared permanent lock and resolves one
authenticated `ACTIVE` revision into one immutable generation. Such readers
observe the complete old or complete new managed snapshot. Original `.spx`
files are never rewritten, so Git, editors, build tools, and other raw-path
readers do not gain atomic visibility from this protocol.

## Canonical input schemas

Every input/control JSON artifact is exactly one canonical JSON line followed
by one LF. BOMs, CR, extra lines, unknown or reordered keys, noncanonical JSON,
and trailing data fail closed.

Path set schema `semaprax.workspace-path-set.v1` has exact key order
`schema`, `files`; each file has only `path`:

```json
{"schema":"semaprax.workspace-path-set.v1","files":[{"path":"alpha.spx"},{"path":"nested/beta.spx"}]}
```

The array contains 2–16 strictly lexicographically sorted unique logical paths.
Each path is relative, normalized, lowercase ASCII, at most 240 bytes and 16
segments, ends in `.spx`, and has segments of at most 64 bytes beginning with
an ASCII alphanumeric and continuing only with lowercase ASCII alphanumerics,
`.`, `_`, or `-`. Empty, dot-terminated, separator-confused, drive-qualified,
absolute, `.`/`..`, and Windows device-name stems `con`, `prn`, `aux`, `nul`,
`com1`–`com9`, and `lpt1`–`lpt9` are rejected.

The root pointer schema `semaprax.workspace-root.v1` has exact key order
`schema`, `workspace_revision`:

```json
{"schema":"semaprax.workspace-root.v1","workspace_revision":"sha256:<64-lowercase-hex>"}
```

The manifest schema `semaprax.workspace-manifest.v1` has exact key order
`schema`, `files`; each file has exact order `path`, `source_graph_schema`,
`source_revision`, `source_digest`, `bytes`:

```json
{"schema":"semaprax.workspace-manifest.v1","files":[{"path":"alpha.spx","source_graph_schema":"semaprax.semantic-graph.v10","source_revision":"sha256:<64-lowercase-hex>","source_digest":"sha256:<64-lowercase-hex>","bytes":123}]}
```

The workspace patch schema `semaprax.semantic-workspace-patch.v1` has exact
key order `schema`, `base_workspace_revision`, `files`; each file has exact
order `path`, `patch`:

```json
{"schema":"semaprax.semantic-workspace-patch.v1","base_workspace_revision":"sha256:<64-lowercase-hex>","files":[{"path":"alpha.spx","patch":"base sha256:<64-lowercase-hex>\nrename example.alpha to renamed_alpha\n"},{"path":"nested/beta.spx","patch":"base sha256:<64-lowercase-hex>\nrename example.beta to renamed_beta\n"}]}
```

It changes 2–16 strictly sorted unique paths from the exact managed path set.
Each embedded string is the exact owned canonical Semantic Patch input. The
workspace route admits existing Patch v1/v2 and the sole canonical Patch v3
`assign-function-id`; it does not alter any of those grammars or gates.

## Canonical snapshot

`semaprax.workspace-snapshot.v1` has exact top-level key order
`schema`, `workspace_revision`, `files`, `limits`, `budget`, `nonclaims`.
Each file has exact order `path`, `source_graph_schema`, `source_revision`,
`source_digest`, `bytes`.

`limits` has exact order:

```text
max_managed_files
max_total_source_bytes
max_manifest_bytes
max_snapshot_bytes
max_json_depth
max_retained_generations
max_staging_attempts
max_unexpected_inventory_entries
```

`budget` has exact order:

```text
used_managed_files
used_total_source_bytes
used_manifest_bytes
used_snapshot_bytes
used_retained_generations
used_staging_attempts
used_unexpected_inventory_entries
```

The report omits source text, although the owned Rust snapshot exposes the
authenticated source through `WorkspaceFileSnapshot::source()`.

## Canonical preview

`semaprax.semantic-workspace-preview.v1` has exact top-level key order
`schema`, `base_workspace_revision`, `candidate_workspace_revision`,
`workspace_patch_digest`, `files`, `limits`, `budget`, `nonclaims`.
Each changed-file entry has exact order `path`, `patch_schema`, `patch_digest`,
`base_source_graph_schema`, `candidate_source_graph_schema`, `base_revision`,
`candidate_revision`.

`limits` has exact order:

```text
max_managed_files
max_changed_files
max_total_base_source_bytes
max_total_candidate_source_bytes
max_workspace_patch_bytes
max_operations
max_declarations
max_callables
max_call_sites
max_manifest_bytes
max_preview_bytes
max_json_depth
max_retained_generations
max_staging_attempts
max_unexpected_inventory_entries
```

`budget` has exact order:

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
used_preview_bytes
used_retained_generations
used_staging_attempts
used_unexpected_inventory_entries
```

Snapshot and preview carry this exact ordered `nonclaims` array:

```text
no_cross_file_module_type_call_capability_or_identity_resolution
no_repository_impact_review_context_target_or_test_analysis
no_workspace_evidence_or_proof_artifact
not_signature_authenticated_provenance_or_human_approval
no_lock_stage_publish_or_commit_authority
no_atomic_visibility_for_raw_files_git_or_editors
no_create_delete_move_or_flat_materialization
no_network_distributed_nfs_or_overlay_guarantee
no_power_loss_durability_guarantee
no_automatic_rollback_cleanup_or_gc
no_acl_xattr_ads_preservation
no_general_multi_file_repair
no_new_patch_graph_cleanup_backend_or_runtime_semantics
no_external_consumer_compatibility
```

`no_lock_stage_publish_or_commit_authority` describes the snapshot and preview
artifacts: possessing a report grants no authority. It is not a claim that the
live `initialize` and `apply` invocations lack authority. Those routes own the
bounded lock, generation-publication, and `ACTIVE`-pivot authority described
above; no artifact is a reusable authorization token.

## Digests and revisions

Every digest below is SHA-256 over:

```text
domain || u64le(byte_length) || exact_bytes
```

and is encoded `sha256:<64 lowercase hex>`.

- Managed-source digest reuses domain
  `semaprax.semantic-review.source-digest.v1\0` over exact source bytes.
- Embedded-patch digest reuses domain
  `semaprax.semantic-review.patch-digest.v1\0` over exact embedded patch
  bytes.
- Workspace-patch digest uses
  `semaprax.semantic-workspace-patch.digest.v1\0` over the exact whole
  LF-terminated workspace-patch file.
- Workspace revision uses `semaprax.workspace-revision.v1\0` over the exact
  whole LF-terminated canonical manifest.

Per-file `source_revision` and `source_graph_schema` remain the existing
canonical source Graph revision and selected Graph v10–v14 schema. Workspace
v1 does not define a new multi-file semantic Graph.

## Bounds and work ordering

The closed limits are:

- 2–16 managed files and 2–16 changed files;
- 16,777,216 total base or candidate source bytes;
- 4,194,304 workspace-patch bytes;
- 4,096 total embedded patch operations;
- 4,096 parsed declarations, 1,024 parsed callables, and 65,536 parsed call
  sites across the complete workspace;
- 1,048,576 manifest/path-set/ACTIVE bytes;
- 65,536 preview bytes and 33,554,432 snapshot bytes;
- JSON depth 8;
- 32 retained generations, 32 staging attempts, and zero unexpected inventory
  entries.

Inventory walks short-circuit at the bound before unbounded collection.
Parsed-AST declaration/callable/call-site use is accumulated across unchanged
and changed files before repeated verification/HIR work. Candidate source
bytes include unchanged files and debit the remaining 16-MiB budget before
each changed-file preflight. Canonical formatting and all manifest, snapshot,
preview, path-set, and patch rendering run under active bounded sinks; exact
limit values succeed and over-limit values fail closed.

## Authentication and identity boundary

The root, control directory, `generations`, `staging`, selected and retained
generation directories, `files` directories, nested managed directories,
`LOCK`, `ACTIVE`, manifests, staging objects, and managed source files are
authenticated as exact expected real objects. Symlinks, Windows reparse points
including junctions, nonregular files, hard links, path/handle disagreement,
physical identity aliasing, foreign inventory, and cross-volume objects fail
closed. Regular files require link count one. Directory and text handles are
retained and rechecked at final boundaries.

Unix identity is exact device/inode. Windows uses held handles plus volume and
the available 64-bit file index. This is not a uniqueness guarantee for ReFS
128-bit identities or hostile filesystems that expose non-unique indices.

Authored module names and authored explicit/automatic declaration identities
must be unique across the workspace. Shared compiler-owned/prelude identities
are intentionally excluded. There is no cross-file name, type, call,
capability, import, ownership, or identity resolution.

## Diagnostics

- `SPX-G150`: canonical JSON, schema, key order, logical path, or canonical
  source/patch format rejection.
- `SPX-G151`: input, work, inventory, AST, operation, or output bound exceeded.
- `SPX-G152`: stale workspace base, drift, or repeated stale apply.
- `SPX-G153`: authenticated object, manifest/content, identity, uniqueness, or
  structural invariant disagreement.
- `SPX-I209`: workspace input, control-tree I/O, or authentication failure.
- `SPX-I210`: permanent lock contention, acquisition, release, or unavailable
  lock support.
- `SPX-I211`: apply-route staging, immutable-generation publication,
  pre-pivot staged-`ACTIVE`/hook/recheck/replacement/rename failure, or related
  pre-pivot I/O failure. Typed structural disagreement remains `SPX-G153`.
- `SPX-I212`: initialization's separate initial-`ACTIVE` publication family;
  for apply, only failure after the successful `ACTIVE` replacement and the
  resulting post-pivot ambiguity.

Per-file parser/verifier/Patch diagnostics remain their existing exact
families. Workspace v1 does not normalize or widen those semantics.

## Known answers and executable evidence

The two-file canonical fixture freezes:

- initial workspace revision
  `sha256:9a7368825342cee138d02a8037248e9a41ed0479d4f7c32a21c7ee7141cf280c`;
- snapshot JSON SHA-256
  `3646097c9fb8c47bced51cf2c404b886755f657c73c57afb18d25282574f0b80`;
- preview JSON SHA-256
  `a4f1a9467d535aada97e7f253cf51c0d2168b5557a5a400d11692ac6966776b4`.

The three-file mixed Patch v1/v2/v3 fixture freezes snapshot JSON SHA-256
`dfd35db518d0a8d94b83702dd1d2760ce9340b5875e0960ac573f84474c223b5`
and preview JSON SHA-256
`3cbd8d22bc26069387ac8ebce72ca590f095cbaa193b04bdef041e4c06beced1`.

Focused local evidence is 12/12 public workspace integration tests, 5/5
hostile canonical-wire/CLI tests, and 37/37 workspace unit/hook/limit tests.
The root library suite is 482/482. Full workspace/all-target/all-feature,
release, host 11/11 and loader 26/26 doctests, rustdoc `-D warnings`, strict
Clippy, formatting, diff, example, preservation, and independent security
gates are locally green. The exact
`afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` full matrix is hosted green in
[run 31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
including [dependency-policy job
93719800523](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800523),
[Ubuntu job
93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613),
[macOS job
93719800554](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800554),
[Windows job
93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611),
[MSRV job
93719800689](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800689),
and [component job
93719800635](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800635);
all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
strict Clippy and is not green evidence. No earlier Phase A or Phase B workflow
is Phase C publication proof.

Focused commands are:

```sh
cargo test --locked -p semaprax --all-features --test semantic_workspace_transaction_v1 -- --test-threads=1
cargo test --locked -p semaprax --all-features --test workspace semantic_transaction_hostile:: -- --test-threads=1
cargo test --locked -p semaprax --all-features --lib workspace::tests
```

## Exact nonclaims

The Workspace snapshot and preview artifacts do not provide cross-file
semantic resolution, a repository or multi-file semantic Graph,
Impact/Review/Context/Target/test analysis, Workspace Evidence or proof
artifacts, signatures/MACs/authenticated provenance, approval, reusable
authorization, external-consumer compatibility, or new Patch/Graph/
CleanupPlan/backend/runtime semantics. They do not create, delete, move, or
flat-materialize sources and do not atomically update raw files, Git, editors,
or noncooperating readers.

It does not guarantee network/distributed/NFS/overlay behavior, power-loss
durability, automatic rollback, cleanup or garbage collection, ACL/xattr/ADS
preservation, general multi-file repair, or completion of the RFC 0001
multi-file graph/API goal. Existing Semantic Patch v1/v2/v3, single-file A0,
Impact v1, Diagnostic Repair v1, Review v1, Target Evidence v1, and Patch
Evidence v1/v2 bytes, APIs, KATs, authority, and nonclaims remain unchanged.

The additive [Semantic Workspace Patch Evidence
v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) independently binds this exact
preview and per-file child Evidence-v1 facts in a separate outer capsule. It
does not alter any Workspace v1 artifact or authority rule. Snapshot and
preview retain `no_workspace_evidence_or_proof_artifact`; the outer capsule is
the separate proof carrier and still grants no commit authority.

The completion dashboard therefore remains exactly 38 Partial and 18 Missing.
