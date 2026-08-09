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

This remains staged groundwork rather than the sole compiler IR: the current verifier still establishes meaning from parsed AST before HIR resolution. Explicit trivial/imported resource lifecycles, declaration-only interface/import contracts, record declarations, constructors, projections, stable identities, recursive resource/type facts, and by-value recursion rejection now reach validated HIR and Graph v6. The source checker and HIR validator independently replay lifecycle compatibility, lifecycle-effect authority, and prefix-aware partial-place availability. `CleanupInventory` remains a structural discovery boundary. Every `ResolvedFunction` additionally carries `semaprax.cleanup-plan.v1`: typed blocks, edges, lexical regions, entry liveness, storage/leaf flags, atomic call commits, sticky status sources, guarded finalizers, and scalar/owned result publication. The builder covers every current HIR expression and normal/checked-failure path; the validator reconstructs the plan from core HIR rather than trusting attached metadata.

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
Exhaustive process-crash/fatal-allocator evidence and representative Android/iOS
execution,
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
Windows dynamic runtime and the bounded arm64-Simulator path are green; Android
runtime remains absent. The legacy loader constructors reject the full v3 magic in their shared
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
execution, or Android, and exposes no
public admission. Private bounded process-lifetime static-registration logic
now binds exact descriptor and entry addresses to the same host ledger; its
non-Apple fake-function test proves retention and quarantine only, with no
`dlopen` or unload claim. A mandatory macOS gate requires the loader and host
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

## Record groundwork and backend gate

Canonical source accepts nominal records with persistent field IDs, source-ordered construction, shorthand expansion, and chained projection. The verifier reports unknown, duplicate, missing, or mismatched constructor fields deterministically and rejects direct or indirect by-value layout cycles. Resolved HIR distinguishes place projections from projections of temporary values, and its validator rejects foreign/reordered fields and inconsistent facts.

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
aggregates, multiple resource identities, broader control flow, hostile
co-resident JavaScript, cross-realm/worker identity, and Components remain gated
with `SPX-W111` or the record diagnostic; the native production-host connection
is still absent.

## Semantic graph

`semaprax.graph.v6` is serialized exclusively from validated resolved HIR. It contains the canonical human-source revision, module capabilities, entrypoint declaration ID, explicit-versus-automatic declaration identity origin, resource/lifecycle/interface/import/record/field/function nodes, typed ownership and result-publication contracts, effects/authority/failure meaning, structural contracts, sorted declaration-ID call dependencies, the structural body graph, and the complete cleanup plan for every selected function. Cleanup vectors preserve canonical execution order and use tagged storage/place/status/transition/edge/region/exit forms; serialization never sorts malformed input into apparent validity. Context closure follows a resource through its lifecycle, full interface contract, import signatures, nominal types, and each selected function's self-contained cleanup plan. The source revision algorithm is unchanged, so caches key by both graph schema and revision.

Integer literals are decimal JSON strings rather than JSON numbers, so JavaScript and TypeScript agents preserve every `i64` value exactly. `let` bindings expose one value identity; the enclosing statement does not reuse that ID as a second identity domain.

Expression and value IDs are deterministic but revision-scoped; only explicitly authored declaration IDs are persistent across revisions. Automatic name-derived declaration IDs remain visibly marked unstable. Spans are intentionally absent because canonical source revisions ignore whitespace while spans do not.

Context slicing starts from a display name or exact declaration ID, with exact IDs taking precedence on collisions. It walks declaration-ID call dependencies from preconditions, bodies, and postconditions to a bounded depth and closes referenced record types transitively through their field types. Unrelated declarations remain excluded. Every result declares a `module` or `context` view. Context views record root, depth, truncation, and frontier IDs so an omitted call dependency is distinguishable from a dangling reference. Callers, tests, packages, targets, and generated artifacts will become additional typed edges.

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

The C emitter materializes subexpressions in source order before calls and operators. This is required because C does not define function-argument evaluation order, while SEMAPRAX does. Lazy boolean operators and `if` expressions lower to explicit branches, and generated local names cannot collide with source identifiers. Every scalar function uses `(context, parameters..., result_out) -> status_token`: nested calls share one invocation-local arena, zero means success, contract and checked-arithmetic failures return exact normalized records, and `result_out` is written only at the final success commit. The executable wrapper translates a completed root failure back into the existing process diagnostics and exit codes.

The planned development backend is Cranelift. The planned optimizing pipeline uses multi-level IR with LLVM, while portable components lower through the WebAssembly Component Model. Backend changes must preserve the graph and verification contracts.

## Ownership seed

Resource declarations and records containing resources introduce non-copy semantic values. Function parameters state whether they receive ownership, borrow for the duration of a call, or participate in explicit shared ownership. `let` and record construction transfer owned values left-to-right while preserving borrowed/shared modes. Moving an owned record field invalidates that place and its parents while leaving disjoint siblings available. The verifier joins field state across `if` and lazy boolean control flow, distinguishes definite from conditional partial moves, replays the same rules at the public HIR boundary, and prevents borrowed/shared fields from crossing owned boundaries.

This is the first ownership IR, not a complete borrow checker. Mutable alias exclusion, inferred reborrows, lifetime parameters, regions, destructors, ARC operations, and FFI ownership remain explicit completion gates.

## WebAssembly bootstrap backend

The direct Wasm encoder emits standard WebAssembly core modules without requiring a Rust target installation or an external assembler. User functions compile to typed Wasm functions and `main` is exported as `semaprax_main`. Contracts trap through a host import. Arithmetic lowers to a small generated JavaScript host that performs checked `i64` operations with `BigInt`, preserving the safe arithmetic semantics instead of silently accepting Wasm's wrapping operators.

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
