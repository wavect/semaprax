# Roadmap

The roadmap follows risk, not feature spectacle. Stable semantic editing, ownership inference, and component boundaries are more important than accumulating syntax.

## 0.1 — Executable semantic seed

Status: implemented in this repository.

- Canonical source and typed expression core.
- Stable declaration identities.
- Revisioned semantic graph and context slices.
- Versioned byte/node-bounded agent context with stable replay frontiers.
- Offline context economics with exact goldens and conservative quality routing.
- Effects, module permits, and contract guards.
- Machine-readable diagnostics.
- Atomic, stale-safe semantic renames.
- Checked native code generation.

## 0.2 — Useful core language

Status: in progress. Resource ownership boundaries and explicit
lifecycle/interface contracts, lexical `let`, typed `if/else`, partial-place
diagnostics, record construction/projection/immutable-update semantics, Graph v7, validated stable-ID HIR/type
facts, a mandatory replay-validated cleanup plan, versioned normalized-status
and semantic-event-dictionary types, native scalar status/out execution, a
browser-loadable scalar Wasm backend, and one narrow direct-trivial-resource
Wasm owned ABI are implemented. A private generated-callable native host now
connects the exact loader, authority, ledger, strict codecs, event dictionary,
and compiler-owned trace-path certificate. That host and the real Wasm lane
match the reference outcome, complete trace, publication, and final logical
liveness for all 14 cases at native O0/O2; the remaining public and language
gates below are not.

- Records, variants, `Option`, and `Result`.
- Exhaustive pattern matching.
- Generic functions with constraints.
- Modules, imports, and multi-file graph commits.
- First-class diagnostic repair operations.
- Property tests generated from types and contracts.
- A persistent graph daemon and JSON-RPC agent transport.
- Complete ownership/lifetime/region analysis across control flow.

Exit criterion: build a non-trivial CLI and edit it entirely through semantic transactions.

The aggregate tranche is specified in [RFC
0002](RFC-0002-ALGEBRAIC-DATA.md). RFC 0003 phases 1–2 now supply explicit
trivial/imported lifecycle syntax, declaration-only interface/import contracts,
source/HIR validation, and a target-neutral cleanup plan. Resolved functions
carry typed blocks/edges/regions/exits, guarded liveness, atomic call commits,
sticky status sources, cleanup order, and result publication; validation
independently rebuilds the plan, and Graph v7 serializes it. Checked Native64
and Wasm32 layouts cover nested `i64`, `bool`, and direct trivial-resource
fields; immutable-update cleanup consumes the base first, evaluates authored
replacements left-to-right, transfers untouched fields, and cleans displaced
live fields exactly once in reverse order. Empty records have frozen size and
alignment one on both profiles. The bounded production slice executes public
nested `i64`/`bool` records through native C11/Clang at O0/O2 and browser Wasm
under Node, including pointer parameters, caller-owned results, poison
preservation, exact evaluation order, and Wasm shadow-stack re-entry. A private
test-only resource-record scenario is projected from the same cleanup plan into
C and real Wasm with an exact common finalization trace and zero liveness; it
does not open public resource execution or any aggregate ABI.

Phase 3 now composes its formerly separate native evidence layers for the
private direct-trivial slice. Feature-gated compiler emission produces the
complete generated provider and descriptor v2; compile-time guards prove the C
compiler's architecture/OS/environment/object/endian profile or fail closed.
The host independently authenticates descriptor bytes, strict wire codecs,
dictionary, and the separately fingerprinted compiler trace-path trie-DFA,
then invokes the exact loader-instance callable through the same-thread
authority and atomic ownership ledger. Real generated shared libraries at O0
and O2 match the reference executor for all 14 scenarios, as does real
Node/Wasm.

