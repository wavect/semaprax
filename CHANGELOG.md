# Changelog

All notable changes to SEMAPRAX are documented here.

## 0.2.0 — 2026-08-07

- Added resource declarations and explicit `own`, `borrow`, and `shared` boundaries.
- Added stable resource identities and atomic resource/type-boundary renames.
- Added straight-line move analysis with use-after-move and illegal-transfer diagnostics.
- Added lexical `let` bindings, typed `if/else`, and conservative control-flow ownership joins.
- Distinguished definitely moved resources from resources moved on only some paths.
- Exposed resource nodes and parameter ownership in the semantic graph.
- Upgraded the graph to v2 with deterministic revision-scoped expression and binding nodes.
- Added structured contract graphs and contract dependencies to bounded agent context.
- Added a direct WebAssembly core backend and generated browser package.
- Preserved checked `i64` arithmetic through audited WebAssembly host imports.
- Added the authoritative full-goal completion matrix.
- Verified the compiler, native executable, and WebAssembly package on macOS, Linux, and Windows CI runners.
- Made native expression evaluation explicitly left-to-right instead of inheriting C's unspecified call-argument order.
- Hardened public backends to reject unverified programs with diagnostics instead of panicking.
- Added repository agent guidance, evidence rules, MSRV/package gates, and a documented Graphify adoption decision.
- Added a fail-closed resolved HIR with persistent nominal/call identities, deterministic lexical place identities, and centralized type facts/layout keys.
- Migrated native and Wasm semantic lowering to validated HIR and added malformed-HIR cross-backend rejection parity.
- Upgraded the semantic graph to v3 backed by validated HIR, with resolved declaration/value/type identities, centralized type facts, explicit identity origin, fail-closed public APIs, and bounded-context frontier metadata.
- Upgraded the semantic graph to v4 with persistent record and field nodes, resolved construction/projection references, recursive field-type context closure, and fail-closed graph integrity checks.
- Replaced FNV revision tokens atomically across graphs, semantic patches, CLI output, and `semaprax.web.v2` manifests with domain-separated SHA-256 content addresses.
- Split source verification behind a compatibility facade and additive HIR analysis API while freezing complete ordered diagnostic JSON behavior.
- Added canonical record declarations, construction, projection, persistent field identities, deterministic field diagnostics, recursive facts/layout keys, and by-value recursion rejection.
- Upgraded the semantic graph to v4 with record/field nodes and stable constructor/projection references; native and Wasm record builds fail closed pending aggregate cleanup and layout support.
- Added prefix-aware ownership for resource-containing record fields, preserving disjoint siblings while rejecting definite and conditional partial-place reuse in both source verification and hostile-HIR replay.
- Fixed canonical formatting of record constructors in contracts and `if` conditions so parse-format-parse remains valid.
- Hardened semantic resource renames so record initializer expressions cannot be mistaken for type annotations.
- Published design-only RFC 0003 for exactly-once cleanup, logical resource imports, and a shared native/Wasm status-and-out ABI; executable cleanup remains gated on its conformance evidence.
- Closed the pre-cleanup backend gap by rejecting bare resource modules with `SPX-B104`/`SPX-W111`; record diagnostics retain precedence when both declaration kinds are present.
- Implemented RFC 0003 phase 1 with mandatory persistent trivial/imported resource lifecycles, declaration-only interface/import contracts, recursive lifecycle-effect authority checks, and hostile-HIR validation while retaining fail-closed resource execution.
- Upgraded the semantic graph to v5 with resource-drop, interface, and logical-import nodes plus lifecycle-aware bounded context closure and exact snapshots.
- Migrated legacy resource fixtures explicitly and extended atomic resource renames through import parameter types without rewriting lifecycle IDs or logical keys.
- Added a mandatory, deterministic `CleanupInventory` to resolved functions, cataloging owned droppable storage and exact nested resource-leaf flags while independently rejecting hostile inventory mutations before backend gates.
- Corrected the proposed cleanup-plan schema to require atomic call commits, exact single-flag finalizer guards, explicit entry liveness, edge-based cleanup continuation, and sticky failure-status identities; executable cleanup remains unimplemented.
- Implemented RFC 0003 phase 2 with a mandatory target-neutral cleanup CFG on every resolved function, covering all current HIR expressions, lexical exits, guarded reverse finalization, caller-owned argument epochs, atomic call commits, checked and contract failures, partial record construction, whole-value normalization, and scalar/owned result publication.
- Added independent cleanup-plan reconstruction after core HIR and inventory validation, plus focused deterministic and hostile-HIR tests that preserve `SPX-H006` precedence across native and Wasm consumers.
- Upgraded the semantic graph to v6 with complete tagged cleanup plans per selected function while retaining the canonical source revision algorithm.
- Added public `semaprax.status.v1` normalized-status types, exact compiler-owned contract/arithmetic mappings, and a bounded context-local immutable status arena; token zero remains success and no physical token is serialized. This is protocol/runtime groundwork, not a backend status ABI implementation.
- Added public `semaprax.conformance-trace.v1` semantic event/result types and deterministic canonical JSON for ownership transitions, calls/imports, frame-local failure selection, infallible finalization, and result publication.
- Added independent attached-plan coverage and exhaustive current-CFG replay plus a scenario-driven single-frame reference executor with guarded cleanup, sticky normalized failure, exact trace emission, and explicit caller out-slot publication state. Recursive calls, callable imports, and native/Wasm instrumentation remain unimplemented, so no backend conformance is claimed.
- Documented strict status/trace schema rejection, compatibility, cache-binding, event-order, and no-physical-data rules in [Conformance trace v1](docs/CONFORMANCE-TRACE-V1.md).

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
