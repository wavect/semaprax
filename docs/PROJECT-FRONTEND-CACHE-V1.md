# Project Frontend Cache v1

Audience: embedding-host authors, compiler contributors, and reviewers.

Status: code and regression evidence authored, **unrun**. Tests, compiler,
interpreter, executable, and long quality gates were deliberately skipped under
the user's instruction. No verified equivalence, latency, memory, or complete
incremental-compiler claim follows from this change.

This opt-in invocation-owned cache actually avoids the lexer/parser and
canonical formatter calls for unchanged, unaffected modules. It retains
compiler-created source ASTs, then clones them for a fresh build. Every module
still undergoes semantic resolution, imported-stub validation, cross-file
identity/import/dependency checks, complete linking, graph construction, and
the ordinary manifest-selected Project profile admission. There is **no checked
HIR reuse**. This is incremental frontend reuse, not incremental semantic
verification or a persistent compiler cache.

## Host API

```rust
ProjectFrontendSource::new(path: &str, source: &str)
    -> Result<ProjectFrontendSource, Vec<Diagnostic>>
ProjectFrontendCache::new() -> ProjectFrontendCache
cache.build(&ProjectManifest, &[ProjectFrontendSource])
    -> Result<ProjectFrontendBuild, Vec<Diagnostic>>
```

Source construction bounds and owns the supplied strings; it does not admit
their syntax, canonical form, path inventory, types, or effects. The manifest
owns the exact source path set. `build` performs full admission from caller-owned
bytes and returns an immutable `Arc<ProjectRevision>` through `revision()` or
`into_revision()`, plus a deterministic work report through `to_json()`.
No filesystem path is opened. There is no singleton, serialized-AST constructor,
cache root, source publication, interpreter call, or target execution.

```rust
ImageWorkspace::with_frontend_cache(Arc<ProjectSemanticImage>)
    -> Result<ImageWorkspace, Vec<Diagnostic>>
workspace.refresh_owned_sources(&ProjectManifest, &[ProjectFrontendSource], expected_old_image)
    -> Result<ImageRefreshReport, Vec<Diagnostic>>
```

Initial opt-in priming performs a full cold build and exact-compares its source,
manifest, Workspace, and graph facts with the retained image. Priming has a real
cost; a previously admitted image does not contain the cached source ASTs.
`refresh_owned_sources` then consumes proposed source bytes directly, without
requiring a preliminary cold build of the changed revision. After successful
admission it derives the next image, or reuses the old image `Arc` when every
revision fact is identical. Even that unchanged-source route runs the full
semantic/link/profile pipeline. Cache and image replacements commit only after
the complete bounded report is ready; rejection leaves both previous values
unchanged.

The existing `ImageWorkspace::new` and ordinary `refresh` route retain their
legacy output bytes and behavior. On an opted-in workspace, `refresh` can also
use cached parsing to independently rebuild a changed supplied revision, but
the caller may already have spent cold-build work obtaining that revision.
The owned-source API is the route that avoids that redundant preliminary work.

## Exact keys and conservative invalidation

One cache retains a single successful Project context. The context binds the
compiler package name/version, explicit compatibility identity
`semaprax.project-frontend-canonical-ast.v1`, and the entire canonical Project
manifest, including its profile and source inventory. Compatibility is not a
compiler-binary identity. A different context invalidates all entries.

Within a context, a hit requires an identical path and **exact canonical source
bytes**, including the source module identity and span-producing spelling. A
hash alone cannot admit a hit. Changed, newly present, and removed paths seed
invalidation. The transitive reverse closure of old authenticated module imports
also invalidates consumers. A newly introduced import necessarily belongs to a
changed source and cannot evade this rule. Unaffected modules may reuse their
ASTs. All new imports, signatures, effects, named types, cycles, identity
collisions, and profile constraints are independently validated afterward by
the unchanged full pipeline; the invalidation set is never semantic authority.

Cache entries become reusable only after the complete Project and its work
report succeed. A parser success followed by type, link, profile, graph, or
report failure does not install any entry. Private staging may share existing
immutable ASTs with the previous cache. There is no cache adoption from user
JSON, serialized HIR, an Image Store receipt, or disk artifacts.

## Actual work and bounds

The recursively key-sorted compact JSON report has one final LF, schema
`semaprax.project-frontend-cache-work.v1`, and a 65,536-byte maximum. It contains
compiler compatibility, `context_digest`, `project_revision`,
`manifest_context_reset`, sorted `invalidated_sources`, `work`, `retained`,
`limits`, and explicit nonclaims. The context digest is SHA-256 of
`semaprax.project-frontend-cache.context.v1\0`, the little-endian `u64` context
byte length, and the exact context bytes. It is descriptive, not admission
authority.

`work` records successful-build module parser calls, canonicalizer calls, cache
hits/AST clones, parsed/reused source bytes, and modules resolved. Parser and
formatter counters are incremented at the calls they describe; a cache hit
actually bypasses both calls. `checked_HIR_reused` is always zero and both full
cross-file checking and full linking/profile admission are marked mandatory.
These are operation counts, not elapsed-time benchmarks or proof of lower
overall memory usage. Canonical source bytes are still charged into the frozen
graph builder accounting on hits, preserving ordinary cold graph/Image bytes.

The cache retains at most 16 modules, at most 16 MiB of exact source bytes, and
an existing conservative AST-clone/HIR construction prebound of at most 16 MiB.
The latter is a compiler construction charge, **not** an allocator or RSS bound.
Cache hits deep-clone the AST for the fresh build; ordinary parsed sources,
linked HIR, report values, and staged old/new generations can coexist under
their existing separate admission bounds. No aggregate process-heap bound or
constant-time refresh is claimed. Cache eviction is single-context replacement,
not an unbounded revision history or background GC.

Opted-in source refresh adds `frontend_work` to the existing refresh report and
labels its compiler work `cached_parsing_full_semantic_link_and_profile_rebuild`.
The outer report's old/new union-based invalidation describes changed image
facts; the nested report describes the actual cache invalidation and work. The
ordinary source-backed [Image Store](SEMANTIC-IMAGE-STORE-V1.md) remains a cold
source-rebuild store and is unaffected by this in-memory optimization.

## Diagnostics and evidence still required

`SPX-G255` reports invalid cache requests or a raw-source refresh without opt-in;
`SPX-G256` reports source/cache/report capacity; `SPX-G257` rejects disagreement
with an independently admitted revision. Existing `SPX-G251` guards expected
image identity. Ordinary source, canonicalization, graph, type, link, and profile
diagnostics propagate unchanged. Oversized inputs are bounded before internal
copies, and diagnostics do not echo submitted source bytes.

Implementation owners are `src/project/incremental.rs`, the narrow preflight
and graph-build seams, `src/project/build.rs`, and `src/project/image_store.rs`.
Authored regressions in `tests/project_frontend_cache_v1.rs` compare cold and
cached source/Workspace/graph/Image bytes; assert actual parser/formatter call
counts for unchanged, leaf, and provider changes; require complete signature
rechecking; check noncanonical/duplicate-source rejection and cache rollback;
reset on manifest changes; and exercise direct owned-source image refresh and
stale/failed refresh preservation.

Those regressions still need execution. Checked-HIR reuse, dependency-sensitive
incremental typechecking, warm cross-process loading, target work reuse,
incremental candidate application, and measured latency/memory benchmarks remain
outside this slice and are not completion claims.