This proves the narrow private semantics, not the public native boundary.
Compiler resource builds retain `SPX-B104` while broader fatal-process recovery
and quiescence remain nongeneralized and representative Android/iOS device
runtime evidence is absent. The pinned-nightly Rust-host ASan lane and the
full Linux/macOS/Windows matrix are green in [public run
31259216533](https://github.com/wavect/semaprax/actions/runs/31259216533); the
Rust-host evidence is the narrower [job
93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).
The public build-only API/CLI now emits one selected direct-trivial callable-v2
bundle with deterministic hashed metadata, but it exposes no execution,
admission, or adoption surface.
Imported lifecycles, calls, general aggregates, broader control flow,
cross-realm/worker identity, stable public aggregate ABIs, variants, matching,
concurrency, and fork recovery remain subsequent work.

Callable v3 is a separate bounded physical tranche: graph-derived providers
execute all 14 normal corpus scenarios through the private desktop loader and
OS-seeded receipt ledger at `-O0`/`-O2`, with zero measured Rust heap growth
across the irreversible interval. Decode-reserve failure quarantines exact
evidence, and seven joint provider/loader/host fixtures add physical-failure,
malformed, interruption, replay, and conflict evidence. Canonical pre-execute
unwind is also settled without entering execute. The private static-registration
logic now has a mandatory gate through the same host ledger for five distinct
iOS device, simulator, and Catalyst Rust targets, with no dynamic loader
surface. One exact arm64-Simulator link/runtime path is implemented and green in
hosted run 31318280135; app-host/device and broader iOS corpus evidence,
fatal-allocation/process-crash injection, Android device/lifecycle breadth,
and quiescence remain. The bounded Android JNI/APK path is green in [run
31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
None of these
private steps opens public admission or `SPX-B104`.

A first private native-desktop application seam has a hosted-green macOS
package/runtime path in [run 31338834586, job
93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230)
and a Windows path pending after [failed run 31340893685, job
93314358662](https://github.com/wavect/semaprax/actions/runs/31340893685/job/93314358662). It packages one
exact callable-v3 owned-identity provider with the existing loader and
authenticated receipt ledger; the macOS process rotates ownership across two
calls and verifies exact replay. This is headless packaging evidence,
not AppKit/SwiftUI/WinUI, accessibility, lifecycle, signing, installation, or
public application-language support.

A second private desktop seam now composes that engine behind one real AppKit
window/button and one real Win32 window/button. Each adapter verifies its
native accessibility name, sends a delayed button action through the platform
event loop, binds the engine bytes to a deterministic package manifest before
launch, requires the exact engine result, and reaches native close and
termination before publishing success. AppKit bounds engine termination; Win32
freezes its imported DLL set and rejects every export directory. Strict AppKit compilation and source
locks are green; packaged macOS AppKit execution is green in [job
93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230),
while Windows remains pending after [failed run 31340893685, job
93314358662](https://github.com/wavect/semaprax/actions/runs/31340893685/job/93314358662). General SEMAPRAX UI/state syntax, SwiftUI/WinUI, broad
accessibility/lifecycle, signing, installers, and distribution remain later.

The bounded Apple Swift/iOS application milestone is green in [run
31338834586, job
93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228):
target-bound device and Simulator static slices, a private XCFramework, Swift 6
complete-concurrency checking, and installed arm64-Simulator applications for
explicit and deterministic ARC cleanup. Physical-device, public-framework,
UI/accessibility, and general lifecycle work remains later.

WIT/component work has started with the deterministic private `SPXWIT01`
schema/adapter bundle and Node evidence. A separate standards-valid scalar
Component Model v1 artifact has a frozen digest, independent exact-profile
parser, hostile mutation coverage, and a private Node subset runtime. Checked
component v2 now composes the exact SEMAPRAX-generated scalar core with a
frozen checked runtime, passes pinned upstream validation and rehashed hostile
cross-type gates, and executes generated success, overflow, and contract
failure through its authenticated private `evaluate()` API. Portable Result
Component v3 now privately composes its exact checked two-scalar status/out core as
`result<s64, status>` and has local typed Wasmtime evidence for success,
addition overflow, division by zero, false precondition, and false
postcondition. Its independent/upstream validators, poison/sticky-status core
evidence, zero-import empty-linker/no-WASI runtime, and isolated locked
dependency/MSRV graph are implemented; the hosted Wasmtime gate is green in
[run 31338834586, job
93309086213](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086213).
Source-language `Result`/`Option`, records/resources/imports,
async/capabilities, multi-engine/browser execution, public API, and
`SPX-B104` remain later gates.

The model-backed, proposed [RFC 0004 native call recovery and settlement
contract](RFC-0004-NATIVE-CALL-SETTLEMENT.md) specifies the bounded linear
frame, certified checkpoint, idempotent settlement, receipt, and quiescence
model proposed for the physical-failure blocker. The hidden target-neutral model
and private compiler derivation from validated cleanup HIR now exist for the
current direct-trivial owned slice. A private `SPXNPRF1` proof envelope and
independent semantic parser bind that graph to the exact callable-v2 contract;
this grants no authority. The separate private [callable ABI
v3](NATIVE-CALLABLE-ABI-V3.md) fixes the descriptor/hash/graph/capacity and
linkage metadata plus seven complete, independently encoded/parsed runtime
roles: six provider wires and one host-only HMAC receipt. Its six-argument
execute ABI, payload-bearing frame, tags, digest DAG, and changed private known
answers are frozen. `CertifyOutcome` binds its embedded ordinal/outcome witness
to the
trace-certificate fingerprint through a nonzero host-recomputed digest and
rejects resealed mutations; this is not independent host acceptance of the
trace-path DFA certificate. The emitter is bound to its compiler build target
with no public/general Android/iOS/Windows machine-code cross-emission; a hidden
selector can emit exact target-bound iOS and Android evidence providers. Windows dynamic
runtime is green. Hosted run 31318280135 linked and executed the
`token.discard-two` provider on arm64 iOS Simulator at `-O0`/`-O2`. A pinned
Android job compiles x86_64/arm64 Bionic providers and runs the x86_64 path in
an API-35 emulator; hosted run 31320436726 is green. iOS device/app lifecycle
execution and the remaining iOS corpus are
absent. The five
gated iOS device, simulator, and macabi target identities remain distinct.
Private graph-derived
providers execute all 14 normal scenarios at `-O0`/`-O2`; the desktop v3 loader
binds exact descriptor bytes and all entry points to one root image; and the
host provides exact-instance receipt authority, authoritative owner
generations, atomic receipt/ledger publication, cached replay, and drop-safe
quarantine. One joint path now covers all 14 normal scenarios at `-O0`/`-O2`
without measured Rust heap growth across its irreversible interval. Canonical
pre-execute unwind now reaches authenticated abort receipt without entering
execute. Fatal allocator/crash recovery, hosted Android app/JNI execution and
device breadth, broader iOS runtime, and public compiler admission stay closed with
`SPX-B104`. A private
process-lifetime exact-address static registry now reaches the shared ledger in
non-Apple fake-function evidence; it makes no `dlopen`, unload, or device claim.

With the current private descriptor-v3 metadata defined, the hidden settlement
model starts at
the authenticated post-`CallCommit` boundary and makes one exact
`SettlementDecisionCommit`, provider settlement, and model `ReceiptCommitted`
eligibility executable.
Its 29 focused tests prove pre-decision unwind selects `Abort(HostUnwind)`,
post-decision unwind resumes the locked decision, every-finalizer interruption
quarantines without retry, candidate/committed replay is exact, and hostile
phase mutations preserve evidence. The private physical path proves
exact-instance reservation, allocation-free `CallCommit`, host-only
receipt authentication, one authoritative ledger publication, refreshed owned
generations, infallible pre-reserved quarantine on postcommit drop, and an
all-14-scenario provider/loader/host composition at `-O0`/`-O2`. The normal
joint path records zero Rust allocation/reallocation calls across the
irreversible interval; decode-reserve failure quarantines exact evidence, and
seven joint fixtures exercise returned failure, malformed wires, durable
interruption, replay, and conflict under normal builds, with provider sanitizer
evidence. Canonical pre-execute unwind is wired and the private static registry
exists. The bounded arm64 iOS Simulator and x86_64 Android Emulator providers
are green. The RFC-0003 private JNI/Kotlin ownership and
exception-normalization adapter is now implemented and source-locked: a
same-package no-UI Instrumentation APK packages exact x86_64 JNI and O0/O2
providers through plugin-free Gradle 9 `--offline`, while arm64 is
compile-and-inspect only. Its Kotlin wrapper confines the host to one
`HandlerThread`, uses `SPXAJH01` handles and `SPXAJS01` statuses, treats
`consume()` as the exact evidence path, and dispatches non-throwing Cleaner
fallback through the identical `PhantomReference` action. That exact hosted
API-35 x86_64 APK/Emulator execution is green in [run 31338834586, job
93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
GC collection, process-exit cleanup, AAR/application lifecycle/UI, device execution, broader
iOS corpus/app lifecycle, crash/fatal-allocation injection, and quiescence
evidence remain.
The pure model itself grants none of that authority and does not open
`SPX-B104`.

The dedicated Linux
[dynamic-provider sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 generated-provider cases through the host. It linked
the sanitizer runtimes without instrumenting Rust host code; unrelated
Clippy/GCC failures kept the overall workflow run red, so Windows and the full
platform matrix were unproven by that historical run. The later narrow Windows
callable/dependency-isolation run is green; broader application-platform
completion remains open.

The active phase-3 gate is [Owned resource vertical slice
v1](OWNED-RESOURCE-VERTICAL-V1.md). It requires one production-reachable,
thread-confined native host and one instance-confined Wasm host to execute the
same admitted direct-trivial-resource cleanup plan with exact reference-trace
equivalence. The private host now meets that semantic corpus requirement. The
remaining fallback cleanup/quiescence, public execution/admission, and mobile-
profile requirements keep the gate open, and every broader resource or
aggregate shape remains closed. Rust-host ASan instrumentation is green in the
public run cited above.

## 0.3 — Ownership and fast development

- Values, unique ownership, borrowed views, and regions.
- Escape analysis with actionable lifetime diagnostics.
- Explicit shared immutable reference counting.
- Restricted `unsafe` modules and review summaries.
- Cranelift JIT/AOT development backend.

Exit criterion: implement a zero-copy parser and server without a tracing GC.

## 0.4 — Components and packages

- Interface-first package format and target matrices.
- WIT import/export and WebAssembly Component output.
- Portable canonical ABI plus native fast ABI.
- Generated C headers and safe wrapper annotations.
- Capability-sandboxed reproducible package builds.
- Provenance, SBOM, license, and unsafe-code metadata.

Exit criterion: compose SEMAPRAX, Rust, and JavaScript components behind one interface contract.

## 0.5 — Concurrency and applications

- Structured tasks, cancellation, and deterministic scheduling.
- Effect handlers for deterministic tests.
- Application state and semantic UI dialects.
- DOM/CSS server rendering and hydration.
- Platform adapters beginning with web, then Apple and Android.

Exit criterion: ship one offline-first web/mobile validation application with shared logic and native escape hatches.

## 1.0 criteria

- Versioned language, graph, package, and component specifications.
- Reproducible native and component builds on supported targets.
- Stable debugger and profiler integration.
- Audited ownership and unsafe boundaries.
- Compatibility policy and migration tooling.
- At least one production validation system maintained across releases.
