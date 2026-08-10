# Full-goal completion matrix

This document is the authoritative audit checklist for the complete SEMAPRAX objective. A row is complete only when the linked implementation and automated evidence prove the stated gate. Design text, generated placeholders, or a successful build on a narrower target do not satisfy a broader gate.
The dashboard is refreshed at meaningful executable-evidence milestones, not
for each internal refactor, so progress remains visible without inflating
status from configuration or design alone.

Status values:

- **Implemented** — the gate is covered by executable evidence.
- **Partial** — useful implementation exists, but the full gate is not proven.
- **Missing** — no qualifying implementation exists yet.

## Milestone dashboard

This compact view is the release-truth summary; the detailed rows below remain
the completion contract.

| Milestone | Status | Evidence boundary |
| --- | --- | --- |
| Human and agent semantic projections | Partial | Canonical `.spx`, validated stable-ID HIR, atomic single-file renames, Graph v10 for legacy/Result programs, feature-minimal Graph v11 for bounded Option propagation, program-derived Graph v12 for any authenticated generic record declaration, and Graph v13 for any authenticated explicit record pattern are executable. V13 binds recursive exact record/member/binding identity and takes precedence over v12/v11/v10; wildcard-only record matches remain schema-neutral, and every context uses the same program-wide schema. Immutable record update, generic Copy record/variant construction, exact structured concrete arguments, authenticated ordinary prelude types, and typed propagation meaning are included; multi-file typed repair/impact remains open |
| Ownership and cleanup meaning | Partial | Move/partial-place checks plus independently rebuilt and replayed CleanupPlan v2 plans, and feature-minimal v3 plans for bounded Option propagation, are executable, including exact body/Result-residual/Option-None Copy-result staging and shared postcondition/publication joins; general lifetimes, aliases, concurrency, FFI, and public physical cleanup remain open |
| Aggregate records v1 bounded execution | Partial | Construction/projection/update, stable IDs, checked Native64/Wasm32 layouts, frozen one-byte/alignment-one empty records, and cleanup are executable; nested public scalar records and exact-instance generic Copy records with ordered direct `i64`/`bool` arguments run through native C11 O0/O2 and Node/Wasm, with the generic-record gate hosted green in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995). Bounded irrefutable Copy-record matches now destructure exact nested/generic instances with scalar or whole-record bindings, ignored fields, one evaluation, scalar arms, program-wide Graph v13, unchanged straight-line CleanupPlan v2/v3, Native O0/O2, and 4,096-entry Node/Wasm; the Ubuntu gate is hosted green in [run 31373317800, job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130), and independent security review is green. One private shared-plan resource harness separately proves an exact cross-backend cleanup trace and zero liveness. Stable public aggregate ABIs, public resource-record execution, nested/resource/non-Copy generic breadth, refutable or ownership-aware matching, and general aggregate execution remain open |
| Copy variants + bounded generics/prelude/`?` | Partial | Nominal variants with explicit direct `i64`/`bool` arguments, ordinary compiler-owned `Option<T>`/`Result<T,E>`, exhaustive copy match, exact-instance layouts, and Native O0/O2 plus Node/Wasm are hosted green in [run 31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201). Bounded postfix `?` for direct-scalar Copy `Result<T,E>` is hosted green in [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690). The analogous direct-scalar Copy `Option<T>` to `Option<U>` tranche is hosted green in [run 31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277): one evaluation, exact outer payload-free `None`, later-work skip, shared postconditions/commit, physical-status separation, poison, invalid-tag closure, CleanupPlan v3/Graph v11 authentication, and Native O0/O2 plus Node/Wasm equivalence. Generic functions/inference, nested/resource/non-Copy generic records or variant arguments, non-copy propagation/matching, residual conversion, `?` in contracts, stable public aggregate ABI, callable/component signatures, and public resource admission remain open |
| Native code and interop | Partial | Scalar C11/Clang and bounded private callable/resource evidence exist; public general native execution and C/Objective-C/Swift/Kotlin ecosystem import remain open |
| Web and portable components | Partial | Scalar Core Wasm, public scalar/nested and generic Copy records, bounded Copy-record patterns, bounded generic/prelude/typed-`?` copy-variant Core Wasm, narrow owned-resource Wasm, and private WIT/component evidence exist. Portable Result v3, Source-Result v4, Scalar Algebraic v5, and Nested Record v6 runners are hosted green, with v6 evidence in [run 31365363898, job 93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974). Private Generic Record Component v7 freezes four exact `Duo`/`Phantom` concrete-instance exports under `semaprax:private@0.5.0`; its pinned Rust 1.97.1/Wasmtime 47 runner is hosted green in [run 31373317800, job 93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922). General algebraic mapping, general/empty/generic/resource component records, imports/capabilities, callable/FFI aggregate signatures, browser/multi-engine conformance, and public API/ABI remain open |
| Desktop and mobile applications | Partial | Private macOS engine/AppKit ([job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230)), Windows engine/Win32 UI ([job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480)), Swift/iOS XCFramework/app ([job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228)), and Android JNI/Kotlin app ([job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206)) gates are green. Public SDKs, UI language, lifecycle breadth, signing/distribution, and device breadth remain open |
| Full SEMAPRAX product objective | Partial | No single lane proves native mobile + desktop + web + broad interop + full ownership/lifetime safety together; the global goal is not complete |

