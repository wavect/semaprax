# Image Workspace Frontend Cache v1

Audience: embedding hosts, agent authors, compiler maintainers, and reviewers.

Status: implementation and focused executable regressions authored, **unrun**.
Tests, compiler/interpreter execution, and long local gates are intentionally
skipped under the user's instruction. This is not verified completion or
measured performance evidence.

`VNextSession::open_with_frontend_cache(&absolute_manifest, policy)` enables
invocation-owned parsed/canonical AST reuse for v5 live refresh. The existing
`open` constructor remains cold. `VNextPolicy`, protocol method authority,
capability reports, image bytes, and image identity do not change. Requests
cannot enable, disable, seed, or deserialize a cache.

The initial cached load reads each declared source through the existing Project
filesystem authority and builds the initial cache during its single frontend
pass. It does not first build a cold snapshot and then rebuild to prime a cache.
Every refresh independently authenticates the same manifest, directory chain,
regular files, physical identities, permissions, bounded bytes, and declared
paths before handing source bytes to the frontend. Source-exact cache hits do
not bypass hardlink/symlink checks or held-input rechecks.

Cache keys and accounting follow [Project Frontend Cache v1](PROJECT-FRONTEND-CACHE-V1.md):
exact source bytes plus canonical manifest and compiler compatibility facts.
Changed modules and the old reverse import dependency closure invalidate ASTs.
Unaffected ASTs are cloned rather than parsed and canonicalized again. Every
module still passes source checks, resolution, cross-file linking, and full
Project profile admission; checked HIR is rebuilt. There is no disk cache,
cross-process warm HIR, incremental semantic verification, or backend bypass.

Only cached `workspace/refresh-preview` and `workspace/refresh` responses add an
optional `frontend_work` object, using the existing
`semaprax.project-frontend-cache-work.v1` schema. Cold responses omit it and keep
their existing bytes. It reports actual parser/canonicalizer calls, source bytes,
AST clones, invalidated source paths, and full resolution/admission work. The
existing limits remain 16 modules, 16 MiB aggregate source bytes, 16 MiB AST
construction prebound, and 64 KiB for the work report. These are logical bounds,
not allocator/RSS measurements or runtime speed claims. Staging retains old
and proposed state concurrently and response rendering has its own transport
bound; overflow fails closed.

Cache entries are staged alongside the proposed authenticated snapshot and
image. Preview drops the staged cache and never revives an absorbing stale
snapshot. An unexpected revision, malformed source, failed semantic admission,
changed manifest, response overflow, or final authentication failure leaves
the current cache/image/registry unchanged. Successful explicit refresh renders
and rechecks before swapping all retained state, retaining historical complete
candidates and clearing drafts/attempts exactly as the ordinary v5 route does.
Startup-only Git host attachment and approval guards remain unchanged.

`tests/image_workspace_frontend_cache_v1.rs` authors cold identity/discovery
equivalence, zero-parser warm refresh, leaf/provider invalidation, preview and
failed-refresh rollback, full semantic rejection, stale-session recovery, and
physical hardlink rejection despite exact cached source bytes. Existing cache,
v5 registry lifecycle, and publication tests remain additional unrun evidence.
No latency, throughput, memory, or concurrent-refresh benchmark is claimed.
