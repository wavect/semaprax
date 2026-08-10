# SEMAPRAX

> **The agent-native systems programming language**
> **Meaning in. Verified machine code out.**

SEMAPRAX is an experimental programming system where source code is the human projection and a stable, queryable semantic graph is the agent interface. The v0.2 prototype accepts a small typed language, verifies its declared meaning, and lowers it to a native executable or a deployable browser/WebAssembly package.

```text
Human source       Atomic semantic patches
     \                    /
      Versioned semantic graph
       /       |          \
  types     effects     contracts
       \       |          /
       validated stable-ID HIR
          /             \
  C11 native lane   Wasm core lane
          |             |
 native executable  browser package
```

This repository is an executable architectural seed, not a claim that the full language described in the RFC already exists. The prototype deliberately tackles the differentiator first: stable semantic identity, graph-native context, machine diagnostics, capability-aware verification, stale-safe transactions, deterministic lowering, and real native output.

## Try it

Requirements: Rust 1.85+ and Clang. Node.js 22+ is required for the shown browser/Wasm verification command.

```sh
cargo run -- check examples/meaning.spx
cargo run -- graph examples/meaning.spx
cargo run -- context examples/meaning.spx app.main --depth 1 --max-bytes 65536 --max-nodes 256
cargo run -- run examples/meaning.spx
cargo run -- run examples/control_flow.spx
cargo run -- build examples/native_callable.spx --target native-callable --function example.token.identity -o target/native-callable
cargo run -- build examples/control_flow.spx --target web -o target/control-flow-web
node scripts/verify-web.mjs target/control-flow-web
```

The native `run` commands compile and execute host binaries; the control-flow example prints:

```text
42
```

Install the CLI locally:

```sh
cargo install --path .
semaprax build examples/meaning.spx -o meaning
./meaning
```

## The v0.2 language

```semaprax
module examples.meaning;

@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(19, 23)
}
```

Implemented today:

- `i64` and `bool`, typed functions, calls, unary and binary expressions.
- Resources with explicit, persistent trivial/imported lifecycles, declaration-only interface/import contracts, and `own`, `borrow`, and `shared` function boundaries.
- Lexical `let` bindings and typed `if/else` expressions.
- Control-flow-aware move checking with prefix-aware record-field state and definite or conditional use-after-move diagnostics.
- Canonical record declarations, construction, projection, and immutable update in `check`, resolved HIR, and semantic Graph v10, with persistent record/field IDs and authored-order replacement semantics.
- Checked deterministic Native64 and Wasm32 record layouts plus target-neutral cleanup plans for partial construction and immutable update. These are compiler groundwork; backend execution is claimed only where its dedicated artifact gates pass.
- The bounded public scalar-record slice lowers nested `i64`/`bool` construction,
  projection, and immutable update through native C11/Clang at O0/O2 and browser
  Wasm executed by Node. It preserves base-first/authored-order evaluation,
  status/out poison, internal pointer parameters, caller-owned results, and Wasm
  shadow-stack restoration. Empty records have the same frozen one-byte,
  alignment-one representation on both layout profiles.
- One private test-only resource-record harness projects the shared cleanup plan
  into C11 O0/O2 and real Wasm with an exact common finalization trace and zero
  final liveness. It is proof scaffolding only: public resource-bearing record
  admission, callable/component aggregate signatures, and `SPX-B104`/`SPX-W111`
  remain unchanged.
- A bounded copy-variant slice supports nominal variant templates instantiated
  explicitly with direct `i64`/`bool` arguments, plus monomorphic unit and
  direct-scalar variants. It includes qualified construction and exhaustive
  copy-only `match` with scalar `i64`/`bool` arm results. Compiler-owned ordinary
  `Option<T>` and `Result<T, E>` use this same mechanism rather than backend
  intrinsics. Concrete instances have independently authenticated internal
  Native64/Wasm32 layouts and physical symbols while retaining the template's
  persistent case/field identities. Native C11/Clang at O0/O2 and real
  Node/Wasm prove construction, matching, poison preservation, invalid-tag
  closure, and distinct `i64`/`bool` instances. CleanupPlan v2 replays the
  cleanup-free copy branches by exact scrutinee expression and stable case ID.
  The bounded postfix `?` slice propagates only ordinary compiler-owned
  `Result<T, E>` values whose type arguments are direct `i64`/`bool`: it
  evaluates the operand once, reconstructs an outer `Result<U, E>` on `Err`,
  skips later body expressions, and reaches the same postconditions and final
  publication join as the ordinary body value. Native C11 at O0/O2 and real
  Node/Wasm prove different source/outer layouts, exact `Err` propagation,
  physical-status separation, complete caller-output poison, invalid-tag
  closure, and Wasm re-entry. Resource/nested arguments, generic functions or
  records, non-copy matching or propagation, residual conversion, `?` in
  contracts, stable public aggregate ABI, and callable/component aggregate
  admission remain closed. Generic/prelude verification is hosted green in
  [run 31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201),
  and the typed-`?` tranche is hosted green across the configured matrix in
  [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690).
