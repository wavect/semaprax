# Full-goal completion matrix

Status: living internal audit of the current product contract.

Audience: maintainers, contributors, reviewers, and technical evaluators.

This document is the authoritative status audit for the complete SEMAPRAX
objective. It deliberately separates three questions:

1. What must the mature product do?
2. What bounded evidence exists today?
3. What still has to be proven before a row is complete?

Historical status transitions belong in the [changelog](../CHANGELOG.md).
Protocol details, exact known-answer digests, test counts, and CI run IDs belong
in the linked versioned specifications. Future sequencing belongs in the
[roadmap](ROADMAP.md).

## Status rules

| Status | Meaning |
| --- | --- |
| Implemented | The full completion gate is covered by executable evidence on every required target. |
| Partial | Useful executable evidence exists, but the full completion gate remains open. |
| Missing | No qualifying executable evidence exists for the row. |

Design text, generated placeholders, source compilation alone, a proof-only
model, or a narrower private target does not complete a broader row. Local,
hosted, private, public, and proof-only evidence are distinct.

In the programme audit below, **Authored, unrun** means that implementation and
executable evidence are present in the current source tree, but this audit did
not execute them. It is weaker than local green evidence and cannot support a
hosted or public promotion claim.

## Current summary

**Overall product objective: Partial**

The long-term contract below contains **49 requirements: 49 Partial, 0
Implemented, 0 Missing**. Each row has at least one bounded executable slice,
but none satisfies its full gate.

The previous “56 Partial / 0 Missing” dashboard mixed these 49 long-term
requirements with seven later milestone rows. That made the denominator change
as work was added. This matrix now keeps the product contract fixed and tracks
release-specific evidence separately.

The strongest current vertical slices are:

- canonical source, stable-ID HIR, and deterministic semantic graph queries;
- bounded replay-checked single-file and managed-workspace changes;
- scalar and selected Copy/owned-data execution through interpreter, native
  C11/Clang, and Core WebAssembly/Node lanes, including the closed flat
  [Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) slice;
- bounded multi-file Project manifests through the additive Project v8
  `owned-data-api.v1` implementation, with one canonical descriptor driving
  Web/npm and unpublished safe Rust package generation;
- the authored frame-payload corpus across interpreter, native C11 O0/O2,
  Core Wasm/Node, generated npm, and generated Rust consumer lanes, plus the
  read-only Project Agent Transport v5 descriptor/carrier surface;
- private desktop/mobile and host-integration evidence.

The largest remaining gaps are general ownership and lifetime safety, stable
public aggregate/resource/component ABIs, a package and dependency ecosystem,
production application tooling, broad target conformance, and the final 1.0
validation product.

## v0.2 product-exit audit

This audit measures the current release objective. “Prior-head” means the
linked specification records hosted evidence for an earlier exact commit; it
does not promote the current head. “Local” means executable repository evidence
exists but has not completed exact-head hosted promotion.

| Exit criterion | Evidence | Remaining gate |
| --- | --- | --- |
| Multi-module calculator project | Exact-head hosted | Keep Project Manifest admission and source closure green on subsequent release candidates. |
| Same verified calculator logic on native and browser lanes | Exact-head hosted | Preserve the identical success/failure corpus on subsequent release candidates and broaden browser engines only when claimed. |
| Several stable-ID functions callable from TypeScript and Rust | Exact-head hosted; builder remains unpublished | Publish an intentionally supported Rust entry point. |
| Browser calculator consumes Project exports | Exact-head Chromium, including the display-renamed fixture | Add multi-engine evidence only when broader browser compatibility is claimed. |
| Project daemon inspect/derive/preview/apply/rebuild loop | Exact-head hosted | Preserve Transport v4's bounded authority contract on subsequent release candidates. |
| Stable external API survives a display rename | Exact-head hosted | Preserve the complete renamed Project and consumer proof on subsequent release candidates. |
| Project tests demonstrate native/Wasm equivalence | Exact-head hosted | Preserve the full entry/test and consumer corpus on subsequent release candidates. |
| Multi-module line-filter product | Local | Add exact-head hosted and real-browser/multi-engine evidence. |
| Full promotion CI for every public release claim | Exact-head hosted | Keep all blocking release jobs green on the final release commit without diagnostic masks. |

