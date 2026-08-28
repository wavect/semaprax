# Semantic Workspace v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: workspace tool authors and compiler contributors.

Semantic Workspace v1 is the managed, read-only source authority for unified
cross-file SEMAPRAX semantics. It authenticates 2–16 existing canonical `.spx`
files, resolves them together once, and publishes an immutable generation by
replacing one `ACTIVE` file. Initialization never rewrites the original source
paths.

This protocol is distinct from
[Semantic Workspace Transaction v1](SEMANTIC-WORKSPACE-TRANSACTION-V1.md).
Both use `.semaprax-workspace` and its single permanent `LOCK`, but their
`ACTIVE` and manifest schemas are disjoint. An ordinary Workspace v1 reader
must reject a Semantic Workspace v1 root, and a semantic reader must reject an
ordinary root.

## Public API and command

The sole public initializer is:

```rust
pub fn initialize(
    root: &std::path::Path,
    path_set_path: &std::path::Path,
) -> Result<String, Vec<Diagnostic>>
```

The returned string is the `sha256:` workspace revision. The fixed CLI is:

```text
semaprax semantic-workspace-init <root> <path-set.json>
```

Success writes exactly:

```text
initialized semantic graph workspace; workspace is <revision>\n
```

Wrong arity exits 2, writes no stdout, and writes exactly:

```text
semantic-workspace-init requires exactly <root> <path-set.json>
```

Domain failures exit 1 through the common diagnostic renderer and write no
stdout.

## Canonical control documents

Every document in this section is compact UTF-8 JSON with exactly one terminal
LF, no BOM, no CRLF, no insignificant whitespace, JSON depth at most 8, and
keys in the stated order.

The caller-owned path set has schema
`semaprax.workspace-semantic-path-set.v1`:

```json
{"schema":"semaprax.workspace-semantic-path-set.v1","files":[{"path":"a.spx"},{"path":"b.spx"}]}
```

`files` contains 2–16 strictly increasing, unique canonical relative managed
paths. Absolute paths, `.` or `..` components, empty components, noncanonical
separators, aliases, symlinks, junctions, reparse points, and duplicate physical
identities are rejected.

The published `ACTIVE` document has schema
`semaprax.workspace-semantic-root.v1` and exact key order:

```text
schema,workspace_revision
```

The selected generation manifest has schema
`semaprax.workspace-semantic-manifest.v1`. Its top-level key order is
`schema,files`; each file has exact key order:

```text
path,source_graph_schema,source_revision,source_digest,bytes
```

Manifest files retain path order. `source_graph_schema` is exactly one of
`semaprax.graph.v10` through `semaprax.graph.v14` selected by the authenticated
per-file HIR. `source_revision` is the existing Graph revision of that file's
canonical AST. `source_digest` is
`sha256("semaprax.semantic-review.source-digest.v1\0" ||
u64_le(source_byte_length) || exact_canonical_source_bytes)`. There is no
additional LF beyond any LF already present in the canonical source bytes.
`bytes` is the exact canonical UTF-8 source length.

The workspace revision is:

```text
sha256(
  "semaprax.workspace-semantic-revision.v1\0" ||
  u64_le(manifest_byte_length) ||
  exact_manifest_bytes_including_LF
)
```

The wire spelling is `sha256:` followed by 64 lowercase hexadecimal digits.

## Initialization authority

Initialization performs the following ordered operation:

1. Canonicalize and authenticate `root` without following aliases.
2. Open, inspect, bounded-read, and retain the path set and every original
   source exactly once.
3. Validate canonical paths, distinct physical identities, permissions, one
   volume, source bytes, and complete directory ancestry before creating the
   control directory.
4. Parse and resolve all managed sources in one unified Phase-A build. Derive
   every manifest fact, render and independently replay the manifest, and bind
   the workspace revision before the first filesystem publication write.
5. Create `.semaprax-workspace`, create-new `LOCK`, and hold one exclusive lock.
6. Create a first-free staging slot in the bounded range 0–31 without deleting,
   adopting, overwriting, or cleaning foreign entries.
7. Write the exact manifest and canonical sources, synchronize them, and deeply
   authenticate the staged generation.
8. Publish a create-new immutable generation named by the revision. An exactly
   authenticated existing destination may be reused; any alias, substitution,
   corruption, or no-clobber disagreement fails closed.
9. Stage a create-new semantic `ACTIVE`, perform two complete held-object,
   source, directory, inventory, permission, identity, and same-volume checks,
   and repeat the second check immediately before the sole `ACTIVE` rename.
10. Set the pivot boundary immediately after that rename, deeply authenticate
    the new `ACTIVE` and generation without a second resolver pass, perform a
    terminal held-identity check, then synchronously checked-unlock.

There is no retry, deletion, cleanup, rollback, or overwrite fallback. Before
the pivot, publication failures are `SPX-I211`; after a successful `ACTIVE`
rename, every uncertainty, including unlock failure, is `SPX-I212`. Lock
contention and checked-unlock ownership use `SPX-I210`. Authentication and
inventory failures preserve the Workspace `SPX-G150`–`SPX-G153` and
`SPX-I209` families.

## Limits

| Field | Maximum |
| --- | ---: |
| `managed_files` | 16 |
| `total_source_bytes` | 16,777,216 |
| `path_set_bytes` | 1,048,576 |
| `manifest_bytes` | 1,048,576 |
| `active_bytes` | 1,048,576 |
| JSON depth | 8 |
| retained generations | 32 |
| staging attempts | 32 |
| unexpected inventory entries | 0 |

Zero or one source reports `SPX-G174` with
`Semantic Workspace requires 2..16 source files`; more than 16 reports
`SPX-G175` for `managed_files`. Canonical grammar/mode errors are `SPX-G174`.
Semantic Workspace storage bounds are `SPX-G175`. The unified Workspace
Semantic Graph build retains its own `SPX-G170`–`SPX-G173` diagnostics.

## Explicit nonclaims

Semantic Workspace v1 grants no raw working-tree, Git, editor, unmanaged-file,
network, package-registry, backend, runtime, target execution, patch, review,
approval, signature, reusable authorization, cleanup, rollback, garbage
collection, or source-rewrite authority. It does not promise ACL, xattr, or ADS
preservation, cross-volume atomicity, NFS/overlay behavior, or power-loss
durability. Process-termination tests establish only the observed pre-pivot or
post-pivot `ACTIVE` state under the tested local filesystems; they are not a
power-loss claim.

## Evidence status

The local parser, initializer authority, replay, hostile filesystem, and
process-boundary gates are present at the current implementation head. The
exact literal path-set/manifest/`ACTIVE` replay test pins workspace revision
`sha256:88181393a052db1605145236cd3fd2e7f3f24256ce0c90d7968d939fc6a4c4ef`.
The exact-head hosted matrix remains pending. This document makes no completion
status promotion.
