# Full-goal completion matrix

This document is the authoritative audit checklist for the complete SEMAPRAX objective. A row is complete only when the linked implementation and automated evidence prove the stated gate. Design text, generated placeholders, or a successful build on a narrower target do not satisfy a broader gate.

Status values:

- **Implemented** — the gate is covered by executable evidence.
- **Partial** — useful implementation exists, but the full gate is not proven.
- **Missing** — no qualifying implementation exists yet.

## Defining product contract

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Agent-native semantic program | Partial | `graph`, `context`, stable declaration IDs, and revision-bound rename transactions | Versioned multi-file graph API covers declarations, types, ownership, effects, contracts, targets, tests, packages, generated artifacts, typed repairs, impact, and semantic review |
| Human-readable program | Partial | Canonical `.spx` source and formatter | Complete language round-trips deterministically; graph-aware merge/diff, debugger source mapping, and normal Git/editor workflows are verified |
| Meaning in, verified machine code out | Partial | Typed scalar core, effect checks, runtime contract guards, checked arithmetic, native host executable | All safe-language guarantees survive every backend; native artifacts and portable components pass conformance suites on every supported target |
| Atomic agent changes | Partial | Single-file stable-ID function/resource renames update calls and ownership type boundaries with stale revision rejection | Typed, transactional multi-file edits support every public semantic operation and either commit fully or leave all source/graph state unchanged |

## Language and safety

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Records and algebraic variants | Missing | — | Construction, field access/update, exhaustive matching, layout, graph representation, and all backends verified |
| Functions, closures, interfaces, implementations, generics | Partial | Monomorphic named scalar functions | Closures, constraints, coherent implementations, specialization boundaries, and separate compilation verified |
| `Option` and `Result`; no null or unchecked exceptions | Missing | — | Standard types, propagation syntax, FFI mappings, exhaustive handling, and diagnostics verified |
| Immutable-by-default values and explicit mutation | Missing | — | Local, field, collection, and cross-task mutation rules verified |
| Unique ownership and move safety | Partial | Resource types, explicit `own` boundaries, straight-line use-after-move and illegal-transfer diagnostics | Use-after-move, double-free, invalid transfer, conditional moves, destructors, and FFI ownership are compile-time checked |
| Borrowed views and lifetime safety | Partial | Non-consuming `borrow` boundaries and move-after-borrow behavior | Mutable/shared aliasing, escaping borrows, reborrows, slices, and zero-copy FFI pass positive and compile-fail suites |
| Regions/arenas | Missing | — | Region inference and annotations prevent escape; bulk release and destructor behavior are verified |
| Shared immutable ARC and opt-in managed zones | Missing | — | Retain/release correctness, cycle policy, escape optimization, and concurrency constraints verified |
| Restricted `unsafe` and raw memory | Missing | — | Unsafe boundaries are explicit graph nodes with capability, audit summary, lint, and platform conformance coverage |
| Checked/wrapping/saturating arithmetic | Partial | Checked `i64` arithmetic in the C/Clang lane | Full numeric family, explicit alternative modes, SIMD behavior, and backend equivalence verified |
| Effects and capabilities | Partial | Declared function effects, module permits, and call-edge propagation | Inference, parameterized capabilities, no ambient authority, handlers, dependency summaries, and platform manifests verified |
| Contracts and progressive verification | Partial | Contract type checking and runtime guards | Static discharge, bounded symbolic/SMT checks, counterexamples, invariants/state machines, property tests, and proof obligations verified |
| Structured concurrency | Missing | — | Scoped tasks, cancellation, cleanup, `Sendable`/`Shareable`, deterministic scheduler, actors/reducers, and synchronization verified |
| Typed hygienic generation | Missing | — | Deterministic sandboxed generation is graph-visible and cannot perform unrestricted textual rewriting |

## Compiler and output targets

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Fast development lane | Missing | — | Cranelift JIT/AOT, incremental affected-node builds, hot reload, and debugger mapping verified |
| Optimizing native lane | Partial | Host C11/Clang AOT bootstrap | LLVM/MLIR lowering, LTO/PGO, cross-compilation, CPU specialization, debug/release parity, and reproducibility verified |
| WebAssembly core/components | Partial | Direct Wasm core module, browser ES runtime, checked arithmetic imports, contracts, HTML and capability manifest | Browser/WASI modules and Component Model artifacts, canonical ABI, async/resources, sandboxing, and conformance verified |
| Embedded and real-time | Missing | — | Bare-metal artifacts, no-runtime/no-allocation/no-blocking profiles, MMIO/volatile/atomics, linker control, and hardware/emulator tests verified |
| SIMD and GPU | Missing | — | Portable SIMD plus SPIR-V/WebGPU/platform kernels and memory/effect rules verified |

