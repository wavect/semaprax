# Compiler architecture

Status: living internal implementation and trust-boundary map.

Audience: compiler contributors and reviewers.

This document owns the current implementation map, data flow, and trust
boundaries. It does not own product status, protocol details, historical
changes, or test inventories:

- current status: [completion matrix](COMPLETION-MATRIX.md);
- exact protocols and ABIs: their versioned reference documents;
- required checks: [quality gates](QUALITY-GATES.md);
- history: [changelog](../CHANGELOG.md).

SEMAPRAX v0.2 is a set of bounded vertical slices through a larger language
design. The architecture keeps human source, verified meaning, agent
projections, mutation authority, and target execution distinct.

## System shape

```text
canonical .spx source or held Project inputs
                    |
              lexer + parser
                    |
                 AST
                    |
        resolver + semantic verifier
                    |
          validated stable-ID HIR
                    |
       canonical cleanup-plan builder
                    |
          independent plan replay
             /       |       \
 semantic graph   interpreter   target lowering
      |                         /            \
 context/impact/review      native C11    Wasm Core
      |                          |             |
 evidence + transactions      Clang       JS/Node host
```

No backend bypasses source verification or validated-HIR checks. Cleanup-plan
vectors are canonical execution order and must not be sorted or repaired by a
graph projection or backend.

## Representations

The registry compiler and unpublished full toolchain share one compiler library
and `src/cli_driver.rs`. The standalone binary supplies no private-host hooks.
`crates/semaprax-toolchain` owns the `new` and `doctor` implementations,
Project Native Rust package publication adapter, and safe Windows revision-store
host. No private crate is a normal or optional dependency of the registry
package. Compiler-owned SDK replay and Windows carrier preparation/replay remain
before/around explicit injected host calls; opaque prepared facts are not
filesystem authority. See [development](DEVELOPMENT.md) for binary selection.

The full toolchain's calculator generator checks exact owned template bytes
before staging through `NewProjectAuthority` in the lower package crate.
Publication latches before final held-byte and destination-path authentication;
the CLI additionally rechecks the original requested parent spelling. Failure
after that latch cannot regain cleanup authority. See [Calculator project
publication v1](NEW-PROJECT-PUBLICATION-V1.md) for the correction and unrun gates.

The full-toolchain CLI doctor routes fixed `--version` probes through the safe
`semaprax-native-rust-interop-platform` facade and the existing platform-sys
quarantine's separate `doctor/` module. That module owns bounded combined
output, deadline observation, and private Unix-group or Windows-job settlement;
it does not alter the authenticated build runner. The private CLI retains report
policy and UTF-8/version parsing, with no unsafe code. Trusted installed tools
remain outside a network sandbox; see the [doctor lifecycle
contract](DOCTOR-PROBE-V1.md).

### Canonical source

`src/lexer.rs`, `src/parser.rs`, and `src/ast.rs` parse human-readable source.
`src/format.rs` is the canonical source projection. Revision digests bind the
canonical bytes, not incidental whitespace.

Source is the canonical Git representation. A managed workspace publishes an
immutable generated source set for cooperating readers; it does not rewrite the
original files or grant atomic visibility to Git, editors, or arbitrary raw
path readers.

### Validated HIR

`src/verify.rs`, `src/source_verify.rs`, and `src/hir.rs` own checked meaning.
The `src/hir/` modules own validation, inspection indexes, declaration lookup,
and bounded Project linking.

HIR carries resolved identities and typed operations. A backend or report may
apply a stricter admission profile, but it may not reinterpret unresolved AST
or silently widen the verified program.

### Cleanup meaning

`src/cleanup.rs` inventories structurally owned leaves.
`src/cleanup_plan.rs` and `src/cleanup_plan/` own the target-neutral cleanup
control-flow schema, builder, validation, execution model, and independent
replay.

Current graph versions select the minimum schema needed by the admitted
feature, from legacy scalar/Result meaning through Option, aggregates,
generics, loops, byte data, command I/O, and owned-byte record/variant matching. The
owning feature specifications define exact schema numbers and preservation
requirements; architecture depends only on monotonic, deterministic selection.

[Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) is the
closed non-Copy sum slice. Cleanup Inventory v2 identifies owned leaves by
stable case and field identity; CleanupPlan v6 authenticates conditional entry,
selected-case transfer, and exact case liveness; Graph v22 projects those
facts. Interpreter, native, and Wasm lower only the active case field-by-field.
Invalid owned tags or tag/liveness disagreement fail-stop before payload
authority, cleanup, or result publication. Nested/generic owned variants,
non-Copy propagation, and public aggregate ABIs remain outside this boundary.