[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) phases 1–2 are
implemented. Every resolved function carries an independently rebuilt
target-neutral cleanup CFG with storage/leaf liveness, regions, atomic call
commits, sticky failures, guarded finalization, and result publication; Graph
v10 serializes CleanupPlan v2, while authenticated bounded Option propagation
uses program-level Graph v11 and per-function CleanupPlan v3 without migrating
legacy/Result-only output; any authenticated generic record declaration takes
program-level Graph v12 precedence, and any authenticated explicit record
pattern takes Graph v13 precedence, while retaining the appropriate per-function
cleanup schema. Wildcard-only record matches do not select v13. Phase-3 evidence now includes status/trace types, independent
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
fallback cleanup/quiescence generalization, production Android/iOS profiles,
and execution/admission; `SPX-B104` remains closed. The bounded Linux Rust-host
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
proved that bounded path. The standalone-process slice does not prove device or
app lifecycle execution, the remaining corpus on iOS, exhaustive crash/fatal-
allocator failure injection, Android device execution, quiescence,
malicious-code containment, physical-finalizer generality, or public admission.
It is not the Android APK/JNI gate; that separate green hosted evidence is
recorded below. `SPX-B104` remains closed.
The mandatory Android job compiles the loader/host and exact
providers for x86_64 and arm64, then runs one x86_64 `token.discard-two`
dynamic provider through the same receipt ledger at O0/O2 in an API-35
emulator. [Run 31320436726, job
93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
is green. This bounded native-process evidence does not by itself satisfy the
Android application row's JNI/Kotlin/APK/lifecycle/UI gate.

A separate private [Android JNI ownership adapter
v1](ANDROID-JNI-OWNERSHIP-V1.md) is now implemented and CI-configured. Local
Rust/C tests and source locks cover the closed `RegisterNatives` table,
`SPXAJH01` handle ownership, `SPXAJS01` status/exception normalization,
HandlerThread confinement, deterministic `PhantomReference` cleanup action,
poison-preserving outputs, exact finalizer evidence, and the plugin-free offline
APK packaging contract. The same-package no-UI Instrumentation APK is configured
to install on API 35 x86_64 and exact-match one app-private result after O0
explicit `consume()` and O2 Cleaner paths; arm64 is compile/ELF inspection only.
The exact APK build/install/Instrumentation path is green in [run 31338834586,
job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
This moves only the Java/Kotlin and Android rows to **Partial**; it proves no GC collection,
process-exit cleanup, AAR, UI/accessibility, general lifecycle, device or arm64
runtime, public ABI/admission, or `SPX-B104` change.

The private [Apple Swift ownership adapter
v1](APPLE-SWIFT-OWNERSHIP-V1.md) and [WIT boundary
v1](WIT-COMPONENT-BOUNDARY-V1.md) are implemented with local Rust/Node and
source-lock evidence. Swift/iOS is **Partial** for the closed same-thread
wrapper and green bounded XCFramework/Simulator-app gate in [run 31338834586,
job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228).
WIT is **Partial** for deterministic schema/adapter output, a separate
independently parsed scalar Component Model fixture, checked v2 composition,
and private Portable Result Component v3. V3 binds the exact generated scalar
core to `result<s64, status>`, passes independent and upstream validation, and
has local typed Wasmtime evidence for success, addition overflow, division by
zero, false precondition, and false postcondition with zero imports, an empty
linker, and no WASI. Poison preservation and sticky status selection remain
separately frozen at the generated-core boundary. Its isolated runtime graph
cannot widen the public compiler graph or MSRV. The current prelude-bound KAT
migration and standalone runner are hosted green in [run 31347109201, job
93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).
The separate private Source-Result Component v4 connects one exact
effect-free source closure using compiler-owned `Result<i64, bool>`, postfix
`?`, and `Result<bool, bool>` to WIT 0.2
`result<result<bool, bool>, status>`. Its source/core/profile/component and
layout-v2 bindings have frozen KATs; independent and upstream validators,
every-byte/cross-profile hostile tests, generated-core Node outcomes, default-
consumer hiding, and CI source locks are green. The isolated runner executes
typed inner `Ok`/`Err`, outer status, residual short-circuit, sticky failure,
shared postconditions, re-entry, fresh instances, and fuel failure in [run
31356536123, job
93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
Private Scalar Algebraic Component v5 separately freezes the six direct-copy
`Option`/complete `Result`-matrix exports, exact layouts and identity mapping,
hostile profile closure, and zero-import runner source/KATs; pinned Rust 1.97.1
typed execution is hosted green in [run 31360176398, job
93367728269](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728269).
Private Nested Record Component v6 separately freezes WIT package
`semaprax:private@0.4.0`, interface `nested-records`, world
`semaprax-private-v6`, and one fixed nested scalar-record `transform` export.
Exact source/core/Inner-layout/Outer-layout/profile/component/DAG KATs,
independent/upstream validation, hostile closure, local generated-core
execution, default-consumer hiding, source locks, and security review are
green. Its pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
31365363898, job
93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974).
Private Generic Record Component v7 separately freezes WIT package
`semaprax:private@0.5.0`, interface `generic-records`, world
`semaprax-private-v7`, and four exact exports over ordered `Duo<i64, bool>`,
`Duo<bool, i64>`, `Phantom<i64>`, and `Phantom<bool>` instances. Exact source/
core/layout/Graph-v12/plan/profile/component identity, same-layout Phantom
separation, independent/upstream validation, hostile closure, local generated-
core Node execution, default-consumer hiding, source locks, and security review
are green. Its pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in
[run 31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
V1-v6 bytes remain
unchanged. General source `Result`/`Option`/`?`
component mapping, general/empty/generic/resource record mapping, imports/capabilities, callable/FFI
aggregate signatures, public component API/ABI, and `SPX-B104` remain absent.
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
| Agent-native semantic program | Partial | Graph v10 serializes validated legacy/Result HIR plus complete CleanupPlan v2 plans; program-level Graph v11 and per-function CleanupPlan v3 are derived only from authenticated bounded Option propagation; program-level Graph v12 takes precedence for authenticated generic record declarations; Graph v13 takes highest precedence for authenticated explicit record patterns while wildcard-only record matches remain schema-neutral. Exact `try_result`/`try_option` carrier identities, generic-record owner/index parameters and structured concrete instances, recursive record-pattern instance/member/binding identities, mixed legacy-root context schema binding, recursive type facts, bounded context, prelude-bound SHA-256 revision-v2 renames, and fail-closed APIs are executable while prior v10-v12 outputs remain unchanged when v13 is not selected; the record-pattern Ubuntu gate is hosted green in [run 31373317800, job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130) | Versioned multi-file graph API covers callers, targets, tests, packages, generated artifacts, typed repairs, impact, and semantic review |
| Human-readable program | Partial | Canonical `.spx` source and formatter | Complete language round-trips deterministically; graph-aware merge/diff, debugger source mapping, and normal Git/editor workflows are verified |
| Meaning in, verified machine code out | Partial | Typed scalar core, effect checks, native status/out contracts and checked arithmetic with exact normalized failure codes, poison-preserving result publication, and a host-native scalar executable | All safe-language guarantees survive every backend; native artifacts and portable components pass conformance suites on every supported target |
| Atomic agent changes | Partial | Single-file stable-ID function/resource renames update calls and ownership type boundaries with domain-separated SHA-256 stale/legacy revision rejection | Typed, transactional multi-file edits support every public semantic operation and either commit fully or leave all source/graph state unchanged |

## Language and safety

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial | Canonical record declarations/construction/projection/immutable update plus bounded record and variant templates explicitly instantiated with direct `i64`/`bool` arguments, ordinary compiler-owned `Option<T>`/`Result<T,E>`, exhaustive copy-variant matching, bounded irrefutable Copy-record destructuring, and direct-scalar postfix `?`; stable template member/case IDs; owner/index-ordered multi-parameter record substitution; deterministic diagnostics; exact instance/cache/digest/symbol and recursive pattern/member/binding confusion rejection; recursive facts/cycle rejection; record partial-place ownership; checked deterministic Native64/Wasm32 internal layouts; independently replayed cleanup; Graph v10/v11/v12 plus explicit-record-pattern-precedence v13. Public nested scalar records, generic Copy records, and bounded generic/prelude/typed-`?` values execute through native C11 O0/O2 and Node/Wasm, with generic records hosted green in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995). Record patterns prove recursive/whole-record bindings, one evaluation, failure order, poison, postconditions, unchanged straight-line cleanup, and 4,096-entry Wasm re-entry in hosted [run 31373317800, Ubuntu job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130); one private shared-plan resource-record harness proves exact cleanup trace and zero liveness | Public resource-bearing record execution/admission, nested or resource generic arguments/fields, resource- or record-bearing variant payloads, generic functions/inference, refutable/literal/guard/or/rest/nested-variant patterns, non-Copy ownership-aware matching/propagation, residual conversion, `?` in contracts, callable/component aggregate signatures, stable public aggregate ABIs, and general native/Wasm aggregate execution verified; ordinary `SPX-B104`/`SPX-W111` gates remain closed |
| Functions, closures, interfaces, implementations, generics | Partial | Monomorphic named functions, bounded explicitly instantiated generic Copy record and variant declarations with direct `i64`/`bool` arguments, and declaration-only resource interface/import contracts | Generic functions/inference, nested/resource/non-Copy instantiations, callable imports, closures, constraints, coherent implementations, specialization boundaries, and separate compilation verified |
| `Option` and `Result`; no null or unchecked exceptions | Partial | Ordinary compiler-owned `semaprax.prelude.v1` variants with explicit direct `i64`/`bool` arguments, exhaustive copy matching, and native C11 O0/O2 plus Node/Wasm execution. Bounded `Result` postfix `?` proves one evaluation, exact residual `E`, outer-layout reconstruction, shared postconditions/publication, status separation, and poison in hosted [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690). Bounded `Option` postfix `?` proves one evaluation, exact outer payload-free `None`, shared postconditions/publication, status separation, poison, CleanupPlan v3, and program-bound Graph v11 (raised to v12 by a generic record declaration or v13 by an explicit record pattern) in hosted [run 31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277) | Nested/resource arguments, general/non-copy `?`, residual conversion, FFI/component mappings, non-copy ownership modes, and a stable public aggregate ABI verified |
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
| Optimizing native lane | Partial | Validated stable-ID HIR lowers to sequenced C11/Clang AOT, including public nested scalar records, exact-instance generic Copy records, bounded irrefutable Copy-record patterns, and bounded copy variants/matches executed at O0/O2 with internal pointer parameters and caller-owned aggregate result storage; generic record C symbols bind the full record ID plus ordered arguments even for same-layout instances, and record patterns stage one scrutinee before recursive projection. A public build-only callable-v2 API/CLI emits one selected direct-trivial function as a strict host shared library plus deterministic descriptor/dictionary/certificate and hashed manifest; the private host executes the 14-case corpus at O0/O2; Linux provider sanitizers, Windows callable/dependency isolation, and pinned-nightly Rust-host ASan evidence are green in the current Linux/macOS/Windows hosted-CI matrix | Public resource execution/admission, stable public aggregate ABI, refutable/non-Copy matching, general fallback cleanup/quiescence, Android/iOS profiles, LLVM/MLIR lowering, LTO/PGO, cross-compilation, CPU specialization, debug/release parity, and reproducibility verified |
| WebAssembly core/components | Partial | Validated stable-ID HIR lowers to direct Wasm core with browser ES runtime, checked arithmetic, contracts, HTML, `semaprax.web.v3`, public nested scalar and exact-instance generic Copy records, bounded irrefutable Copy-record patterns, bounded generic/prelude copy variants, and hosted bounded typed-`Result` and typed-`Option` propagation with complete poison, exact concrete layouts, invalid-tag closure, shared postconditions, and restored shadow-stack state. Generic records are hosted green through 4,096-call Node re-entry in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995); record-pattern Node/Wasm 4,096-entry evidence is hosted green and security-clean in [run 31373317800, Ubuntu job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130). A Node-executed `semaprax.wasm-owned.v1` subset covers one direct trivial resource. Private components v1-v6 retain their exact evidence; private Generic Record Component v7's pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run 31373317800, job 93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922) | Browser/WASI modules, general source-language algebraic Component mapping, general/resource-bearing records or variants, refutable/non-Copy patterns, imports, async/capabilities, multi-engine conformance, general canonical resource ABI, stable public aggregate ABI, callable/FFI aggregate signatures, sandboxing, cross-realm/worker identity, public component API/ABI, and production native-host/Wasm conformance verified |
| Embedded and real-time | Missing | — | Bare-metal artifacts, no-runtime/no-allocation/no-blocking profiles, MMIO/volatile/atomics, linker control, and hardware/emulator tests verified |
| SIMD and GPU | Missing | — | Portable SIMD plus SPIR-V/WebGPU/platform kernels and memory/effect rules verified |

## Ecosystem interoperability

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Missing | — | Resolver, lockfile, compatibility, implementations, capabilities, conformance tests, provenance, signatures, licenses, SBOM, and reproducibility verified |
| Portable canonical ABI and native fast ABI | Missing | — | Equivalent interface semantics with documented copy/borrow behavior and cross-language conformance verified |
| C and Objective-C | Missing | — | Header import, raw bindings, ownership annotations, safe wrappers, error/string/buffer mappings, and tests verified |
| C++ | Missing | — | Stable shim workflow, exception/ownership policy, maintained adapters, and unsafe classification verified |
| Java and Kotlin | Partial | Private generated JNI shim plus minSdk-28 Kotlin ownership wrapper: closed `RegisterNatives`, HandlerThread confinement, generation-tagged handles, fixed status/exception normalization, deterministic identical Cleaner action, explicit `consume()` ownership transfer, and green API-35 x86_64 APK/Instrumentation evidence in [run 31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206) | JVM metadata import, public JNI generation, general Android lifecycle/ownership integration, bidirectional calls, and representative hosted conformance verified |
| Swift and Apple frameworks | Partial | Private Swift 6 ownership wrapper, stable-thread static host, generation-tagged handles, target-bound device/simulator fixtures, and bounded XCFramework/installed-Simulator execution are green in [run 31338834586, job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228) | Public Swift/Objective-C bindings, async/result/ownership breadth, framework metadata import, distributable XCFramework output, and representative tests verified |
| JavaScript and TypeScript | Missing | — | Declaration import, promise/error/typed-array/callback/resource mapping, browser/Node hosts, and component transpilation verified |
| WIT and WebAssembly Components | Partial | Deterministic private `SPXWIT01` boundary plus standalone v1, checked v2, Portable Result v3, Source-Result v4, Scalar Algebraic v5, and Nested Record v6 remain frozen; v3-v6 typed runners are hosted green, including v6 in [run 31365363898, job 93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974). Private Generic Record Component v7 fixes package `semaprax:private@0.5.0`, interface `generic-records`, world `semaprax-private-v7`, and four exact ordered `Duo`/same-layout-distinct-`Phantom` concrete-instance exports. Its source/core/layout/Graph-v12/plan/profile/component KATs, canonical reconstruction, mutation/reindexing/cross-version closure, upstream validation, default-consumer hiding, local Node core execution, source locks, and security review are green; pinned Rust 1.97.1/Wasmtime 47 hosted execution is green in [run 31373317800, job 93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922) | General source selection/export, general/empty/generic/resource record mapping or algebraic carriers, imports/capabilities, futures/streams, callbacks/reentrancy, browser and multi-engine conformance, package/version negotiation, callable/FFI aggregate signatures, public API/ABI, and `SPX-B104`/`SPX-W111` admission verified |
| OpenAPI, Protobuf/gRPC, GraphQL, and SQL | Missing | — | Deterministic schema import/generation, compatibility/migration rules, and live conformance fixtures verified |

## Application platforms

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| First-class application/state/UI dialect | Missing | — | Typed state/actions/update/view, semantic controls, accessibility, navigation, localization, assets, platform blocks, and custom rendering verified |
| Web | Partial | Deployable HTML/ES module/Wasm package with an accessible scalar entry and a narrow Node-executed owned-resource adapter | DOM/CSS output, accessible HTML, SSR, hydration, general Wasm resource/components support, browser capabilities, Canvas/WebGPU escape hatch, and deployable sample verified |
| iOS | Partial | Existing private static callable runtime plus a Swift 6 same-thread host, device/universal-Simulator XCFramework construction, and two installed arm64-Simulator app paths are green in [run 31338834586, job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228) | Public native/Swift host, distributable framework/app project, UIKit/SwiftUI adapter, lifecycle, accessibility, signing metadata, and physical-device plus representative simulator samples verified |
| Android | Partial | Private same-package no-UI Instrumentation APK executes on an API-35 x86_64 Emulator with offline plugin-free packaging, exact JNI/O0/O2 inventory and ownership assertions in [run 31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206); arm64 remains compile/ELF inspection only | Public native code and Kotlin/JNI host, AAR/app project, Compose/View adapter, lifecycle, accessibility, manifests/packaging, and representative emulator plus device samples verified |
| macOS | Partial | A private headless `APPL` engine and AppKit frontend with one visible window/button, native accessibility label, pre-launch engine digest, bounded terminate/kill path, and ordered control/close/terminate evidence are green in [run 31338834586, job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230) | Public/general AppKit or SwiftUI host, signed engine provenance, menus/navigation, comprehensive accessibility/lifecycle, signing/notarization metadata, and representative sample verified |
| Windows | Partial | A private portable PE engine package and separate Win32 GUI-subsystem frontend with one visible window/button, `IAccessible` name, pre-launch engine digest, exact imported-DLL/no-export-directory contract, and ordered control/destroy/quit path are green in [run 31343897595, job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480). Earlier hosted evidence also confirms the callable corpus and dependency isolation in [run 31257545008, job 93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756) | Public/general Win32 or WinUI host, signed engine provenance, comprehensive accessibility/lifecycle, installer/MSIX/signing metadata, and representative application sample verified |
| Linux | Partial | Host compilation exercised in CI | Native application, selected UI adapter, accessibility, AppImage/deb/rpm metadata, and sample verified |
| Edge and server | Partial | Host-native scalar CLI only | Server runtime, async I/O, HTTP/data adapters, native/WASI output, observability, deployment, and load/conformance tests verified |
| Plugins | Missing | — | Capability-limited Component Model plugins, lifecycle, versioning, resource limits, and hostile-plugin tests verified |

## Agent economics, review, and operations

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial | Versioned deterministic `semaprax.agent-context.v1` adds exact whole-JSON byte and function-node budgets, used/omitted/deferred accounting, closed truncation reasons, query-bound stable-ID progress frontiers with non-dangling emitted call edges, aggregate pagination plus individual-page oversize rejection, strict CLI options, and selectable compact contracts, parameter/result ownership, effects, and reference-closed types. Offline `semaprax.agent-context-economics.v1` freezes four maintenance questions, two exact context goldens, canonical exact-case/separator-normal/Windows-forbidden-and-reserved-safe non-symlink source containment, exact manifest/context digests and label IDs, supported-facet-only source/context byte and non-model lexical-unit economics, reviewed relevance/evidence recall, mutations, and conservative explicit-or-unique-target-merge-base plus dirty-Git-reconciled quick/changed/full routing with an exact ordered executable gate plan. The small corpus records context larger than source; cleanup/lifecycle/import and target/diagnostic/test facets remain unavailable, and Graphify remains deferred under ADR 0001 | Exact model-token budgets, real target/diagnostic/test graph edges, cleanup/lifecycle/import facets, answer-quality/relevance guarantees, persistent indexing, representative large-repository/model benchmarks, and measured savings verified |
| Impact analysis before modification | Missing | — | Call/type/contract/test/schema/migration/target/capability consumers are computed incrementally and verified on real repositories |
| Typed holes and compiler-generated repairs | Missing | — | Obligations and valid repair operations are machine-readable and proven sound by compile-fail/repair tests |
| Proof-carrying patches | Missing | — | Patch claims, tests, capability deltas, target expectations, and proof artifacts are independently verified before commit |
| Semantic human review | Missing | — | Behavioral/API/security/memory/unsafe/target/migration summaries are deterministic and checked against known changes |
| Sandboxed builds and dependencies | Missing | — | No ambient network/home/secrets; declared build capabilities and hostile package tests verified |
| Debugger, profiler, diagnostics, and operations | Partial | Stable diagnostics for the scalar seed | Source-level debugging/profiling, crash/trace mapping, observability, deployment diagnostics, and every backend verified |

## Final validation product

Completion requires one maintained offline-first product built from a shared SEMAPRAX codebase with web, iOS, Android, macOS, Windows, and Linux clients; native notifications and secure storage; local databases; native/WASI backend; authentication; background synchronization; a custom accelerated visual; one C library; one JavaScript package; and one WebAssembly component. Every artifact must be built and exercised in CI or on representative simulators/devices, with platform-specific implementations declared rather than hidden behind false portability.
