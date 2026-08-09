# Full-goal completion matrix

This document is the authoritative audit checklist for the complete SEMAPRAX objective. A row is complete only when the linked implementation and automated evidence prove the stated gate. Design text, generated placeholders, or a successful build on a narrower target do not satisfy a broader gate.

Status values:

- **Implemented** — the gate is covered by executable evidence.
- **Partial** — useful implementation exists, but the full gate is not proven.
- **Missing** — no qualifying implementation exists yet.

[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) phases 1–2 are
implemented. Every resolved function carries an independently rebuilt
target-neutral cleanup CFG with storage/leaf liveness, regions, atomic call
commits, sticky failures, guarded finalization, and result publication; Graph
v6 serializes it. Phase-3 evidence now includes status/trace types, independent
plan replay, a reference executor, native scalar status/out execution,
deterministic host templates and [descriptor-only
v1](NATIVE-ADAPTER-DESCRIPTOR-V1.md), authenticated capability mechanics, and a
real unpublished native host that connects the v1 descriptor, exact loader
lease, same-thread OS-seeded authority, ownership ledger, and opaque owners in
Linux/macOS fixtures. Private [callable descriptor
v2](NATIVE-CALLABLE-ABI-V2.md) now binds compiler-derived execution/cleanup,
event-dictionary, and trace-path-certificate fingerprints plus exact symbols,
capacities, signature, and result. The compiler emits the complete guarded C11
provider; the ownership host independently parses and authenticates the
descriptor, dictionary, and trie-DFA certificate, then invokes the exact
instance-bound callable through its authority and atomic ledger.

WebAssembly separately implements the narrow [owned ABI
v1](WASM-OWNED-ABI-V1.md) for one direct trivial-resource identity with real
Node execution. Generated native C and Wasm now emit deterministic
dictionary-authenticated semantic ordinals for the same authoritative 14-case
corpus. Real generated native shared libraries execute through the physical
ownership host at O0/O2, and native/Node-Wasm both match the exact reference
trace, outcome, publication, and final logical liveness. Every excluded Wasm
shape remains `SPX-W111`. Public native resource execution remains blocked by
fallback cleanup/quiescence generalization, Android/iOS profiles, and public
execution/admission; `SPX-B104` remains closed. The bounded Linux Rust-host
ASan requirement is green in [public run 31259216533, job
93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).
That run also proves the build-only callable bundle on
[Ubuntu](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277094),
[macOS](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277081),
and [Windows](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277085),
without implying application-platform or public-execution support.
The build-only public callable-v2 API/CLI emits one deterministic hashed bundle
for a selected direct-trivial owned function. The generated corpus and hardened
dependency isolation passed on Windows in [run 31257545008, job
93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).