The v0.2 release objective remains **Partial**. The upstream baseline blocking
matrix is green at its exact commit
`4cc03820c86e70527cb65c4b10ee3841c7af167d` in
[run 33259787886](https://github.com/wavect/semaprax/actions/runs/33259787886),
but the line-filter still lacks the stated browser/runtime breadth and the Rust
builder remains unpublished. That prior exact-head run does not execute or
promote WP-01–WP-15, Project v8, Agent Transport v5, Project v9, or Project v10
work authored later in this source tree.

Evidence owners: [Project Manifest v1](PROJECT-MANIFEST-V1.md) and its additive
v2–v7 references, [Bounded Language Command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md),
[Project Agent Workflow](PROJECT-AGENT-WORKFLOW-V1.md),
[Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), and
[Native Rust Interoperability](NATIVE-RUST-INTEROP-V1.md).

## WP-01–WP-15 implementation and promotion audit

This table tracks the bounded developer-preview programme separately from the
49-requirement product contract. It records what is present in the current
source tree, not what was executed by this documentation-only audit. No row below
changes a long-term status to Implemented.

| Work package | Current source state | Authored evidence | Remaining gate |
| --- | --- | --- | --- |
| WP-01 CI decomposition | Authored, unrun | Dedicated desktop/native product matrix, release aggregation, and CI contract evidence | Execute the workflow and preserve a blocking exact-head release aggregation. |
| WP-02 deterministic version | Authored, unrun | Human and canonical JSON version renderers with injected commit identity and focused CLI evidence | Execute local gates and bind the exact release commit in hosted artifacts. |
| WP-03 release artifacts | Authored, unrun | Deterministic Unix/Windows packaging, manifest/checksum workflow, smoke route, and contract evidence | Exercise every advertised archive on its build host at the release head. |
| WP-04 v0.2 tagged artifact/release promotion | Pending | Promotion criteria are specified; no tagged artifact or release-promotion record was added | Exercise the final tagged artifacts at one exact head, pass the release gate, and record its run, checksums, and artifact inventory. |
| WP-05 `doctor` | Authored, unrun | Injected-host checks, human/JSON rendering, real multicall/PATH resolution, strict version tokens, and [bounded subprocess/fail-stop settlement fixtures](DOCTOR-PROBE-V1.md) | Execute physical Linux/macOS/Windows gates and establish the programme's separate no-network requirement; environment filtering is not a sandbox and this does not promote v0.2. |
| WP-06 `new` | Authored, unrun | Compiled-in calculator template, staged authenticated publication, and hostile path/failure evidence | Execute the generator, Project check/test, and publication gates. |
| WP-07 quickstart | Authored, unrun | Executable quickstart document and mirrored repository test | Execute the documented sequence against the candidate compiler. |
| WP-08 v8 specification | Specified | [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) freezes identifiers, admission, lifetime, compatibility, and twelve completion gates | Keep the specification synchronized with the additive implementation and promotion evidence. |
| WP-09 canonical descriptor | Authored, unrun | Validated-HIR derivation, canonical bytes/digest, independent replay, stable host naming, and hostile descriptor evidence | Execute focused replay/KAT and legacy-preservation gates. |
| WP-10 direct `Bytes` npm/Wasm | Authored, unrun | Profile-specific carrier, fresh copy-out, preallocation whole-tuple input admission, intrinsic-brand hostility, selected-call-path private-frame exclusion, and settlement evidence | Execute Node/browser, capacity, settlement, and legacy-byte gates. |
| WP-11 `Option<Bytes>` / `Result<Bytes, i64>` | Authored, unrun | Fixed tags, active-payload handling, TypeScript mapping, cleanup evidence, and reference-interpreter normalization | Execute interpreter/native/Wasm/npm branch and hostile-tag equivalence gates. |
| WP-12 safe native/Rust SDK | Authored, unrun; unpublished | Root-owned HIR/provider, held-tool publication, safe API/private FFI, nonreused handle serials, all 4,096 slots, foreign-context/reincarnation and exhaustion/contention evidence, per-invocation whole-context settlement before return, inactive-payload rejection, unwind/copy-failure and external-consumer fixtures | Execute O0/O2, locked offline consumer, hostile provider/handle, sanitizer, and Linux/macOS/Windows gates; intentionally decide publication support. |
| WP-13 Project v8 activation | Authored, unrun | Exact v8 manifest/profile parsing, bounded multi-root linking, semantic-recipe replay, and CLI `web`/`npm`/`rust` routing | Execute v1–v7 KAT preservation and every v8 target route at one candidate head. |
| WP-14 frame-payload product | Authored, unrun | One committed corpus and display-rename proof across interpreter, native O0/O2, Core Wasm/Node, generated npm, and locked/offline Rust; shared Node/browser corpus runner, explicit strict-TypeScript positive/negative and provisioned Chromium fixtures | Execute the complete identical corpus and external consumers on required hosts; explicitly select provisioned TypeScript/browser gates and establish required browser breadth. |
| WP-15 v8 promotion | Pending | The twelve-gate contract is specified; no dedicated exact-head v8 promotion matrix or run is recorded | Add and pass every blocking Project/npm/browser/Rust/equivalence/sanitizer/hostile job on one exact commit, with no skip or allowed failure. |
| Agent Transport v5 follow-on | Authored, unrun; unpromoted | Opt-in read-only descriptor and inline npm methods with typed descriptor binding, bounded framing, stale-subject rejection, and zero publication/process authority evidence | Execute focused gates, preserve v2–v4 bytes, then include the surface in an exact-head promotion decision. |
| Project v9 flat owned record follow-on | Implementation including ordinary Phase-A admission authored, unrun; unpublished; unpromoted | [Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md), [Project Profile Admission v1](PROJECT-PROFILE-ADMISSION-V1.md), exact descriptor, Wasm/npm adapter with bounded pre-copy tuple admission, root native provider, safe Rust whole-context settlement guard, and v1-v10 Revision Store evidence | Execute ordinary Project admission, Revision Store replay, hostile input/provider and cross-target physical consumers, and preservation gates, then make an explicit v9 publication and promotion decision before any dependent profile. |
| Project v10 owned UTF-8 follow-on | Authored, unrun; unpublished; unpromoted; blocked on v9 promotion | [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md), distinct descriptor/digest, bounded Wasm/npm input and inline String ownership/call settlement, v10-only native String owner ledger and physical allocation fixtures, validating native provider, safe Rust `String` with whole-context settlement guard, and hostile UTF-8 evidence | First promote v9; execute exact replay/carrier, ownership/capacity/hostility, native allocation settlement, valid/invalid UTF-8, host consumers, native sanitizers, and v1-v9 byte preservation on every required target before publication or promotion. Ordinary native and earlier-profile String failure cleanup remains a separate open gap. |
| Project Revision Store v1 follow-on | Unix implementation and additive Windows-entry authority/identity/physical fixtures authored, unrun; unpromoted | [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md), [Windows-entry v1](PROJECT-REVISION-STORE-WINDOWS-V1.md), unchanged ordinary v1 bytes, Unix current-euid `0700` and Windows effective-SID/protected-DACL fixed-local-NTFS held-root authority, bounded replay, complete selected-entry semantic replay, one no-replace publication pivot, authority-neutral location, and read-only inert-stage quarantine | Complete and execute literal v1-v10 round trips and the full hostile programme on required Unix and Windows hosts at one exact head; opt-in provisioned-host fixtures and static review alone do not promote support. Measure explicit retained revision reuse without promoting the store to an ambient cache. |

The Project v8 implementation and executable evidence are therefore
**authored in the current source tree but unrun and unpromoted**. The generated
npm and Rust packages remain developer-preview and unpublished surfaces.
WP-15 is the explicit blocker for describing the bounded owned-data API as
hosted, supported, or released.

## Long-term product contract

The “Evidence owner” column points to the document that defines the strongest
current bounded slice. It is not a claim that the linked slice completes the
row.

### Semantic foundation

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Agent-native semantic program | Partial | [RFC 0001](RFC-0001.md), [Agent Context v2](AGENT-CONTEXT-V2.md), [Project Agent Transport v5](PROJECT-AGENT-TRANSPORT-V5.md), [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md) | The complete program graph is persistent, queryable, schema-versioned, and validated on representative repositories. |
| Human-readable program | Partial | [RFC 0001](RFC-0001.md) | Canonical source round-trips every stable language feature with migrations and reviewable diffs. |
| Verified source semantics | Partial | [Architecture](ARCHITECTURE.md) | All admitted language features reach validated HIR only after complete type, effect, contract, and ownership checks. |
| Cross-backend semantic equivalence | Partial | [Conformance Trace v1](CONFORMANCE-TRACE-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) | Every supported backend passes the same complete behavior, failure, cleanup, and contract corpus. |
| Atomic agent changes | Partial | [Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md), [Workspace Change v1](SEMANTIC-WORKSPACE-CHANGE-V1.md) | General supported single- and multi-file semantic changes replay and publish atomically with recovery and provenance. |

### Language and safety

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [Owned Byte Record Algebra](OWNED-BYTE-RECORD-ALGEBRA-V1.md), [Owned Byte Variant Algebra](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) | General nested/generic/resource aggregates, matching, layout, cleanup, and public ABIs are verified. |
| Functions, closures, interfaces, implementations, generics | Partial | [RFC 0001](RFC-0001.md) | Closures, interfaces/implementations, inference, constraints, specialization, and cross-target execution are complete. |
| `Option` and `Result`; no null or unchecked exceptions | Partial | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [Owned Byte Variant Algebra](OWNED-BYTE-VARIANT-ALGEBRA-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) | General Copy and owned propagation/matching, residual conversion, ABI, and target behavior are verified. |
| Immutable-by-default values and explicit mutation | Partial | [Explicit Mutation v1](EXPLICIT-MUTATION-V1.md), [Field Mutation v1](FIELD-MUTATION-V1.md) | Aggregate, collection, borrowed, and concurrency-aware mutation rules are verified. |
| Unique ownership and move safety | Partial | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md), [Owned Byte Variant Algebra](OWNED-BYTE-VARIANT-ALGEBRA-V1.md), [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md), [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) | General owned values, aliases, deeper nested aggregates, control flow, FFI, cleanup, and target execution are verified. |
| Borrowed views and lifetime safety | Partial | [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md), [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md), [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md) | General lifetime inference, mutable and escaping borrows, borrowing beyond the authored-unrun direct owned-byte field profile, cross-file use, and public host ABI behavior are verified. |
| Regions and arenas | Partial | [Region Report v1](REGION-REPORT-V1.md) | Region inference and runtime placement are implemented and verified; the report alone is insufficient. |
| Shared immutable ARC and managed zones | Partial | [ARC Zone Model v1](ARC-ZONES-V1.md) | Language, runtime, cycle, escape, and concurrency semantics execute on supported targets. |
| Restricted `unsafe` and raw memory | Partial | [Unsafe Boundaries v1](UNSAFE-BOUNDARIES-V1.md) | Raw memory operations, review policy, capability rules, and target conformance are implemented and verified. |
| Checked, wrapping, and saturating arithmetic | Partial | [RFC 0001](RFC-0001.md) | All numeric widths and named arithmetic modes have complete cross-backend semantics and tests. |
| Effects and capabilities | Partial | [Capability Manifest v1](CAPABILITY-MANIFEST-V1.md) | Declared effects and build/runtime capabilities are enforced end to end, including dependencies and hosts. |
| Contracts and progressive verification | Partial | [RFC 0001](RFC-0001.md) | Static discharge, bounded proof, runtime obligations, counterexamples, and repair evidence are integrated. |
| Structured concurrency | Partial | [Scoped Task Model v1](SCOPED-TASKS-V1.md) | Language syntax, checking, runtime scheduling, cancellation, cleanup, and target execution are verified. |
| Typed hygienic generation | Partial | [Hygienic Generation v1](HYGIENIC-GEN-V1.md) | General typed synthesis is scoped, hygienic, deterministic, and integrated with multi-file semantics and review. |

