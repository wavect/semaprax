# Source-backed Semantic Image Store and Refresh v1

Audience: embedding-host authors, agent builders, compiler contributors, and reviewers.

Status: implementation and regression evidence authored, unrun. Compiler,
interpreter, test, executable, and long quality gates were deliberately skipped
under the user's instruction. This is not verified cold-load, durability,
performance, or full-programme completion evidence.

This additive lifecycle persists an image's exact canonical Project inputs
through the existing [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md).
Loading authenticates those immutable inputs, performs the ordinary complete
Project source rebuild, and derives the image again. No serialized HIR, typed
index, graph, or image JSON is trusted as compiler input. Persistent storage
therefore retains the source subject, not a warm cross-process compiler cache.
Existing Semantic Image v1 bytes and nonclaims remain unchanged.

## Host-selected persistence

```rust
persist_semantic_image(root, &image, expected_image_digest)
    -> Result<ImageStoreReceipt, Vec<Diagnostic>>
load_semantic_image(root, receipt_bytes, expected_image_digest)
    -> Result<Arc<ProjectSemanticImage>, Vec<Diagnostic>>
```

Both functions are explicit host operations. They do not discover, create, or
adopt an ambient cache root. On supported Unix hosts, the host provides an
existing absolute normalized real directory owned by the effective user with
exact mode `0700`, along with the existing store's exclusive-root and ancestor
mutation guarantees. Secure handle-relative reads/writes, locking, exact file
inventory, no-follow checks, source reconstruction, bounded entry counts, and
atomic no-replace publication are owned entirely by the existing store.
Unsupported hosts retain its fail-closed behavior; this API does not silently
select the separate Windows private-host route.

A repository-local host can explicitly select a directory named
`.semaprax-images`, already excluded by the repository's
`**/.semaprax-images/` ignore rule. Other host-selected roots and saved receipt
paths must likewise be excluded from Git by that host. This API performs no
repository discovery, ignore-file mutation, filesystem cleanup, or GC.

Persistence checks the expected image digest, prepares the complete bounded
receipt before any store effect, and invokes `project_revision_store::persist`
for the image's retained revision. It compares the published locator facts with
the expected facts. Duplicate entries reject under the existing no-adoption
rule; they are not overwritten or treated as successful publications. Store
errors and post-publication uncertainty retain their original meanings. A
receipt is neither reusable authority nor proof that the entry still exists.

The on-disk tree is exactly the existing store entry: canonical `entry.json`,
Project manifest, Workspace manifest, and declared source files under the
content-addressed entry digest. There is no additional `image.json`, HIR blob,
index file, or mutable image pointer. The caller receives the image receipt
separately and may retain it outside the strict store inventory.

## Receipt and cold load

`ImageStoreReceipt` exposes `to_json`, `receipt_digest`, `entry_digest`,
`image_digest`, and `project_revision`. Its compact, recursively key-sorted JSON
has one terminal LF, a maximum size of 8,192 bytes, and schema
`semaprax.semantic-image-store.v1`. Its exact fields are:

- `schema`, `compiler`, `image_schema`, `image_digest`, and `image_bytes`;
- `project_revision`, `workspace_revision`, and `project_graph_digest`;
- `revision_store` with the existing entry schema and `entry_digest`;
- the fixed nonclaim list.

Compiler metadata contains package name, package version, and the explicit
Semantic Image serialization compatibility identity. It does not claim a
compiler-binary identity. The receipt digest is SHA-256 over
`semaprax.semantic-image-store.receipt.v1\0`, little-endian `u64` exact byte
length, and the complete receipt bytes including LF, rendered as lowercase
`sha256:` text. It is returned separately rather than embedded recursively.

Load checks the caller's expected image digest, the receipt bound, exact closed
schema and compiler compatibility, bounded canonical digest fields, image
length, and canonical receipt encoding before opening an entry. Unknown keys,
duplicate keys, alternate JSON encodings, changed compatibility, or wrong
expected image identity fail closed. A root/path cannot be supplied inside the
receipt.

The existing store then independently reads and rebuilds the exact canonical
source subject. Image derivation must reproduce the expected image digest and
byte length, and re-identifying that derived image must reproduce every
canonical receipt byte. The image digest binds the exact canonical image bytes;
the original serialized image is neither stored nor deserialized. Missing or
corrupted entries reject without reconstructing data from a working copy or
trusting a previously retained image.

Manual edits in the original source checkout do not alter an older immutable
store entry. Loading that entry intentionally yields its historical revision,
not a claim about current disk freshness. Hosts requiring current checkout
state must independently admit those current sources.

## Retained workspace refresh

```rust
ImageWorkspace::new(Arc<ProjectSemanticImage>) -> ImageWorkspace
ImageWorkspace::image() -> &Arc<ProjectSemanticImage>
ImageWorkspace::refresh(new_admitted_revision, expected_old_image_digest)
    -> Result<ImageRefreshReport, Vec<Diagnostic>>
```

The workspace retains no store root or filesystem authority. The caller
supplies an independently admitted `Arc<ProjectRevision>` and the exact old
image expectation. For an unchanged Project revision, refresh compares the
complete retained canonical manifest, source, Workspace, and graph facts and
reuses the original image `Arc`. The caller may already have spent compilation
work obtaining its fresh revision; this reuse does not claim that work was
avoided.

For a changed revision, refresh reconstructs the full Project through
`build_owned` from the supplied canonical manifest and sources, exact-compares
the rebuilt revision/graph/source facts, and derives a new image. It does not
splice cached HIR, skip source verification, or claim incremental compilation.

The deterministic report identifies changed source paths, then computes the
transitive reverse dependency closure using the union of old and new explicit
function/type import edges. Both graph versions matter when imports are added
or removed. Manifest or source-inventory changes conservatively invalidate all
old/new paths. Lists sort by path. `unchanged_source_facts` describes paths
outside that invalidation set; it does not assert reuse of their HIR or a
semantic equivalence proof.

The report schema is `semaprax.semantic-image-refresh.v1`, bounded to 65,536
bytes. It binds old/new image and Project digests, changed/invalidated paths,
manifest/inventory flags, the invalidation basis, `image_arc_reused`, and
`compiler_work` as either `retained_image_arc_reused` or
`complete_source_rebuild_and_image_derivation`. Its separate digest uses
`semaprax.semantic-image-refresh.report.v1\0` with the same byte-length and
exact-canonical-JSON construction as receipts. Only after the entire report is
representable does refresh replace the retained image. Failure leaves the
previous `Arc` unchanged.

`SPX-G249` covers malformed receipt/selector/compatibility facts, `SPX-G250`
covers lifecycle report/input capacity, and `SPX-G251` covers stale image or
replay disagreement. Existing store, source-admission, and image-derivation
diagnostics retain their owning meanings.

## Authored evidence and limits

`src/project/image_store.rs` owns this lifecycle. Five authored, unrun tests in
`tests/semantic_image_store_v1.rs` cover unchanged `Arc` reuse, manual source
and manifest refresh, reverse-import invalidation, stale/invalid-source
preservation, cold rebuilding after dropping retained state, working-copy
independence, duplicate persistence, corruption/deletion, and hostile receipt
or compiler-locator substitution. Disk cases are limited to the existing
supported Unix store platforms.

This slice does not provide persistent HIR/index reuse, warm cross-process
compilation, selective module recompilation, automatic cache discovery,
eviction, crash recovery, new durability guarantees, signatures/approval,
current-source freshness, target execution, or evidence that the full
graph-operational persistence/performance programme is complete.
