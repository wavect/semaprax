# Image Workspace Frontend Cache v1

Audience: embedding hosts, agent authors, compiler maintainers, and reviewers.

Status: implementation and focused executable regressions authored, **unrun**.
Tests, compiler/interpreter execution, and long local gates are intentionally
skipped under the user's instruction. This is not verified completion or
measured performance evidence.

`VNextSession::open_with_frontend_cache(&absolute_manifest, policy)` enables
invocation-owned parsed/canonical AST reuse for v5 live refresh. The existing
`open` constructor remains cold. The additive
`VNextSession::open_with_semantic_cache(&absolute_manifest, policy)` additionally
retains compiler-created checked modules; `open_with_frontend_cache` keeps its
existing AST-only behavior. `VNextPolicy`, protocol method authority,
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
Unaffected ASTs are cloned rather than parsed and canonicalized again. On the
AST-only route every module still passes resolution and checked HIR is rebuilt.
The semantic-cache route retains previously checked module HIR only when the
complete compiler-created synthetic AST, including imported stubs, matches
exactly and the source/dependency/context invalidation rules permit reuse.
Changed modules and their invalidated reverse import closure are resolved again.
Both routes still authenticate fresh source bytes and perform HIR validation,
full cross-file checks, linking, and complete Project profile admission. See
[Project Semantic Cache v1](PROJECT-SEMANTIC-CACHE-V1.md) for the checked-module
matching and accounting contract. Neither route loads serialized or untrusted
HIR, retains a disk cache, performs cross-process warm HIR recovery, or bypasses
backend admission. Bounded checked-module reuse is not general incremental
semantic verification.

Only cached `workspace/refresh-preview` and `workspace/refresh` responses add an
optional `frontend_work` object. The AST-only route keeps the existing
`semaprax.project-frontend-cache-work.v1` schema and `checked_HIR_reused: 0`.
The explicitly selected semantic route uses the distinct
`semaprax.project-semantic-cache-work.v1` schema with compiler compatibility
`semaprax.project-checked-module-hir.v1`; its `modules_resolved` counts actual
resolver calls and `checked_HIR_reused` counts actual checked-module hits. Cold
responses omit the object and keep their existing bytes. Both schemas report
actual parser/canonicalizer calls, source bytes, AST clones, invalidated source
paths, and mandatory full cross-file/link/profile work. The
existing limits remain 16 modules, 16 MiB aggregate source bytes, 16 MiB AST
construction prebound, and 64 KiB for the work report. These are logical bounds,
not allocator/RSS measurements or runtime speed claims. Staging retains old
and proposed state concurrently and response rendering has its own transport
bound; overflow fails closed.

AST and optional checked-module entries are staged alongside the proposed authenticated snapshot and
image. Preview drops the staged cache and never revives an absorbing stale
snapshot. An unexpected revision, malformed source, failed semantic admission,
changed manifest, response overflow, or final authentication failure leaves
the current cache/image/registry unchanged. Successful explicit refresh renders
and rechecks before swapping all retained state, retaining historical complete
candidates and clearing drafts/attempts exactly as the ordinary v5 route does.
Startup-only Git host attachment and approval guards remain unchanged.

`tests/image_protocol/workspace_frontend_cache_v1.rs` authors cold identity/discovery
equivalence, zero-parser warm refresh, leaf/provider invalidation, preview and
failed-refresh rollback, full semantic rejection, stale-session recovery, and
physical hardlink rejection despite exact cached source bytes. Existing cache,
v5 registry lifecycle, and publication tests remain additional unrun evidence.
No latency, throughput, memory, or concurrent-refresh benchmark is claimed.

The CLI's closed host-policy v4 adds a required `semantic_cache` boolean to all
v3 fields. It requires `frontend_cache: true` when enabled. Older policy versions
reject the new field rather than silently accepting it. This is a host startup
selection; there is no cache toggle in the protocol or change to the Git
startup-only attachment/approval guard.

A separate [persistent cache](PERSISTENT-SEMANTIC-CACHE-V1.md) now supplies
`open_with_retained_semantic_cache` through authenticated host-policy v5 entry
selection. The constructors described above still create in-process caches;
none implicitly opens a root or trusts submitted HIR. The persistent route
has its own key-custody and compiler-installation trust contract.
`tests/workspace/session_semantic_cache_cli.rs` additionally authors unchanged
refresh checked-HIR reuse, cold/AST/semantic image and discovery equivalence,
old-version rejection, strict boolean/dependent selection, and RPC override
rejection. Its direct semantic-session regression also authors absorbing source
drift, preview and wrong-revision rollback, one-module resolution/two-module
reuse on successful recovery, and all-module reuse only after adoption. These
regressions also remain unrun.
