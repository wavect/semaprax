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
- Bounded generic Copy records admit explicit templates such as `Box<T>` and
  `Duo<T, U>` whose fields are direct `i64`/`bool` values or their own type
  parameters, with explicit direct-scalar instantiation only. Exact concrete
  instances bind HIR substitution, layout caches and digests, native symbols,
  Graph v12 structured arguments, and Native64/Wasm32 lowering. C11 O0/O2 and
  Node/Wasm evidence covers construction, projection, immutable update,
  pass/return, multi-parameter order, poison-preserving failure, and repeated
  entry and is hosted green in [run 31365363898, Ubuntu job
  93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995).
  Generic-record inference, nested/resource/non-Copy arguments, and public
  aggregate/callable/FFI ABI admission remain closed.
- Bounded generic Copy functions admit one or two owner/index-stable type
  parameters, direct `i64`/`bool` or own-parameter by-value signature slots,
  and explicit direct-scalar call arguments such as `id<i64>(value)`. Every
  unused template is checked over all direct-scalar substitutions without
  creating executable instances; only explicitly referenced concrete
  instances receive domain-separated HIR execution identities, Graph v14
  instance nodes, exact native symbols, and exact Wasm function indices.
  Canonical source/HIR/Graph KATs and local strict C11 O0/O2 plus 4,096-entry
  Node/Wasm evidence are green; the hosted matrix is green in [run 31385406865,
  Ubuntu job
  93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). Inference,
  constraints, aggregate/resource/non-Copy signatures, effects, generic-to-
  generic calls, recursion, generic entrypoints, callable/resource admission,
  general/public Component mapping, and a stable public ABI remain closed; the
  exact private v9 profile below is separately gated.
  The same-schema Graph-v14 JSON correction that restored the missing
  `type_parameters` array delimiters is separately hosted green in [run
  31390043736, Ubuntu job
  93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296);
  the execution run above predates that serializer correction.
- Bounded irrefutable Copy-record destructuring extends `match` with exact
  named-field record patterns, recursive record subpatterns, renamed or
  shorthand bindings, ignored fields, and whole Copy-record bindings. The
  scrutinee is evaluated once and the sole arm returns only `i64` or `bool`.
  Exact concrete record/member/binding identities are authenticated in HIR and
  Graph v13; absent a generic function declaration, v13 is selected program-
  wide only by an explicit record pattern and takes precedence over v12/v11/
  v10. Graph v14 takes precedence when a generic function is present. A sole
  top-level `_` remains binding-free and schema-neutral. CleanupPlan v2/v3 does
  not migrate because admitted
  record patterns are straight-line and Copy-only. Strict C11 O0/O2 and
  4,096-entry Node/Wasm evidence covers nested and generic instances, whole-
  record bindings, one-evaluation failure order, poison, and postconditions;
  the Ubuntu gate is hosted green in [run 31373317800, job
  93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130),
  and independent security review reports no P0/P1. Refutable/literal/guard/or/
  rest patterns, nested variant patterns, non-Copy/resource matching,
  aggregate arm results, and public aggregate ABI admission remain closed.
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
  The bounded postfix `?` slice propagates ordinary compiler-owned
  direct-scalar Copy `Result<T, E>` into `Result<U, E>` with exact `E`, and
  direct-scalar Copy `Option<T>` into `Option<U>`. It evaluates the operand
  once, reconstructs the exact outer `Err` or payload-free `None`, skips later
  body expressions, and reaches the same postconditions and final publication
  join as the ordinary body value. Option propagation upgrades only affected
  function cleanup plans to v3 and only programs containing it to Graph v11;
  Result-only and propagation-free output remains CleanupPlan v2/Graph v10.
  Native C11 at O0/O2 and real Node/Wasm prove different source/outer layouts,
  physical-status separation, complete caller-output poison, invalid-tag
  closure, and Wasm re-entry. Resource/nested arguments, generic-function use
  of `?`, non-copy matching or propagation, residual conversion, `?` in
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
- JSON semantic Graph v10, conditionally v11 for bounded Option propagation,
  v12 when any authenticated generic record declaration is present, v13 when
  any authenticated explicit record pattern is present, or v14 when any
  authenticated generic function declaration is present,
  with owner/index-stable generic parameters, exact
  concrete nominal arguments, an authenticated compiler-owned prelude,
  persistent variant/case/payload identities, revision-scoped
  construction/match/pattern structure, immutable record update, exact
  evaluation-once `try_result`/`try_option` source/target instances and
  shared-epilogue meaning, exact generic-record templates/instances, recursive
  exact-instance record-pattern fields/bindings, exact generic-function
  templates/instances/call sites, complete
  CleanupPlan v2/v3 staging, dependency-bounded Agent Context v1 slices, and
  additive forward/reverse/both Agent Context v2 call traversal with separate
  traversal/reference frontiers. V1 remains the exact CLI default. Revision v2
  binds both canonical source and the exact prelude contract.
