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
- Split private native host contracts into deterministic authority-free templates and runtime binding. Resource preflight now derives and discards each template from its exact already-admitted cleanup/value evidence without replanning; complete ordered scalar/resource metadata, exact same-type owner results, lifecycle identity, module ABI and function-template fingerprints, mismatched-evidence rejection, binding-instance-distinct process-local authority, cross-ABI binding rejection, and internally observed thread affinity at binding and synchronous registry execution are unit-tested while public execution remains gated.
- Corrected the native conformance probe to consume the value planner's exact owned-result parameter/owner ordinal instead of choosing the first same-typed input. The sanitizer-backed corpus now proves returning the second of two identical resource types with both distinct and identical opaque payloads, plus reverse cleanup and no result publication when its precondition fails.
- Added a private descriptor-only physical native ABI stage derived solely from sealed admitted host templates. Its canonical pointer-free wire binds explicit schema, target, semantic/physical module, function, ordered scalar/resource/lifecycle, and exact result identities. A host-only provider compile-guards every encoded target property; strict separate C11/C++ translation units plus a real shared-library/export/dynamic-consumer test verify the deterministic getter as the sole export. Compiler preflight discards every staged artifact, exports no callable owner API, creates no runtime authority, and leaves the exact public `SPX-B104` gate unchanged.
- Added a disconnected private native capability-token codec with an exact 64-byte canonical envelope and full RustCrypto HMAC-SHA256 tag. Pinned audited crypto, a published RFC 4231 vector, independently reproduced owner/result full-token goldens, every-bit/arbitrary-byte/length/structure hostility, exact function-template result scoping, function-independent owner scoping, cross-context rejection, stale/max-generation checks, and explicit entropy/module-lifetime/linearity nonclaims establish authentication mechanics without creating compiler or runtime authority.
- Added a private OS-backed native capability authority without connecting it to compiler preflight or any export. Exactly pinned `getrandom` 0.4.3 supplies one fail-closed seed for the secret, nonzero random epoch, and opaque thread-binding nonce; kind-specific non-formatting credentials seal immutable module/resource context and reject every operation off the actually captured Rust thread. Test-only deterministic entropy, independently reproduced authority goldens, error/zero/context/thread hostility, MSRV, and desktop OS smoke preserve the public `SPX-B104` gate while module retention, ledger integration, fork safety, and callable ownership remain blocked.
- Added a private fake-backed `NativeModuleLease` topology and required the native capability authority plus every staged owner/result credential wrapper to retain the exact allocation instance. Tests cover equal-fingerprint instance separation, process-incarnation rejection, one-way draining, lease-derived fingerprints, cross-instance rejection despite equal bearer bytes, drop-order retention, concurrent final release, and absence of retention cycles. There remains no production constructor, platform loader handle, code-identity admission, physical pin/unload protocol, ledger integration, callable export, or change to `SPX-B104`.
- Added an unpublished `semaprax-native-loader` workspace quarantine around the unavoidable trusted-library load, fixed-getter lookup, and bounded descriptor-read/compare unsafe edge. Its same-thread opaque explicit-retain lease, exact-pinned `libloading` 0.8.9, compile-fail trait checks, workspace-wide gates, and real Linux/macOS fixtures remain disconnected from the compiler, authority, and public adapter. Malicious modules, same-image provenance, hardened Windows loading, physical unmapping, quiescence, and code authenticity remain blockers.
- Added a blocking, immutable-action-pinned `cargo-deny` CI gate for the complete native/mobile/Wasm dependency graph: RustSec advisories, unapproved licenses, duplicate or wildcard versions, Git dependencies, and registries outside crates.io fail the build with no advisory exceptions.
- Added a root-frame native trace-storage scaffold with exact compiler-status/event validation and a pre-ownership attachment handshake. Canonically zeroed one-shot contexts, buffers, and event slots use owner/generation checks to reject rebinding, aliasing, double attachment, and capacity underflow before execution.
- Added the unpublished `semaprax-native-host` physical ownership stage. It strictly decodes the compiler-derived descriptor, retains the exact real loader instance through its same-thread OS-seeded authority and opaque owners, authenticates owner/result credentials, connects the private ownership ledger, preserves owners on precommit rejection, rotates owned results, and gates new work after draining. Linux/macOS real-loader fixtures exercise that plumbing; Windows has compile coverage only. The loaded provider still exposes only its descriptor getter, trusted Rust closures stand in for generated callable code, and compiler resource builds retain `SPX-B104`.
- Added the first narrow public `semaprax.wasm-owned.v1` Core Wasm execution path for one direct trivial-resource identity. Generated adapters consume replay-validated terminal cleanup order, stage owner handles atomically, normalize contract/arithmetic/adapter status records, preserve poisoned result storage on failure, rotate owned results, and reject excluded shapes with `SPX-W111`. The generated JavaScript keeps host imports private, binds calls to exact generated metadata and SHA-256-authenticated Wasm bytes, rejects non-canonical ABI arguments, checks result ranges before ownership commit, and uses one-shot trusted adoption tickets; Node tests cover the admitted slice. This is not WebAssembly Component resources or production native-host conformance.
- Added `semaprax.semantic-event-dictionary.v1`, which assigns deterministic
  nonzero ordinals to exact semantic event shapes. Generated cleanup C and the
  real Wasm owned adapter emit those ordinals from their executed control flow;
  the host-side materializer rejects zero or unknown ordinals without inferring
  or repairing events.
- Unified the authoritative direct-trivial-resource conformance corpus at 14
  named scenarios. Native generated C at O0/O2 and real Node/Wasm independently
  materialize to the exact reference executor trace, normalized outcome, and
  canonical JSON for zero/max payloads, reverse cleanup, contract and checked
  failures, scalar publication, owned identity/selection, and failed owned
  publication. The native evidence remains a test-only harness rather than the
  production ownership host.
- Added private callable native descriptor v2, derived from the sealed compiler
  host template plus execution/cleanup and semantic-dictionary evidence. Its
  canonical pointer-free wire binds eleven fingerprints, exact symbols,
  request/response capacities, complete ordered signature, opaque-`u64` owned
  payload kind, and result mapping. The unpublished host's independent strict
  parser accepts compiler output and rejects every single-byte mutation,
  truncation, and trailing byte.
- Extended the native loader quarantine with Unix
  `RTLD_NOW | RTLD_LOCAL`, exact callable-v2 symbol admission, bounded
  preallocated one-shot byte calls, and exact-instance rejection. This transport
  fixture is not generated SEMAPRAX resource execution: the physical ownership
  host still uses a trusted Rust closure, Windows runtime and callable-path
  sanitizer evidence remain absent, and `SPX-B104` stays closed.
- Migrated browser manifests from `semaprax.web.v2` to `semaprax.web.v3`. Version 3 retains module, graph revision, Wasm entry, and capabilities while adding the required `owned_abi` object with schema `semaprax.wasm-owned.v1` and a declaration-ordered function mapping; scalar-only packages use an empty function array. Version-2-only consumers must reject or explicitly migrate rather than inferring ownership ABI metadata.

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
