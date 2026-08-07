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

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
