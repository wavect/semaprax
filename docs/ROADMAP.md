# Roadmap

The roadmap follows risk, not feature spectacle. Stable semantic editing, ownership inference, and component boundaries are more important than accumulating syntax.

## 0.1 — Executable semantic seed

Status: implemented in this repository.

- Canonical source and typed expression core.
- Stable declaration identities.
- Revisioned semantic graph and context slices.
- Versioned byte/node-bounded Agent Context v1 with stable replay frontiers,
  plus additive forward/reverse/both Agent Context v2 call traversal with
  separate traversal/reference frontiers and direction-bound replay. V1
  remains byte-compatible by default; v2 and v1-preservation gates are hosted
  green in [run 31397881268, Ubuntu job
  93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327).
- Bounded Semantic Impact v1 previews one Patch v1/v2
  file read-only, with exact source-consumer provenance and finite reverse-call
  impact for generic call-instance changes. Repository-wide/non-call impact,
  persistent incrementality, repair, ranking, and general review remain open.
  Separate bounded Review v1 is described below; Impact itself emits no review
  sections. Its
  exact `1b3731a` full matrix is hosted green in [run 31408654657 attempt
  2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2),
  including [Ubuntu job
  93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404).
- Bounded Semantic Review v1 locally emits one fixed-section read-only report
  for Patch v1/v2 through complete nontruncated Impact-v1 evidence and for the
  sole canonical Patch v3 through the shared identity rebase. Its Patch
  v1/v2/v3 report KATs, local 10/10 integration, 4/4 hook/limit units, library
  408/408, full preservation, and security gates are green. The exact
  `2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in
  [run 31423743369 attempt
  1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
  including [Ubuntu job
  93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170);
  all 12 jobs passed. Context, target/test execution, proof/verifier/provenance, approval,
  A0 authority, and repository/multi-file review remain open.
- Semantic Patch Evidence v1 emits and independently replays bounded capsules
  for Patch v1/v2 and the sole Patch v3 operation. Its separate
  `patch-with-evidence` route acquires the unchanged A0 lock and requires exact
  replay before staging; ordinary `patch` remains unchanged. A+B is 11/11
  integration plus 5/5 internal units, Phase C is 16/16 integration plus 11/11
  hook/limit units, and library 420/420 plus doctest 37/37 are locally green.
  The exact `34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted
  green in [run
  31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
  including [Ubuntu job
  93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
  all 12 jobs passed. The earlier `e04c2c9` run failed only the Rust 1.97 lint
  and is not green evidence. Signatures/provenance, approval, target/test execution, general
  formal proofs, repository/multi-file scope, and reusable authorization remain
  open.
- Semantic Target Evidence v1 reports exact compiler projections for one
  patch: base/candidate Graph JSON, typed zero capability delta, production C11
  source, and structurally validated Wasm core. Evidence v2 additively binds
  that report into generation, verification, and lock-first A0 apply. Target
  9/9, target units 4/4, Evidence-v2 8/8, library 439/439, full local gates,
  and security are green. The exact
  `fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in [run
  31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793),
  including [Ubuntu job
  93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631);
  all 12 jobs passed. The artifacts execute no
  target/project test, carry no authority, and change no status. Their
  single-file artifact scope is unchanged by the separate workspace tranche.
- Semantic Workspace Transaction v1 is the first bounded managed-generation
  publication tranche. It authenticates 2–16 canonical existing sources,
  previews unchanged admitted per-file Patch v1/v2/v3 operations, publishes a
  complete immutable generation, and pivots only `ACTIVE` for cooperating
  locked readers. Integration 12/12, hostile wire/CLI 5/5, workspace units
  37/37, library 482/482, full local gates, preservation, and security are
  green. The exact `afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted
  green in [run
  31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
  including [Ubuntu job
  93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613)
  and [Windows job
  93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611);
  all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
  strict Clippy and is not green evidence. Raw-source/Git/editor atomicity, cross-file
  module/type/call/capability semantics, repository Graph/analysis,
  create/delete/move, materialization, recovery/GC, and power-loss durability
  remain open. No completion status changes.
- Offline context economics with exact goldens and conservative quality routing.
- Effects, module permits, and contract guards.
- Machine-readable diagnostics.
- Hardened atomic, stale-safe single-file semantic renames with authenticated
  regular source/stage identity, a cooperating create-new sibling lock,
  bounded create-new staging, and identity-aware cleanup. Unix device/inode is
  exact; Windows held same-file volume plus 64-bit file-index comparison does
  not claim ReFS 128-bit or hostile non-unique-index uniqueness. Predictable-
  name collision/stale-lock DoS, crash-left locks, the trusted final portable
  path window, power-loss durability, and general raw-source/repository
  multi-file commits remain open. The separate managed workspace protocol has
  its own bounded `ACTIVE` publication boundary.
- Checked native code generation.

## 0.2 — Useful core language

Status: in progress. Resource ownership boundaries and explicit
lifecycle/interface contracts, lexical `let`, typed `if/else`, partial-place
diagnostics, record construction/projection/immutable-update semantics, bounded
explicitly instantiated generic Copy records, generic copy variants/exhaustive
matching, ordinary prelude `Option`/`Result`,
bounded direct-scalar `Result` and `Option` propagation, bounded irrefutable
Copy-record destructuring, bounded explicitly instantiated direct-scalar Copy
generic functions, feature-minimal Graph v14/v13/v12/v11/v10 lattice for
generic functions/explicit record patterns/generic records/Option propagation/legacy, validated
stable-ID HIR/type facts, mandatory replay-validated CleanupPlan v2/v3 plans, versioned normalized-status
and semantic-event-dictionary types, native scalar status/out execution, a
browser-loadable scalar Wasm backend, and one narrow direct-trivial-resource
Wasm owned ABI are implemented. The bounded generic-record Native O0/O2 and
Node/Wasm gate is hosted green in [run 31365363898, Ubuntu job
93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995).
A private generated-callable native host now
connects the exact loader, authority, ledger, strict codecs, event dictionary,
and compiler-owned trace-path certificate. That host and the real Wasm lane
match the reference outcome, complete trace, publication, and final logical
liveness for all 14 cases at native O0/O2; the remaining public and language
gates below are not.

- Complete record breadth plus nested/resource-bearing generic records and
  variants and
  non-copy `Option`/`Result` instantiations.
- Refutable, literal, guarded, alternative, nested-variant, and ownership-aware
  pattern matching beyond the bounded irrefutable Copy-record and exhaustive
  copy-variant slices.
- Generic functions beyond the bounded direct-scalar slice: inference,
  constraints, aggregate/resource/non-Copy signatures, generic composition,
  and separate compilation.
- Modules, imports, and multi-file graph commits.
- Diagnostic repairs beyond the implemented bounded `SPX-S103`
  `assign-function-id` tranche: typed holes, other diagnostics/declaration
  kinds, ranking, composition, automatic application, and multi-file repair.
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
independently rebuilds the plan, and the program-derived Graph v10-v14 lattice
serializes it. Checked Native64
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

Bounded Generic Copy Records now admits explicitly instantiated templates with
one or more owner/index-stable parameters, direct scalar or own-parameter
fields, and direct `i64`/`bool` arguments. Exact instances key HIR facts,
Native64/Wasm32 layouts, digests, caches, native symbols, and Graph v12;
same-layout instances remain distinct. Native C11 O0/O2 and 4,096-call
Node/Wasm evidence covers `Box<T>`, multi-field `Pair<T>`, ordered
`Duo<T,U>`, construction/projection/update/pass-return, both bool arms,
failure order, and poison. CleanupPlan remains canonical v2 because this slice
is resource-free Copy. Generic-record inference, nested/resource/non-Copy
arguments or fields, public aggregate/callable/FFI ABI, and public resource
admission remain subsequent work.

Bounded Generic Copy Functions now admit one or two owner/index-stable type
parameters, direct scalar or own-parameter by-value signature slots, and
explicit ordered `i64`/`bool` call arguments. Verification checks unused
templates over every direct-scalar substitution without materializing an
instance. Separate HIR template/monomorphic/instance vectors, domain-separated
execution identities, exact native symbols and Wasm indices, and program-wide
Graph v14 authenticate only explicitly referenced instances. Module, Agent
Context, and bounded-context KATs are frozen; local strict C11 O0/O2,
failure/poison, 4,096-entry Node/Wasm, hostile boundary, and independent
security gates are green, and the hosted matrix is green in [run 31385406865,
Ubuntu job
93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). CleanupPlan
v2 remains unchanged and template-ID-only; HIR and Graph authenticate the exact
instance before replay. The corrected Graph-v14 module, Agent Context, and
bounded-context JSON/KAT bytes are separately hosted green in [run 31390043736,
Ubuntu job
93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296);
the execution run above predates that serializer correction. Inference,
constraints, aggregate/resource/non-Copy
signatures, effects, generic-to-generic calls, recursion, entrypoints,
callable/resource admission, general/public Component mapping, and public ABI
remain subsequent work.

Bounded Irrefutable Copy-Record Patterns now admit one explicit exact-field
record pattern or a binding-free top-level wildcard over one evaluated Copy
record scrutinee. Explicit patterns support recursive record subpatterns,
renamed/shorthand scalar or whole-record bindings, and ignored fields; the sole
arm remains `i64`/`bool`. HIR authenticates full concrete instance plus stable
record/field/binding IDs. Explicit patterns select program-wide Graph v13 above
v12/v11/v10 unless a generic function selects v14; wildcard-only record matches
retain the prior schema.
CleanupPlan v2/v3 stays straight-line with no new slots, transitions, status
sources, or decision edges. Native C11 O0/O2 and 4,096-entry Node/Wasm cover
nested/generic instances, whole-record binding, both bool paths, one call
scrutinee, failure order, postconditions, and poison; the Ubuntu gate is hosted
green in [run 31373317800, job
93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130),
and independent security review is clean. Refutable/literal/guard/or/rest/nested-variant patterns,
resource/non-Copy ownership modes, aggregate arm results, and public aggregate
ABI remain subsequent work.

Executable Copy Variants + Match now includes nominal templates with explicit
direct `i64`/`bool` arguments, alongside monomorphic unit/direct-scalar cases.
Compiler-owned ordinary `Option<T>` and `Result<T, E>` use the same checked
variant mechanism. Graph v10 and revision v2 authenticate owner/index-stable
parameters, exact concrete arguments, and `semaprax.prelude.v1`; internal
layout digest v2 authenticates concrete instances plus template/substituted
field types. Native C11 O0/O2 and Node/Wasm evidence proves distinct concrete
symbols/layouts, authored construction order, one scrutinee evaluation,
selected-arm-only execution, complete poisoned outputs on failure, invalid-tag
closure, equivalent results, and Wasm shadow-stack re-entry. CleanupPlan v2
remains cleanup-free for this Copy slice and binds each branch to the exact
scrutinee expression plus stable template case ID. The preceding non-generic
matrix is hosted green in [run
31343897595](https://github.com/wavect/semaprax/actions/runs/31343897595);
generic/prelude verification is hosted green in [run
31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201).

The bounded typed-`?` tranche accepts only compiler-owned direct-scalar Copy
`Result<T, E>` into `Result<U, E>` with exact `E`, or `Option<T>` into
`Option<U>`. It evaluates the operand once, stages `Err` or payload-free `None`
as a normal outer result rather than a physical status, skips later body
expressions, and joins the ordinary path before shared postconditions and
publication. Result retains CleanupPlan v2/Graph v10; Option uses per-function
CleanupPlan v3 and program-bound Graph v11. Native C11 O0/O2 and Node/Wasm
prove different source/outer layouts, status separation, poison, invalid-tag
closure, and re-entry. Result is hosted green; Option is hosted green in [run
31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277).
This does not open generic-function use of `?`,
nested/resource arguments, resource- or record-bearing payloads, non-copy
ownership/propagation, residual conversion, `?` in contracts, a stable public
aggregate ABI, callable/component signatures, or public resource admission.
The Result configured matrix is green in [run
31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690).

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
cross-realm/worker identity, stable public aggregate ABIs, nested/resource
generic arguments, generic-function inference/constraints/richer signatures or
broader non-Copy generic records, resource-bearing variants,
ownership-aware matching, general/non-copy/residual-converting `?`,
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
and a Windows path green in [run 31343897595, job
93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480).
It packages one
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
while the Windows Win32 package/runtime is green in [run 31343897595, job
93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480).
General SEMAPRAX UI/state syntax, SwiftUI/WinUI, broad
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
dependency/MSRV graph are implemented. The current prelude-bound KAT migration
and standalone runner are hosted green in [run 31347109201, job
93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).
Private Source-Result Component v4 now connects one exact effect-free
source closure using `Result<i64, bool>`, postfix `?`, and
`Result<bool, bool>` to the distinct WIT 0.2 type
`result<result<bool, bool>, status>`. Deterministic source/core/profile/
component/layout KATs, exact admission, independent/upstream validation,
hostile mutation and cross-version closure, generated-core execution, and CI
source locks are green. Its isolated typed Wasmtime matrix executes with
zero imports, empty linker/no WASI, repeated/fresh instances, and out-of-band
fuel failure and is hosted green in [run 31356536123, job
93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
Private Scalar Algebraic Component v5 adds a separate default-off WIT 0.3
profile with six fixed exports for `Option<i64>`, `Option<bool>`, and the
complete direct-copy `Result<T, E>` matrix over `i64`/`bool`, each nested inside
outer physical status. Exact source/core/profile/component/layout/mapping KATs,
canonical reconstruction, hostile reindexing/mutation/cross-version closure,
upstream validation, and zero-import runner execution are hosted green on
pinned Rust 1.97.1 in [run 31360176398, job 93367728269](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728269).
It does not
open general source selection, resources/non-copy carriers, imports,
capabilities, async, public ABI, or `SPX-B104`/`SPX-W111`. General source
`Result`/`Option`/`?` mapping, records/resources/imports, async/capabilities,
callable/FFI aggregate signatures, multi-engine/browser execution, public
component API/ABI, and `SPX-B104` remain later gates.

Private Nested Record Component v6 adds a separate default-off exact WIT
fixture for package `semaprax:private@0.4.0`, with one `transform` export over fixed nested scalar `inner`/`outer`
records. Exact source/core/layout/profile/component/DAG authentication,
independent/upstream validation, hostile closure, core execution,
default-consumer hiding, source locks, and security review are locally green.
Its pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
31365363898, job
93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974).
This does not open general/empty/generic/resource records, algebraic nesting,
imports/capabilities/async, public ABI, browser/multi-engine support,
package/version negotiation, or `SPX-B104`/`SPX-W111`.

Private Generic Record Component v7 adds a separate default-off exact WIT
fixture for package `semaprax:private@0.5.0`, interface `generic-records`, and
world `semaprax-private-v7`. Four exports map ordered `Duo<i64, bool>`/
`Duo<bool, i64>` and same-layout-distinct `Phantom<i64>`/`Phantom<bool>`
instances through unchanged outer status. Exact source/core/four-layout/Graph-
v12/plan/profile/component authentication, independent/upstream validation,
hostile reindexing/cross-version closure, local Node core execution, default-
consumer hiding, source locks, strict gates, and security review are green.
The pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
V1-v6 remain unchanged;
general source selection/export, general/nested/resource/non-Copy record
mapping, imports/capabilities/callbacks/async, callable/FFI or public ABI,
browser/multi-engine support, package negotiation, and `SPX-B104`/`SPX-W111`
remain later gates.

Private Record-Pattern Projection Component v8 adds a separate default-off
exact WIT fixture for package `semaprax:private@0.6.0`, interface
`record-pattern-projections`, and world `semaprax-private-v8`. Four ordered
monomorphic exports preserve or invert the exact `marker` projection from the
distinct same-layout `Phantom<i64>` and `Phantom<bool>` instances. Exact
source/core/two-layout/Graph-v13/plan/profile/component/DAG KATs,
independent/upstream validation, all-pair identity hostility, local Node core
execution, poison/invalid-value closure, default-consumer hiding, source locks,
strict gates, and security review are green. Its pinned Rust 1.97.1/Wasmtime 47
zero-import typed runner is hosted green in [run 31385406865, job
93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268).
V1-v7 remain unchanged. This does not open generic-function
components, general source selection, general/empty/nested/resource/non-Copy
record mapping, imports/capabilities, callbacks/async, callable/FFI or public
ABI, browser/multi-engine support, package negotiation, or
`SPX-B104`/`SPX-W111`.

Private Generic-Function Instance Component v9 adds a ninth separate
default-off exact WIT fixture for package `semaprax:private@0.7.0`, interface
`generic-function-instances`, and world `semaprax-private-v9`. Three phantom
Copy templates materialize exactly six explicitly referenced, ordered
Graph-v14 `FunctionInstanceId`s behind six identical scalar result signatures;
the source has no authored record or layout roots. Exact source/Graph/core/
plan/profile/raw/DAG KATs are
`218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c`,
`62907c4b95495bb573b2b37de9f0b08c7a82218934154521e8c0c8396158cc6e`,
`9f178207a0406f740198ee8c71d5d008efdf4d995ff04e11e80ea73b79155d44`,
`edd11c98bbc902d9dbc9c942375477fcf1e6c3f1befbe3c4a9f260107104485e`,
`365897ddb2770cc25a11690dddbfef5d232244ec5d328c79a24a1410e684615e`,
`3cf6c7d7d02e838fb374478a2b5b25077c7c612ad36e30deaffd15311a25a688`,
and `2623ff9a7eda5526616a15befd4951de86874a59911dcba2a7d3bcc2d178a474`.
Local core 5/5, component 4/4, CI-lock 4/4, full gates, every-byte and
all-15-pair-swap hostility (eight observable/seven identity-only), and
security review are green. Its zero-import, empty-linker, no-WASI pinned Rust
1.97.1/Wasmtime 47 typed runtime is hosted green in [run 31392541096, job
93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
V1-v8 remain
unchanged. This does not open inference/constraints, general source selection
or generic-function Component mapping, aggregates/resources/non-Copy values,
imports/capabilities, callbacks/async, callable/FFI or public ABI,
browser/multi-engine support, package negotiation, or
`SPX-B104`/`SPX-W111`.

Private Source-Option Propagation Component v10 adds a tenth separate
default-off exact WIT fixture for package `semaprax:private@0.8.0`, interface
`option-propagation`, and world `semaprax-private-v10`. Its sole export maps
the exact compiler-owned `Option<i64>` through postfix `?` to `Option<bool>`
under Graph v11 and CleanupPlan v3. Exact source/Graph/prelude/two-layout/plan/
core/profile/raw/DAG KATs are
`98b8fc892c183499153142d5bbdb4162e31bda95ef145d34dbb1ff57c9b8fc72`,
`96083f90fab18c919a96cee48109e606e089159e109869a42bdf48831743d45d`,
`d37bad7e3911669bbf2c66b25c8b31d5c2e36eb181cc54fdc86c3a49a8fb9c5e`,
`79194fc88011ac060877e60293d0a4272429dd9e2d720674d0d54e804562deda`,
`dec126293ece7ec0e48d3d85ccdb494f7c7cfe4c3d4a9b1a61b50f6f862ff038`,
`d07fa51fc6f192a43318140264fa0e5964933ed90bc065cc8c74708e258ff92f`,
`16d1d34024e3fad920d8d00a61d7cb3bd010335ca382f23615b3b3da4143aaec`,
`f53a0c21638b5a360faa19ad4fdef68f6d861a5baffe39422847128686e82bef`,
`f5770bdfdbc862ea39640b2c706c1d9ea171164c220d18366e25b3219443ad0d`,
and `90ab80260c84abfe85d1edc666ab3750b81388e6e4cffd7ca21c301b9d0ee589`.
Typed/raw, hostile, strict, and security gates are green; its zero-import pinned
Rust 1.97.1/Wasmtime 47 v3-v10 runner is hosted green in [run 31396483313, job
93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502).
V1-v9 remain unchanged. V10 does not open general source selection/export,
general `Result`/`Option`/`?` or algebraic Component mapping,
nested/resource/non-Copy carriers, imports/capabilities, callbacks/async,
callable/FFI or public ABI, browser/multi-engine support, package negotiation,
or `SPX-B104`/`SPX-W111`.

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

The bounded Semantic Patch v2 milestone adds atomic persistent member/case
renames and exact generic call-argument replacement without advancing Graph
beyond v14 or CleanupPlan beyond its existing v2/v3 selection. It remains a
single-file, trusted-patch-input capability; authenticated multi-file repair,
generic composition, and broad type renames remain roadmap work. Its exact
`f95d243` full matrix is hosted green in [run 31401200449 attempt
2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2),
including [Ubuntu job
93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044).

Bounded Diagnostic Repair v1 now discovers and instantiates one exact
`SPX-S103` automatic-function identity repair, classified
`breaking_identity_rebase`. Its isolated Semantic Patch v3 admits and applies
only the canonical three-line `assign-function-id` operation through unchanged
A0; Impact v1 remains v1/v2-only. Local Phase A integration is 13/13; the
Phase B semantic integration corpus is 7/7; v3 A0 hook units are 4/4;
aggregate v3 integration-plus-hook evidence is 9/9; and library 404/404,
full-preservation, and security gates are green. The exact `dae957a` full
matrix is hosted green in [run 31418476217 attempt
1](https://github.com/wavect/semaprax/actions/runs/31418476217/attempts/1),
including [Ubuntu job
93553147265](https://github.com/wavect/semaprax/actions/runs/31418476217/job/93553147265);
all 12 jobs passed. The breaking operation rebases Graph-v10 identity content
and may rebase identity-bearing CleanupPlan content, but widens no schema or
semantic shape and changes no backend/runtime semantics. General, typed-hole,
ranked/composed, repository-wide, and multi-file repair remains roadmap work.

Bounded Semantic Review v1 now classifies every admitted Patch v1/v2/v3
operation across exact `behavior`, `api_identity`, `security_authority`,
`memory_ownership`, `target_artifact`, `migration`, and `unsafe` wire sections.
V1/v2 embed complete nontruncated Impact v1 evidence; v3 embeds the shared
identity rebase without widening Impact. Local 10/10 integration, 4/4
hook/limit units, library 408/408, full preservation, and security gates are
green. The exact `2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is
hosted green in [run 31423743369 attempt
1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
including [Ubuntu job
93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170);
all 12 jobs passed. This advances deterministic review only:
general repository review, authenticated provenance, target execution, human
approval workflows, multi-file transactions, and the 0.2 CLI exit criterion
remain open.

Semantic Patch Evidence v1 advances only the bounded Proof-carrying patches
row from Missing to Partial. It freezes canonical capsules and independent
receipts for every currently admitted Patch v1/v2/v3 operation and gates a
separate A0 route on exact replay before stage preparation. Capsule KATs are
`03befad24157620b56138e84d4495b1973d141275ee728493d5fbe4f0f6f09aa`,
`23742f9b8a323003237106d7a800cc8fb98f53a68bd72f5e0961cf47c63f7bba`, and
`d682e08b125451af3ed49dce03a0814e83ca5e665224fc3bc7ab7b314827f62c`;
receipt KATs are
`1f2733743aaf2f9d2b9ad6bf2709a6867f169f596be01a9d53e92daecb8730a1`,
`6d8b13b3f54277e66a1ee501e1e71d6fe959a2ebcdbaa158a7ece20dde054e48`, and
`13a99674a4c014d9f7f315d8108c3e5c870dcac2c5950ff3035ca1a1c155361b`.
Local A+B 11/11 plus 5/5, Phase C 16/16 plus 11/11, library 420/420,
doctest 37/37, preservation, and security are green. The exact
`34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted green
in [run 31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
including [Ubuntu job 93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
all 12 jobs passed. The earlier `e04c2c9` run failed only the Rust 1.97 lint
and is not green evidence. General formal proofs, signatures/provenance, approval, target/test or
consumer-compatibility proof, repository/multi-file analysis, and comprehensive
claims/capability/target verification remain roadmap work.

Target Evidence v1 and Semantic Patch Evidence v2 are an additive bounded
projection/evidence tranche, not the multi-file architecture tranche and not a
0.2 exit-criterion claim. Their target report KATs are
`b00f9a0a6757e6da6f7cf32771172da02c93615e5fce697c71a24fbf28e5e011`,
`00c835772f189110c3cee85088ec34862c0f2cb0716ffe43b71675ec40fa22a2`, and
`fdcd73f2e40123434243c89ab5e3839d77d349305f5f984aca865ffd671aa6e1`;
Evidence-v2 KATs are frozen in
[`SEMANTIC-PATCH-EVIDENCE-V2.md`](SEMANTIC-PATCH-EVIDENCE-V2.md). Local
evidence is Target 9/9, target units 4/4, Evidence-v2 8/8, and library 439/439.
The exact `fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in
[run 31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793),
including [Ubuntu job
93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631);
all 12 jobs passed. The dashboard stays 38 Partial/18 Missing.

Semantic Workspace Transaction v1 now implements a bounded part of the
strategic multi-file architecture without replacing the broader tranche. Its
immutable managed generations plus one authenticated `ACTIVE` pivot give
cooperating readers old-or-new visibility for 2–16 existing canonical files.
Five revision/snapshot/preview KATs, integration 12/12, hostile 5/5, workspace
units 37/37, library 482/482, full local gates, preservation, and security are
green. The exact `afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted
green in [run
31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
including [Ubuntu job
93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613)
and [Windows job
93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611);
all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
strict Clippy and is not green evidence. The 0.2 exit criterion, modules/imports and a unified
multi-file Graph, repository Impact/Review/Context/Target/test integration,
general repairs/operations, raw-tree materialization, create/delete/move,
automatic recovery/GC, and power-loss durability remain roadmap work. Totals
stay 38 Partial/18 Missing.

Semantic Workspace Patch Evidence v1 is the additive proof-carrier tranche on
top of that managed publication model. It binds an exact Workspace Patch and
preview to sorted per-file Review and child Patch-Evidence-v1 facts, freezes
homogeneous v1/v2/v3 and mixed capsule/receipt KATs, and gates an opt-in apply
route on exact replay before candidate generation or staging. Local public
6/6, apply 5/5, hostile 2/2, units 8/8, shared Workspace 39/39, root library
496/496, and preservation 107/107 are green. The exact
`388986a6f12ef97b0c8b40e76466fdc83f211b39` matrix is hosted green in [run
31487851406](https://github.com/wavect/semaprax/actions/runs/31487851406), with
all 12 jobs passing. It grants no authority, performs no target/test or
cross-file semantic reasoning, and includes neither Target Evidence nor Evidence v2. The next
strategic architecture tranche remains a unified multi-file semantic Graph
with real module/import/type/call/capability/identity resolution and repository
Impact/Review/Context/Target/test consumers, followed by broader operations and
raw-tree/product integration. This additive milestone changes no status: totals
remain 38 Partial/18 Missing.

- Versioned language, graph, package, and component specifications.
- Reproducible native and component builds on supported targets.
- Stable debugger and profiler integration.
- Audited ownership and unsafe boundaries.
- Compatibility policy and migration tooling.
- At least one production validation system maintained across releases.
