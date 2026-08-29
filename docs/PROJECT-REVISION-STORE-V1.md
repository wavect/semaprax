# Project Revision Store v1

Status: additive implementation and a focused Unix evidence subset authored;
the complete evidence programme, local execution, and hosted execution are not
claimed.

Audience: compiler contributors, host integrators, and agent-tool authors.

Project Revision Store v1 is an explicitly invoked, content-addressed,
immutable store for the exact canonical manifest and source inputs already
owned by one authenticated `project::ProjectRevision`. It is a narrow injected
library boundary. It is not a default cache, a Project loader bypass, a build
cache, a daemon capability, or an authority carried by a receipt.

The store is additive. Project Manifest v1-v10, Project Agent Transport v2-v5,
Workspace, graph, carrier, diagnostic, and target bytes are unchanged.

## Public API

```rust
pub fn persist(
    root: &Path,
    revision: &project::ProjectRevision,
    expected_project_revision: &str,
) -> Result<ProjectRevisionStoreReceipt, Vec<Diagnostic>>;

pub fn load(
    root: &Path,
    entry_digest: &str,
    expected_project_revision: &str,
) -> Result<project::ProjectRevision, Vec<Diagnostic>>;
```

`ProjectRevisionStoreReceipt` is opaque, is not `Clone`, `Default`, or serde
data, and exposes only borrowed `entry_digest()`, `project_revision()`,
`workspace_revision()`, and `project_graph_digest()` getters. It has no root,
path, handle, stage, write, build, mutation, or reusable authorization
authority.

Both operations require an absolute, normalized store root selected by the
host. The root must already exist. A relative root, `.` or `..` component,
symlink/reparse point, non-directory, or path/held-handle disagreement rejects
before a store effect. Every invocation opens and owns a fresh root authority;
no authority object escapes or can be reused.

The safe implementation is admitted only on Unix hosts with the existing
`rustix` handle-relative filesystem primitives and atomic no-replace rename.
Other hosts reject before opening an entry or creating a stage. This is not a
Windows store-support claim.

## Immutable entry

One published entry has this exact relative tree, where `<entry-hex>` is the
64 lowercase hexadecimal digits of `entry_digest` without `sha256:`:

```text
<entry-hex>/
  entry.json
  semaprax.toml
  workspace-manifest.json
  sources/
    <the exact manifest-declared source paths>
```

There are no optional files. Each source path retains its canonical Project
relative spelling and depth. Every directory is real and every file is one
regular non-symlink, non-hard-linked object. Publication never modifies an
existing entry.

`entry.json` is one compact canonical UTF-8 JSON line with one terminal LF,
no BOM or CR, depth at most 8, and exact top-level order:

```text
schema,project_schema,project_revision,workspace_manifest_schema,
workspace_revision,project_graph_digest,manifest,workspace_manifest,sources,
limits,budget,nonclaims
```

The schema is `semaprax.project-revision-store-entry.v1` and
`workspace_manifest_schema` is
`semaprax.workspace-semantic-manifest.v1`. `manifest` and
`workspace_manifest` each have exact child order `digest,bytes`. Every source
has exact order:

```text
path,source_graph_schema,source_revision,source_digest,bytes
```

Sources retain authenticated manifest order, which is strictly path-sorted.
The entry binds the exact Project schema, Project revision, Workspace manifest
schema and revision, Project graph digest, canonical manifest digest and byte
count, canonical Workspace manifest digest and byte count, and every source
path/schema/revision/digest/byte count.

The canonical manifest digest domain is
`semaprax.project-revision-store.manifest-digest.v1\0`; the Workspace manifest
digest domain is
`semaprax.project-revision-store.workspace-manifest-digest.v1\0`. Both use:

```text
SHA-256(domain || u64_le(exact_byte_length) || exact_bytes)
```

Source digests retain the existing
`semaprax.semantic-review.source-digest.v1\0` domain. Workspace revision and
Project revision are independently rebuilt with their existing v1 domains and
exact canonical inputs. The entry digest is:

```text
SHA-256(
  "semaprax.project-revision-store.entry-digest.v1\0" ||
  u64_le(entry_json_length) || exact_entry_json_bytes_including_LF
)
```

The directory name must exactly equal that digest. Digest collision or an
existing destination rejects without adoption, replacement, deletion, or
byte comparison as authority.

## Independent replay

Before persistence performs its first effect, it independently reconstructs
and exact-compares:

1. canonical Project manifest bytes from the typed manifest;
2. every source digest and byte count;
3. the canonical Workspace manifest from the source facts;
4. the Workspace revision from that manifest;
5. the Project revision from canonical manifest and Workspace revision;
6. the closed canonical `entry.json`; and
7. the content-addressed entry digest.

After writing a stage and again after publication, replay reads through held
directory/file descriptors, authenticates the exact recursive inventory and
identities, owns each bounded file exactly once, reconstructs all typed facts,
and requires byte-identical canonical rendering. It then rebuilds the ordinary
Project revision from the stored manifest and sources through the real Project
parser, unified Phase-A build, profile admission, linker, HIR validation, and
backend admission. The rebuilt Project, Workspace, and graph digests must equal
the stored subject.

`load` performs the same complete held-root and entry replay before returning
the newly rebuilt authority-neutral `ProjectRevision`. The caller-provided
`expected_project_revision` must exactly equal the replayed subject. A stale,
foreign, malformed, truncated, or self-consistently reminted entry fails
closed.