- A validated stable-ID HIR shared by native and Wasm lowering, with explicit entry, result, binding, expression, and place identities.
- A mandatory target-neutral cleanup CFG for every function, independently rebuilt and independently replayed against core HIR/inventory, with exhaustive current-CFG path-state checks plus a scenario-driven reference trace executor.
- Versioned target-neutral normalized-status, conformance-trace,
  semantic-event-dictionary, and trace-path-certificate protocols, plus
  invocation-local immutable status arenas with zero-success/one-based tokens.
  The compiler turns each admitted cleanup CFG into a deterministic trie-DFA;
  the native host authenticates and walks it without allocation before
  materializing events. Generated callable C loaded through the private native
  ownership host, the independent reference executor, and real Node/Wasm now
  agree exactly for the authoritative 14-case owned-resource corpus at native
  O0 and O2. General and public native resource conformance remain gated.
- Checked integer arithmetic in generated programs; native failures use exact normalized arithmetic codes and propagate without terminating an internal SEMAPRAX frame.
- Typed `requires` and `ensures` contracts, enforced by native and Wasm artifacts. Native scalar contracts publish no caller result on failure.
- Explicit function effects checked against module capabilities and callers.
- Persistent declaration identity through `@id`.
- NUL-free persistent semantic identities across source, resolved HIR, cleanup metadata, graph serialization, and native C literals.
- Deterministic formatting and domain-separated SHA-256 graph revisions.
- JSON semantic Graph v10 with owner/index-stable generic parameters, exact
  concrete nominal arguments, an authenticated compiler-owned prelude,
  persistent variant/case/payload identities, revision-scoped
  construction/match/pattern structure, immutable record update, exact
  evaluation-once `try_result` source/residual instances and shared-epilogue
  meaning, complete CleanupPlan v2 staging, and dependency-bounded context
  slices. Revision v2 binds both canonical source and the exact prelude
  contract.
- JSON-line diagnostics for agent consumption.
- Atomic semantic rename patches with stale-revision rejection.
- Native AOT output through a readable C11 lowering and Clang.
- Direct WebAssembly core output with a generated ES-module runtime, HTML entry point, capability manifest, checked arithmetic, and contract traps.
- A deliberately narrow `semaprax.wasm-owned.v1` Core Wasm path for one direct
  `drop trivial` resource identity. It executes validated-plan terminal cleanup,
  normalized status/out publication, and scalar or owned-input results through
  a generated instance-confined JavaScript host. The host binds its private
  ownership imports to the exact generated Wasm bytes with SHA-256 and rejects
  non-canonical ABI arguments; broader shapes remain gated.

The public build-only `native-callable` target now preflights one selected,
explicitly identified direct-trivial owned function and emits a strict
host-platform shared-library bundle with descriptor, dictionary, trace
certificate, canonical hashed manifest, and source. It does not load, invoke,
or adopt resources. Ordinary native resource builds still return `SPX-B104`.