`src/loan_plan.rs` owns the additive plan schema, builder, and replay;
`src/graph_loan.rs` owns its Graph projection. The
[Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md) is a bounded, target-neutral proof
plan for synchronous immutable loans. It assigns dense
resolved-function-local loan identities, authenticates exact owner places and
parent reborrow provenance, and records path-sensitive last-use edges for
multiple loans. Validated HIR independently rebuilds and exactly replays the
plan before Graph v23/v24 may project it. Try propagation retains distinct normal
and residual-return CFG successors so later uses cannot extend a loan across
an early return. Semantic Workspace v1 rejects a nonempty loan plan combined
with an owned-variant Graph v22 base schema instead of masking that older
contract. The plan neither changes CleanupPlan
liveness nor creates runtime
references; legacy programs retain their prior Graph and cleanup bytes.
The additive authored-but-unrun
[Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md)
preserves one direct stable field-ID projection through byte-slice provenance,
aliases, ranges, and the same plan. Additive Graph v24 owns the new facts while
unprojected Graph v23 schema selection and serialized fields remain unchanged.
The interpreter, native C11, and
Core-Wasm lanes lower that exact profile.
General nesting, public borrowed ABIs, and hosted promotion remain outside this
architecture boundary.

### Semantic graph

`src/graph.rs` and `src/graph_cleanup.rs` project validated program and cleanup
meaning. `src/call_index.rs`, `src/impact.rs`, `src/review.rs`,
`src/properties.rs`, and `src/hygienic.rs` build bounded read-only views over
verified representations.

A graph, report, review, or evidence capsule is descriptive data. It is not a
capability, signature, approval, or commit token.

Target Evidence reports the pinned wasmparser 0.258.0 validator. Validator
version facts participate in report hashes and Evidence v2 replay; dependency
updates require capsule regeneration without granting target execution or
publication authority. See [Target Evidence v1](SEMANTIC-TARGET-EVIDENCE-V1.md).

## Compiler and execution lanes

### Interpreter

`src/interpreter.rs` evaluates an admitted verified-HIR profile with bounded
fuel and normalized runtime statuses. `src/hosted_interpreter.rs` adds the
bounded host-facing execution used by Project profiles. The interpreter is a
development and conformance lane, not a target backend or proof engine.

`src/interpreter/internal_strings.rs` owns the additive `interpret-strings`
facade and strict report boundary. A private profile selects internal String
callee admission and a distinct report schema/domain through the existing
source evaluator and renderer. External scalar/borrowed inputs and scalar
results stay unchanged; ordinary, Project, prepared, and effectful evaluators
retain their prior admission. This route adds no second execution engine or
target runtime. See [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md).

`src/interpreter/prepared.rs` owns the additive cached closure/index types,
cooperative cancellation seam, prepared evaluation entry, and expression-trace
traversal hook. The root interpreter retains the shared evaluator and only the
minimal crate-private reexports needed by the Project lane.

`src/project/prepared_interpreter/` adds an authority-neutral retained Project
lane over the same evaluator. It caches the exact admitted entry/test closure
indexes once, owns one sequential fixed-stack worker, observes monotonic
cooperative cancellation only at evaluator step boundaries, and emits bounded
`semaprax.project-source-trace.v1` expression origins. Plain replay proves the
closed canonical wire; revision-bound replay additionally matches every event
to retained HIR and authenticated source facts. It does not independently
re-execute the dynamic path and grants no debugger, target, I/O, build, or
publication authority.

Within the retained Project lane, `model.rs` owns the public prepared options,
outcomes, cancellation handle, and worker-slot model; `origin.rs` owns exact
entry/test source-origin indexing and duplicate-fact disagreement checks; and
`worker.rs` owns fail-fast execution admission, the bounded fixed-stack worker
lifecycle, evaluation/cancellation dispatch, and trace assembly.

`worker/replacement.rs` owns the additive expected-revision check and complete
prepared-state handoff. Execution and replacement share fail-fast admission;
the existing worker prepares both candidate closures and origin facts before
swapping them together with their immutable revision. Ordinary rejection
preserves the old state; a panic or lost replacement acknowledgement makes
the worker terminal. This adds no filesystem refresh or incremental compiler
cache. See [Prepared Project Revision Replacement v1](PROJECT-PREPARED-REVISION-REPLACEMENT-V1.md).

Within that lane, `trace/model.rs` owns the closed wire vocabulary, digest
domain, normalized status parsing, and trace data model; `trace/render.rs` owns
bounded prefix selection and canonical rendering; and `trace/verify.rs` owns
closed-wire replay plus revision-bound closure/source checks. This split and
its focused evidence are authored but unpromoted; the completion matrix remains
the sole status authority.

### Native bootstrap backend

`src/codegen.rs` owns native orchestration and admission. The
`src/codegen/native_*` modules own C11 emission, runtime statuses, aggregate
and byte-data lowering, command I/O, callable bundles, resource fixtures,
capability envelopes, conformance traces, and private host contracts.

The public executable lane emits C11 and invokes an explicitly admitted Clang.
Private callable and resource lanes are narrower host-integration evidence;
they do not establish a stable general native ABI.

