# Compiler architecture

SEMAPRAX v0.2 is a vertical slice through the future compiler. Each layer has a narrow contract so the prototype can grow without turning its source syntax into its internal API.

```text
.spx source
    |
lexer -> parser -> parsed AST
                    |
              semantic verifier
                    |
                resolved HIR
                     |
         replay-validated cleanup plan
                /    |     \
 semantic graph  C11 IR  Wasm core
    /       \             |           |
 agent queries    tx    Clang     browser host
                         |           |
                  native executable  web package
```

## Source projection

`lexer` and `parser` accept a deliberately small grammar. `format::canonical` is the single source projection. Graph revisions hash this canonical form rather than incidental whitespace, so formatting-only edits do not invalidate an agent transaction. The cross-protocol revision token is `sha256:<64 lowercase hex digits>` over `b"semaprax.graph-revision.v1\0" || canonical_source_utf8`. It is collision-resistant content addressing and stale-base detection, not a signature or MAC.

Declarations should carry an explicit `@id`. Automatic identities are accepted for exploration but produce `SPX-S103`, because a name-derived ID cannot survive a rename.

## Verification

The public verifier compatibility facade and HIR analysis boundary share one source verifier, preserving the established ordered diagnostics while removing a `hir -> verify` dependency cycle. Warnings-only analysis retains diagnostics and still produces resolved HIR; any source error fails closed before lowering. The verifier builds the module symbol table and checks:

1. Unique names and stable IDs.
2. Parameter, expression, call, and return types.
3. Boolean preconditions and postconditions.
4. Function effects against module permits.
5. Transitive effect declarations at each call edge.
6. Lexical local scope and conservative ownership joins across branches and lazy boolean paths.
7. Explicit resource lifecycle cardinality, interface/import authority and failure contracts, and lifecycle effects on every function that can own a resource.
8. The native entry-point shape.

The initial contract lane is progressive: contracts are type-checked at compile time, required to be effect-free, and guarded in generated native and Wasm code. Static proof is a later lane, and its absence is reported honestly in the project status.

Generated arithmetic is checked for overflow, zero division, and the signed division edge case. Failures have stable process exit codes and explicit diagnostics rather than C undefined behavior.

## Resolved HIR groundwork

`hir` is the fail-closed boundary between verified human syntax and future semantic consumers. It resolves nominal types, resource lifecycles, interfaces, logical imports, record fields, projections, and calls through persistent declaration IDs, assigns deterministic structural identities to parameters, locals, expressions, and result values, and represents rooted field references as places. Spans and display names remain diagnostic metadata rather than semantic identity.

The declaration index is the single current source of target-independent type facts: whether a type is copyable, contains resources, is sized, needs destruction, and its name-independent layout key. Generic parameter identities include their owner declaration plus index, and nominal identities include the complete resolved argument tree.

The native and Wasm emitters now consume only validated HIR for semantic lowering; their parsed-AST entry points are compatibility wrappers that resolve first. A centralized HIR validator rejects duplicate/non-canonical identities, invalid declarations and nominal types, lexical-scope violations, inconsistent expression/call types, definite or conditional resource reuse, contract transfers, undeclared or unpermitted effects, effectful contracts, invalid result bindings, and an invalid entrypoint before either backend emits an artifact. Only after those checks pass, `cleanup` independently rebuilds the structural storage inventory; `cleanup_plan` then rebuilds and exact-compares the complete target-neutral plan. A hostile direct-HIR transform therefore cannot remove, retarget, reorder, or forge cleanup meaning before reaching Graph or a backend.

This remains staged groundwork rather than the sole compiler IR: the current verifier still establishes meaning from parsed AST before HIR resolution. Explicit trivial/imported resource lifecycles, declaration-only interface/import contracts, record declarations/updates, bounded explicitly instantiated generic Copy records, bounded copy-variant templates/construction/exhaustive matching, typed ordinary-`Result` and ordinary-`Option` propagation, stable type/member/case identities, recursive resource/type facts, and by-value recursion rejection now reach validated HIR and the semantic graph. Generic parameters are owner/index-stable and the admitted concrete arguments are direct `i64`/`bool`; generic record fields are restricted to direct scalars or parameters owned by that record, and every construction/update/projection substitutes the exact ordered concrete instance. The compiler-owned `semaprax.prelude.v1` injects ordinary `Option<T>` and `Result<T, E>` variants before checking. The bounded postfix `?` form accepts only direct-scalar Copy instances: `Result<T, E>` requires an enclosing `Result<U, E>`, while `Option<T>` requires an enclosing `Option<U>`. It evaluates its operand once, reconstructs the exact outer `Err` or payload-free `None`, and routes both ordinary-body and propagated results through shared postconditions and publication. The source checker and HIR validator independently replay lifecycle compatibility, lifecycle-effect authority, prefix-aware partial-place availability, exact generic substitution, exact construction, copy-match exhaustiveness, and every compiler-owned carrier/member/source/target identity. `aggregate_layout` computes checked deterministic Native64 and Wasm32 record layouts keyed by the full record ID plus ordered arguments; its digest and native symbol bind the same exact instance even when two instances have identical physical fields. `variant_layout` computes independently reconstructable per-concrete-instance internal layouts with declaration-order `u32` tags, an aligned maximum-payload area, and one inert byte for an empty payload. Its v2 digest authenticates the full concrete instance and both template and substituted field types; physical tags and representation are unchanged from v1. `CleanupInventory` remains a structural discovery boundary. Every `ResolvedFunction` carries a cleanup plan: v2 remains canonical unless authenticated Option propagation is present, which requires v3. Both schemas include typed blocks, edges, lexical regions, entry liveness, storage/leaf flags, atomic call commits, sticky status sources, guarded finalizers, scalar/owned result publication, and exact body-versus-propagated Copy-result staging; v3 adds an authenticated payload-free Option-None source. Generic records add no cleanup action because the admitted instances are direct-scalar Copy values; canonical replay remains bound to exact HIR types. Immutable update consumes its base first, evaluates replacements in authored order, transfers untouched fields, and cleans displaced live fields exactly once in reverse order. Copy matches branch on an exact scrutinee expression and stable case IDs without inventing droppable payload leaves; distinct concrete instances therefore cannot share a cleanup decision. Propagation uses complementary predicates on the authenticated success case and cannot be confused with physical failure selection. The builder covers every current HIR expression and normal/checked-failure path; the validator reconstructs the plan from core HIR rather than trusting attached metadata.

