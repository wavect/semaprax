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
- Initially added the private native capability-token codec as a disconnected exact 64-byte canonical envelope with a full RustCrypto HMAC-SHA256 tag. Pinned audited crypto, a published RFC 4231 vector, independently reproduced owner/result full-token goldens, every-bit/arbitrary-byte/length/structure hostility, exact function-template result scoping, function-independent owner scoping, cross-context rejection, stale/max-generation checks, and explicit entropy/module-lifetime/linearity nonclaims established the mechanics later connected by the callable host.
- Added a private OS-backed native capability authority without connecting it to compiler preflight or any export. Exactly pinned `getrandom` 0.4.3 supplies one fail-closed seed for the secret, nonzero random epoch, and opaque thread-binding nonce; kind-specific non-formatting credentials seal immutable module/resource context and reject every operation off the actually captured Rust thread. Test-only deterministic entropy, independently reproduced authority goldens, error/zero/context/thread hostility, MSRV, and desktop OS smoke preserve the public `SPX-B104` gate while module retention, ledger integration, fork safety, and callable ownership remain blocked.
- Added a private fake-backed `NativeModuleLease` topology and required the native capability authority plus every staged owner/result credential wrapper to retain the exact allocation instance. Tests cover equal-fingerprint instance separation, process-incarnation rejection, one-way draining, lease-derived fingerprints, cross-instance rejection despite equal bearer bytes, drop-order retention, concurrent final release, and absence of retention cycles. There remains no production constructor, platform loader handle, code-identity admission, physical pin/unload protocol, ledger integration, callable export, or change to `SPX-B104`.
- Added an unpublished `semaprax-native-loader` workspace quarantine around the unavoidable trusted-library load, fixed-getter lookup, and bounded descriptor-read/compare unsafe edge. At this initial stage its same-thread opaque explicit-retain lease, exact-pinned `libloading` 0.8.9, compile-fail trait checks, workspace-wide gates, and real Linux/macOS fixtures were isolated from the compiler and authority; later entries connect them privately while the public adapter remains gated.
- Added a blocking, immutable-action-pinned `cargo-deny` CI gate for the complete native/mobile/Wasm dependency graph: RustSec advisories, unapproved licenses, duplicate or wildcard versions, Git dependencies, and registries outside crates.io fail the build with no advisory exceptions.
- Added a root-frame native trace-storage scaffold with exact compiler-status/event validation and a pre-ownership attachment handshake. Canonically zeroed one-shot contexts, buffers, and event slots use owner/generation checks to reject rebinding, aliasing, double attachment, and capacity underflow before execution.
- Added the unpublished `semaprax-native-host` physical ownership stage. It strictly decodes compiler-derived descriptors, retains the exact real loader instance through its same-thread OS-seeded authority and opaque owners, authenticates owner/result credentials, connects the private ownership ledger, preserves owners on precommit rejection, rotates owned results, and gates new work after draining. The later callable-v2 work below replaces its original trusted-closure execution fixture; compiler resource builds still retain `SPX-B104`.
- Added the first narrow public `semaprax.wasm-owned.v1` Core Wasm execution path for one direct trivial-resource identity. Generated adapters consume replay-validated terminal cleanup order, stage owner handles atomically, normalize contract/arithmetic/adapter status records, preserve poisoned result storage on failure, rotate owned results, and reject excluded shapes with `SPX-W111`. The generated JavaScript keeps host imports private, binds calls to exact generated metadata and SHA-256-authenticated Wasm bytes, rejects non-canonical ABI arguments, checks result ranges before ownership commit, and uses one-shot trusted adoption tickets; Node tests cover the admitted slice. This is not WebAssembly Component resources or production native-host conformance.
- Added `semaprax.semantic-event-dictionary.v1`, which assigns deterministic
  nonzero ordinals to exact semantic event shapes. Generated cleanup C and the
  real Wasm owned adapter emit those ordinals from their executed control flow;
  the host-side materializer rejects zero or unknown ordinals without inferring
  or repairing events.