`native_emit/owned_strings.rs` also owns the authored ordinary/stdout inline
String ledger. Only String-bearing functions stage bounded output for hoisted
owner cells; String-free functions retain direct emission. Ordinary String
helper discovery includes materialized generic instances as well as
monomorphic functions. The authored [owned-data provider correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md)
extends the same ledger, length-header runtime, and instance discovery to v8/v9
provider emission without widening public or Project admission. Full provider
translation units with Strings intentionally change; String-free output,
v10 selection and versioned command projections remain unchanged.
See [Native Inline String Settlement
v1](NATIVE-INLINE-STRING-SETTLEMENT-V1.md) for exact compatibility and open gaps.
The ordinary/stdout runtime selector also reuses the existing length-header
String helpers to preserve embedded NUL. Representation selection is separate
from v10 provider carrier support, so this does not add byte-carrier machinery
to ordinary String-only output. See [Native String Contents
v1](NATIVE-STRING-CONTENTS-V1.md).

### WebAssembly backend

`src/wasm.rs` and `src/wasm/` emit Core WebAssembly and generated host
carriers for admitted profiles. Scalar, selected aggregate, text, byte-data,
owned, and command-I/O paths remain separately admitted. The owned-byte variant
path uses active-case field moves and hard traps for malformed carriers; legacy
Copy-variant status behavior remains unchanged. The default product
is not a general WebAssembly Component Model runtime.

The separately selected `wasm::internal_strings` API authors a standalone
String-settling profile. `internal_strings/admission.rs` owns selection and
static work limits; `aggregate/internal_strings.rs` owns the new ten-import
module and checked mint lowering, reusing the private String owner cells and
common status epilogue. The existing aggregate entry points explicitly leave
that mode off. Generated modules pass structural validation before return.
`internal_strings/runtime/` separates exact input/artifact admission, bounded
UTF-8 arena ownership, and a scalar-only poisoned-on-uncertainty facade.
Capacity refusals run generated cleanup; unexpected traps do not promise
settlement or permit reuse. See [Standalone Wasm Internal String Settlement
v1](WASM-INTERNAL-STRINGS-V1.md). Evidence is authored and unrun; ordinary Wasm
imports, Project v1-v10 and Target Evidence do not select this profile.

`internal_strings/web.rs` and its small rendering/template modules expose that
profile through an explicit source-only Web build selector. The early CLI
branch bounds and authenticates the source snapshot before compilation and
rechecks it before reusing the existing fresh-output scalar Web publisher.
Its eight-file package preserves exact compiler/runtime outputs; a separate
manifest binds artifacts, while the local browser console authenticates the
descriptor before constructing scalar controls. Legacy `build_web` rejects
String-bearing ordinary and materialized generic functions before output
creation because its runtime lacks those imports. Raw emission and successful
String-free legacy packages stay unchanged. See [Standalone internal String Web
package v1](WASM-INTERNAL-STRINGS-WEB-V1.md); all new executable gates are unrun.

`src/wit_component.rs` and `src/wit_component/` provide default-off private
boundary evidence. They cannot be cited as public Component Model execution.

### Shared runtime status

`src/runtime_status.rs`, `src/semantic_trace.rs`, `src/conformance.rs`, and
`src/trace_path_certificate.rs` normalize failures and execution traces. The
first selected failure is sticky: cleanup cannot replace it, and result
publication occurs only after postconditions and non-result cleanup.

## Agent query and mutation architecture

### Single-file queries and changes

`src/agent_transport.rs` serves a bounded JSON-RPC loop over one checked
program. `src/patch.rs` owns the supported single-file transaction format and
A0 commit boundary. `src/repair.rs`, `src/impact.rs`, and `src/review.rs` are
read-only planners and projections.

`src/patch_evidence.rs` independently reconstructs supported evidence. The
evidence-gated apply route acquires ordinary A0 authority first, replays before
staging, and rechecks the unchanged source before commit. Ordinary `patch`
remains a separate legacy route.

### Managed workspace

`src/workspace.rs` owns immutable generations and the authenticated `ACTIVE`
pivot. `src/workspace_patch_evidence.rs` binds exact per-file child evidence
and replays it before candidate creation.

`src/semantic_workspace.rs`, `src/workspace_graph.rs`, and
`src/workspace_analysis.rs` own cross-file initialization, graph construction,
context, impact, and review. `src/semantic_workspace_change.rs` and its modules
own replacements-only evidence and publication. Operations and structural
change are separate, bounded derivation layers in
`src/semantic_workspace_operations.rs` and
`src/semantic_workspace_structural_change.rs`.

Only the live workspace invocation owns the final publication pivot. Evidence
capsules never carry reusable authority.

### Operational semantic images

`src/project/image.rs` derives an immutable, bounded Semantic Workspace
Image from one already admitted `Arc<ProjectRevision>`. It retains validated
HIR in memory and projects the complete Project graph plus existing typed
stable-ID and adjacency indexes. Canonical `.spx` remains the Git authority;
image bytes are optional caller-persisted data, never trusted serialized HIR.
Replay freshly derives and exact-compares the complete canonical image before
returning it. Digest-bound symbol, Context, and Impact queries grant no file,
cache, execution, or commit authority. Compiler package version binding is not
a binary fingerprint. This foundation and its regressions are authored, unrun;
incremental rechecking, general graph mutation, and persistent daemon caches
remain outside it. See [Semantic Workspace Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md).