Root-inventory admission does not compile every unrelated retained Project.
For each of at most 32 retained entries it reads at most the 1,048,576-byte
canonical `entry.json`, verifies that its content-address digest equals the
directory name, and authenticates the complete held structural inventory,
exact file sizes, modes, link counts, and stable identities. Thus unrelated
metadata replay is cumulatively bounded by 33,554,432 bytes and 9,280
inventory objects. Complete byte/digest/semantic replay is reserved for the
selected load subject and the newly staged and published persistence subject.
Same-size corruption of an unrelated retained entry therefore does not grant
authority: selecting it still requires complete replay, while structural or
metadata corruption prevents every operation immediately.

## Publication and authority order

Persistence has this fixed order:

1. validate the expected subject and prepare the complete canonical carrier;
2. open the absolute root component-by-component with `O_NOFOLLOW` and retain
   its identity;
3. acquire one non-blocking advisory lock on the held root, rebind the supplied
   path to that exact identity, and authenticate the bounded root inventory;
4. create exactly one `.stage-<entry-hex>` directory with create-new semantics;
5. create every directory and file relative to held descriptors with
   `O_NOFOLLOW`, `O_EXCL`, and no path rediscovery;
6. synchronize and independently replay the complete staged inventory;
7. recheck root identity and the exact root inventory;
8. atomically rename the stage to `<entry-hex>` in the same held root using
   no-replace semantics;
9. authenticate the published name against the retained stage identity,
   independently replay the entry again, recheck root identity/inventory, and
   only then return the prebuilt receipt.

There is no retry, cleanup, deletion, adoption, overwrite, rollback, recovery,
eviction, or garbage collection. Before the rename, failure leaves any partial
stage inert and preserves all bytes. After the rename, any uncertainty is
post-pivot ambiguity: the immutable entry may be visible and is never removed
automatically. A later operation rejects every retained stage or unexpected
root entry instead of treating it as owned residue.

## Exact limits

| Field | Maximum |
| --- | ---: |
| retained published entries | 32 |
| simultaneous stage entries | 1 during the owning invocation, 0 on entry |
| manifest bytes | 65,536 |
| Workspace manifest bytes | 1,048,576 |
| sources | 16 |
| total source bytes | 16,777,216 |
| source path bytes | 240 |
| source path depth | 16 |
| entry JSON bytes | 1,048,576 |
| recursive inventory entries | 290 |
| JSON depth | 8 |
| unexpected inventory entries | 0 |

Every limit uses checked cumulative accounting before allocation or filesystem
effect. Exact capacity succeeds and capacity plus one rejects. Reads take at
most the declared limit plus one byte so truncation, growth, and trailing data
are distinguishable.

The entry's `limits` object carries the fixed values in the table's order.
Its `budget` object carries the corresponding `used_` fields, including exact
manifest, Workspace-manifest, source, entry-JSON, and recursive-inventory byte
or count use. The fixed ordered `nonclaims` array is:

```text
not_a_default_or_ambient_cache
not_signature_authenticated_provenance_or_approval
no_reusable_authorization_token
no_network_process_tool_environment_template_patch_or_build_authority
no_source_workspace_manifest_or_project_mutation
no_daemon_transport_or_protocol_authority
no_target_execution_or_artifact_publication
no_dependency_registry_or_package_resolution
no_raw_path_trust_or_symlink_traversal
no_adoption_overwrite_cleanup_recovery_eviction_or_gc
no_power_loss_network_nfs_overlay_or_durability_guarantee
no_acl_xattr_or_ads_preservation
no_windows_store_support
no_external_consumer_compatibility_or_release_promotion
```

## Diagnostics

- `SPX-G190`: root, digest, expected-subject, canonical JSON, schema, key
  order, path, or closed grammar rejection.
- `SPX-G191`: byte, count, depth, inventory, or retained-entry limit exceeded.
- `SPX-G192`: stale/foreign subject, independent replay, digest, canonical
  render, or typed binding disagreement.
- `SPX-G193`: exact stage, entry, file, directory, link, identity, or inventory
  authentication disagreement.
- `SPX-I215`: root/entry open, read, write, synchronize, or pre-publication
  filesystem failure, unsupported host, or no-replace publication failure.
- `SPX-I216`: uncertainty after the successful same-root publication rename.

Diagnostics never echo manifest/source bytes and never normalize a failed
store operation into Project success.

## Evidence and nonclaims

Authored evidence must cover canonical persist/load round-trip for every
admitted Project profile; deterministic entry JSON and digest; exact-capacity
and plus-one rejection; stale expected subject; manifest/source/Workspace/
Project/graph binding mutations; same-byte path substitution; root/entry/file
symlinks and hard links; partial stage; root and nested foreign bytes; existing
destination collision; stage and published truncation/growth; permission and
identity drift before and after the pivot; post-pivot ambiguity; and exact
legacy Project Manifest v1-v10 and Transport v2-v5 byte preservation.

This store does not persist HIR, graph indexes, compiler output, packages,
artifacts, patches, templates, locks, approvals, credentials, or executable
authority. Loading recompiles semantic meaning from exact stored inputs; it is
not a serialized-verifier bypass or an incremental compiler cache. The store
does not discover roots, follow symlinks, watch files, start a daemon, open a
network service, invoke a process/tool, build a target, mutate source, evict,
recover, repair, clean, or garbage collect. Authored but unrun evidence does
not establish local, hosted, cross-platform, public, mature, or production
support.

The focused authored gate is:

```sh
cargo test --locked -p semaprax --all-features --lib project_revision_store::tests -- --test-threads=1
```
