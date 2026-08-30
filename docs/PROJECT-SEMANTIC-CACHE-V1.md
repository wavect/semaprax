# Project Semantic Cache v1

Status: Partial; implementation and focused regression evidence authored, unrun.

Audience: compiler contributors, embedding hosts, and semantic workspace agents.

This opt-in invocation-owned cache retains compiler-checked module HIR alongside
the source AST cache. A checked-module hit skips that module's `hir::resolve`
call and clones its checked HIR. It reruns HIR validation and the complete
cross-file, import-stub, linking, graph, and Project-profile admission gates.
This is bounded module reuse, not a general incremental compiler or a proof
that source edits preserve behavior.

## Host API and compatibility

`ProjectFrontendCache::new_with_semantic_cache()` selects checked-module reuse.
`is_semantic_cache_enabled()` reports that choice. The existing `build(manifest,
sources)` API consumes bounded canonical source proposals and returns the
admitted immutable Project revision plus descriptive work counters.

`ProjectFrontendCache::new()` and `Default` retain their existing AST-only
behavior, frontend report schema, and zero checked-HIR reuse. No caller silently
changes semantic admission mode. Both constructors are invocation-owned and
open no filesystem root or grant source, test, execution or publication
authority. The separate [persistent cache](PERSISTENT-SEMANTIC-CACHE-V1.md)
can now restore compiler-sealed HIR through a host-selected authenticated store;
there is still no public untrusted HIR constructor or implicit cache discovery.

`ImageWorkspace::with_semantic_cache(image)` independently primes a checked-module
cache against an already admitted image. `refresh_owned_sources` then admits
proposed canonical source directly without a preliminary cold changed-revision
build. An unchanged image can keep its existing `Arc`; invalid proposals preserve
both the previous image and cache. Host sessions and their startup policy are
separate adapters over this library behavior.

## Exact reuse and invalidation

The cache binds compiler package name/version, semantic-cache compatibility,
and the entire canonical Project manifest. Any context change resets all
entries. Compatibility is not a compiler binary fingerprint.
The separate persistent-store envelope additionally binds the compiler
executable bytes under its explicit trusted static-installation contract.

Changed, added, and removed source paths seed the existing conservative reverse
import invalidation. Every old transitive consumer of a changed provider is
invalidated even when its own source text did not change. A new import belongs
to an edited module and cannot become an unchanged hit. Independent unaffected
modules can still reuse checked HIR.

A hit additionally requires exact equality of the synthetic `Program` presented
to the resolver, including authored declarations, bodies, identities, spans,
and imported stubs. Source hashes or signature summaries alone cannot establish
that equality. A provider's signature or an importing module's alias/binding
change therefore cannot reuse stale resolved calls. Module body edits cannot
reuse their own previous checked HIR. Reuse does not authorize caller-supplied
AST/HIR or bypass the independent retained-HIR checks.

The freshly validated checked clone is charged the resolver's retained bounded
output consumption so cache hits preserve graph-builder accounting and cold
graph/image bytes. Cloning can shrink loan-plan vector/string capacities;
the original aggregate loan-plan charge is retained and any clone-capacity
difference is restored before existing graph filtering/accounting. The
nonempty-loan regression checks this path, including a range and sibling views
of owned bytes, instead of relying only on empty scalar loan plans.
This charge is construction accounting, not a measurement
of allocator use or peak process memory. Cold and warm routes still rebuild
the Project's linked representation and graph; neither promises constant-time
refresh or unchanged target artifacts after a source edit.

Staged entries become reusable only after the whole build and bounded work
report succeed. A failure after some modules have resolved does not commit
those partial entries. Restoring the original sources after a rejected proposal
can reuse the previous successful cache.

## Work report and limits

Semantic mode emits `semaprax.project-semantic-cache-work.v1`, with compatibility
`semaprax.project-checked-module-hir.v1`. It preserves the frontend report's
field structure but uses this separate schema so the old frontend contract's
`checked_HIR_reused: 0` remains true.

`modules_resolved` counts actual resolver calls. `checked_HIR_reused` counts
actual checked-module hits. Parser, canonicalizer, and AST reuse counters retain
their existing meanings. `full_cross_file_checks` and
`full_link_and_profile_admission` remain true on hits. Counters describe work,
not benchmark results, behavioral equivalence, or independently executed tests.

The cache retains one successful context under the existing source count,
source byte, and compiler-construction bounds. Checked retention additionally
uses `MAX_PROJECT_CHECKED_MODULE_CACHE_PREBOUND` (16 MiB), enforced against the
whole synthetic AST/HIR construction estimate before checked clones are made.
This is separate from the existing 16 MiB source and AST-construction limits;
it is not an allocator byte limit. Reports retain the 65,536-byte
bound and deterministic sorted-key compact JSON with one final LF. Source ASTs,
synthetic resolver inputs, checked module clones, newly linked HIR, and staged
generations may coexist. No aggregate process-heap or RSS bound is claimed.
Ordinary frontend capacity/grammar and compiler semantic diagnostics propagate;
cache policy cannot make an inadmissible program valid.

## Authored evidence

`tests/project_semantic_cache_v1.rs` covers unchanged warm counters and exact cold
source/graph/image identity; AST-only compatibility; leaf, provider, and private
body invalidation; changed import signatures/bindings; matching cold rejection
diagnostics; failed-build rollback; manifest reset; and owned-source image
refresh with failed-proposal rollback.

These tests were not run. No compiler, interpreter, generated client, target,
or long quality gate was executed. Hosted executable evidence and broader
incremental-verification performance work remain required before stronger
completion or speed claims.