### Project profile and daemon

`src/project/candidate/` owns immutable source-derived candidate overlays and
closed typed intentions. The engine mutates invocation-local ASTs; canonical
source then re-enters complete Project admission and a second source replay.
Candidates retain reviewable replacements/diffs and target projection facts,
never filesystem or publication authority. `src/project/image_facets.rs`
projects actual retained HIR into revision-bound paginated facets without
changing the original image wire. `src/image_transport.rs` exposes these
queries through a separately selected read-only protocol whose one method
catalog also generates discovery/schema/client material. Existing transports
keep their method sets. See [Candidates](PROJECT-CANDIDATES-V1.md),
[facets](SEMANTIC-IMAGE-FACETS-V1.md), and
[Image Agent Protocol](IMAGE-AGENT-PROTOCOL-V1.md).

The additive [candidate-only protocol](IMAGE-CANDIDATE-PROTOCOL-V2.md) selects
ephemeral candidate/draft registry authority at host startup; it cannot grant
source writes, runtime tests or builds. Responses are bounded and held source
inputs are reauthenticated before registry mutations become visible. Typed
[body-hole drafts](PROJECT-CANDIDATE-HOLES-V1.md) expose context while blocking
materialization until all holes are filled through full candidate admission.
[Ordered signature mapping](PROJECT-SIGNATURE-EVOLUTION-V1.md) stages every
original built-in Copy argument once, left-to-right, before retaining,
reordering or removing parameters. These additions are authored, unrun.

`src/project/manifest.rs` parses the bounded `semaprax.toml` profiles.
`src/project/` owns held input authority, immutable revisions, semantic
admission, linking, execution, builds, npm carriers, rename planning, and the
unpublished native Rust SDK bridge.

`src/project/admission/` is the sole exhaustive, authority-neutral Phase-A
profile dispatcher. It consumes the already linked entry HIR and exact retained
Project subject, runs the unchanged schema-selected target admission, and
retains v8/v9/v10 descriptors only as sealed compiler state. A prepared value
is neither evidence nor effect authority, and later public consumers still
replay descriptor bytes against retained HIR. This closes the ordinary v9
Project route without modifying an earlier profile or target schema. See
[Project Profile Admission v1](PROJECT-PROFILE-ADMISSION-V1.md).

Project v8 adds one closed `owned-data-api.v1` route. `src/project/public_api.rs`
derives and independently replays the sole semantic API descriptor from the
authenticated linked-HIR subject. `src/project/npm/owned_data.rs` and the
owned-data Wasm lowering consume that descriptor for the npm package. The
shared `src/project/npm/owned_data_input_v8.js` admits complete input tuples
before snapshot allocation, using captured brand/buffer intrinsics. Its
historical filename and helper bytes remain unchanged; v9/v10 explicitly
select the same admission without altering v8 JavaScript for that extension.
The subsequent shared [owned npm invocation state](OWNED-NPM-INVOCATION-V1.md)
is composed by `src/project/npm/owned_invocation.rs` from small private arena,
instantiation, invocation, result and facade templates under
`src/project/npm/owned_invocation/`. It reserves busy before preflight,
authenticates recoverable semantic failures by
exact local error identity, and makes post-entry uncertainty and caught reentry
absorbing poison. Arena imports and result publication observe that same state;
guarded settlement/scratch cleanup preserves the first thrown value, including
falsy values. This correction changes v8/v9/v10 runtime JavaScript and integrity
bindings, not descriptors, Wasm or host signatures. Its regressions are unrun. The
private `src/wasm/aggregate/owned_stack.rs` derives selected call-path frame
extents from the shared HIR call index and actual lowering plans so raw outputs
cannot overlap deeper helper frames. Native owned handles pair all 4,096 slots
with nonreused atomic issuance serials within one linked provider runtime;
contexts remain thread-confined. These corrections have authored, unrun
evidence and do not promote any profile. The
reference-interpreter entry in `src/interpreter.rs` returns a normalized
scalar/owned/variant value and one explicit copy-out-and-settle boundary event;
it does not grant target or publication authority.

Immutable Project revisions and snapshots expose separate read-only descriptor
accessors for the v8 owned-data, v9 flat-owned-record, and v10 owned-UTF8
profiles. Each accessor replays the sealed Phase-A descriptor against the
retained linked HIR and exact Project subject; Transport v5 continues to call
only the v8-specific accessor and is not widened. CLI success labels distinguish
the newly reachable v9 npm and Rust products without changing target authority
or promotion state.

The Rust target deliberately crosses a dependency-inverted trust boundary:

```text
held Project v8 snapshot + validated linked HIR
                    |
       canonical descriptor + semantic replay
                    |
       root-owned native provider C emission
                    |
  semaprax-native-rust-owned-data-package
       | held compiler/archive tools
       | deterministic safe/FFI package rendering
       | no-clobber staged publication + held-stage verification
                    |
       unpublished compiler-free Rust package
```