### Compiler and output targets

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Fast development lane | Partial; prepared Project interpreter/source trace authored and unrun | [Interpreter v1](INTERPRETER-V1.md), [Prepared Project Interpreter and Source Trace v1](PROJECT-PREPARED-INTERPRETER-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) | Execute and promote the prepared-worker/trace evidence; then incremental refresh, debugging, hot reload, and semantic equivalence must meet the development-performance target. |
| Optimizing native lane | Partial | [Architecture](ARCHITECTURE.md) | The production native backend covers the mature language, optimization, debug mapping, and supported hosts. |
| WebAssembly core and components | Partial | [Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), [Wasm Owned ABI](WASM-OWNED-ABI-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) | Stable Components, resources, capabilities, multi-engine conformance, and packaging are verified. |
| Embedded and real-time | Partial | [Freestanding Profile v1](FREESTANDING-V1.md) | Hardware profiles, linker control, interrupts/RTOS, timing constraints, and representative targets are verified. |
| SIMD and GPU | Partial | [SIMD Report v1](SIMD-REPORT-V1.md) | Vector/GPU lowering, legality, memory behavior, target selection, and performance evidence are implemented. |

### Ecosystem interoperability

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Partial; semantic lock snapshot, authenticated dependency ranges/selection, and multi-package capsule/build v2 authored, locally unrun and unpromoted | [Package Report v1](PACKAGE-REPORT-V1.md), [Semantic Package Report v2](PACKAGE-REPORT-V2.md), [Offline Package Lock v1](OFFLINE-PACKAGE-LOCK-V1.md), [Offline Semantic Lock v2](OFFLINE-SEMANTIC-PACKAGE-LOCK-V2.md), [Offline Semantic Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md), [Compatibility Evidence v1](OFFLINE-PACKAGE-COMPATIBILITY-EVIDENCE-V1.md), [Offline Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md), [Offline Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md), [Published Semantic Lock Snapshot v1](OFFLINE-PUBLISHED-SEMANTIC-LOCK-SNAPSHOT-V1.md), [Multi-Package Source Capsule v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md), [Effect-Free Wasm Package Build v1](OFFLINE-PURE-WASM-PACKAGE-BUILD-V1.md), [Linked Scalar Wasm Package Build v2](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md) | Execute the authored snapshot/range/lock/resolver/capsule/v1-v2 build/publication evidence; then add general consumer/source compatibility and version negotiation, supported publication, trusted provenance, registry, and conformance. |
| Portable canonical ABI and native fast ABI | Partial | [ABI Report v1](ABI-REPORT-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) | Stable aggregate/resource/borrowed ABIs and cross-language conformance cover supported architectures. |
| C and Objective-C | Partial | [C Header v1](C-HEADER-V1.md) | Import/export, ownership, errors, compiled consumers, Objective-C adapters, and compatibility are verified. |
| C++ | Partial | [C++ Shim v1](CXX-SHIM-V1.md) | Compiled C++ consumers, ownership, exceptions, templates/adapters, and compatibility are verified. |
| Java and Kotlin | Partial | [Android JNI Ownership v1](ANDROID-JNI-OWNERSHIP-V1.md) | Public JVM/JNI artifacts, ownership, exceptions, packaging, and conformance are verified. |
| Swift and Apple frameworks | Partial | [Swift Ownership v1](APPLE-SWIFT-OWNERSHIP-V1.md) | Public Swift/Objective-C API, distributable frameworks, lifecycle, ownership, and device evidence are verified. |
| JavaScript and TypeScript | Partial | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md), [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md), [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md) | Stable general bindings, owned resources, async/callbacks, packaging, and browser/runtime breadth are verified. |
| WIT and WebAssembly Components | Partial | [WIT Boundary v1](WIT-COMPONENT-BOUNDARY-V1.md) | Source-selected interfaces and resources run through a supported Component Model toolchain on multiple runtimes. |
| OpenAPI, Protobuf/gRPC, GraphQL, and SQL | Partial | [OpenAPI v1](OPENAPI-V1.md) | Import/export, compatibility, live conformance, and all named schema families are verified. |