- JSON-line diagnostics for agent consumption.
- Atomic Semantic Patch v1 function/resource renames plus bounded
  [`semaprax.semantic-patch.v2`](docs/SEMANTIC-PATCH-V2.md) persistent
  member/case renames and exact generic-call type-argument replacement, with
  stale-revision rejection and a selective post-HIR semantic-delta gate. The
  exact v2 matrix is hosted green in [run 31401200449 attempt
  2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2),
  including [Ubuntu job
  93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044).
- Read-only
  [`semaprax.semantic-impact.v1`](docs/SEMANTIC-IMPACT-V1.md) previews one Patch
  v1/v2 file with exact operation/change/source-consumer provenance and a
  deterministic byte/node/depth-bounded reverse-call closure for exact generic
  call-instance changes. It does not apply the patch. The exact `1b3731a`
  matrix is hosted green in [run 31408654657 attempt
  2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
  including [Ubuntu job
  93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).
- Read-only [Semantic Review v1](docs/SEMANTIC-REVIEW-V1.md) emits one
  canonical fixed-section review for Patch v1/v2 or the sole canonical Patch
  v3 identity assignment. V1/v2 embed complete nontruncated Impact v1 evidence;
  v3 embeds the shared repair identity rebase without widening Impact. Its
  exact Patch v1/v2/v3 report KATs are `054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946`,
  `37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421`, and
  `081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544`.
  Local Review integration 10/10, hook/limit 4/4, library 408/408, full
  preservation, and security gates are green. The exact
  `2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in
  [run 31423743369 attempt
  1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
  including [Ubuntu job
  93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170);
  all 12 jobs passed.
  Review has no flags, Context, target/test execution, verifier/proof artifact,
  human approval, authenticated patch provenance, or apply/commit authority.
- [Semantic Patch Evidence v1](docs/SEMANTIC-PATCH-EVIDENCE-V1.md) generates
  and independently verifies exact bounded capsules for Patch v1/v2 and the
  sole canonical Patch v3 operation. The separate `patch-with-evidence` route
  acquires the ordinary A0 lock, requires exact replay before staging, then
  commits through unchanged A0. Capsule/receipt KATs for v1/v2/v3 are
  `03befad24157620b56138e84d4495b1973d141275ee728493d5fbe4f0f6f09aa` /
  `1f2733743aaf2f9d2b9ad6bf2709a6867f169f596be01a9d53e92daecb8730a1`,
  `23742f9b8a323003237106d7a800cc8fb98f53a68bd72f5e0961cf47c63f7bba` /
  `6d8b13b3f54277e66a1ee501e1e71d6fe959a2ebcdbaa158a7ece20dde054e48`,
  and `d682e08b125451af3ed49dce03a0814e83ca5e665224fc3bc7ab7b314827f62c` /
  `13a99674a4c014d9f7f315d8108c3e5c870dcac2c5950ff3035ca1a1c155361b`.
  Local A+B is 11/11 integration plus 5/5 units; Phase C is 16/16 integration
  plus 11/11 hooks/limits; library 420/420 and doctest 37/37 are locally green.
  The exact `34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is
  hosted green in [run
  31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
  including [Ubuntu job
  93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
  all 12 jobs passed. `e04c2c9` was the failed Rust 1.97 lint predecessor, not
  green evidence. Ordinary `patch` remains unchanged, and the capsule is not
  provenance, approval, target/test
  execution, general formal proof, commit authority, or a reusable token.
- [Semantic Target Evidence v1](docs/SEMANTIC-TARGET-EVIDENCE-V1.md) adds a
  read-only `target-evidence` projection over exact base/candidate Graph JSON,
  a typed zero capability delta, production C11 source, and structurally
  validated Wasm core bytes. [Semantic Patch Evidence
  v2](docs/SEMANTIC-PATCH-EVIDENCE-V2.md) binds that report into additive
  generation, exact-replay, and lock-first A0 commands while preserving every
  Evidence v1 byte and command. Target is 9/9, target units 4/4, Evidence-v2
  8/8, and library 439/439 locally. The exact
  `fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in [run
  31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793),
  including [Ubuntu job
  93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631);
  all 12 jobs passed. Reports and
  capsules execute no target/project test, grant no authority, change no
  completion status, and do not replace multi-file work.
- [Semantic Workspace Transaction
  v1](docs/SEMANTIC-WORKSPACE-TRANSACTION-V1.md) adds a bounded real 2–16-file
  publication path over managed immutable generations. `workspace-init`
  authenticates canonical pre-existing sources and publishes generation zero;
  `workspace-snapshot` and `workspace-preview` are shared-lock read-only
  projections; `workspace-apply` holds the exclusive permanent lock, publishes
  one complete candidate generation, and pivots only `ACTIVE` after two final
  checks. Cooperating readers see the complete old or new managed snapshot.
  Raw source files, Git, and editors are not atomically updated. Frozen KATs
  include initial revision
  `sha256:9a7368825342cee138d02a8037248e9a41ed0479d4f7c32a21c7ee7141cf280c`,
  snapshot SHA `3646097c9fb8c47bced51cf2c404b886755f657c73c57afb18d25282574f0b80`,
  and preview SHA `a4f1a9467d535aada97e7f253cf51c0d2168b5557a5a400d11692ac6966776b4`.
  Local evidence is integration 12/12, hostile wire/CLI 5/5, workspace units
  37/37, and library 482/482 with full local gates and security green. The exact
  `afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted green in [run
  31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
  including [Ubuntu job
  93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613)
  and [Windows job
  93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611);
  all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
  strict Clippy and is not green evidence. This changes none of the 38 Partial/18 Missing
  statuses and leaves general cross-file semantics, repository Graph/analysis,
  create/delete/move, raw-tree materialization, automatic recovery/GC, and
  power-loss durability open.
- [Semantic Workspace Patch Evidence
  v1](docs/SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md) adds canonical outer
  capsules and independent receipts over one exact Workspace Patch/preview and
  per-path Review plus child Patch Evidence-v1 digests. Its opt-in apply route
  takes the exclusive Workspace lock first, requires exact replay before any
  candidate or staging creation, then uses the unchanged managed-generation
  publication core. Capsule/receipt KATs cover homogeneous v1/v2/v3 and mixed
  children. Local public 6/6, apply 5/5, hostile 2/2, units 8/8, shared
  Workspace 39/39, root library 496/496, and preservation 107/107 are green;
  the exact `388986a6f12ef97b0c8b40e76466fdc83f211b39` matrix is hosted green in
  [run 31487851406](https://github.com/wavect/semaprax/actions/runs/31487851406),
  with all 12 jobs passing. The capsule grants no authority,
  executes no target/test, performs no cross-file semantic reasoning, embeds
  no Target Evidence or Evidence v2, and changes none of the 38 Partial/18
  Missing statuses.
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
Private Source-Result Component v4 now connects one exact verified source
closure using ordinary `Result<i64, bool>`, postfix `?`, and
`Result<bool, bool>` to the separate WIT 0.2 export
`result<result<bool, bool>, status>`. Local deterministic emission, independent
profile parsing, upstream Component validation, hostile mutation tests, and
generated-core Node execution are green. The isolated Wasmtime runner is
configured to cover both language-result arms and boolean payloads, residual
short-circuiting, sticky arithmetic failure, pre/postconditions, re-entry,
fresh instances, and out-of-band fuel failure; that v4 runtime evidence is
hosted green in [run 31356536123, job
93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
Private Scalar Algebraic Component v5 separately freezes six exports covering
`Option<i64>`, `Option<bool>`, and every direct-copy `Result<T, E>` combination
for `T, E` in `i64`/`bool`, each nested inside an unchanged outer physical
status result. Exact source/core/profile/component KATs, stable-ID-to-export
mapping, canonical layouts, hostile reindexing/mutation closure, upstream
validation, and zero-import isolated-runner source locks are green. The pinned
Rust 1.97.1 typed Wasmtime matrix is hosted green in [run 31360176398, job
93367728269](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728269).
Private Nested Record Component v6 is a separate default-off exact fixture for
WIT package `semaprax:private@0.4.0`, interface `nested-records`, and world
`semaprax-private-v6`. Its sole `transform` export maps a fixed nested
`inner`/`outer` scalar record through the unchanged outer status result. Exact
source/core/layout/profile/component/DAG KATs, independent/upstream validation,
hostile closure, default-consumer hiding, local core execution, and security
review are green. The isolated pinned Rust 1.97.1/Wasmtime 47 typed runtime is
hosted green in [run 31365363898, job
93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974).
V1-v5 bytes remain unchanged. This remains **Partial** WIT evidence only: v4-v6 admit only
their exact private closures. General `Result`/`Option` component
mapping, general/empty/generic/resource records, imports, async, capabilities, multi-engine/browser
conformance, a public WIT surface, callable/FFI aggregate signatures, and any
`SPX-B104` change remain absent.

Private Generic Record Component v7 is a separate default-off exact fixture for
WIT package `semaprax:private@0.5.0`, interface `generic-records`, and world
`semaprax-private-v7`. Four exports map the exact concrete instances
`Duo<i64, bool>`, `Duo<bool, i64>`, `Phantom<i64>`, and `Phantom<bool>` through
the unchanged outer status result. Source/core/layout/Graph-v12/plan/profile/
component identity, ordered type arguments, and the distinct identity of the
same-layout Phantom instances are frozen; local exact/upstream validation,
hostile cross-index/cross-version closure, generated-core Node execution,
default-consumer hiding, source locks, strict gates, and independent security
review are green. The isolated Rust 1.97.1/Wasmtime 47 typed runtime is hosted
green in [run 31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
V1-v6 bytes remain unchanged. This opens no general generic-record exporter,
source selection, nested/resource/non-Copy records, imports/capabilities,
callbacks/async, callable/FFI ABI, browser/multi-engine conformance, package
negotiation, public API/ABI, or `SPX-B104`/`SPX-W111` gate.

Private Record-Pattern Projection Component v8 is a separate default-off exact
fixture for WIT package `semaprax:private@0.6.0`, interface
`record-pattern-projections`, and world `semaprax-private-v8`. Four ordered
monomorphic exports preserve or invert the `marker` field of the distinct
same-layout `Phantom<i64>` and `Phantom<bool>` instances. The exact source,
generated core, two layouts, Graph v13, projection plan, profile, component,
and artifact DAG are frozen. Local independent/upstream validation, all-pair
identity-swap rejection, generated-core Node behavior/poison/invalid-value
closure, default-consumer hiding, source locks, strict gates, and independent
security review are green. The isolated pinned Rust 1.97.1/Wasmtime 47 runner
is hosted green in [run 31385406865, job
93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268). V1-v7 bytes remain
unchanged. V8 is monomorphic record-pattern evidence, not generic-function or
general source-selection/component support, and it opens no imports,
capabilities, resources, callable/FFI or public ABI, browser/multi-engine
claim, package negotiation, or `SPX-B104`/`SPX-W111` gate.

Private Generic-Function Instance Component v9 is a ninth separate default-off
exact fixture for WIT package `semaprax:private@0.7.0`, interface
`generic-function-instances`, and world `semaprax-private-v9`. Three phantom
Copy templates (`preserve<T>`, `invert<T>`, and `ordered<T,U>`) materialize
exactly six explicitly referenced Graph-v14 `FunctionInstanceId`s in frozen
export order. All six exports have the identical
`(marker: bool, control: s64) -> result<bool, status>` signature and introduce
no authored record or layout roots. Exact source, Graph v14, generated core,
plan, profile, raw-component, and artifact-DAG SHA-256 KATs are respectively
`218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c`,
`62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e`,
`9f178207a0406f740198ee8c71d5d008efdf4d995ff04e11e80ea73b79155d44`,
`edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e`,
`365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e`,
`3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688`,
and `2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474`.
Local core 5/5, component 4/4, CI-lock 4/4, full gates, all 15 pair-swap
rejections (eight behaviorally observable and seven identity-only), and
independent security review are green. Its zero-import, empty-linker, no-WASI
pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
31392541096, job
93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
V1-v8 bytes remain unchanged. This exact profile is not general generic-
function Component support and opens no inference/constraints, general source
selection/export, aggregates/resources/non-Copy values, imports/capabilities,
callbacks/async, callable/FFI or public ABI, browser/multi-engine conformance,
package negotiation, or `SPX-B104`/`SPX-W111` gate.

Private Source-Option Propagation Component v10 is a tenth separate default-off
exact fixture for WIT package `semaprax:private@0.8.0`, interface
`option-propagation`, and world `semaprax-private-v10`. Its sole export is the
exact compiler-owned `Option<i64>` through postfix-`?` projection
`evaluate(input: option<s64>, divisor: s64) -> result<option<bool>, status>`.
It binds source revision
`sha256:98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72`,
Graph v11 `96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d`,
prelude v1 `d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e`,
`Option<i64>`/`Option<bool>` layouts
`79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda` and
`dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038`,
CleanupPlan v3 `d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f`,
generated core `16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec`,
profile `f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef`,
component bytes `f5770bdfdbc862ea39640b2c706c1d9ea171164c220d18366e25b3219443ad0d`,
and artifact DAG `90ab80260c84abfe85d1edc666ab3750b81388e6e4cffd7ca21c301b9d0ee589`.
Typed and raw gates cover `Some`/`None`, contracts, arithmetic and sticky
failure, status-first/tag-last publication, full poison, invalid tags/bools/
status, repeated and fresh instances, and fuel exhaustion outside the typed
status. The zero-import pinned Rust 1.97.1/Wasmtime 47 v3-v10 runner is hosted
green in [run 31396483313, job
93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502).
V1-v9 bytes and KATs remain unchanged. V10 does not establish general source
selection/export, general `Result`/`Option`/`?` or algebraic Component mapping,
nested/resource/non-Copy carriers, imports/capabilities, callbacks/async,
callable/FFI or public ABI, browser/multi-engine conformance, package
negotiation, or `SPX-B104`/`SPX-W111` widening.

Not implemented yet: public native resource execution/admission,
general-shape native/reference/Wasm trace conformance, the general Wasm resource ABI,
recursive reference execution, callable imports/adapters, a stable public aggregate
ABI and general aggregate execution, nested or resource-bearing generic
arguments, generic-function inference/constraints, general generic-function
Component mapping, or aggregate/resource/non-Copy signatures, resource-bearing
variants,
refutable and ownership-aware/non-Copy matching, general/non-copy/residual-converting `?`, lifetime
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

That command retains the exact `semaprax.agent-context.v1` default. Supplying
an explicit direction selects the additive v2 call-graph query:

```sh
semaprax context examples/meaning.spx app.main --direction reverse --depth 1
```

It can then submit a transaction:

```text
base sha256:<64-lowercase-hex-digits>
rename math.add to checked_add
require no-new-effects
```

That schema-less form retains exact v1 function/resource rename behavior.
Persistent record/case-member, variant-case, and generic-call type-argument
edits opt into the bounded v2 grammar with first non-comment line
`schema semaprax.semantic-patch.v2`; see [Semantic Patch
v2](docs/SEMANTIC-PATCH-V2.md).

```sh
semaprax patch examples/meaning.spx change.spatch
```

The same patch can be previewed without acquiring commit authority or changing
source:

```sh
semaprax impact examples/meaning.spx change.spatch --depth 1
```

The canonical report binds the source base/candidate revisions and a digest of
the exact processed patch bytes. It is bounded single-file call impact, not
repository-wide or general non-call impact; see [Semantic Impact
v1](docs/SEMANTIC-IMPACT-V1.md).

An agent can discover the first bounded compiler repair for an exact automatic
function identity and then instantiate it with a caller-selected persistent
ID, without writing source:

```sh
semaprax repairs examples/meaning.spx assign-function-id auto:example.helper
semaprax repair examples/meaning.spx <repair-id> --persistent-id example.helper
```

The first command returns canonical `semaprax.diagnostic-repair.v1` JSON. The
second independently proves the one-edit identity rebase and returns canonical
`semaprax.diagnostic-repair-preview.v1` JSON containing one exact three-line
`semaprax.semantic-patch.v3` operation. Its classification is
`breaking_identity_rebase`: the selected automatic declaration ID and its
revision-scoped derived IDs intentionally change. Passing that generated patch
to `semaprax patch` reruns the same closed repair/rebase gate and applies it
through unchanged A0. Semantic Impact v1 rejects every syntactically valid,
canonical v3 as `SPX-G110` before semantic selector interpretation; malformed
or noncanonical v3 remains `SPX-G101`.
The operation changes Graph-v10 revision/identity/callee/derived-ID content and
may rebase identity-bearing CleanupPlan content, without widening either schema
or semantic shape and without changing backend/runtime semantics.
See [Bounded Diagnostic Repair v1 and Semantic Patch
v3](docs/DIAGNOSTIC-REPAIR-V1.md).

An agent or human can request the bounded deterministic review without writing
source:

```sh
semaprax review examples/meaning.spx change.spatch
```

The seven canonical sections are `behavior`, `api_identity`,
`security_authority`, `memory_ownership`, `target_artifact`, `migration`, and
`unsafe`. Patch v1/v2 reports carry the complete nontruncated Impact v1 report;
the sole canonical v3 report carries the exact shared identity-rebase evidence
and no Impact object. This is a bounded classification report, not Context,
target execution, a public verifier/proof artifact, human approval, or commit
authority; see [Semantic Review v1](docs/SEMANTIC-REVIEW-V1.md).

The patch updates the declaration and verified call sites together. If the
graph changed since the agent observed it, SEMAPRAX returns `SPX-G409` and
leaves the source untouched. Commit A0 additionally authenticates a canonical
regular source leaf, serializes cooperating writers with a create-new sibling
lock, stages through bounded create-new siblings, and rechecks source and stage
identity plus exact bytes immediately before the final rename. Unix uses exact
device/inode identity. Windows holds same-file handles and compares volume plus
the available 64-bit file index; this is not a uniqueness claim for ReFS
128-bit or hostile non-unique-index environments. Identity-aware cleanup never
removes a foreign replacement. The protocol remains single-file and
cooperative: predictable names permit collision/stale-lock denial of service,
crashes may leave locks, the containing directory is trusted in the final
portable path-based rename window, and parent-directory sync, power-loss
durability, multi-file commits, and general typed repair/impact remain open.

The separate managed workspace protocol provides bounded publication without
widening single-file A0:

```sh
semaprax workspace-init . paths.json
semaprax workspace-snapshot .
semaprax workspace-preview . change.wspatch
semaprax workspace-apply . change.wspatch
```

The canonical workspace patch binds an exact base workspace revision and 2–16
sorted managed paths, each containing an unchanged admitted Patch v1/v2 or the
sole canonical Patch v3 operation. The live apply invocation owns its bounded
lock/generation/`ACTIVE` authority; snapshot and preview artifacts grant none.
Original sources are not rewritten, and the protocol is not cross-file
resolution, a repository transaction, or a proof/provenance token. See
[Semantic Workspace Transaction
v1](docs/SEMANTIC-WORKSPACE-TRANSACTION-V1.md) and [ADR
0002](docs/decisions/0002-managed-workspace-generations.md).

An opt-in evidence-gated workspace route is:

```sh
semaprax workspace-patch-evidence . change.wspatch > evidence.json
semaprax verify-workspace-patch-evidence . change.wspatch evidence.json
semaprax workspace-apply-with-evidence . change.wspatch evidence.json
```

The outer capsule binds exact independently rebuilt per-file Patch Evidence v1
facts by digest; child artifacts remain single-file and retain their
`no_multi_file_transaction` nonclaim. Exact replay happens before candidate or
staging creation. The live invocation, not the artifact, owns the existing
Workspace lock/generation/`ACTIVE` authority. See [Semantic Workspace Patch
Evidence v1](docs/SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md).

## CLI

| Command | Purpose |
| --- | --- |
| `check <file> [--json]` | Parse, type-check, verify contracts and effects |
| `graph <file>` | Emit the revisioned semantic program graph |
| `context <file> <symbol> [bounded options]` | Emit deterministic default [`semaprax.agent-context.v1`](docs/AGENT-CONTEXT-V1.md) JSON, or additive [`semaprax.agent-context.v2`](docs/AGENT-CONTEXT-V2.md) forward/reverse/both call context with `--direction` |
| `context-benchmark <manifest>` | Run deterministic offline [`semaprax.agent-context-economics.v1`](docs/AGENT-ECONOMICS-V1.md) byte/node/non-model lexical-unit and evidence scoring |
| `build <file> [--target native\|native-callable\|web] [--function stable-id] [-o path]` | Produce a native executable, build-only callable bundle, or browser/Wasm package |
| `run <file>` | Build and run in one step |
| `fmt <file> [--check]` | Apply or verify canonical formatting |
| `patch <file> <patch.spatch>` | Apply an atomic semantic transaction |
| `workspace-init <root> <path-set.json>` | Initialize a bounded managed immutable-generation workspace without modifying original sources |
| `workspace-snapshot <root>` | Emit the authenticated selected managed workspace snapshot |
| `workspace-preview <root> <patch.wspatch>` | Preview a canonical 2–16-file managed workspace transaction without publication authority |
| `workspace-apply <root> <patch.wspatch>` | Publish a complete authenticated managed generation by pivoting only `ACTIVE` |
| `workspace-patch-evidence <root> <patch.wspatch>` | Emit canonical bounded Workspace Patch Evidence v1 without creating candidate state |
| `verify-workspace-patch-evidence <root> <patch.wspatch> <evidence.json>` | Independently replay a workspace capsule and emit its canonical receipt |
| `workspace-apply-with-evidence <root> <patch.wspatch> <evidence.json>` | Require exact workspace evidence replay before candidate generation, staging, and the existing `ACTIVE` pivot |
| `patch-evidence <file> <patch.spatch>` | Emit canonical bounded Semantic Patch Evidence v1 without writing source |
| `verify-patch-evidence <file> <patch.spatch> <evidence.json>` | Independently replay a capsule and emit its canonical verification receipt |
| `patch-with-evidence <file> <patch.spatch> <evidence.json>` | Require exact evidence replay before A0 staging and commit |
| `impact <file> <patch.spatch> [--depth N] [--max-bytes N] [--max-nodes N]` | Preview deterministic bounded single-file source consumers and reverse-call impact without applying the patch |
| `review <file> <patch.spatch>` | Emit the fixed-section bounded Semantic Review v1 report without applying the patch |
| `repairs <file> assign-function-id <automatic-function-id>` | Discover the bounded read-only `SPX-S103` function-identity repair |
| `repair <file> <repair-id> --persistent-id <persistent-id>` | Instantiate and independently prove the bounded repair as read-only Patch v3 preview JSON |

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