The root crate retains semantic authority and contains no new unsafe Rust. The
lower package crate receives only the already replayed descriptor, selected
stable IDs, provider bytes and their digests. It independently parses the
closed descriptor, checks provider integrity and the compiler-declared textual
descriptor binding, selects only the exact current host target, and owns
external tool and filesystem effects. It has no HIR or code-generation
authority and therefore does not independently authenticate provider semantics;
the root compiler owns that replay-equal proof. Generated safe code forbids
unsafe code; the private generated FFI sibling remains the quarantine for
opaque provider handles. Neither layer transfers provider allocation into a
host allocator.

The generated owned-data Rust invocation guard proves whole-context settlement
through the existing context-close ABI before any outward value or recoverable
error. Inner owner guards settle before context closure, including on unwind.
Only a proven-closed context may be reinitialized on the next invocation;
uncertain settlement is fail-stop. This resets a private invocation counter,
not the linked provider's nonreused handle serial. Unknown tags or forged
inactive handles grant no payload-operation authority. The correction changes
generated safe/private Rust and integrity bindings, not provider C/ABI or public
types, and does not establish safety against arbitrary hostile native code.
Initialization rejection retains its existing error with no further provider
operation in that invocation or its cleanup. A later explicit invocation may
attempt initialization again; only success creates the close obligation.

`examples/frame-payload-project`, `examples/frame-payload-web`,
`examples/frame-payload-rust`, and `tests/frame_payload_product_v1.rs` form one
authored validation product over an identical corpus. Its lanes cover the
reference interpreter, native C11 O0/O2, Core Wasm/Node, generated npm, and
generated Rust package, including stable-ID display rename and settlement
facts. The external Rust fixture has a committed standalone lock and uses
`--locked --offline`; strict TypeScript 5.8.3 and Chromium/Playwright 1.62.0
fixtures require explicit local provisioning and download nothing. Node and
browser entry points share one corpus runner. The browser gate consumes
host-provisioned before/after artifacts rather than authenticating their source
derivation; the Project test owns that rename proof. These gates are authored
but unrun and do not establish exact-head hosted promotion. Gate selection is
documented in the [web consumer](../examples/frame-payload-web/README.md) and
[browser fixture](../platform-tests/frame-payload-browser-v1/README.md).

`src/project/flat_owned_record.rs` is the authority-free additive Project-v9
description layer. It authenticates the exact one-direct-`Bytes` flat record
result shape from HIR, independently replays its versioned descriptor, and
projects safe TypeScript/Rust types plus an opaque-handle settlement plan.
`src/project/npm/flat_owned_record.rs`, the aggregate-aware owned-data Wasm
adapter, and the root native provider wire that descriptor to the additive npm
and safe-Rust package routes. The lower unpublished package crate replays the
descriptor, provider integrity binding, held tools, and publication facts,
then verifies the renamed stage through its retained stage authority. It does
not authenticate provider semantics; root HIR/codegen replay alone owns that
proof. No route exposes a target aggregate layout. See [Public Flat Owned
Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md).

The compiler-private `src/project/npm/semantic_recipe_v8.rs` is shared by
owned-data npm and Project Rust replay. Its collision-only type-name projection
retains original presentation facts, then reconstructs the resolved declaration
index through the existing owned-data linker before descriptor or target replay.
It does not grant publication authority or replace the descriptor's identity
contract. Source identity literals use the source formatter, independently of
the canonical JSON descriptor writer.

Project v10 adds the distinct `owned-utf8-api.v1` descriptor without treating
text as raw `Bytes`. The root Wasm and native adapters retain the exact byte
length, validate UTF-8 before host publication, and preserve the opaque
provider-handle settlement boundary. npm consumes the carrier before fatal
decoding to a JavaScript string. The dependency-inverted lower Rust package
replays the v10 digest domain and provider-integrity binding, owns held tools
and publication, verifies the renamed stage through retained stage authority,
copies and settles the handle, and only then constructs a safe `String`;
invalid UTF-8 therefore cannot escape or retain provider ownership. It receives
no HIR and cannot independently prove provider semantics; root HIR/codegen
replay alone owns that proof. V8/v9 renderer and carrier identities remain
separate. See [Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md).

For v10 Wasm, `src/wasm/aggregate/owned_strings.rs` owns inline String cell
settlement derived from validated HIR and exact emitter locals, separately
from resource CleanupPlan liveness. Place reads clone; temporary moves clear;
scope and failure exits sweep nonescaping owners. `owned_stack.rs` derives a
checked selected-call-path arena bound from these cells plus Bytes cleanup
leaves. For the v10 native provider only,
`src/codegen/native_emit/owned_strings.rs` owns a separate per-function physical
String ledger. Bounded staged emission hoists initialized owner cells before
failure branches; expression lowering moves ownership at binding, branch,
call, and result boundaries. Normal scope cleanup and the common epilogue
settle those cells without reinterpreting resource CleanupPlan liveness.
Neither ledger confers provider-handle or publication authority. The separate
ordinary/stdout correction reuses the native ledger without changing v10
provider bytes. Frozen earlier provider and command profiles retain their
existing unselected-String cleanup limitation; Wasm accounting and
context-handle closure alone do not prove native allocation settlement.