### Application platforms

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| First-class application/state/UI dialect | Partial | [UI Schema v1](UI-SCHEMA-V1.md) | Typed state/update/view, semantic controls, accessibility, navigation, assets, and platform escape hatches execute. |
| Web | Partial | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md) | Accessible DOM/CSS, SSR/hydration, packaging, multi-engine execution, and a deployable sample are verified. |
| iOS | Partial | [Swift Ownership v1](APPLE-SWIFT-OWNERSHIP-V1.md) | Public framework/app generation, lifecycle, accessibility, signing metadata, and device/simulator samples are verified. |
| Android | Partial | [Android JNI Ownership v1](ANDROID-JNI-OWNERSHIP-V1.md) | Public AAR/app generation, lifecycle, accessibility, packaging, and emulator/device samples are verified. |
| macOS | Partial | [Desktop App v1](DESKTOP-NATIVE-APP-V1.md) | Public host/UI generation, lifecycle, accessibility, packaging, signing/notarization, and a sample are verified. |
| Windows | Partial | [Desktop UI v1](DESKTOP-NATIVE-UI-V1.md) | Public host/UI generation, lifecycle, accessibility, MSIX/signing metadata, and a sample are verified. |
| Linux | Partial | [Roadmap](ROADMAP.md) | A supported UI/runtime adapter, accessibility, distribution formats, and a representative application are verified. |
| Edge and server | Partial | [Bounded Language Command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md) | General I/O, async services, HTTP/data adapters, observability, deployment, and load/conformance tests are verified. |
| Plugins | Partial | [Plugin Manifest v1](PLUGIN-MANIFEST-V1.md) | Capability-limited loading, lifecycle, compatibility, resource limits, packaging, and hostile-plugin tests are verified. |