The bounded record-pattern tranche is irrefutable and Copy-only. One explicit
named record pattern or a top-level wildcard consumes exactly one evaluated
record scrutinee and has exactly one scalar `i64`/`bool` arm. Explicit patterns
require every stable field exactly once and admit recursive record subpatterns,
shorthand or renamed bindings, ignored fields, and whole Copy-record bindings.
HIR binds the full concrete record instance, stable record/field IDs, and
canonical binding identities recursively, so equal layouts cannot substitute
for equal type identity. Record matching adds no cleanup slot, transition,
status source, or decision edge: CleanupPlan v2/v3 replay authenticates the HIR
skeleton and lowers the match straight-line. Native stages the aggregate once;
Wasm projects from one frame value and copies whole-record bindings into their
own frame slots. Refutable/literal/guard/or/rest/nested-variant patterns,
resource/non-Copy modes, and aggregate arm results remain outside admission.

The bounded generic-function tranche is likewise direct-scalar and Copy-only.
Source admits one or two owner/index-stable function type parameters and
requires explicit ordered `i64`/`bool` arguments at every call. Parameter and
result slots are by-value direct scalars or parameters owned by that function;
templates are effect-free and reject aggregate/resource syntax, ownership
modes, generic-to-generic calls, transitive generic cycles, recursion, and a
generic entrypoint. Verification checks every unused template over all `2^N`
direct-scalar substitutions without creating executable evidence. Resolved HIR
keeps monomorphic functions, function templates, and explicitly referenced
concrete instances in separate vectors. A concrete `FunctionInstanceId`
derives from the persistent template ID plus ordered arguments, and its
domain-separated execution identity scopes parameter/result/expression IDs;
same-signature templates and instances cannot substitute for one another.
Only explicitly referenced instances lower to native or Wasm. Their attached
CleanupPlan remains canonical v2 and its propagated-call status producer stays
template-ID-only; HIR validates the exact concrete instance before independent
plan replay, while Graph v14 carries the exact instance meaning. Generic
functions grant no callable, settlement, semantic-trace, resource/owned, FFI,
or component authority.

The target-neutral runtime protocol is split from physical target state. `semaprax.status.v1` contains only a stable `domain_id`, nonzero code, class, and retryability; the invocation-local arena assigns immutable one-based tokens while reserving zero for success and rejects cross-context and same-nonce cross-arena resolution. `semaprax.conformance-trace.v1` records semantic ownership, import, write-once failure selection, finalization, and result publication without pointers, handles, tokens, offsets, or host exceptions. Attached plans are independently checked against inventory and exact typed-HIR control/event coverage, then exhaustively replayed across the current acyclic CFG for ordered liveness, sticky failures, exact region-leave chains, reverse cleanup, and typed whole-result publication. The deterministic single-frame reference executor models an uninitialized/published caller out slot; record results remain rejected until the trace schema can preserve aggregate semantic values. The native scalar C lane shares one caller-supplied context across nested calls, returns exact compiler statuses, and commits its out slot only after postconditions.

For callable-v2 admission, `semaprax.trace-path-certificate.v1` compiles that
independently replay-validated cleanup CFG into a canonical trie-DFA. Its
accepting states bind both the exact semantic-ordinal sequence and terminal
scalar-success, owned-success, or selected-failure outcome. Descriptor v2 binds
the certificate fingerprint separately from the event-dictionary fingerprint;
the host authenticates both and performs an allocation-free DFA walk before it
materializes any semantic events. The dictionary is therefore only the
vocabulary, never an authorization to omit, duplicate, or reorder cleanup.

For the admitted native resource shape, compiler preflight derives and discards
an authority-free host template and its canonical pointer-free [native adapter
descriptor v1](NATIVE-ADAPTER-DESCRIPTOR-V1.md). That descriptor-only provider
still exports only its immutable getter. A second private compiler stage now
derives exact [callable descriptor-v2
metadata](NATIVE-CALLABLE-ABI-V2.md) from the sealed template, generated
execution/cleanup fingerprint, deterministic semantic-event dictionary, and
trace-path certificate. It binds twelve independently domain-separated
fingerprints, exact getter and callable symbols, request/response capacities,
the complete ordered signature, and result mapping. The unpublished host
independently parses that v2 wire and rejects every single-byte mutation,
truncation, or trailing byte in cross-crate fixtures.

The public build-only compiler callable stage emits the complete provider
translation unit: generated value/cleanup/status/trace execution, strict
bounded request and response codecs, physical target guards, one descriptor-v2
getter, and one callable. The target guards fail C compilation when
architecture, OS, environment, object format, pointer width, or endianness
cannot be proven; MSVC uses its supported target architecture instead of
assuming GNU byte-order builtins.

`preflight_native_callable_bundle` accepts one explicitly identified function
with at least one direct `own` trivial-resource parameter. The CLI target
`native-callable` compiles that exact provider for the host and commits a new
bundle containing the shared library, C source, descriptor, event dictionary,
trace certificate, canonical file-hash manifest, and manifest checksum. It
refuses observed files, directories, and dangling symlinks and stages beside a
canonical trusted output parent; portable `std` cannot make the final directory
rename no-replace against an adversarial concurrent parent mutation. The API
does not load, invoke, adopt, mint authority, or connect callable v3. Exact
build-only bundle emission is green on [Ubuntu](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277094),
[macOS](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277081),
and [Windows](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277085)
hosted CI; this is not mobile or application-host evidence.

Callable v3 is a separate private tranche. Graph-derived strict-C11 providers
execute all 14 authoritative normal corpus scenarios at `-O0` and `-O2`. The
loader independently admits an exact
dynamic image only when the getter, execute, settle, and returned descriptor
storage share canonical root-image provenance, then retains an immutable copy
of the admitted bytes. The host independently creates
one 64-byte OS fill split into a receipt MAC key and instance binding, and its
fixed-capacity ledger/facade atomically commits authenticated terminal state
and exact replay. One joint O0/O2 test now invokes all 14 authoritative
scenarios through generated provider, dynamic loader, and host ledger, proving
copied-evidence decoding, replay, generation refresh, finalizer order,
cross-instance rejection, and pin lifetime. Its counting allocator observes
zero Rust heap growth from immediately before `CallCommit` through
`ReceiptCommit`; injected decode-reserve failure quarantines exact evidence and
the image pin. Seven returned-physical-failure, malformed-wire, durable-boundary,
replay, and conflict fixtures also cross provider, loader, and host at O0/O2.
Canonical pre-execute unwind skips provider execute, binds exact zero response
storage, settles certified abort cleanup, and commits one host receipt.
Exhaustive process-crash/fatal-allocator evidence and broader Android/iOS
application execution,
quiescence, malicious-code containment, public admission, and `SPX-B104`
remain closed.