`src/project_revision_store.rs` and `src/project_revision_store/unix.rs` own
the additive [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md). The
authority-neutral layer independently binds canonical manifest/source facts,
Workspace and Project revisions, and the Project graph digest into one closed
content-addressed entry. The Unix authority layer opens one host-injected
absolute root component-by-component, requires current-euid ownership and
exact `0700` mode, and treats the advisory lock only as cooperating-caller
serialization under an explicit host guarantee excluding uncooperative
same-principal mutation. It performs all reads and effects relative to held
descriptors, retains every created parent authority, publishes a completely
replayed create-new stage with a same-root no-replace rename, and settles held
directories before and after the pivot. Root admission authenticates and
caches unrelated content-addressed metadata and structural/identity facts once
per invocation; later checks revalidate names and top identities without
rereading metadata. Selecting an entry or
publishing a new one additionally owns all bytes and rebuilds meaning through
the ordinary Project Phase-A/HIR path; stored bytes never bypass verification.
For read availability, one exact inert stage-shaped top identity may be cached
and rechecked without being opened or traversed; persistence still rejects all
residue. A pure locator exposes the deterministic subject digest so callers can
resolve post-pivot ambiguity only through ordinary full load replay. Neither
surface adopts, deletes, repairs, or authorizes the stage. Darwin callers also
own the explicit host precondition excluding ancestor and ACL-granted mutation
authority that owner/mode checks cannot prove.
The separately selected [Windows-entry-v1 route](PROJECT-REVISION-STORE-WINDOWS-V1.md)
uses additive APIs and a distinct schema/domain; ordinary v1 stays Unix-only
with unchanged bytes. Its Windows orchestration and unpublished
`semaprax-project-revision-store-windows-sys` quarantine accept only drive-absolute fixed local NTFS,
opens every component relative to retained handles, authenticates exact token
SID/protected-DACL/identity/link/stream/short-name facts, serializes by a
validated root-identity mutex, and performs one no-replace non-POSIX
handle-relative rename. Unsafe Windows calls remain quarantined behind opaque
safe handles and facts; raw handles and unsafe FFI do not enter the compiler crate. Other
hosts fail before an entry effect. No store handle, receipt, daemon integration, build authority,
cleanup, recovery, eviction, or garbage collection is exposed.

`src/project_transport/` and `src/bin/semapraxd.rs` retain one authenticated
Project revision for bounded requests. Read-only v2 is the default. Explicit
opt-ins add one server-derived rename, the bounded workflow, or the additive
read-only Project v8 descriptor/npm carrier surface. Transport v5 compares the
carrier's independently replayed typed descriptor binding with the retained
canonical descriptor before returning either. These profiles do not add
general patch, filesystem, process, publication, network, persistence, or
recovery authority.

## Reports and projections

Read-only commands are implemented in focused modules such as
`src/abi_report.rs`, `src/c_header.rs`, `src/cxx_shim.rs`,
`src/capability_manifest.rs`, `src/freestanding_object.rs`, `src/openapi.rs`,
`src/package_report.rs`, `src/plugin_manifest.rs`, `src/region_report.rs`,
`src/simd_report.rs`, and `src/ui_schema.rs`.

`src/package_lock.rs` is an authority-free additive offline graph layer above
the Interface Package Report. It accepts only explicit already-owned subject
envelopes, independently replays each exact report, rejects coordinate and
graph confusion, derives deterministic dependency-first order, exact target
intersection, and transitive declared-capability closure, then emits an
independently replayable lock to memory/stdout. `src/cli/package_lock.rs`
retains each explicitly
named input handle, rejects duplicate held file identities, and reads it once;
neither layer discovers paths, resolves or fetches versions, runs scripts,
compiles targets, publishes files, or treats optional license/provenance claims
as signed or trusted facts. See [Offline Package Lock v1](OFFLINE-PACKAGE-LOCK-V1.md).

`src/package_report_v2.rs` and `src/package_report_v2/` define the additive
self-contained Semantic Package Report v2. Its verifier rebuilds the exact
report from embedded canonical source through the ordinary verifier and
validated HIR. Stable-ID type/ownership/effect/contract facts, reachable
nominal closure, and closed ternary target proofs are read-only evidence; the
surface performs no compatibility decision and grants no package authority.
See [Semantic Package Report v2](PACKAGE-REPORT-V2.md).

Additive `package_lock_v2` binds exact source-replayed V2 reports into a
bounded offline graph. `package_compatibility` emits stable-ID-only comparison
evidence; unknown closure or lock-context drift is indeterminate. See
[Lock v2](OFFLINE-SEMANTIC-PACKAGE-LOCK-V2.md) and
[Compatibility Evidence v1](OFFLINE-PACKAGE-COMPATIBILITY-EVIDENCE-V1.md).

