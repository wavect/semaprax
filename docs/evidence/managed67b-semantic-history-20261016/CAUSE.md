# Actual67b managed wire drift: intended imported-nominal metadata correction

Read-only source investigation; no compiler, test, model, graph refresh, ROOT edit
or allowed-path mutation. Frozen source:67b1b02141f19040fdd1b45bc9bea16dcff5d74e.
The strict planner failure remains valid and preserved.

## Exact cause

Upstream commit **f82cec490db31898d9876645876ae98549611baf**, “Classify imported
nominal graph profiles with authenticated borrowed context,” introduced the
behavior. It entered this branch in actual Main8a merge16c6b7f56b145ed085e4e7964caa3b1716159375
after55e. It is independent of Maina3 ownership_flags changes.

At current `src/workspace_graph/source_schema.rs:455–477` the caller pays and
constructs `nominal_context::Types`, then passes it instead of caller-local
`module.types`. `src/graph/owned_nested_outcome.rs:49–60` now asks
`types.find_in_context` for nominal declarations. The old lookup was local-only.

The captured fixture is exact: `tests/workspace/semantic_graph.rs:39–63` imports
`public.point` from public.provider into public.app. Point is a Record containing
only value:i64. The provider remains graph.v10. The caller does not emit an
independent local copy of that type.

Previously `src/hir/collection_outcome/nested/profile.rs:77–78` could not find
that imported declaration and conservatively returned true, selecting graph.v74
(owning nested outcomes). With authenticated cross-module lookup it finds a
Record; the Variant-only predicate at80–81 returns false. No higher owning
profile is needed, so classification reaches the ordinary graph.v10 base.

`nominal_context.rs:18–47` requires a unique candidate and matching retained
kind/origin/owner/path/module facts. It grants no validation or execution
authority and retains the local emitted view. Missing, foreign and duplicate
context still returns None and retains the conservative fallback.

## Owning contract and existing controls

`docs/ARCHITECTURE.md:2216–2226` explicitly assigns this imported-nominal metadata
selection to the authenticated context adapter. The upstream changelog explicitly
announces this correction. `docs/SEMANTIC-WORKSPACE-V1.md:92–103` requires each
schema tag to be selected from authenticated per-file HIR; the tag is not simply
a monotone maximum of all declared/imported schemas.

`nominal_context/tests.rs:35–73` deliberately compares frozen local-only v74
with actual imported-context v67 for its separate String-record fixture.
`:76–142` retains malformed/missing/foreign/duplicate conservative controls.
Those are source-visible assertions, not a newly executed result from this audit.
The actual scalar Point fixture analogously selects v10, not v67.

## Revision cascade and disposition

Actual55e and67b public wires show only public.app's source_graph_schema changing
v74→v10 among module facts; the provider remainsv10. Source revision/digest and
fixture bytes remain unchanged (independent reviewer confirmed complete fixture
SHAe3c58313d27ef22772716d9f91011c22c9aa22846b5f52fa691c3fde372b593f).

`semantic_workspace.rs:663–689` includes source_graph_schema in exact canonical
manifest bytes. `:808–815`, matching SEMANTIC-WORKSPACE-V1:105–112, hashes domain,
manifest length and full manifest includingLF. Therefore the workspace_revision
439c…→0a324… and subsequent context/review/digest changes are the expected
consequence of real metadata drift. They must not be normalized as budget drift.

Verdict: the source change is intentional and consistent with the current owning
metadata contract, rather than a leaf-ownership regression or changed input.
The older budget-only updater correctly refuses it. Any future pin migration
must explicitly account for this exact source-bound semantic metadata change
and retain complete historical wires; this audit authorizes no checker widening,
revision normalization, pin application or issue closure.