The newer callable-v3 work remains private. Graph-derived strict-C11 providers
now execute all 14 authoritative normal corpus scenarios through the exact
dynamic-image loader and authoritative host receipt ledger at `-O0` and `-O2`,
with mixed scalar/bool/owned inputs, independent pre-settle evidence validation,
exact replay, generation refresh, cross-instance rejection, and unload pinning.
A counting allocator observes zero Rust allocations or reallocations from
immediately before `CallCommit` through authenticated `ReceiptCommit` in every
normal case; injected reusable-decode reserve failure preserves exact evidence
and the image pin in quarantine. Seven physical-failure, malformed-output,
interruption, replay, and conflict fixtures now pass through provider, exact
loader, and host at `-O0`/`-O2`; the provider subset also runs under Clang
ASan+UBSan. Canonical pre-execute `AbortHostUnwind` skips provider execute,
binds zero-filled response storage, performs certified abort settlement, and
commits an authenticated host receipt. Bounded process-lifetime static-
registration logic feeds the same ledger in non-Apple fake-function evidence
without `dlopen` or unload. The mandatory macOS CI gate requires that
static-only path to type-check for five distinct iOS device, simulator, and
Catalyst Rust targets, with no resolved `libloading` or dynamic `open_*`
surface. The same gate is configured to generate one exact
`token.discard-two` provider for arm64 iOS Simulator, link it with the private
host as an ad-hoc-signed standalone Mach-O, and run provider → static
registration → authenticated receipt/ledger commit at `-O0` and `-O2`. [Run
31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
proved that exact Simulator path.
It is one bounded Simulator process, not device execution, Apple app lifecycle,
UIKit/Swift or XCFramework integration, general mobile support, or public
admission. It is also not exhaustive process-crash or fatal-allocator evidence,
quiescence, or containment of malicious native code. A separate pinned Android
gate now has target-bound Bionic/ELF providers, dynamic-loader/host composition,
and x86_64 Emulator execution. [Run 31320436726, job
93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
proved the exact O0/O2 path. `SPX-B104` stays closed.

A separate private Android JNI/Kotlin application tranche is now implemented
and source-locked. It generates the same bounded provider plus a
`JNI_OnLoad`/`RegisterNatives` shim, packages exactly
`libsemaprax_jni.so`, `libsemaprax_provider_o0.so`, and
`libsemaprax_provider_o2.so` into a same-package, no-UI framework
Instrumentation APK, and builds through a plugin-free Gradle 9 task in
`--offline` mode using pinned runner Kotlin 2, Android build-tools 35.0.0, and
NDK 27.2.12479018. The minSdk-28 wrapper confines the native host to a
`HandlerThread`; `OwnedSession.consume()` is the exact fallible evidence path,
while `AutoCloseable.close()` and the API-28 `PhantomReference`/
`ReferenceQueue` fallback only enqueue the same non-throwing cleanup action.
Deterministic Cleaner tests call the identical registered action through
`cleanForTest()`; they do not infer GC collection or process-exit cleanup.
Local Rust/C and repository source-lock gates pass, and arm64 JNI/provider ELFs
are compile-and-inspect only. The dedicated API-35 x86_64 APK/Emulator path is
green in [run 31338834586, job
93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206),
so this is **Partial** Java/Kotlin and Android application evidence—not AAR,
lifecycle/UI, device, general-resource, or public native support.

A private [Apple Swift ownership adapter](docs/APPLE-SWIFT-OWNERSHIP-V1.md) is
also implemented and CI-configured. It reuses the exact iOS static callable-v3
host behind a Swift 6 stable-thread wrapper. The hosted lane is configured to
build private device/Simulator XCFramework slices and install arm64-Simulator
applications; that bounded compilation/runtime path is green in [run
31338834586, job
93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228).
This is **Partial** Swift/iOS evidence, not a public framework, physical-device,
UI, or general lifecycle claim.

The private [WIT boundary v1](docs/WIT-COMPONENT-BOUNDARY-V1.md) adds a
deterministic, mutation-closed WIT/schema/JavaScript bundle with Node execution
and exact status bounds. A separate standards-valid scalar Component Model
binary has a frozen digest, an independent exact-profile parser, and a private
Node runtime for its authenticated embedded core module. Checked component v2
additionally composes the exact SEMAPRAX-generated scalar core with a frozen
checked-runtime core, validates the complete component with pinned upstream
`wasmparser`, and executes success, overflow, and contract failure through its
private `evaluate()` API. Portable Result Component v3 now composes the exact
private `result<s64, status>` projection and locally executes typed success,
addition-overflow, division-by-zero, precondition, and postcondition outcomes
through Wasmtime with zero imports, an empty linker, and no WASI. The standalone
runtime/dependency graph cannot widen the public compiler graph or MSRV. The
prelude-bound result-v3 KAT migration and standalone Wasmtime runner are hosted
green in [run 31347109201, job
93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).
This remains **Partial** WIT evidence
only: source-language `Result`/`Option` are not wired into this component
profile, and records, resources, imports, async, capabilities,
multi-engine/browser conformance, a public WIT surface, and any `SPX-B104`
change remain absent.

Not implemented yet: public native resource execution/admission,
general-shape native/reference/Wasm trace conformance, the general Wasm resource ABI,
recursive reference execution, callable imports/adapters, a stable public aggregate
ABI and general aggregate execution, nested or resource-bearing generic
arguments, generic functions or records, resource-bearing variants,
ownership-aware matching, general/non-copy/residual-converting `?`, lifetime
and alias analysis, user-facing
regions, effect handlers, static contract proofs, Cranelift, LLVM/MLIR IR,
composed engine-native WebAssembly Component backend, packages, concurrency,
or cross-platform UI. Native
resource builds retain `SPX-B104`; Wasm admits only the documented narrow slice
and rejects every excluded resource shape with `SPX-W111`; record execution
outside any specifically evidenced backend slice remains gated with
target-specific diagnostics.

Behind the internal native-host feature, the compiler emits one complete,
strict-C11 callable provider: generated value/cleanup/status/trace execution,
strict request/response codecs, compile-time physical-target guards, one exact
callable, and its immutable descriptor-v2 getter. The unpublished
`semaprax-native-host` connects that provider to the exact-instance loader
lease, OS-seeded same-thread [capability
authority](docs/NATIVE-CAPABILITY-TOKENS-V1.md), non-mutating ledger plan plus
atomic commit, and non-copying owner/result wrappers. Its safe scalar/owned call
surface executes all 14 authoritative cases from real generated shared
libraries at O0/O2, authenticates the event dictionary and trace-path
certificate, rotates owned results, and proves final logical liveness against
the reference corpus.

This is still a private gate, not public native resource lowering. A physical
provider failure or malformed response currently retires committed logical
owners as an adapter failure, but does not yet prove a general canonical
fallback cleanup trace or finalizer/quiescence protocol. The dedicated Linux
[callable-host sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green: all 14 O0/O2 cases ran from dynamically loaded ASan/UBSan-instrumented
generated providers through the Rust host. That job linked the sanitizer
runtimes but did not sanitizer-instrument the Rust host code itself, and the
overall workflow run was not green because unrelated Clippy/GCC failures
stopped the platform jobs before runtime evidence. The dependency-policy job in
that run was also green.
The generated callable corpus and hardened dependency-collision fixture are
confirmed on Windows in [run 31257545008, job
93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).
The bounded private Android JNI/Kotlin APK now executes in hosted CI, but
Android device/lifecycle/UI breadth, iOS device/app execution, and public
execution/admission remain outstanding.

The private [native desktop application v1](docs/DESKTOP-NATIVE-APP-V1.md)
packages the exact callable-v3 owned-identity provider and authenticated host as
a headless macOS `APPL` bundle or Windows portable PE application directory.
The macOS engine package/runtime is green in [run 31338834586, job
93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230),
including two generation-rotating owned calls and exact receipt replay. The
Windows engine package/runtime is green in [run 31343897595, job
93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480),
including the strict PE inspection and two generation-rotating owned calls.
This is not UI, accessibility, lifecycle,
installer/signing, or public admission evidence. `SPX-B104` therefore remains
unchanged.

The next private [native desktop UI v1](docs/DESKTOP-NATIVE-UI-V1.md) layer is
implemented and CI-configured over that private engine. AppKit and Win32 each
create one real visible native window and button, verify the button's native
accessibility name, dispatch a delayed control event through the OS event loop,
verify the packaged engine bytes against a deterministic SHA-256 manifest before
launch, and close through the native lifecycle. AppKit also enforces a bounded
engine deadline. The macOS AppKit package/runtime is green in [run 31338834586,
job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230).
The Windows Win32 package/runtime is green in [run 31343897595, job
93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480).
The colocated
manifest is consistency evidence, not signed provenance. This is a bounded
private fixture, not SEMAPRAX UI syntax, SwiftUI/WinUI, a full accessibility or
lifecycle claim, distribution, or public admission.

[RFC 0004](docs/RFC-0004-NATIVE-CALL-SETTLEMENT.md) now records the proposed
callable-v3 recovery/settlement foundation for that physical-failure gap:
bounded host-owned linear frames, compiler-certified checkpoints, idempotent
settlement, authenticated receipts, and explicit call/module quiescence. Its
hidden target-neutral model and compiler derivation now serialize through the
private [settlement-proof v1](docs/NATIVE-CALLABLE-SETTLEMENT-PROOF-V1.md)
envelope and an independent host parser. That proof binds exact v2 metadata but
grants no authority. The private [callable ABI v3 descriptor/wire
contract](docs/NATIVE-CALLABLE-ABI-V3.md) now fixes the current private
`SPXNABI3` descriptor order, hash DAG, capacities, settlement graph, linkage
metadata, six provider wire codecs, and one host-only HMAC receipt codec. A
`CertifyOutcome` edge carries the exact ordinal/outcome witness plus a nonzero
digest bound to the trace-certificate fingerprint; the host recomputes that
digest but does not independently accept the trace-path DFA certificate. The
six-argument execute ABI, payload-bearing frame cells, closed tags, exact
capacities, digest DAG, and receipt authentication transcript replace the
former provisional identities and freeze new private v3 known answers.
The ordinary emitter is bound to its compiler build target and has no public or
general machine-code cross-target configuration; a closed hidden selector emits
complete target-bound iOS evidence providers for five enumerated targets.
It also emits closed arm64 and x86_64-emulator Android dynamic providers
with exact Bionic/ELF guards. Windows dynamic runtime evidence is green and the
bounded arm64-Simulator path is green in hosted run 31318280135; the new Android
Emulator path is green in hosted run 31320436726. The private
physical tranche now has graph-derived
providers for all 14 normal corpus scenarios running through the generated-
provider → desktop-loader → receipt-ledger path at `-O0`/`-O2`. That joint
path proves exact descriptor/image/instance binding, pre-settle copied-evidence
validation, replay, finalizer order, generation refresh, pin lifetime, and zero
measured Rust heap growth across the irreversible interval. Canonical
pre-execute unwind and seven physical failure paths reach the same host
authority. It does not prove fatal allocator/process-crash containment,
representative or general mobile application/device execution, or expose a
public compiler path.
`SPX-B104` remains unchanged.

The hidden phase-aware transaction model now starts from the authenticated
post-`CallCommit` state and separates one exact `SettlementDecisionCommit`,
provider-candidate evidence, and model `ReceiptCommitted` eligibility. Before
the decision lock, host unwind selects
`Abort(HostUnwind)`; afterward it resumes the locked decision. Conflicts and
interruption while `Finalizing` quarantine without retry, while exact
candidate/committed replay preserves evidence. This model allocates and grants
no exact-instance, host-authentication, ledger-publication, FFI/provider, or
physical-finalizer authority. Public ownership still requires a future
host-authenticated `ReceiptCommit`; no public execution path is wired.

An unpublished [native loader quarantine](docs/NATIVE-MODULE-LOADER.md) has
separately documented unsafe boundaries for descriptor-only admission and exact
callable-v2 admission. It eagerly resolves one private callable and exposes only
instance-bound, preallocated one-shot prepared calls—never a raw handle,
generic lookup, or callable pointer. The ownership host now consumes the v2
lease and callable transport, but the unsafe caller must still establish trusted
image and dependency provenance. Both legacy loader entry points reject
`SPXNABI3` metadata before canonicalization, native image load, or symbol
lookup, with no fallback to v2. A separate private v3 constructor admits only
an exact descriptor whose getter, execute, settle, and descriptor storage all
belong to the same canonical root image. This is not a malicious-plugin
boundary and does not weaken `SPX-B104`.

The current critical-path implementation contract is [Owned resource vertical
slice v1](docs/OWNED-RESOURCE-VERTICAL-V1.md): one deliberately narrow,
production-reachable owned-resource corpus must execute with exact
native/reference/Wasm status, cleanup, publication, and semantic-trace equality
before either backend gate can open. The private generated-callable host and
real Wasm lane now prove that equality for the authoritative 14 cases, including
native O0/O2 and logical final liveness. The fail-closed pinned-nightly
[Rust-host ASan lane](docs/RUST-HOST-SANITIZERS.md) is green in public CI for
the instrumented Rust host and real callable corpus. The physical/malformed-
response fallback, mobile profiles, and public native execution/admission
remain absent. The corrected build-only bundle is green on [Ubuntu
CI](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277094),
[macOS CI](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277081),
and [Windows CI](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277085),
including Windows callable/dependency isolation. This proves no app-platform
support or public loading, invocation, adoption, or authority; ordinary native
resource execution retains `SPX-B104`.
The document remains a gate, not a completion claim.

## Agent protocol

Every graph response has a schema and revision:

```sh
semaprax graph examples/meaning.spx
```

An agent can request only the meaning around one symbol:

```sh
semaprax context examples/meaning.spx app.main --depth 1 --max-bytes 65536 --max-nodes 256
```

It can then submit a transaction:

```text
base sha256:<64-lowercase-hex-digits>
rename math.add to checked_add
require no-new-effects
```

```sh
semaprax patch examples/meaning.spx change.spatch
```

The patch updates the declaration and verified call sites together. If the graph changed since the agent observed it, SEMAPRAX returns `SPX-G409` and leaves the source untouched.

## CLI

| Command | Purpose |
| --- | --- |
| `check <file> [--json]` | Parse, type-check, verify contracts and effects |
| `graph <file>` | Emit the revisioned semantic program graph |
| `context <file> <symbol> [bounded options]` | Emit deterministic [`semaprax.agent-context.v1`](docs/AGENT-CONTEXT-V1.md) JSON with budgets, filters, omitted counts, and replay frontier |
| `context-benchmark <manifest>` | Run deterministic offline [`semaprax.agent-context-economics.v1`](docs/AGENT-ECONOMICS-V1.md) byte/node/non-model lexical-unit and evidence scoring |
| `build <file> [--target native\|native-callable\|web] [--function stable-id] [-o path]` | Produce a native executable, build-only callable bundle, or browser/Wasm package |
| `run <file>` | Build and run in one step |
| `fmt <file> [--check]` | Apply or verify canonical formatting |
| `patch <file> <patch.spatch>` | Apply an atomic semantic transaction |

## Why SEMAPRAX

Most coding agents edit character ranges and reconstruct meaning repeatedly. SEMAPRAX instead gives declarations persistent identity, exposes typed relationships directly, makes authority visible in signatures, and accepts changes as revision-bound semantic operations. The intended result is fewer tokens, fewer retries, and smaller trust boundaries without sacrificing readable source or Git review.

The long-term compiler has two output principles:

- Native machine code where performance and platform integration matter.
- WebAssembly Components where portability and capability sandboxing matter.

Read [RFC 0001](docs/RFC-0001.md) for the language system, [RFC 0002](docs/RFC-0002-ALGEBRAIC-DATA.md) for algebraic data and aggregate ownership, [RFC 0003](docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) for implemented lifecycle source/resolution and the proposed exactly-once cleanup/runtime phases, and the model-backed, proposed [RFC 0004](docs/RFC-0004-NATIVE-CALL-SETTLEMENT.md) for the native recovery/settlement contract. [Settlement proof v1](docs/NATIVE-CALLABLE-SETTLEMENT-PROOF-V1.md) specifies the private authority-free compiler/host proof envelope, while [callable ABI v3](docs/NATIVE-CALLABLE-ABI-V3.md) freezes the separate private descriptor, wire, and bounded physical-slice contract. [Conformance trace v1](docs/CONFORMANCE-TRACE-V1.md) fixes the target-neutral status/trace projection, and [host ownership transactions v1](docs/HOST-OWNERSHIP-TRANSACTIONS-V1.md) fixes the preflight/commit/publication semantics that future ecosystem adapters must preserve. [The architecture](docs/ARCHITECTURE.md) describes the current implementation, [the quality gates](docs/QUALITY-GATES.md) define executable contribution evidence, [protocol migrations](docs/MIGRATIONS.md) cover agent-facing compatibility, [the roadmap](docs/ROADMAP.md) gives the staged path forward, and the [full-goal completion matrix](docs/COMPLETION-MATRIX.md) records requirement-by-requirement evidence.

## Status

SEMAPRAX is pre-alpha research software. Its syntax, graph schema, diagnostics, and ABI will change. Do not use it for production or safety-critical workloads.

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](CONTRIBUTING.md) and choose an issue aligned with the current stage. Design changes should begin as an RFC because coherence is a core product property.

Licensed under Apache-2.0.