Additive `package_resolver` is an authority-free deterministic selector over a
finite caller-owned catalog of those exact source-replayed V2 subjects. It
normalizes strict semantic versions and the three frozen range forms, applies
target and declared-capability admission, explores one transactionally bounded
DFS trace, and emits exact replay evidence containing one unchanged Lock-v2
result. The focused public evidence is authored but unrun. This layer performs
no discovery, registry or network access, fetch, build-script or target
execution, cache, publication, signature/provenance authentication, or runtime
capability enforcement. See
[Offline Deterministic Package Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md).

Additive `package_resolution_snapshot` packages exact caller-owned Resolver-v1
input, unchanged resolution evidence, and the returned unchanged Lock-v2 into
three independently replayable byte strings. Raw Subject-v2 envelopes remain
embedded byte-for-byte and are never JSON-re-rendered. The pure layer has no
filesystem authority. The lower `semaprax-offline-wasm-package` crate exposes
only one fixed three-file create-new publication facade through its existing
held/no-replace authority state machine; an internal sealed inventory preserves
the build-v1/v2 names, order, bytes, and failure selection. Evidence is authored
but unrun and the surface is unpromoted. See [Offline Published Semantic Lock
Snapshot v1](OFFLINE-PUBLISHED-SEMANTIC-LOCK-SNAPSHOT-V1.md).

Additive `package_lock_v3` authenticates package dependency ranges in new
Subject-v3 envelopes and binds each range to the selected coordinate in a new
dependency-first Lock-v3 graph. `package_resolver_v2` intersects those root
and transitive ranges during deterministic bounded search and exactly replays
Lock v3. These modules are authored but locally unrun. They do not widen the
v1/v2 subjects, locks, resolver, CLI, capsule, build, compatibility, or
publication surfaces and gain no registry, network, acquisition, cache, build,
execution, or publication authority. See [Lock v3](OFFLINE-SEMANTIC-PACKAGE-LOCK-V3.md)
and [Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md).

Additive `package_source_capsule` consumes exact Resolver-v1 replay and two
through four caller-owned canonical implementation sources. The ordinary
semantic-workspace graph derives function imports over synthetic logical paths,
exact-compares that direct module graph with the selected Subject-v2 graph,
and exact-compares normalized scalar interface vectors with selected Report-v2
facts before using a package-only variant of the existing authority-free scalar
HIR linker. It retains every explicit root export and its transitive callees,
uses the byte-lowest root-owned `fn() -> i64` only as the HIR anchor, and leaves
the Project linker's authored-`main` rule unchanged. Report source is interface
evidence only; capsule source is the sole executable code. The explicit
selected root and only its sorted explicit export IDs are bound in the capsule,
while a crate-private replay seam retains linked HIR for the separate
linked-build consumer. The authored surface is unrun and adds no build or
publication authority. See [Offline Multi-Package Source Capsule
v1](OFFLINE-MULTI-PACKAGE-SOURCE-CAPSULE-V1.md).

Additive `package_build` consumes that exact resolver evidence only through an
independent replay route. The v1 profile deliberately admits one selected,
dependency-free Subject v2 whose embedded canonical source is rebuilt through
Report v2 and the ordinary verifier/HIR path. It then reuses the unchanged
Public Scalar Export Profile v1 to emit one structurally validated Core-Wasm
module, a canonical manifest, and independently replayable evidence. The
module's seven fixed `env` function imports are recorded as runtime semantic
dependencies; they are not SEMAPRAX capability declarations. Submitted
manifest/evidence facts never become reconstruction inputs. The pure compiler
layer has no filesystem, process, registry, network, publication, runtime, or
sandbox authority. See [Offline Effect-Free Scalar Core-Wasm Package Build
v1](OFFLINE-PURE-WASM-PACKAGE-BUILD-V1.md).

Additive `package_build_v2` consumes only the capsule's independently replayed
private receipt and retained linked HIR. It emits a distinct canonical v2
manifest/evidence pair around the unchanged effect-free scalar Core-Wasm
emitter, binds the complete selected package closure plus capsule, source-set,
link, and root-export facts, and revalidates the exact seven-import/export
inventory. Its two-package, cross-pair, mutation, bound, fixed-point, and
publisher evidence is authored but unrun. It adds no source reconstruction,
external tool, registry, runtime, or publication authority. See [Offline
Linked Scalar Core-Wasm Package Build v2](OFFLINE-LINKED-SCALAR-WASM-PACKAGE-BUILD-V2.md).

The separate `semaprax-offline-wasm-package` crate is the only publication
boundary for both build profiles. Its safe facade replays the complete caller-owned
build before acquiring a held destination, stages exactly three create-new
files, exact-compares held staged bytes before settle, and performs one
no-replace directory publication. Previsibility cleanup is limited to the
authenticated stage inventory; publication uncertainty is fail-stop. This is
create-new local publication, not acquisition, a registry/cache, provenance,
runtime enforcement, or a hermetic operating-system build sandbox. Its
authored evidence is unrun and the crate is unpromoted. Every platform has the
explicit host precondition excluding every uncooperative mutation of the
destination path, parent, ancestors, or stage for the invocation. Unix/macOS
additionally requires and checks a current-euid-owned exact-mode-0700 parent;
the precondition includes Darwin ACL-granted authority because POSIX directory
creation cannot atomically return the created directory handle.