- Unified the authoritative direct-trivial-resource conformance corpus at 14
  named scenarios. Real compiler-generated native shared libraries at O0/O2 now
  execute through the exact loader/authority/ledger ownership host, while real
  Node/Wasm executes the same cases. Both materialize to the exact reference
  trace and normalized outcome for zero/max payloads, reverse cleanup, contract
  and checked failures, scalar publication, owned identity/selection, and failed
  owned publication; native also proves result rotation and final logical
  liveness.
- Added private callable native descriptor v2, derived from the sealed compiler
  host template plus execution/cleanup, semantic-dictionary, and trace-path
  evidence. Its canonical pointer-free wire binds twelve fingerprints, exact symbols,
  request/response capacities, complete ordered signature, opaque-`u64` owned
  payload kind, and result mapping. The unpublished host's independent strict
  parser accepts compiler output and rejects every single-byte mutation,
  truncation, and trailing byte.
- Extended the native loader quarantine with Unix `RTLD_NOW | RTLD_LOCAL`, exact
  callable-v2 symbol admission, bounded preallocated one-shot byte calls, and
  exact-instance rejection, then connected that transport to the ownership host
  with strict request/response codecs and allocation-free postcommit decoding.
- Added `semaprax.trace-path-certificate.v1`: the compiler deterministically
  compiles every admitted cleanup path into a canonical trie-DFA separately
  fingerprinted into descriptor v2, symbols, and call contracts. Host admission
  authenticates it and rejects omitted, duplicated, reordered, or wrong-outcome
  traces before semantic materialization.
- Added complete private callable-provider emission with exact physical result
  and outcome namespaces, owned-payload integrity checks, and compile-time
  architecture/OS/environment/object/pointer/endian guards. Exact MSVC/GNU
  source known answers and deliberate target/payload mismatch fixtures fail
  closed without touching the response.
- Made formatting, Clippy, tests, docs, builds, and the Rust 1.85 gate run every
  workspace feature so staged production surfaces cannot escape CI. `SPX-B104`
  remains closed for general physical/malformed-response fallback cleanup and
  quiescence, Android/iOS profiles, and public native execution/admission.
- Added a separate fail-closed Linux Rust-host ASan lane pinned to
  `nightly-2026-07-16`: it rebuilds the target standard library, proves active
  Rust instrumentation with both an intentional fault and binary/compiler
  inspection, and runs the real callable host plus generated corpus. The exact
  lane passed in [public run 31259216533, job
  93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065),
  alongside a fully green current hosted-CI matrix for Linux, macOS, Windows,
  MSRV, dependency policy, and provider sanitizers. This is not mobile or app-
  platform evidence. Rust-host UBSan is not claimed and `SPX-B104` remains
  closed.
- Recorded the green public Linux
  [callable-host sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801):
  all 14 authoritative O0/O2 cases executed from dynamically loaded
  ASan/UBSan-instrumented generated providers through the Rust host. The Rust
  host code itself was not sanitizer-instrumented. The dependency-policy job was
  also green, but unrelated Clippy/GCC failures stopped the platform jobs before
  runtime evidence and kept the overall workflow run red; no Windows runtime
  evidence is inferred from this job.
- Added the hidden callable-v3 settlement foundation and proposed RFC 0004: a
  bounded target-neutral certificate/frame/receipt model with one all-live
  start, typed progress, exhaustive owner-state enumeration, exact accept/abort
  cleanup actions, idempotent quiescent receipts, deterministic fingerprints,
  and hostile mutations. A private compiler deriver now preserves exact HIR
  result-staging/finalization timing and binds terminal settlement to accepted
  semantic trace paths for the authoritative 14-case direct-trivial corpus.
  The model deliberately provides no invocation or module-instance reservation,
  physical finalizer authority, descriptor/provider, loader/host, public
  compiler, or backend runtime evidence; `SPX-B104` remains closed.
- Added a private linear settlement transaction model with closed `Executing`,
  `DecisionLocked`, `ActionInProgress`, `ProviderSettled`, model
  `ReceiptCommitted`, and absorbing `Quarantined` phases. Its 29 focused tests
  cover phase-aware unwind, every-finalizer interruption without retry, exact
  candidate/committed replay, hostile mutation/cross-binding, and preserved
  evidence while keeping provider `Published` unauthoritative. The model
  allocates and grants no exact-instance reservation, host authentication,
  ledger publication, FFI/provider, or physical-finalizer authority; this
  changes no v2 bytes or public/runtime gate.