The proposed [RFC 0004 native call recovery and settlement
contract](RFC-0004-NATIVE-CALL-SETTLEMENT.md) defines a bounded callable-v3
frame/checkpoint/settlement/receipt foundation for the physical-failure and
quiescence gap. Its hidden target-neutral model and private compiler derivation
from validated cleanup HIR are implemented for the current direct-trivial
owned slice. A separately versioned private `SPXNPRF1` envelope now binds the
exact v2 call contract and trace certificate to a bounded binary graph, which
the host parses independently without loading or executing it. The separate
private `SPXNABI3` contract now fixes descriptor/hash/graph/capacity and
linkage metadata plus seven independently encoded/parsed bounded runtime
codecs: a six-argument execute ABI, payload-bearing frame, exact tags/digests,
candidate, and host-only HMAC receipt. `CertifyOutcome` binds an embedded
ordinal/outcome witness to
the trace-certificate fingerprint through a nonzero host-recomputed digest;
this is not independent host acceptance of the trace-path DFA certificate, and
resealed witness/digest mutations are rejected. The ordinary machine-code
emitter is build-target-bound with no public/general cross-target configuration;
a hidden closed selector emits complete target-bound iOS evidence providers for
five enumerated targets and closed arm64/x86_64 Android dynamic providers with
exact Bionic/ELF guards. Graph-derived private
providers now execute all 14 authoritative normal scenarios through exact
dynamic-image admission and the receipt ledger at `-O0`/`-O2`. That joint path
proves exact descriptor/instance binding, pre-settle copied-evidence validation,
replay, generation refresh, finalizer order, pin lifetime, and zero measured
Rust heap growth from immediately before `CallCommit` through `ReceiptCommit`.
Injected decode-reserve failure quarantines exact evidence/pins, and seven
joint failure/interruption fixtures cover physical return, malformed wires,
durable boundaries, replay, and decision conflict. Canonical pre-execute unwind
skips provider execute and reaches authenticated abort receipt commit. A
bounded static-registration model feeds the same ledger in non-Apple
fake-function evidence. A mandatory macOS gate now requires this static-only
composition to type-check for five distinct iOS device, simulator, and Catalyst
Rust targets, with dynamic loader and desktop v1/v2 surfaces absent. The same
mandatory job is configured to link and execute one exact arm64 iOS Simulator
`token.discard-two` provider through static registration and the authenticated
ledger at `-O0`/`-O2`; [run 31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
proved that bounded path. The standalone-process slice does not prove device or app
lifecycle execution, the remaining corpus on iOS, exhaustive crash/fatal-
allocator failure injection, Android app/JNI/device execution, quiescence,
malicious-code containment, physical-finalizer generality, or public admission;
`SPX-B104` remains closed.
The mandatory Android job compiles the loader/host and exact
providers for x86_64 and arm64, then runs one x86_64 `token.discard-two`
dynamic provider through the same receipt ledger at O0/O2 in an API-35
emulator. [Run 31320436726, job
93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
is green. This bounded native-process evidence does not satisfy the Android
application row's JNI/Kotlin/APK/lifecycle/UI gate.
The hidden linear phase model now starts from the sole authenticated
post-`CallCommit` state and exercises exact `SettlementDecisionCommit`,
provider-candidate, model-`ReceiptCommitted`, and absorbing `Quarantined`
evidence. Its 29 focused tests cover phase-aware unwind,
every-finalizer interruption, exact candidate/committed replay, hostile
cross-binding and state mutation, and preserved evidence. It deliberately
allocates and grants no exact-instance reservation, host authentication, ledger
publication, provider/FFI, loader retention, or physical finalizer authority.
Those physical gates remain required; this adds no native runtime evidence to a
completion row and leaves `SPX-B104` closed.

The dedicated Linux
[callable-host sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 dynamically loaded ASan/UBSan provider cases. It did
not instrument the Rust host code. The dependency-policy job was also green,
but unrelated Clippy/GCC failures kept that historical overall workflow red; it
is not the later Windows evidence linked above.

## Defining product contract

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Agent-native semantic program | Partial | Graph v6 serializes validated HIR plus complete target-neutral cleanup plans with lifecycle/interface/import/record/field identities, ownership/authority/failure contracts, expression meaning, recursive type facts, bounded lifecycle/call/type context, SHA-256 revision-bound renames, and fail-closed APIs | Versioned multi-file graph API covers callers, targets, tests, packages, generated artifacts, typed repairs, impact, and semantic review |
| Human-readable program | Partial | Canonical `.spx` source and formatter | Complete language round-trips deterministically; graph-aware merge/diff, debugger source mapping, and normal Git/editor workflows are verified |
| Meaning in, verified machine code out | Partial | Typed scalar core, effect checks, native status/out contracts and checked arithmetic with exact normalized failure codes, poison-preserving result publication, and a host-native scalar executable | All safe-language guarantees survive every backend; native artifacts and portable components pass conformance suites on every supported target |
| Atomic agent changes | Partial | Single-file stable-ID function/resource renames update calls and ownership type boundaries with domain-separated SHA-256 stale/legacy revision rejection | Typed, transactional multi-file edits support every public semantic operation and either commit fully or leave all source/graph state unchanged |

## Language and safety

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial | Canonical record declarations/construction/projection, stable field IDs, deterministic field diagnostics, recursive facts/cycle rejection, prefix-aware partial-place ownership, partial-initialization cleanup plans, validated record HIR, and Graph v6; executable backends fail closed | Immutable update, variants, exhaustive matching, target layouts, and native/Wasm cleanup execution verified |
| Functions, closures, interfaces, implementations, generics | Partial | Monomorphic named functions plus declaration-only resource interface/import contracts | Callable imports, closures, constraints, coherent implementations, specialization boundaries, and separate compilation verified |
| `Option` and `Result`; no null or unchecked exceptions | Missing | — | Standard types, propagation syntax, FFI mappings, exhaustive handling, and diagnostics verified |
| Immutable-by-default values and explicit mutation | Missing | — | Local, field, collection, and cross-task mutation rules verified |
| Unique ownership and move safety | Partial | Explicit trivial/imported lifecycles; move/partial-place analysis; replay-validated cleanup plans; hostile-HIR parity; a private exact-instance native callable host integrating OS-seeded authority, non-mutating ledger plans, atomic owner commit, generation rotation, strict codecs, and a compiler-authenticated trace-path DFA; one narrow Node-executed Wasm slice; exact reference/native-host-O0/O2/Wasm outcomes, traces, publication, and final logical liveness for all 14 cases; a green Linux dynamically loaded generated-provider ASan+UBSan corpus; and a green fail-closed pinned-nightly Rust-host ASan job inside the current hosted-CI matrix | Open the public native gate only after general physical/malformed-response fallback cleanup and quiescence and mobile evidence, then extend exactly-once/double-free proof through loops, closures, concurrency, and FFI ownership |
| Borrowed views and lifetime safety | Partial | Non-consuming `borrow` boundaries and move-after-borrow behavior | Mutable/shared aliasing, escaping borrows, reborrows, slices, and zero-copy FFI pass positive and compile-fail suites |
| Regions/arenas | Missing | — | Region inference and annotations prevent escape; bulk release and destructor behavior are verified |
| Shared immutable ARC and opt-in managed zones | Missing | — | Retain/release correctness, cycle policy, escape optimization, and concurrency constraints verified |
| Restricted `unsafe` and raw memory | Missing | — | Unsafe boundaries are explicit graph nodes with capability, audit summary, lint, and platform conformance coverage |
| Checked/wrapping/saturating arithmetic | Partial | Checked `i64` arithmetic in the C/Clang lane returns exact `semaprax.arithmetic.v1` statuses without internal process termination | Full numeric family, explicit alternative modes, SIMD behavior, and backend equivalence verified |
| Effects and capabilities | Partial | Declared function effects, module permits, and call-edge propagation | Inference, parameterized capabilities, no ambient authority, handlers, dependency summaries, and platform manifests verified |
| Contracts and progressive verification | Partial | Contract type checking and runtime guards | Static discharge, bounded symbolic/SMT checks, counterexamples, invariants/state machines, property tests, and proof obligations verified |
| Structured concurrency | Missing | — | Scoped tasks, cancellation, cleanup, `Sendable`/`Shareable`, deterministic scheduler, actors/reducers, and synchronization verified |
| Typed hygienic generation | Missing | — | Deterministic sandboxed generation is graph-visible and cannot perform unrestricted textual rewriting |

## Compiler and output targets

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Fast development lane | Missing | — | Cranelift JIT/AOT, incremental affected-node builds, hot reload, and debugger mapping verified |
| Optimizing native lane | Partial | Validated stable-ID HIR lowers to sequenced C11/Clang AOT; a public build-only callable-v2 API/CLI emits one selected direct-trivial function as a strict host shared library plus deterministic descriptor/dictionary/certificate and hashed manifest; the private host executes the 14-case corpus at O0/O2; Linux provider sanitizers, Windows callable/dependency isolation, and pinned-nightly Rust-host ASan evidence are green in the current Linux/macOS/Windows hosted-CI matrix | Public resource execution/admission, general fallback cleanup/quiescence, Android/iOS profiles, LLVM/MLIR lowering, LTO/PGO, cross-compilation, CPU specialization, debug/release parity, and reproducibility verified |
| WebAssembly core/components | Partial | Validated stable-ID HIR lowers to direct Wasm core with browser ES runtime, checked arithmetic, contracts, HTML, `semaprax.web.v3`, and a Node-executed `semaprax.wasm-owned.v1` subset for one direct trivial resource, including same-realm duplicated-host isolation and exact semantic ordinal/reference equality for the shared 14-case corpus | Browser/WASI modules and Component Model artifacts, general canonical resource ABI, async, sandboxing, cross-realm/worker identity, and production native-host/Wasm conformance verified |
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
| Web | Partial | Deployable HTML/ES module/Wasm package with an accessible scalar entry and a narrow Node-executed owned-resource adapter | DOM/CSS output, accessible HTML, SSR, hydration, general Wasm resource/components support, browser capabilities, Canvas/WebGPU escape hatch, and deployable sample verified |
| iOS | Missing | — | Native code, Swift host, XCFramework/app project, UIKit/SwiftUI adapter, lifecycle, accessibility, signing metadata, and device/simulator sample verified |
| Android | Missing | — | Native code, Kotlin host, JNI, AAR/app project, Compose/View adapter, lifecycle, accessibility, manifests, and device/emulator sample verified |
| macOS | Partial | Host-native command-line executable | Native app bundle, AppKit/SwiftUI host, menus/windows/accessibility, packaging/signing metadata, and sample verified |
| Windows | Partial | `windows-latest` compiles/tests the compiler and callable host, builds native and web artifacts, and confirms the generated O0/O2 callable corpus plus root-sibling/CWD/legacy-`PATH` dependency isolation in [run 31257545008, job 93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756) | Native app, WinUI host, accessibility, packaging metadata, and representative application sample verified |
| Linux | Partial | Host compilation exercised in CI | Native application, selected UI adapter, accessibility, AppImage/deb/rpm metadata, and sample verified |
| Edge and server | Partial | Host-native scalar CLI only | Server runtime, async I/O, HTTP/data adapters, native/WASI output, observability, deployment, and load/conformance tests verified |
| Plugins | Missing | — | Capability-limited Component Model plugins, lifecycle, versioning, resource limits, and hostile-plugin tests verified |

## Agent economics, review, and operations

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial | Dependency-depth context slices plus repository agent guidance; Graphify adoption is evidence-gated in ADR 0001 | Exact token budgets, contracts/tests/effect/ownership/target filters, relevance guarantees, and large-repository benchmarks verified |
| Impact analysis before modification | Missing | — | Call/type/contract/test/schema/migration/target/capability consumers are computed incrementally and verified on real repositories |
| Typed holes and compiler-generated repairs | Missing | — | Obligations and valid repair operations are machine-readable and proven sound by compile-fail/repair tests |
| Proof-carrying patches | Missing | — | Patch claims, tests, capability deltas, target expectations, and proof artifacts are independently verified before commit |
| Semantic human review | Missing | — | Behavioral/API/security/memory/unsafe/target/migration summaries are deterministic and checked against known changes |
| Sandboxed builds and dependencies | Missing | — | No ambient network/home/secrets; declared build capabilities and hostile package tests verified |
| Debugger, profiler, diagnostics, and operations | Partial | Stable diagnostics for the scalar seed | Source-level debugging/profiling, crash/trace mapping, observability, deployment diagnostics, and every backend verified |

## Final validation product

Completion requires one maintained offline-first product built from a shared SEMAPRAX codebase with web, iOS, Android, macOS, Windows, and Linux clients; native notifications and secure storage; local databases; native/WASI backend; authentication; background synchronization; a custom accelerated visual; one C library; one JavaScript package; and one WebAssembly component. Every artifact must be built and exercised in CI or on representative simulators/devices, with platform-specific implementations declared rather than hidden behind false portability.