These modules must:

- consume verified representations;
- use closed admission and exclusion vocabularies;
- emit deterministic bounded output;
- independently replay digest-authenticated envelopes where specified;
- make target execution and unsupported surfaces explicit non-claims.

A report can deepen a completion row from Missing to Partial. It cannot prove
the runtime or ecosystem feature it describes.

## Private host and proof boundaries

The following areas are deliberately quarantined from the public compiler
contract:

- `crates/semaprax-native-loader`: unsafe dynamic-loader boundary;
- `crates/semaprax-native-host`: connected callable and settlement host;
- `crates/semaprax-native-rust-interop-*`: unpublished deterministic Rust SDK
  builder and platform-specific publication authority;
- `crates/semaprax-native-rust-owned-data-package`: dependency-inverted
  Project-v8 held-tool, archive, deterministic rendering, publication, and
  held-stage verification authority; it receives no source, HIR, or provider
  semantic-authentication authority;
- `src/native_settlement.rs`, `src/arc_zones.rs`, and `src/scoped_tasks.rs`:
  target-neutral proof models rather than wired runtime features;
- `src/agent_runtime.rs` and `src/economic_agent.rs`: injected-host Rust APIs
  with no built-in provider transport, keys, wallet, or ambient authority;
- `platform-tests/`: installed application and runtime fixtures whose claims
  count only when the owning hosted jobs are green.

Private or proof-only evidence may validate a design boundary without creating
a supported language, CLI, ABI, or runtime surface.

## Trust boundaries and invariants

1. Safe source must have equivalent checked behavior on every backend that
   claims to implement the admitted feature.
2. Evaluation order is left to right; lazy boolean operands execute only when
   required.
3. Public declaration IDs persist; expression IDs may be revision-scoped.
4. Source formatting, graph JSON, reports, diagnostics, patches, and generated
   artifacts covered by a contract are deterministic.
5. Failed or stale transactions leave authoritative source or the active
   generation unchanged.
6. Capabilities are explicit. Compiler and generated code gain no ambient
   filesystem, process, network, home, or secret authority.
7. Ownership errors are compile-time diagnostics, never backend accidents.
8. Owned calls stage arguments left to right and transfer them together at the
   declared commit boundary.
9. Proof data never authorizes a physical finalizer, build, or publication.
10. No feature is complete without the completion matrix's executable gate.

## Repository map

| Area | Primary owners |
| --- | --- |
| Source projection | `src/ast.rs`, `src/lexer.rs`, `src/parser.rs`, `src/format.rs` |
| Verification and HIR | `src/verify.rs`, `src/source_verify.rs`, `src/hir.rs`, `src/hir/` |
| Cleanup and layouts | `src/cleanup.rs`, `src/cleanup_plan.rs`, `src/cleanup_plan/`, `src/aggregate_layout.rs`, `src/variant_layout.rs` |
| Graph and read-only analysis | `src/graph.rs`, `src/graph_cleanup.rs`, `src/call_index.rs`, `src/impact.rs`, `src/review.rs` |
| Single-file transactions | `src/patch.rs`, `src/patch/`, `src/patch_evidence.rs`, `src/repair.rs` |
| Managed workspace | `src/workspace.rs`, `src/workspace_*`, `src/semantic_workspace*` |
| Project, public descriptor, and daemon | `src/project/`, `src/project/public_api.rs`, `src/project_transport/`, `src/bin/semapraxd.rs` |
| Immutable Project revision inputs | `src/project_revision_store.rs`, `src/project_revision_store/unix.rs` |
| Generated Rust package authority | `src/project/native_sdk.rs`, `crates/semaprax-native-rust-owned-data-package/`, `crates/semaprax-native-rust-interop-builder/` |
| Interpreter | `src/interpreter.rs`, `src/interpreter/prepared.rs`, `src/hosted_interpreter.rs`, `src/project/prepared_interpreter/`, `src/project/prepared_interpreter/trace/` |
| Native backend | `src/codegen.rs`, `src/codegen/native_*` |
| WebAssembly backend | `src/wasm.rs`, `src/wasm/` |
| Reports and offline package graph | the focused `*_report`, `package_lock`, `package_resolver`, `package_resolution_snapshot`, schema, manifest, header, and shim modules |
| Effect-free package build and fixed-inventory publication | `src/package_build.rs`, `src/package_build/`, `src/package_build_v2.rs`, `src/package_build_v2/`, `crates/semaprax-offline-wasm-package/` |
| Private host/runtime evidence | `crates/semaprax-native-*`, `platform-tests/` |
| Executable evidence | `tests/`, crate-local tests, `platform-tests/`, `.github/workflows/` |

This table is the single module-level map. Other contributor documents should
link here instead of copying it.
