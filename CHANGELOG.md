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
- Migrated native scalar calls to the RFC 0003 context/status/out convention: internal contract and checked-arithmetic failures now propagate exact normalized statuses without terminating a SEMAPRAX frame, nested calls retain the same token, and caller result storage is written only after successful postconditions.
- Added an executable strict-Clang native ABI matrix covering scalar success, requires/ensures, all eight arithmetic status codes, left-to-right nested failure propagation, arena shape, and poisoned out-slot preservation while retaining the `SPX-B104` resource gate.
- Fixed status-v1 domain identity at 1–255 UTF-8 bytes without NUL and enforced the same byte rule in public status construction, source/HIR validation, and native arena-owned domain storage.
- Added gated native-resource ABI scaffolding with deterministic stable-ID-derived C wrapper and typed finalizer symbols; this does not enable resource execution.
- Added a fail-closed first-slice cleanup-plan index for direct trivial resources and a checked max-path trace-capacity preflight. Records, imported lifecycles, projections, generics, and every nested call remain rejected behind `SPX-B104` until their executable conformance evidence exists.
- Added an unreachable plan-driven native cleanup C scaffold with exact terminal liveness/status assertions, clear-before-trivial-finalization, owned-result publication checks, and a compiler-owned C binding namespace; executable resource lowering remains gated.
- Added deterministic, strongly typed C wrappers for direct opaque resources and staged resource-aware native signatures while preserving the unconditional `SPX-B104` execution gate.
- Extended the gated native cleanup scaffold to emit real root-frame `transfer`, `select_failure`, trivial-finalization, and `result_commit` events from the classified function identity; strict C11 fixtures cover event order, hostile trigraph sequences, and exact UTF-8 identity bytes.
- Made every persistent semantic identity NUL-free at source and hostile-HIR boundaries, including type/expression/place references and attached cleanup inventory/plan metadata, so C and wire encodings cannot silently truncate or alias identities.
- Added an exact test-only native cleanup conformance lane: a bounded versioned binary decoder and validated-HIR identity materializer compare typed traces and canonical JSON with the independent executor across zero/max opaque payloads, reverse finalization, contract/arithmetic failure, owned publication, failed owned postconditions, O0/O2, ASan, and UBSan. Production resource lowering remains gated.
- Replaced injected native cleanup observations with a typed, cleanup-CFG-synchronized value planner that executes real Boolean contracts, `i64` comparison, checked addition, scalar publication, and owned transfers inside the exact conformance lane. Added portable `i64::MIN` C emission and independent resource-lifecycle/transfer-type coherence checks; the public resource host gate remains closed.
- Added the private host-ownership transaction v1 reference model: linked-runtime-unique non-clone registries, generation/provenance tokens, immutable function contracts, atomic multi-owner ingress, and must-complete typed execution scopes make rejection versus executed success/failure and owned-result publication executable without exposing raw pointers or weakening `SPX-B104`.
- Added a root-frame native trace-storage scaffold with exact compiler-status/event validation and a pre-ownership attachment handshake. Canonically zeroed one-shot contexts, buffers, and event slots use owner/generation checks to reject rebinding, aliasing, double attachment, and capacity underflow before execution.

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