The unpublished `semaprax-native-host` now performs the complete private
connection. It strictly decodes descriptor v2, authenticates the dictionary and
trace certificate, opens and retains one exact callable loader instance,
constructs an OS-seeded same-thread [capability
authority](NATIVE-CAPABILITY-TOKENS-V1.md), verifies owner/result credentials,
builds a non-mutating fully allocated ledger plan, and commits all owners once
before invoking the prepared byte call. The safe scalar and owned APIs decode
only physical completion, walk the certificate, reconcile success or semantic
failure, and return authenticated semantic events. Hostile providers cover all
defined and reserved physical results, malformed response fields, dictionary
and certificate rejection, draining, reusable precommit rejection, and
cross-instance confinement.

The authoritative 14-case fixture compiles real generated shared libraries at
O0 and O2, loads them through this host, and exactly matches reference outcome,
status, trace, publication, owner rotation, and final logical liveness. This is
still a private feature, and `SPX-B104` remains closed. After a physical failure
or malformed response the guard retires committed logical owners as an adapter
failure, but general canonical fallback cleanup/finalizer trace and physical
quiescence are not yet proven. The dedicated Linux
[callable-host sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
passed all 14 O0/O2 cases from dynamically loaded ASan/UBSan-instrumented
generated providers through the Rust host. It supplied the sanitizer runtimes
without sanitizer-instrumenting the Rust host code itself. The overall workflow
run remained red because of unrelated Clippy/GCC failures. The distinct pinned-
nightly [Rust-host ASan job](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065)
later passed with the Rust host instrumented inside a fully green current hosted-
CI run; it does not claim Rust-host UBSan or mobile/app-platform coverage.

The hidden `native_settlement` module and proposed [RFC
0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md) now make the target-neutral recovery
state machine executable: bounded dense checkpoints, one all-live start, exact
typed progress, accept/abort action permutations, idempotent cached receipts,
terminal owner dispositions, quiescence validation, and a separate linear
phase-aware transaction. That transaction exposes only its closed phase,
records `Finalizing` before issuing a noncopying finalizer ticket, caches
provider-candidate and model-committed receipt evidence separately, and makes
conflict or uncertainty an absorbing quarantine. A private compiler
deriver now constructs this graph from validated cleanup HIR for the current
direct-trivial owned slice, preserves exact result-staging/finalization timing,
and binds terminal edges to accepted semantic trace paths. Exhaustive tests
cover every valid live/dead/single-provisional combination through six owners,
the authoritative 14-case corpus, exact bounds and known answers, hostile graph
and receipt mutations, and non-cloneable/non-formattable frame API gates. The
private [settlement-proof v1](NATIVE-CALLABLE-SETTLEMENT-PROOF-V1.md) encoder now
embeds the exact v2 descriptor and canonical binary graph under a 64 KiB cap.
The unpublished host independently parses and canonically re-encodes the graph,
validates its transition semantics, and requires its source call-contract and
trace-certificate fingerprints to match v2. The v2 loader rejects the proof
magic before opening an image. This proof path has no invocation reservation,
module-instance or frame-generation binding, physical finalizer authority,
embedded descriptor-v3 contract, provider, loader admission, host execution,
or public compiler connection; it is not physical fallback evidence and does
not change `SPX-B104`.

The hidden phase model now keeps the eligibility evidence for three
irreversible physical boundaries distinct: `CallCommit`,
`SettlementDecisionCommit`, and host `ReceiptCommit`. Unwind after
the call commit but before the decision lock selects `Abort(HostUnwind)`;
unwind after the lock resumes the exact decision, while an unknown or
conflicting phase makes the model enter absorbing `Quarantined` while preserving
its evidence. A physical host must additionally quarantine the exact instance.
A physical action must record `Finalizing` before entering its effect and may
record `Dead` only after normal return, so interruption is quarantined and never
retried. A provider terminal state and candidate receipt are evidence
only—including its `Published` disposition. Only independent host validation
plus host-only authentication may commit one public ledger publication. The
model's `ReceiptCommitted` phase is only exact candidate-validation evidence:
it allocates and owns no host secret, ledger, exact-instance reservation,
loader pin, or physical finalizer. The current proof envelope and callable host
do not wire the physical v3 boundary.

The separate [native callable ABI v3](NATIVE-CALLABLE-ABI-V3.md) now fixes that
boundary's private descriptor and wires: sequential `SPXNABI3` fields, an
acyclic hash DAG, bounded graph, exact buffer/instance capacities, a
six-argument execute ABI, payload-bearing frame cells, six provider codecs, a
distinct 524-byte host-only committed receipt, and dynamic-image versus
iOS-static linkage metadata. Each `CertifyOutcome` carries its ordinal/outcome
witness and a nonzero digest bound to the trace-certificate fingerprint; the
host recomputes that digest without independently accepting or walking the
trace-path DFA certificate. Resealed witness/digest mutations fail. Independent
compiler encoders and host parsers freeze the seven complete byte/tag/digest/
HMAC transcripts and their changed private known answers. The
ordinary compiler encoder is bound to its build target and exposes no
public/general machine-code cross-target configuration; a hidden closed selector
emits complete target-bound iOS evidence providers for five enumerated targets.
The same hidden seam emits arm64 and x86_64-emulator Android dynamic
providers whose guards require Android, Bionic, ELF, 64-bit pointers, and
little-endian code generation. Windows dynamic runtime and the bounded
arm64-Simulator path are green. [Run 31320436726, job
93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
also proves the bounded Android Emulator path. The legacy loader constructors reject the full v3 magic in their shared
input validator before canonicalization, image loading, getter lookup, or
callable lookup; their exact callable-v2 classifier remains unchanged. A
separate private v3 constructor binds the getter, execute, settle, and returned
descriptor address to one canonical root image, retains an immutable copy of
the admitted bytes, and returns one exact instance lease. Graph-derived
strict-C11 providers execute all 14 normal corpus scenarios at `-O0`/`-O2`,
and the private host now combines an exact-descriptor-bound receipt
authority with authoritative owner generations, allocation-free `CallCommit`,
atomic receipt/ledger publication, cached replay, and a drop-safe transaction
guard whose postcommit uncertainty is quarantined without retry. One joint
generated-provider → loader → host test covers all 14 normal scenarios at
`-O0`/`-O2`, with zero measured Rust allocations/reallocations across the
irreversible interval and exact quarantine on injected decode-reserve failure.
It does not cover fatal allocator or process-crash containment, iOS device
execution, or Android device/lifecycle breadth, and exposes no public
admission. The separate bounded Android JNI/APK path is green in [run
31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
Private bounded process-lifetime static-registration logic
now binds exact descriptor and entry addresses to the same host ledger; its
non-Apple fake-function test proves retention and quarantine only, with no
`dlopen` or unload claim.

The private [native desktop application v1](DESKTOP-NATIVE-APP-V1.md) packages
that same dynamic callable-v3 boundary as a headless macOS `APPL` bundle and a
Windows portable PE application directory. Local macOS execution admits its
co-located exact provider/descriptor, performs two owned receipt commits with
generation rotation, and replays the first commit exactly. That macOS
package/runtime is green in [run 31338834586, job
93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230).
The Windows package/runtime is green in [run 31343897595, job
93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480),
including the strict PE inspection path. This is an application
process and native-packaging seam only: there is no UI toolkit, accessibility,
lifecycle API, signing, installer, public admission, or `SPX-B104` change.

Private [native desktop UI v1](DESKTOP-NATIVE-UI-V1.md) keeps that Rust process
as a package-bound sibling engine and adds no loader or host authority. A foreground
AppKit executable and a Win32 GUI-subsystem executable own their respective
native window, button, accessibility-name query, timer-dispatched action,
event loop, close, and termination evidence. Only the engine's exact output can
advance the UI fixture to success publication. The UI packagers consume the
already verified engine package, compile the platform frontend twice with the
pinned native linker/SDK roots, inspect a closed artifact/import/framework
inventory, publish and verify a canonical SHA-256 engine manifest before launch,
and launch it in the ordinary platform matrix. AppKit enforces a bounded
terminate/kill deadline; Windows freezes the exact DLL set and rejects any
export directory, including ordinal-only functions. The colocated digest is not
signed provenance. The macOS engine plus AppKit package/runtime is green in
[run 31338834586, job
93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230);
The Windows Win32 package/runtime is green in [run 31343897595, job
93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480).
The adapter remains private
with `SPX-B104` closed.

A mandatory macOS gate requires the loader and host
static-only path to type-check for five iOS device, simulator, and Catalyst Rust
targets, excluding `libloading`, dynamic `open_*`, and the desktop v1/v2 host
API. That same job is configured to cross-emit one exact arm64-Simulator
provider, link it with the private host into a standalone ad-hoc-signed Mach-O,
and execute the unchanged static-registration and receipt ledger at `-O0` and
`-O2` through `simctl`. It requires exact finalizer order/payload, authenticated
no-owned publication, and zero measured Rust allocations across the irreversible
interval. [Run 31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
proved that exact path. It is not an installed app, device run, lifecycle/UI/Swift
integration, general iOS backend, or public admission. `SPX-B104` remains
closed.

The callable-v2 Windows CI lane explicitly reruns its generated O0/O2 corpus
and a loader fixture that places a same-name dependency in CWD and legacy
`PATH`, then removes the root sibling to require fail-closed `LibraryOpen`.
Those v2 gates passed in [run 31257545008, job
93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756),
confirming narrow callable-v2 corpus and dependency-isolation evidence. For
callable v3, [run 31313341303](https://github.com/wavect/semaprax/actions/runs/31313341303)
proved Windows, Linux, macOS, MSRV, dependency policy, generated-provider
ASan+UBSan, and Rust-host ASan gates. None of this is broader Windows
application-platform completion. [Run
31316677457](https://github.com/wavect/semaprax/actions/runs/31316677457)
proved the five-target iOS type-check and no-`libloading` dependency gate. [Run
31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
then proved the bounded single-Simulator runtime; representative Android/iOS
device and broader Simulator/app execution plus
public native execution/admission remain required.

The mandatory Android job compiles the dynamic loader and unchanged host
for `x86_64-linux-android` and `aarch64-linux-android`, builds both exact
Bionic/ELF providers with NDK r27.2, inspects the resulting x86_64 and AArch64
ELFs, and runs `token.discard-two` at O0/O2 in an API-35 x86_64 emulator. It
requires canonical-path `dladdr` provenance, exact finalizer order/payload,
receipt/ledger evidence, and zero measured Rust allocations. [Run 31320436726,
job 93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
proved this bounded runtime path. That standalone process is not the separate
JNI/Kotlin APK, and it proves no public/general JNI, APK/AAR distribution,
lifecycle/UI, device, or general-corpus behavior.

## Private Android JNI application adapter

The separate [`unstable-android-jni-harness`](ANDROID-JNI-OWNERSHIP-V1.md)
tranche implements one private Kotlin/JNI projection of that bounded v3 host.
Its generator emits target-matched strict-C provider and JNI shim sources for
x86_64 and arm64 Android. The build links each shim to the target Rust static
host, requires `JNI_OnLoad` as the only defined global export, checks the exact
Android system-library dependency allowlist and absence of workspace paths,
and packages the x86_64 shim plus O0/O2 providers under their exact names.
arm64 is compile-and-ELF-inspect evidence only.

The application fixture is a same-package, no-UI framework `Instrumentation`
APK with minSdk 28 and target/compile API 35. Its Gradle 9 project declares no
plugin or repository; the offline task invokes a checked packaging script that
requires runner Kotlin 2 and Android build-tools 35.0.0. The resulting APK has
one exact native-library inventory and is aligned, signed with an ephemeral
fixture key, verified, installed only after removing any prior package, and
required to publish an exact app-private
`files/semaprax-android-jni-v1.txt` result.

One `NativeRuntime` owns one `HandlerThread`; all provider admission, adoption,
consumption, receipt commit, drain barriers, and thread-local host destruction
occur there. `SPXAJH01` positive generation-tagged handles keep JVM values
opaque. `OwnedSession.consume()` atomically claims the wrapper cell, restores
the exact handle only for a defined precommit Android-domain rejection, and
never restores it after success or terminal/uncertain execution.
`AutoCloseable.close()` and the API-28 `PhantomReference`/`ReferenceQueue`
Cleaner fallback are non-throwing dispatch paths. The Cleaner thread never
enters native state; deterministic tests call the identical registered action
through `cleanForTest()` and cross a FIFO drain barrier rather than depending on
GC or process exit.

`SPXAJS01` projects only the closed status class/retry/domain fields into a
fixed `u64`; JVM exception class, text, stack, and object remain nonsemantic.
The precommit callback probe recognizes the one declared fixture exception,
maps every other throwable to `semaprax.adapter.unexpected.v1`, clears the JNI
exception, and returns with no pending exception. The installed assertion
contract covers O0 explicit consume, O2 Cleaner consumption, their one-winner
race, stale/forged/cross-runtime/wrong-thread/reentrant rejection, poisoned
output preservation, exact finalizer order `1:13,0:11`, no-owned publication,
zero measured Rust postcommit allocations, healthy host state, and an empty
outer handle table.

The implementation, local Rust/strict-C checks, packaging contract, and CI
source locks are backed by a green API-35 x86_64 APK/Instrumentation execution
in [run 31338834586, job
93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
Consequently this is partial Java/Kotlin and Android application evidence, not AAR, UI,
lifecycle/accessibility breadth, device support, general resource/imported-
finalizer execution, public ABI/admission, or permission to open `SPX-B104`.

The private [Apple Swift ownership adapter
v1](APPLE-SWIFT-OWNERSHIP-V1.md) composes the same iOS static lease and receipt
ledger with a Swift-owned stable thread, opaque generation-tagged sessions,
poison-preserving outputs, and explicit-versus-ARC cleanup arbitration.
Generated C binds fixed hidden evidence hooks; caller-selected hooks and the
legacy raw open are absent. A Swift 6 lane is configured to construct device
and universal Simulator slices plus two installed no-UI apps. Local gates and
the bounded hosted Apple link/app path are green in [run 31338834586, job
93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228).

The private [WIT boundary v1](WIT-COMPONENT-BOUNDARY-V1.md) freezes one scalar
result/status WIT mapping and JavaScript adapter over the existing Wasm
semantics. The same default-off harness now emits a separate standards-valid
scalar Component Model binary with a frozen digest, independently parses its
exact canonical-lift profile, and executes the extracted import-free core
module through Node's standard WebAssembly engine. That bounded runtime parses
the component container but is not engine-native Component Model
instantiation. Checked component v2 separately embeds the unmodified
SEMAPRAX-generated scalar core beside a frozen checked-runtime core, wires the
runtime instance as its exact `env` import, and lifts `semaprax_main` as a
zero-argument `evaluate`. A pinned upstream `wasmparser` gate validates that
composition and rejects rehashed invalid signatures, bodies, cardinalities,
and canonical-lift cross-typing. Node executes generated success, overflow,
and contract-failure paths through the authenticated v2 API.

Portable Result Component v3 is a third exact private profile. It canonically
lifts the checked two-`i64` generated core as `result<s64, status>`, binds
component/core/profile/source digests, and is admitted by both an independent
bounded parser and maintained upstream validation. The standalone Wasmtime
47.0.3 runner re-authenticates immutable bytes, requires zero imports,
instantiates with an empty linker and no WASI or host callbacks, then uses
generated typed bindings for success, addition overflow, division by zero,
false precondition, and false postcondition. Node core evidence independently
freezes poisoned result-slot preservation and sticky first-failure status
selection. Fuel exhaustion is an out-of-band engine error, never a typed
SEMAPRAX status. The runner's unpublished workspace, lockfile, Rust 1.97.1
toolchain, and dependency-denial policy isolate Wasmtime from the root compiler
dependency and MSRV graph. The current prelude-bound KAT migration and
standalone runner are hosted green in [run 31347109201, job
93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).

Private Source-Result Component v4 is a fourth, separately versioned profile.
It admits only the exact effect-free `component.source`/`component.evaluate`
closure whose selected signature is
`(i64, bool, i64) -> Result<bool, bool>`. The generated core is derived from
validated source/HIR and CleanupPlan v2; admission independently binds the
compiler-owned prelude, exact `Result<i64, bool>` and `Result<bool, bool>`
Wasm32 layout-v2 digests, selected closure, source revision, core bytes, and
profile. Its WIT 0.2 interface lifts the source value as
`result<result<bool, bool>, status>`: language `Ok` and residual `Err` remain
the inner result, while a recognized contract/arithmetic status becomes the
outer error. The canonical adapter checks status before reading poisoned
source-result storage, validates boolean values, lowers into separately sized
canonical memory, and traps on invalid internal tags or unknown statuses. It
never transmutes the compiler's internal variant representation into WIT.

The import-free component has an independent exact-profile parser, canonical
LEB/every-byte/truncation/trailing and rehashed cross-profile/type/lift
rejection, plus maintained upstream validation. Local core execution covers
language values, residual short-circuiting, status precedence, poison, and
re-entry. The isolated Wasmtime runner is extended with generated v4 bindings
and ten exact same-instance/fresh-instance outcomes; its execution is hosted
green in [run 31356536123, job
93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
V1-v3 remain unchanged.
Private Scalar Algebraic Component v5 is a separate default-off profile with
six fixed exports for `Option<i64>`, `Option<bool>`, and the complete
direct-copy `Result<T, E>` matrix over `i64`/`bool`. Each language carrier is
nested inside the unchanged outer physical-status result. Admission binds the
capability-free seven-function source table/order, prelude and six Wasm32
layout-v2 digests, stable-ID/core-index/distinct-WIT-type mapping, canonical
outer layouts, fieldwise tag-last reconstruction, and the complete
source/core/profile/component DAG. Exact-profile, reindexing, mutation,
cross-version, invalid-value, upstream-validation, and source-lock gates are
green; isolated typed Wasmtime execution on pinned Rust 1.97.1 is hosted green
in [run 31360176398, job 93367728269](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728269).
V1-v4 bytes remain unchanged. General source `Result`/`Option`/`?`
mapping, user records/variants/resources,
imports, async, capabilities, callable/FFI signatures, multi-engine/browser
execution, public component API/ABI, and `SPX-B104` remain outside this trust
boundary.

Private Nested Record Component v6 is a sixth separate default-off profile for
WIT package `semaprax:private@0.4.0`, interface `nested-records`, and world
`semaprax-private-v6`. It admits only the fixed source IDs `component.inner`,
`component.outer`, `component.transform`, and `app.main`, and exports one
`transform(input: outer, delta: s64) -> result<outer, status>`. The exact source,
generated core, Inner/Outer Wasm32 layouts, profile, component, and complete DAG
are independently authenticated; nested reconstruction remains fieldwise and
publication remains status-first/poison-preserving. Local exact-profile and
upstream validation, mutation/reindexing/cross-version closure, generated-core
execution, default-consumer hiding, source locks, and independent security
review are green. The isolated pinned Rust 1.97.1/Wasmtime 47 typed runtime is
hosted green in [run 31365363898, job
93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974).
This remains exact private-profile evidence rather than a broader runtime-complete
claim. V1-v5 remain unchanged;
the frozen SHA-256 KATs are source
`d1fcbc45b3d86fa1d7910378578828df3c557dba92f90ed9459f928c5bf2fe8a`,
core `42835dcbf98078ac24bfd36568f1b6917b5b64ca2d8265ef4ded161d26438da1`,
Inner layout `186a97e659ee80b641bde566c9875122f8eea4ea265c3a1af97cfb11bef98a87`,
Outer layout `4885c0353cb05928018d3527a13e363cbe10f3a0b9c4f5b0ba15792097fbbe6f`,
profile `9ed506e78134b7de29ed693084ad685068792b80321e29b819cbeb8cf96f17a3`,
component `ad408a7a6a3596a026eb73bc423e59f30350c0e4f7cbc507ce60510eff2b530f`,
and DAG `ca0856fed4eef6ac7d3ab7ed466075c60d7ff4ec0372a891ddf483d199941a3f`;
general/empty/generic/resource records, algebraic nesting, imports,
capabilities, callbacks/async, public ABI, browser/multi-engine support,
package/version negotiation, and `SPX-B104`/`SPX-W111` remain closed.

Private Generic Record Component v7 is a seventh separate default-off profile
for WIT package `semaprax:private@0.5.0`, interface `generic-records`, and world
`semaprax-private-v7`. Four fixed exports cover `Duo<i64, bool>`,
`Duo<bool, i64>`, `Phantom<i64>`, and `Phantom<bool>` while preserving the
unchanged outer status result. Admission binds the exact capability-free source
closure, Graph v12 digest, ordered concrete arguments, four Wasm32 layout
digests, stable source-ID/core-index/distinct-WIT-type/export map, plan/profile/
component digests, and the distinct identity of physically identical Phantom
instances. Exact/upstream validation, same-signature reindexing and cross-
version hostility, generated-core Node execution, default-consumer hiding,
source locks, strict gates, and independent security review are locally green.
The isolated Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
V1-v6 bytes remain unchanged;
there is no general source selection/exporter, nested/resource/non-Copy record
mapping, imports/capabilities/callbacks/async, callable/FFI or public ABI,
browser/multi-engine claim, package negotiation, or `SPX-B104`/`SPX-W111`
widening.

Private Record-Pattern Projection Component v8 is an eighth separate
default-off profile for WIT package `semaprax:private@0.6.0`, interface
`record-pattern-projections`, and world `semaprax-private-v8`. Its exact source
declares generic record `Phantom<T> { marker: bool }`, but all four exported
functions are monomorphic and the profile rejects every generic function
template or instance. Ordered preserve/invert exports cover exact
`Phantom<i64>` and `Phantom<bool>` inputs plus a scalar control and return the
projected boolean inside the unchanged outer status result. Admission binds
the exact source, generated core, two distinct same-layout Wasm32 instance
digests, Graph v13, stable function/core-index/named-WIT-type mapping, fixed
scratch/result plan, profile, component, and artifact DAG. The canonical
adapter validates input booleans before calling, checks physical status before
reading output, reconstructs fieldwise, publishes the result tag last, and
keeps the complete 20-byte result poisoned on failure or invalid values.
Independent/upstream validation rejects every byte mutation and all six
same-signature function-index swaps; only the four polarity-changing swaps are
behaviorally distinguishable, while the two same-polarity cross-instance swaps
remain identity/KAT evidence. Local Node execution, source locks, strict gates,
and independent security review are green. The zero-import, empty-linker,
no-WASI pinned Rust 1.97.1/Wasmtime 47 runner is hosted green in [run
31385406865, job
93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268).
V1-v7 bytes remain unchanged. V8 provides no generic-
function component, general source selection, imports/capabilities/resources,
callable/FFI or public ABI, browser/multi-engine claim, package negotiation, or
`SPX-B104`/`SPX-W111` widening.

## Record lowering and backend gate

Canonical source accepts nominal records with persistent field IDs, source-ordered construction, shorthand expansion, chained projection, and immutable `with` update. The verifier reports unknown, duplicate, missing, or mismatched fields deterministically and rejects direct or indirect by-value layout cycles. Resolved HIR distinguishes place projections from projections of temporary values, preserves base-first and authored replacement order for update, and its validator rejects foreign/reordered fields and inconsistent facts.

Checked declaration-ordered Native64 and Wasm32 layouts cover nested `i64`,
`bool`, and direct trivial-resource fields. Both profiles freeze the empty record
to size and alignment one, giving C11 an inert byte and preventing Wasm frame
slots from aliasing. The production-reachable scalar-record slice lowers nested
`i64`/`bool` construction, projection, and update through native C11/Clang at
O0/O2 and browser Wasm executed under Node. Internal aggregate parameters are
pointers, results use caller-owned storage, failures preserve poisoned output,
and Wasm restores its shadow stack across repeated same-instance calls.

Resources now require one explicitly identified `drop trivial` or `drop
import` strategy. Imported strategies resolve through an explicitly identified
interface/import contract with ownership, authority, consumption,
result-publication, and failure meaning; the v1 source grammar uses the import
`@id` as its logical key while HIR keeps those concepts separate. Resources
and records containing resources remain semantically non-copy. The compiler has
a shared cleanup plan plus target-neutral reference replay/execution, and the
native scalar lane has a non-trapping status/out ABI. The deterministic
`semaprax.semantic-event-dictionary.v1` maps compiler-generated nonzero
ordinals back to exact semantic events without reconstructing execution.
Generated native C at O0/O2 now runs through the physical ownership host, while
real Node/Wasm runs the same authoritative 14-case direct-trivial-resource
corpus. Both materialize to the exact reference trace and normalized outcome;
native additionally verifies publication, owner rotation, and final logical
liveness through the ledger. Native public resource lowering still rejects
with `SPX-B104` for the remaining evidence boundaries above.

Aggregate-resource execution has a narrower private proof boundary. One
test-only scenario is derived from the same validated cleanup plan and projected
into C11 O0/O2 and real Wasm. It covers move-in/out, whole-record leaf
expansion, displaced reverse cleanup, propagated call failure, poisoned result
storage, one exact cross-backend finalization trace, and zero final liveness.
This is not an ordinary production emitter/runtime path and grants no public
resource-record, callable/component aggregate-signature, or ABI authority.

WebAssembly now has one public but deliberately narrow exception to the former
blanket resource gate. `semaprax.wasm-owned.v1` admits exactly one direct,
non-generic `drop trivial` resource identity, direct `own` parameters, scalar
parameters, and either an `i64` or selected owned-input result for a restricted
statement-free contract/body shape. The emitter consumes replay-validated
terminal cleanup vectors without sorting them. Its generated instance host uses
exact export metadata, SHA-256 binding to the exact generated Wasm bytes,
private imports, canonical ABI argument checks, one-shot trusted adoption
tickets, checked/aligned out ranges before ownership commit, instance-tagged
slot/generation handles, a pre-reserved normalized-status cell, semantic ordinal
storage, and poison-preserving publication. One same-realm global allocator
prevents tag reuse across separately evaluated copies of the generated host when
the surrounding realm and reserved binding are trusted; scalar-only packages
allocate no tag. Real Node execution covers the narrow runtime boundary and the
shared 14-case semantic conformance corpus. Imported lifecycles, calls,
resource-containing aggregates, multiple resource identities, broader control flow, hostile
co-resident JavaScript, cross-realm/worker identity, and Components remain gated
with `SPX-W111` or the record diagnostic; the native production-host connection
is still absent.

## Semantic graph

Graph serialization is exclusively from validated resolved HIR. The
program-wide schema lattice is `semaprax.graph.v14` when any authenticated
generic function declaration exists, even when unused; otherwise v13 applies
to an explicit record pattern, v12 to a generic record declaration, v11 to
Option propagation, and byte-compatible v10 to legacy/Result programs. Every
bounded and Agent Context reports the same program choice. V14 adds persistent
or visibly automatic `function_template` declarations, nonpersistent exact
`function_instance` nodes, and `call_instance` expressions carrying the
template, derived instance ID, and ordered concrete arguments. Unused templates
have no fabricated instance; v10-v13 bytes remain unchanged when no generic
function is declared. The frozen v14 SHA-256 KATs are module
`449c74b9284a1e5f00a6823c1e01f87e15fe76882e9fc76512b0d22dc0ce9941`,
Agent Context
`54cfc493bc285fb43767ea37f558e9d59c1c36e32915ab35e640edf422efc28c`,
and bounded context
`880a5f21a12e3c945ec75f08af4889c98a75925dec23f491e01ce4317cea6e1c`.
V13 pattern nodes bind exact concrete record/member/binding identity; v12 record
nodes carry ordered owner/index parameters; v11 authenticates Option
propagation. The graph otherwise retains the canonical source/prelude revision,
declarations, exact types, structural bodies/contracts/calls, and complete
per-function CleanupPlan v2/v3. Cleanup vectors preserve canonical execution
order and never repair malformed input. Generic-function CleanupPlan v2 status
producers intentionally remain template-ID-only; exact instance meaning is
carried by validated HIR and v14 call/instance nodes. Context closure includes
selected function templates and their exact referenced instances without
inventing executable evidence for an unused template. Graph revision v2 and
cache binding remain unchanged.

Integer literals are decimal JSON strings rather than JSON numbers, so JavaScript and TypeScript agents preserve every `i64` value exactly. `let` bindings expose one value identity; the enclosing statement does not reuse that ID as a second identity domain.

Expression and value IDs are deterministic but revision-scoped; only explicitly authored declaration IDs are persistent across revisions. Automatic name-derived declaration IDs remain visibly marked unstable. Spans are intentionally absent because canonical source revisions ignore whitespace while spans do not.

Context slicing starts from a display name or exact declaration ID, with exact IDs taking precedence on collisions. It walks declaration-ID call dependencies from preconditions, bodies, and postconditions to a bounded depth and closes referenced record types transitively through their field types. Unrelated declarations remain excluded. Every result declares a `module` or `context` view. Context views record root, depth, truncation, and frontier IDs so an omitted call dependency is distinguishable from a dangling reference. Callers, tests, packages, targets, and generated artifacts will become additional typed edges.

The additive [`semaprax.agent-context.v1`](AGENT-CONTEXT-V1.md) projection is
the current CLI context contract. It applies exact whole-document byte and
function-node budgets, reports used/omitted budgets, and turns known omitted
functions into stable-ID progress frontiers, with exact deferred counts,
non-dangling emitted call edges, a query-bound minimum-byte cursor, aggregate
pagination by stable-ID re-rooting, and fail-closed rejection only when an
individual page cannot fit the contract maximum. Compact contracts, parameter/result
ownership, effects, and reference-closed types are selectable; cleanup,
lifecycle, and import subgraphs are not claimed by this projection. Graph v10
has no target, diagnostic, or test nodes, so those requests are marked
unavailable rather than inferred. The legacy Rust depth-slice API over Graph v10
remains compatible.

The additive [`semaprax.agent-context-economics.v1`](AGENT-ECONOMICS-V1.md)
layer runs strict checked-in maintenance manifests offline. Canonical
exact-case, separator-normal, Windows-forbidden/reserved-name-safe,
non-symlink source containment, a manifest
digest, exact label arrays, source revision, and context digest bind every
score. Only facets available in Graph
v8 may be scored. It records exact bytes, emitted nodes, and a
repository-defined lexical unit explicitly marked as non-model-token data. The
quality router exposes advisory `quick`, explicit-or-unique-target-merge-base
to HEAD plus dirty-Git-state-reconciled `changed`, and default `full` profiles; its closed v2 plan
carries a canonical base, exact path/invariant/test records, and the profile's
exact ordered gates, which the executor validates before dispatch. Broad graph/CLI,
unknown, wide, or router changes fail closed to the full workspace baseline.

The public parsed-AST graph functions resolve and validate HIR and return diagnostics on failure. Direct HIR rendering remains internal so a caller cannot attach a forged canonical-source revision to transformed HIR. The graph currently rebuilds per command; a later daemon will persist indexed revisions.

## Transactions

The `.spatch` protocol is intentionally smaller than a text patch:

```text
base <graph-revision>
rename <stable-id> to <new-name>
require no-new-effects
```

Application is all-or-nothing:

1. Parse the current source and compare its canonical revision.
2. Resolve operations through stable IDs.
3. Apply declaration and call-edge changes in memory.
4. Reparse and verify the candidate program.
5. Evaluate patch requirements.
6. Canonically render to a sibling temporary file.
7. Atomically rename it over the original.

The protocol will evolve toward structured JSON/CBOR operations with typed payloads, affected-node proofs, target requirements, and multi-file commits.

## Native backend

The v0.2 native backend emits readable C11, then invokes Clang. C is an implementation IR, not a promised ecosystem boundary. This gives the prototype real native binaries, easy inspection, sanitizers, and broad host support with almost no backend dependency surface.

The C emitter materializes subexpressions in source order before calls and operators. This is required because C does not define function-argument evaluation order, while SEMAPRAX does. Lazy boolean operators and `if` expressions lower to explicit branches, and generated local names cannot collide with source identifiers. Generic function symbols derive from the domain-separated exact template-plus-ordered-argument execution identity, so same-signature templates and `i64`/`bool` instances remain distinct; only explicitly referenced instances are emitted. Generic record structs likewise use exact-instance symbols. The bounded copy-variant lane emits deterministic structs with an explicit `uint32_t` tag, a declaration-order payload union, and compile-time size/alignment/offset assertions. Constructors evaluate payloads in authored order, zero the full representation, write payload fields, and publish the tag last. Matches stage the scrutinee once, validate the tag before union access, and execute only the selected arm. Invalid tags terminate through the runtime-invariant path rather than becoming a semantic status. Every function uses `(context, parameters..., result_out) -> status_token`, with internal aggregate parameters passed by pointer and caller-owned aggregate results; nested calls share one invocation-local arena, zero means success, contract and checked-arithmetic failures return exact normalized records, and `result_out` is written only at the final success commit. The executable wrapper translates a completed root failure back into the existing process diagnostics and exit codes.

The planned development backend is Cranelift. The planned optimizing pipeline uses multi-level IR with LLVM, while portable components lower through the WebAssembly Component Model. Backend changes must preserve the graph and verification contracts.

## Ownership seed

Resource declarations and records containing resources introduce non-copy semantic values. Function parameters state whether they receive ownership, borrow for the duration of a call, or participate in explicit shared ownership. `let` and record construction transfer owned values left-to-right while preserving borrowed/shared modes. Moving an owned record field invalidates that place and its parents while leaving disjoint siblings available. The verifier joins field state across `if` and lazy boolean control flow, distinguishes definite from conditional partial moves, replays the same rules at the public HIR boundary, and prevents borrowed/shared fields from crossing owned boundaries.

This is the first ownership IR, not a complete borrow checker. Mutable alias exclusion, inferred reborrows, lifetime parameters, regions, destructors, ARC operations, and FFI ownership remain explicit completion gates.

## WebAssembly bootstrap backend

The direct Wasm encoder emits standard WebAssembly core modules without requiring a Rust target installation or an external assembler. Monomorphic functions and explicitly referenced generic-function instances compile to distinct typed Wasm functions; unused templates allocate no index, and `main` remains exported as `semaprax_main`. The aggregate profile lowers bounded copy variants into checked Wasm32 frame layouts with a `u32` tag and aligned maximum payload, zero-fills before tag-last publication, evaluates a match scrutinee once, and evaluates only the selected scalar arm. An invalid tag uses a private negative sentinel, restores the shadow stack, and traps out of band at the public wrapper; it is never mapped to a language failure. Contracts trap through a host import. Arithmetic lowers to a small generated JavaScript host that performs checked `i64` operations with `BigInt`, preserving the safe arithmetic semantics instead of silently accepting Wasm's wrapping operators.

The web package contains `app.wasm`, `semaprax.js`, `index.html`, `package.json`, and a `semaprax.web.v3` graph-revision/capability manifest. Version 3 adds a required `semaprax.wasm-owned.v1` function map; scalar-only packages carry an empty map and do not allocate owned-runtime identity. This is real browser-executable output; it is not yet the UI dialect, DOM renderer, SSR/hydration system, WASI target, or Component Model backend.

## Development integrity

`AGENTS.md` defines the repository invariants and change protocol. `docs/QUALITY-GATES.md` defines baseline and semantic-layer-specific evidence. CI runs formatting, strict linting, tests, release builds, native execution, Wasm instantiation, crate packaging, and the declared Rust minimum version. These gates reduce regressions; they do not turn a partial completion-matrix row into a completed one.

## Trust boundaries

- Source and patch input are untrusted and fully parsed.
- Names are restricted before reaching C identifiers.
- String data embedded into C diagnostics is escaped.
- Generated C is compiled without shell interpolation.
- Patch writes are revision-bound, verified, and atomic.
- Native dynamic-library loading is confined to unpublished quarantine crates.
  The private callable-v2 host connects exact descriptor/dictionary/certificate
  admission, loader lease, authority, ledger, strict wire transport, and
  compiler-generated execution. Its unsafe admission contract still trusts the
  selected native image and dependencies; it is not a sandbox or code-identity
  proof. Public bundle construction adds no unsafe loading authority; the
  public execution/admission gate remains closed.
- `prepareTrustedAdoption` is the Wasm host's explicit trusted assertion that one unique external ownership identity is being transferred. Tickets are one-shot; exact Wasm byte binding keeps the mutating imports private to the generated artifact; canonical arguments, generated export metadata, and the result range are checked before ownership commit.
- Same-realm Wasm instance tags are coordinated through one host-global allocator. The realm and its reserved binding are trusted host state; hostile pre-poisoning, cross-realm, and worker identity remain outside the implemented guarantee.
- The compiler currently invokes the host `clang`; sandboxed build execution is roadmap work.