### Agent economics, review, and operations

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial | [Agent Context v2](AGENT-CONTEXT-V2.md), [Economics v1](AGENT-ECONOMICS-V1.md) | Exact model-token budgets, broader semantic edges, persistent indexing, and representative measured savings are verified. |
| Impact analysis before modification | Partial | [Semantic Impact v1](SEMANTIC-IMPACT-V1.md) | Repository-wide call/type/contract/test/schema/target/capability consumers are complete and incremental. |
| Typed holes and compiler-generated repairs | Partial | [Diagnostic Repair v1](DIAGNOSTIC-REPAIR-V1.md) | General obligations and composable sound repairs are generated, ranked, reviewed, and replay-verified. |
| Proof-carrying patches | Partial | [Patch Evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md) | General semantic claims, tests, targets, capability deltas, provenance, and compatibility are independently verified before commit. |
| Semantic human review | Partial | [Semantic Review v1](SEMANTIC-REVIEW-V1.md) | Complete repository-wide behavioral, API, security, memory, target, migration, and unsafe summaries are evidence-backed. |
| Sandboxed builds and dependencies | Partial; authority-free linked build authored, unrun and unpromoted | [Capability Manifest v1](CAPABILITY-MANIFEST-V1.md), [Offline Package Lock v1](OFFLINE-PACKAGE-LOCK-V1.md), [Offline Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md), [Multi-Package Source Capsule v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md), [Effect-Free Wasm Package Build v1](OFFLINE-PURE-WASM-PACKAGE-BUILD-V1.md), [Linked Scalar Wasm Package Build v2](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md) | Execute the authored authority-free resolver, source-capsule, v1-v2 internal-Wasm build, and publisher evidence; then verify reproducible acquired inputs and actual least-authority OS sandbox/dependency enforcement. Empty source authority and no external tool execution are not a hermetic sandbox. |
| Debugger, profiler, diagnostics, and operations | Partial | [Architecture](ARCHITECTURE.md) | Source-level debugging/profiling, crash and trace mapping, observability, and deployment diagnostics cover every backend. |

## Final validation product

Completion requires one maintained offline-first product built from a shared
SEMAPRAX codebase with web, iOS, Android, macOS, Windows, and Linux clients;
native notifications and secure storage; local databases; native or WASI
server execution; authentication; background synchronization; a custom
accelerated visual; one C library; one JavaScript package; and one WebAssembly
component.

Every artifact must be built and exercised in CI or on representative
simulators/devices. Platform-specific implementations must be declared rather
than hidden behind false portability. No current narrow prototype satisfies
this final gate.