## Ecosystem interoperability

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Missing | — | Resolver, lockfile, compatibility, implementations, capabilities, conformance tests, provenance, signatures, licenses, SBOM, and reproducibility verified |
| Portable canonical ABI and native fast ABI | Missing | — | Equivalent interface semantics with documented copy/borrow behavior and cross-language conformance verified |
| C and Objective-C | Missing | — | Header import, raw bindings, ownership annotations, safe wrappers, error/string/buffer mappings, and tests verified |
| C++ | Missing | — | Stable shim workflow, exception/ownership policy, maintained adapters, and unsafe classification verified |
| Java and Kotlin | Missing | — | JVM metadata import, JNI generation, Android lifecycle/ownership integration, and bidirectional calls verified |
| Swift and Apple frameworks | Missing | — | Swift/Objective-C bindings, async/result/ownership mappings, framework metadata import, XCFramework output, and tests verified |
| JavaScript and TypeScript | Missing | — | Declaration import, promise/error/typed-array/callback/resource mapping, browser/Node hosts, and component transpilation verified |
| WIT and WebAssembly Components | Missing | — | Import/export, resources, futures/streams, versions, capabilities, adapters, and multi-language composition verified |
| OpenAPI, Protobuf/gRPC, GraphQL, and SQL | Missing | — | Deterministic schema import/generation, compatibility/migration rules, and live conformance fixtures verified |

## Application platforms

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| First-class application/state/UI dialect | Missing | — | Typed state/actions/update/view, semantic controls, accessibility, navigation, localization, assets, platform blocks, and custom rendering verified |
| Web | Partial | Deployable HTML/ES module/Wasm scalar package with an accessible result element | DOM/CSS output, accessible HTML, SSR, hydration, Wasm logic, browser capabilities, Canvas/WebGPU escape hatch, and deployable sample verified |
| iOS | Missing | — | Native code, Swift host, XCFramework/app project, UIKit/SwiftUI adapter, lifecycle, accessibility, signing metadata, and device/simulator sample verified |
| Android | Missing | — | Native code, Kotlin host, JNI, AAR/app project, Compose/View adapter, lifecycle, accessibility, manifests, and device/emulator sample verified |
| macOS | Partial | Host-native command-line executable | Native app bundle, AppKit/SwiftUI host, menus/windows/accessibility, packaging/signing metadata, and sample verified |
| Windows | Partial | `windows-latest` compiles and tests the compiler, builds and executes `meaning.exe` with result 42, and validates the browser/Wasm package in [CI run 31203270295](https://github.com/wavect/semaprax/actions/runs/31203270295) | Native app, WinUI host, accessibility, packaging metadata, and representative application sample verified |
| Linux | Partial | Host compilation exercised in CI | Native application, selected UI adapter, accessibility, AppImage/deb/rpm metadata, and sample verified |
| Edge and server | Partial | Host-native scalar CLI only | Server runtime, async I/O, HTTP/data adapters, native/WASI output, observability, deployment, and load/conformance tests verified |
| Plugins | Missing | — | Capability-limited Component Model plugins, lifecycle, versioning, resource limits, and hostile-plugin tests verified |

## Agent economics, review, and operations

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial | Dependency-depth context slices | Exact token budgets, contracts/tests/effect/ownership/target filters, relevance guarantees, and large-repository benchmarks verified |
| Impact analysis before modification | Missing | — | Call/type/contract/test/schema/migration/target/capability consumers are computed incrementally and verified on real repositories |
| Typed holes and compiler-generated repairs | Missing | — | Obligations and valid repair operations are machine-readable and proven sound by compile-fail/repair tests |
| Proof-carrying patches | Missing | — | Patch claims, tests, capability deltas, target expectations, and proof artifacts are independently verified before commit |
| Semantic human review | Missing | — | Behavioral/API/security/memory/unsafe/target/migration summaries are deterministic and checked against known changes |
| Sandboxed builds and dependencies | Missing | — | No ambient network/home/secrets; declared build capabilities and hostile package tests verified |
| Debugger, profiler, diagnostics, and operations | Partial | Stable diagnostics for the scalar seed | Source-level debugging/profiling, crash/trace mapping, observability, deployment diagnostics, and every backend verified |

## Final validation product

Completion requires one maintained offline-first product built from a shared SEMAPRAX codebase with web, iOS, Android, macOS, Windows, and Linux clients; native notifications and secure storage; local databases; native/WASI backend; authentication; background synchronization; a custom accelerated visual; one C library; one JavaScript package; and one WebAssembly component. Every artifact must be built and exercised in CI or on representative simulators/devices, with platform-specific implementations declared rather than hidden behind false portability.