- Added private callable settlement-proof v1 without consuming the future v3
  ABI version. `SPXNPRF1` embeds the exact unchanged callable-v2 descriptor and
  a canonical pointer-free binary settlement graph under one 64 KiB ceiling.
  Separate compiler and host codecs bind the exact v2 call contract and trace
  certificate, reject rehashed cross-module/changed-trace substitutions and
  hostile graph structure, and reproduce a fixed known answer. The compiler
  enforces the cap while serializing; the v2 loader rejects proof magic before
  opening an image; default consumers cannot import the proof surface. This adds
  no provider, descriptor-v3, loader admission, host settlement execution,
  physical finalizer, public API, mobile evidence, or `SPX-B104` change.
- Froze the private [callable ABI v3 descriptor/wire
  contract](docs/NATIVE-CALLABLE-ABI-V3.md): `SPXNABI3` fixes the descriptor,
  acyclic hash dependencies, recovery graph, capacity budget, dynamic and
  iOS-static linkage roles, six complete provider codecs, and the distinct
  host-only 524-byte committed-receipt codec. The six-argument execute ABI,
  payload-bearing frame cells, closed tags, request/response/decision/action/
  frame/candidate digests, and separate receipt-key HMAC replace the former
  provisional identities and freeze changed private v3 known answers.
  `CertifyOutcome` embeds the canonical ordinal/outcome witness and a nonzero
  trace-certificate-bound evidence digest independently recomputed by the host;
  this binds the witness only and is not host acceptance of the trace-path DFA
  certificate. Resealed witness or digest mutations are rejected.
  Independent compiler encoders and host parsers cover those seven complete
  transcripts. The emitter is bound to its
  build target and provides no Android/iOS/Windows cross-emission evidence. Both
  existing loader constructors now reject v3 magic before path
  canonicalization or image/symbol access, including malformed same-magic
  headers. The compiler/host codec tranche grants no provider,
  loading, settlement, finalizer, ledger, mobile, or public authority and leaves
  v2/proof bytes and `SPX-B104` unchanged.
- Added the first private callable-v3 physical components: two bounded generated
  strict-C11 providers execute scalar-discard and owned-identity settlement at
  `-O0`/`-O2`; an exact dynamic-image loader verifies root provenance for the
  getter, execute, settle, and descriptor storage; and the host has a distinct
  OS-seeded receipt authority with a fixed-capacity atomic ledger/facade. These
  components are not yet connected by one host invocation and do not prove the
  full 14-case physical corpus, exhaustive failure injection, sanitizers,
  Windows v3 runtime, Android/iOS, quiescence, malicious-code containment,
  public admission, or any `SPX-B104` change.
- Added a mandatory Windows callable-v2 dependency-isolation fixture. It places
  a same-name dependency in both CWD and legacy `PATH`, proves the root-image
  sibling wins for descriptor admission and invocation, then removes that
  sibling and requires `LibraryOpen` rather than malicious fallback. CI names
  this fixture and the complete O0/O2 callable corpus explicitly. Both passed
  in [run 31257545008, job 93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).
- Added a public build-only native-callable API and CLI for one explicitly
  selected direct-trivial owned function. It produces a deterministic hashed
  provider bundle and strict host shared library through safe staging and
  observed no-overwrite checks, while exposing no loading, invocation,
  adoption, or authority and retaining ordinary native `SPX-B104`.
- Migrated browser manifests from `semaprax.web.v2` to `semaprax.web.v3`. Version 3 retains module, graph revision, Wasm entry, and capabilities while adding the required `owned_abi` object with schema `semaprax.wasm-owned.v1` and a declaration-ordered function mapping; scalar-only packages use an empty function array. Version-2-only consumers must reject or explicitly migrate rather than inferring ownership ABI metadata.

## 0.1.0 — 2026-08-07

- Introduced the typed SEMAPRAX source subset and canonical formatter.
- Added stable semantic identities, revisioned graph output, and context slicing.
- Added effect/capability verification and typed runtime contracts.
- Added checked arithmetic and native C11/Clang compilation.
- Added machine-readable diagnostics and atomic semantic rename patches.
- Published RFC 0001 and the staged compiler roadmap.
