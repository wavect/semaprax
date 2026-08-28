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
  C11/Clang, and Core WebAssembly/Node lanes;
- bounded multi-file Project manifests with selected Web, npm, native-command,
  and unpublished Rust SDK consumers;
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
| Multi-module calculator project | Evidenced | Keep Project Manifest admission and source closure green on the promotion commit. |
| Same verified calculator logic on native and browser lanes | Prior-head hosted | Run the complete identical success/failure corpus on the exact release head and required hosts. |
| Several stable-ID functions callable from TypeScript and Rust | Prior-head hosted, current builder remains unpublished | Promote the exact-head JS/TS and compiler-free Rust consumers and publish an intentionally supported Rust entry point. |
| Browser calculator consumes Project exports | Prior-head Chromium; expanded three-fixture proof is local | Promote the baseline and display-renamed Project fixtures on the exact release head. |
| Project daemon inspect/derive/preview/apply/rebuild loop | Prior-head hosted | Promote Transport v4 and preserve its bounded authority contract. |
| Stable external API survives a display rename | Prior-head hosted; expanded artifact proof is local | Promote the complete renamed Project and consumer proof on the exact release head. |
| Project tests demonstrate native/Wasm equivalence | Prior-head hosted | Promote the full entry/test and consumer corpus on the exact release head. |
| Multi-module line-filter product | Local | Add exact-head hosted and real-browser/multi-engine evidence. |
| Full promotion CI for every public release claim | Pending | All blocking release jobs must pass at one exact candidate commit without diagnostic masks. |

The v0.2 release objective therefore remains **Partial**. The implementation
criteria have bounded evidence; the mandatory exact-head promotion criterion
has not been met.

Evidence owners: [Project Manifest v1](PROJECT-MANIFEST-V1.md) and its additive
v2–v5 references, [Bounded Language Command I/O](BOUNDED-LANGUAGE-COMMAND-IO-V1.md),
[Project Agent Workflow](PROJECT-AGENT-WORKFLOW-V1.md),
[Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), and
[Native Rust Interoperability](NATIVE-RUST-INTEROP-V1.md).

## Long-term product contract

The “Evidence owner” column points to the document that defines the strongest
current bounded slice. It is not a claim that the linked slice completes the
row.

### Semantic foundation

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Agent-native semantic program | Partial | [RFC 0001](RFC-0001.md), [Agent Context v2](AGENT-CONTEXT-V2.md) | The complete program graph is persistent, queryable, schema-versioned, and validated on representative repositories. |
| Human-readable program | Partial | [RFC 0001](RFC-0001.md) | Canonical source round-trips every stable language feature with migrations and reviewable diffs. |
| Verified source semantics | Partial | [Architecture](ARCHITECTURE.md) | All admitted language features reach validated HIR only after complete type, effect, contract, and ownership checks. |
| Cross-backend semantic equivalence | Partial | [Conformance Trace v1](CONFORMANCE-TRACE-V1.md) | Every supported backend passes the same complete behavior, failure, cleanup, and contract corpus. |
| Atomic agent changes | Partial | [Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md), [Workspace Change v1](SEMANTIC-WORKSPACE-CHANGE-V1.md) | General supported single- and multi-file semantic changes replay and publish atomically with recovery and provenance. |

### Language and safety

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md), [Owned Byte Record Algebra](OWNED-BYTE-RECORD-ALGEBRA-V1.md) | General nested/generic/resource aggregates, matching, layout, cleanup, and public ABIs are verified. |
| Functions, closures, interfaces, implementations, generics | Partial | [RFC 0001](RFC-0001.md) | Closures, interfaces/implementations, inference, constraints, specialization, and cross-target execution are complete. |
| `Option` and `Result`; no null or unchecked exceptions | Partial | [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md) | General Copy and owned propagation/matching, residual conversion, ABI, and target behavior are verified. |
| Immutable-by-default values and explicit mutation | Partial | [Explicit Mutation v1](EXPLICIT-MUTATION-V1.md), [Field Mutation v1](FIELD-MUTATION-V1.md) | Aggregate, collection, borrowed, and concurrency-aware mutation rules are verified. |
| Unique ownership and move safety | Partial | [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) | General owned values, aliases, control flow, FFI, cleanup, and target execution are verified. |
| Borrowed views and lifetime safety | Partial | [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md) | General lifetime inference, reborrowing, escape analysis, cross-file use, and host ABI behavior are verified. |
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
| Fast development lane | Partial | [Interpreter v1](INTERPRETER-V1.md) | Incremental execution, debugging, hot reload, and semantic equivalence meet the development-performance target. |
| Optimizing native lane | Partial | [Architecture](ARCHITECTURE.md) | The production native backend covers the mature language, optimization, debug mapping, and supported hosts. |
| WebAssembly core and components | Partial | [Wasm Scalar Exports](WASM-SCALAR-EXPORTS-V1.md), [Wasm Owned ABI](WASM-OWNED-ABI-V1.md) | Stable Components, resources, capabilities, multi-engine conformance, and packaging are verified. |
| Embedded and real-time | Partial | [Freestanding Profile v1](FREESTANDING-V1.md) | Hardware profiles, linker control, interrupts/RTOS, timing constraints, and representative targets are verified. |
| SIMD and GPU | Partial | [SIMD Report v1](SIMD-REPORT-V1.md) | Vector/GPU lowering, legality, memory behavior, target selection, and performance evidence are implemented. |

### Ecosystem interoperability

| Requirement | Status | Evidence owner | Complete when |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Partial | [Package Report v1](PACKAGE-REPORT-V1.md) | Resolver, lockfile, compatibility, provenance, registry, and conformance are production-ready. |
| Portable canonical ABI and native fast ABI | Partial | [ABI Report v1](ABI-REPORT-V1.md) | Stable aggregate/resource/borrowed ABIs and cross-language conformance cover supported architectures. |
| C and Objective-C | Partial | [C Header v1](C-HEADER-V1.md) | Import/export, ownership, errors, compiled consumers, Objective-C adapters, and compatibility are verified. |
| C++ | Partial | [C++ Shim v1](CXX-SHIM-V1.md) | Compiled C++ consumers, ownership, exceptions, templates/adapters, and compatibility are verified. |
| Java and Kotlin | Partial | [Android JNI Ownership v1](ANDROID-JNI-OWNERSHIP-V1.md) | Public JVM/JNI artifacts, ownership, exceptions, packaging, and conformance are verified. |
| Swift and Apple frameworks | Partial | [Swift Ownership v1](APPLE-SWIFT-OWNERSHIP-V1.md) | Public Swift/Objective-C API, distributable frameworks, lifecycle, ownership, and device evidence are verified. |
| JavaScript and TypeScript | Partial | [Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md) | Stable general bindings, owned resources, async/callbacks, packaging, and browser/runtime breadth are verified. |
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
| Sandboxed builds and dependencies | Partial | [Capability Manifest v1](CAPABILITY-MANIFEST-V1.md) | Resolver, lockfile, package graph, reproducibility, and actual least-authority enforcement are verified. |
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
