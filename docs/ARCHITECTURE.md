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
and `src/cli_driver.rs`; `src/cli_driver/report_options.rs` owns the bounded
option parsers for report and analysis commands while the root retains dispatch.
The standalone binary supplies no private-host hooks.
`crates/semaprax-toolchain` owns the `doctor` implementation and the held-parent
staged publication behind its `new`; the compiler library owns the bounded
standalone `new` route (`src/project/create.rs`), and both share one grammar and
one scaffold. The toolchain also owns the
Project Native Rust package publication adapter, and safe Windows revision-store
host. No private crate is a normal or optional dependency of the registry
package. Compiler-owned SDK replay and Windows carrier preparation/replay remain
before/around explicit injected host calls; opaque prepared facts are not
filesystem authority. See [development](DEVELOPMENT.md) for binary selection.

For Windows Project v8–v10 npm/Web publication, the compiler's opaque
`ProjectNpmPublication` retains preparation and replay under the live snapshot.
The private toolchain's `owned_npm` module alone performs held-handle six-file
publication; the CLI dispatches before legacy output-parent creation. Standalone
publication rejects safely. Cleanup authority ends before settlement/rename;
post-publication byte, inventory and path rechecks cannot regain rollback.
See [Windows owned npm publication](WINDOWS-OWNED-NPM-PUBLICATION-V1.md) for
admission restrictions and the unrun physical gates.

The full toolchain's calculator and library generators check exact owned
template bytes before staging through `NewProjectAuthority` in the lower
package crate. The template choice fixes either the two-file calculator or
three-file library `src` inventory before namespace creation.
Staging selection excludes exact/ASCII-case-equivalent destination names before
creation; the lower authority independently rejects the same collision.
Publication latches before final held-byte and destination-path authentication;
the CLI additionally rechecks the original requested parent spelling. Failure
after that latch cannot regain cleanup authority. See [Calculator project
publication v1](NEW-PROJECT-PUBLICATION-V1.md) for the correction and unrun gates.

`src/project/scaffold.rs` separately derives and independently replays the same
five calculator files, including the in-project `AGENTS.md` guide, as a bounded
`semaprax.project-scaffold.v2` artifact.
The standalone CLI prints those canonical bytes directly to stdout. The
artifact owns no path, file handle, staging namespace, process, or publication
authority; materialization remains entirely caller-owned. See [Public Project
Scaffold Capsule v2](PROJECT-SCAFFOLD-V2.md).

The toolchain library's shared doctor module owns strict bounded `--profile` selection and one
scoped offline-profile admission per report. Missing/unavailable profiles fail
required checks without ambient discovery or tool execution; returned selector
and platform facts are checked before tool callbacks. No production admission
backend is implemented, so the real CLI currently reports unavailable rather
than invoking installed tools. Reports describe selected tools, not ambient
build readiness. No root unsafe code or dependency is added.

The separate safe `DoctorOfflineInput` facade delegates only borrowed-file
sealed-memory acquisition to the existing sys quarantine. Native64 Linux checks
seals before metadata or positional reads and never duplicates/closes the
caller's descriptor. It returns bounded immutable untrusted bytes, not profile
or execution authority; unsupported hosts reject. This input primitive is not
connected to the CLI. See [Doctor sealed input v1](DOCTOR-SEALED-INPUT-V1.md).
The adjacent `offline_input/create` component creates only anonymous
non-executable sealed transport files from explicit bytes, then reuses that
acquisition path before transferring the file and snapshot together. Its Linux
adapter owns bounded writes, seal/property checks and one-shot failure cleanup;
the safe facade only delegates. It performs no path discovery or worker launch.
The explicit executable-image factory shares this ownership/write/acquisition
engine under a private fixed storage policy, with native ELF preflight, checked
owner-only execute mode and mandatory executable sealing. It creates storage,
not an approved image or profile admission; the ordinary input route stays
non-executable and the launcher independently validates its inherited files.
The sys quarantine's unsafe-free `DoctorOfflineBundle` parser consumes that
snapshot into a closed selector/architecture-bound inventory with zero-copy file
views, bounded paths/roles and minimum ELF/interpreter checks. The safe facade
delegates to this single validator; no public constructor bypasses admission.
It owns no OS effects and does not establish full ELF validity, library closure
or execution authority. See [Doctor offline bundle v1](DOCTOR-OFFLINE-BUNDLE-V1.md).
Its `encode` module prepares canonical bytes from explicit borrowed entries and
role indices, with size preflight and replay through that same full validator.
The `request` module derives exact worker-request binding from the retained
opaque bundle. Both preparation routes are unsafe-free and return only bytes;
they do not mint sealed-input or settled-observation authority or discover files.
The separate private `doctor/offline_root` component preallocates a bounded plan
from that opaque bundle and materializes a detached read-only tmpfs inside an
already controlled child context. It owns only newly created descriptors and
does not perform namespace bootstrap, inherited-handle cleanup, root entry or
tool execution. The general-host launch route remains deliberately unwired;
inherited descriptor closure can itself dispatch foreign filesystem flushes.
See [Doctor offline root materialization v1](DOCTOR-OFFLINE-ROOT-V1.md).

The private `doctor/offline_worker` component connects sealed request/bundle
validation (`wire.rs`) to fresh pidfd-owned PID namespaces and bounded capture
(`capture.rs`), controlled root/capability preparation (`child.rs`) and a
default-deny syscall policy (`guard.rs`). The separately invoked
`semaprax-doctor-worker` binary lives in the existing sys quarantine and invokes
only its unsafe dedicated-process entry; no safe embedding facade exposes that
operation. It requires an externally provisioned clean process,
private user/mount namespaces and aggregate resource/lifecycle ownership.
There is no ambient worker discovery, installer or CLI activation. Its reply
binds observations to request bytes, not executable provenance or admitted
version policy. Linux execution and hostile fixtures are authored, unrun;
macOS/Windows require separate native implementations. See
[Provisioned offline doctor worker v1](DOCTOR-OFFLINE-WORKER-V1.md).
Capture's private native-operations adapter owns OS effects, while native and
scripted test operations share the observation/settlement control flow. Test
scripts cannot create process authority. External lifecycle fixtures inspect a
stopped supervisor's pinned child; they do not expose procfs to the tool or
change the production syscall filter.

The separate `doctor/offline_collector` sys component consumes a provisioner-owned
live worker handoff. It alone constructs opaque settled observations after exact
pidfd exit/reap, bounded capture, request/bundle/reply binding and owned-handle
closure. The safe facade reexports only immutable observation types. A small
unpublished `semaprax-doctor-collector` entry crate bridges that unsafe process
boundary to the toolchain library's shared doctor report policy, avoiding a
dependency cycle or an unsafe ordinary CLI. Tool rows stay keyed by role even
when their bundled paths alias. This provisioned entry does not activate ordinary
CLI worker discovery. See [Provisioned offline doctor collector v1](DOCTOR-OFFLINE-COLLECTOR-V1.md).
The adjacent `doctor/offline_launcher` component owns seal-first preflight of
both inputs and approved executable images, high descriptor preparation,
exclusive worker-pipe construction, pidfd-owned cloning and collector exec.
It shares the existing minimum ELF validator and never reopens image paths.
Its separate private binary consumes an already provisioned process, preserving
parenthood while transferring the collector's fixed inventory; it neither
bootstraps namespaces nor authenticates image/loader provenance. See
[Provisioned offline doctor launcher v1](DOCTOR-OFFLINE-LAUNCHER-V1.md).
The outer `doctor/offline_provisioner` sys component now supplies the first
bounded Linux bootstrap around that launcher. Its dedicated private binary
admits one signed release capsule and exact sealed request/bundle/static-image
inventory, creates private user/mount/network/IPC/UTS namespaces, places the
child atomically into one authenticated delegated cgroup-v2 scope, and retains
pidfd plus cgroup authority until reap and `populated 0`. The worker selects one
closed default-deny syscall table per authenticated tool role. This boundary
does not activate ordinary CLI profile discovery, prove hostile binfmt/kernel
policy absent, package a signed production profile, or supply macOS/Windows
confinement. See [Linux production offline doctor provisioner v1](DOCTOR-PRODUCTION-PROVISIONER-V1.md).
The separate `semaprax-doctor-release::install` library boundary owns a
Unix-only, explicitly selected signed-generation store. It retains the exact
private root authority for the store object's lifetime, replays signed release
bytes through held files, publishes immutable generations without adoption,
and changes the fixed `ACTIVE` record only under a cooperative lock and an
expected-current check. Recovery authenticates the complete held inert-stage
inventory before its first effect. This layer does not execute a generation,
activate the ordinary CLI, provide a kernel compare-and-swap against hostile
same-principal writers, or add Windows support. See [Signed doctor generation
install v1](DOCTOR-SIGNED-INSTALL-V1.md).
The collector's lifetime state owns authentication, the irreversible reap transition and
fixed handle closure; only the native adapter performs pidfd and descriptor
operations. The separate report-delivery module owns bounded writes and final
standard-pipe closure after collection. Resource-free scripts exercise the same
state transitions without constructing observations or process authority.

The retained safe `semaprax-native-rust-interop-platform` facade and platform-sys
quarantine's separate `doctor/` module are no longer connected to that CLI route.
This lower-level fixed `--version` probe owns bounded combined
output, deadline observation, and private Unix-group or Windows-job settlement;
it does not alter the authenticated build runner. The private CLI retains report
policy and UTF-8/version parsing, with no unsafe code. Native64 little-endian
Linux x86-64/AArch64 probes additionally install an inherited no-new-privileges
and seccomp syscall-denial layer before exec; unsupported Linux ABIs and setup
failures cannot fall back to unfiltered execution. Its argument-filtered Unix
stream/seqpacket pairs preserve the Rust fork/exec handshake; datagram pairs,
socket creation and named connection/listen/accept operations remain denied.
Its trusted installed tools still
lack complete no-network isolation, including discovery, filesystem and broker
paths; macOS/Windows isolation is unchanged. See the [doctor lifecycle
contract](DOCTOR-PROBE-V1.md).

### Canonical source

`src/lexer.rs`, `src/parser.rs`, and `src/ast.rs` parse human-readable source.
The parser separates recursive prefix atoms from postfix construction so
inactive postfix temporaries do not accumulate during nested atom parsing.
Token order, contextual grammar, spans and diagnostics are unchanged.
`src/parser/patterns.rs` holds match-pattern parsing and `src/parser/types.rs`
type syntax, both moved verbatim, and `src/parser/hints.rs` attaches fix hints to rejections of habits from other
languages (`return`, `for`, `else if`, expression statements, compound
assignment, tuples, indexing, `Some(x)` patterns, `struct`/`enum`/`pub`/`const`
declarations, missing trailing commas, missing or unit result types, valueless
blocks) without admitting any new syntax or changing a stable code.
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

The authored [Acyclic Nested Owned-Byte Records
v1](NESTED-OWNED-BYTE-RECORDS-V1.md) tranche extends only the internal
monomorphic record tree. Source and HIR independently derive bounded complete
stable-field-ID paths; CleanupPlan v7 owns recursive construction, whole moves,
call commits and reverse cleanup; Graph v26/v27 distinguish nested ownership
from nested projected loans. Interpreter, native C11 and Core Wasm consume that
validated meaning without shallow owner copies. Project, FFI, package and
Component boundaries remain closed, and the tranche is not promoted until its
three-engine and hostile-plan gate executes.

The additive [Acyclic Nested Owned-Record Exact Destructuring
v1](NESTED-OWNED-RECORD-DESTRUCTURING-V1.md) consumes the same bounded record
shape and stable paths for recursive `match own` and `match borrow` only.
CleanupPlan v8 authenticates the exact recursive pattern and atomic whole-owner
transition; Graph v28/v29 distinguish destructuring without and with an
authenticated nested projected loan. The interpreter, native C11 and Core Wasm
remain fieldwise, while variants, mutation and every public ABI stay closed.

The successor [Acyclic Nested Owned-Record Immutable Update
v1](NESTED-OWNED-RECORD-UPDATE-V1.md) admits only top-level immutable
reconstruction of that same closed record tree. CleanupPlan v9 authenticates
unchanged descendant transfers, replaced-old settlement, replacement-prefix
completion and one result commit; Graph v30/v31 distinguish reconstruction
without and with universally authenticated nested loans. Interpreter, native
C11 and Core Wasm remain fieldwise. Dotted mutation, resources, variants,
generics and public ABI surfaces remain closed.

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

The public executable lane emits C11 and invokes the host's `clang` command.
The compiler and legacy single-source CLI share the private
`src/native_scratch.rs` helper for exclusive temporary-directory creation,
retained file identity, and explicit fixed-inventory cleanup. Failed or uncertain
work retains its scratch; this does not add a deleting destructor, process
sandbox, or the SDK builder's separate held-tool authority. See
[Native compiler scratch v1](NATIVE-SCRATCH-V1.md).
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
The shared `aggregate/expressions.rs` keeps recursive scalar dispatch and block
statement lowering separate from inactive aggregate-arm temporaries. The
`aggregate.rs` expression wrapper still applies canonical post-transitions and
String scope cleanup exactly once, after the selected handler returns.
Capacity refusals run generated cleanup; unexpected traps do not promise
settlement or permit reuse. See [Standalone Wasm Internal String Settlement
v1](WASM-INTERNAL-STRINGS-V1.md#local-validation-record) for selected local
evidence and remaining gates; ordinary Wasm
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
package v1](WASM-INTERNAL-STRINGS-WEB-V1.md#local-validation-record) for selected
local consumer and boundary evidence; required host/release gates remain open.

`src/wit_component.rs` and `src/wit_component/` provide default-off private
boundary evidence. They cannot be cited as public Component Model execution.

`src/project/scalar_wit.rs` projects an already admitted retained Project-v1
scalar surface into a public, authority-free WIT interface artifact. Stable
declaration IDs become injective hexadecimal WIT names, parameters use ordinal
identities, and every scalar result is represented as `result<T, status>`.
Its canonical subject-bound descriptor is independently replayed against the
retained manifest, HIR, revisions, graph digest, selected order and regenerated
WIT before exposure. This path emits no Core Wasm or Component bytes, performs
no I/O, and does not promote the private Component harness.

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

### Source-owned static protocol conformance

`src/static_protocol.rs` checks persistent, source-owned static implementation
mappings from local protocol requirements to ordinary existing functions.
`candidate/interface.rs` derives eligible member discovery and admits a closed
`implement_interface` intention for the bounded monomorphic local-record slice.
Required members must be covered exactly and signatures must match; canonical
source is reparsed and complete Project admission runs before a candidate is
retained. These mappings do not introduce runtime witnesses, virtual calls,
dynamic dispatch or an unchecked graph-only implementation.

[Static Protocol Conformance v1](STATIC-PROTOCOL-CONFORMANCE-V1.md) and
[Interface Change v1](PROJECT-INTERFACE-CHANGE-V1.md) own the exact subset.
[Image Protocol Conformance v1](IMAGE-PROTOCOL-CONFORMANCE-V1.md) carries
source-bound conformance as a separate derived projection and exposes additive
v4 `protocol/conformance` and `candidate/interface-catalog` queries. The runtime
Graph and earlier protocol method sets retain their existing contracts. Discovery
facts grant neither execution nor publication authority. These authored slices
bring all eleven requested operation classes into bounded scope; they do not
complete general interface semantics or the graph-operational programme, and
current-head compiler/test evidence remains unrun.

### Unified workspace session v5

`project/candidate/archive.rs` packages canonical original source/manifest and
the existing complete-history recovery capsule into an independently replayed
archive. `candidate_archive_store` owns a separate explicit immutable file store:
private host root, held no-follow directory chain, bounded exact inventory,
exclusive create-new stage and no-replace pivot. Load recompiles archived source
and history under the held store input; no serialized HIR is trusted and raw
source is not reconstructed on disk. Post-pivot uncertainty never regains
cleanup or retry authority. This is candidate persistence, not warm HIR reuse.

The same store now has typed draft persistence/load entry points over a private
shared byte-transport seam. Draft source/history/selector replay runs before
root opening on persist and inside the held-input scope on load. Both archive
kinds share the fixed inventory but retain separate schema admission. CLI
`draft_archive.rs` and host-policy v6 add explicit draft storage and startup
selection; neither path restores approvals or opens a new request authority.
See [Typed-draft persistence](DRAFT-ARCHIVE-PERSISTENCE-V1.md).

`image_transport/vnext/recovery.rs` admits independently recovered candidates
only through startup host APIs, under live snapshot authentication and ordinary
registry bounds. Canonical manifest equality permits historical source revisions
without making them current. CLI host-policy v3 supplies at most sixteen explicit
store selections; earlier policy versions stay closed. Git authority opens only
after these loads and remains separately approved. [Candidate Archive](PROJECT-CANDIDATE-ARCHIVE-V1.md),
[Archive Store](CANDIDATE-ARCHIVE-STORE-V1.md), [Recovery](IMAGE-WORKSPACE-ARCHIVE-RECOVERY-V1.md)
and [CLI](CANDIDATE-ARCHIVE-CLI-V1.md) own authored/unrun cases and exact bounds.

`image_transport/vnext.rs` composes a fixed host-selected policy over the
existing candidate engine. Read-only access, candidate preparation, diagnostic
attempts, fixed-policy interpreter tests and pathless carrier builds have
distinct grants; disabled operations are absent from discovery. V1–v4 method
sets and byte contracts remain separate. `workspace/refresh-preview` observes the
new source revision without swapping state. `workspace/refresh` loads only the
fixed manifest and checks exact expected revisions before swapping the retained
snapshot/image. Immutable historical candidate handles survive; drafts and
attempts are cleared on success. The default constructor uses cold source
recovery; the explicit cache constructors below select reuse strategies.

`image_transport/vnext/test_tasks.rs` owns the one-task session registry and its
queued/running/terminal lifecycle. It detaches one immutable exact candidate and
fixed host test policy into a named scheduling worker, while the coordinator
retains every source-authentication and result-release decision. The project candidate layer
shares replay/report rendering between synchronous and cancellable execution;
a fixed-stack prepared evaluator thread supplies the monotonic step-boundary
cancellation flag. A process-wide eight-task permit bounds the scheduling and
evaluator thread pairs across sessions.
Drift, successful refresh, finish, and drop cancel and join before a late report
can escape. [Candidate Test Tasks v1](IMAGE-CANDIDATE-TEST-TASKS-V1.md) owns the
wire shapes, race semantics, authority ledger, and bounds.

The optional `image_transport/vnext/mcp.rs` adapter consumes a configured v5
session. Its pinned MCP lifecycle and stdio framing delegate semantic calls
to the ordinary v5 handler, preserving source and publication boundaries.
`mcp_catalog.rs` derives a collision-checked tool mapping and local-reference
input schemas from selected methods. Response capacity is reserved before
forwarding calls so wrapper escaping cannot hide a completed mutation or
publication. The CLI shares `workspace_session` startup policy loading between
ordinary NDJSON and `serve-workspace-mcp`; no client capability becomes authority.
Selected candidate-test task methods remain ordinary MCP tools; the adapter does
not advertise the optional MCP Tasks facility or map `notifications/cancelled`.
See [MCP Adapter](IMAGE-MCP-ADAPTER-V1.md) for bounds and evidence limits.

Discovery's `clients.rs` retains selected-method request generation and runtime
validation. Its bounded response normalizer lifts only document-local definitions
from reachable schemas into an internal absolute-reference registry and retains
all supported assertions. `repair_schemas.rs` composes the closed repair report
from the existing constructor owner, restricted to actual body repair forms.
`response_types.rs` builds a shared bounded response-type model with guarded
recursion from the already audited reachable payload schemas; `response_types_rust.rs` and
`response_types_script.rs` emit concrete Rust, TypeScript and Python shapes.
Additive typed decoders enter through validators whose work budget is shared
across recursive references and failed alternative trials. Constant discriminants
are checked before recursive fields. Unbundled reports
remain opaque, and no generated helper owns transport or publication authority.
See [Typed Response Clients](IMAGE-TYPED-RESPONSE-CLIENTS-V1.md).

`request_types.rs` separately models selected complete request shapes, retaining
document-local reference scopes and admitting recursive constructor shapes.
Its Rust and script emitters supply additive typed request builders through the
existing outer validator. They do not add nested runtime validation, semantic
admission, transport or publication authority. See
[Typed Request Clients](IMAGE-TYPED-REQUEST-CLIENTS-V1.md).

`project/candidate/draft_navigation.rs` derives compact hole summaries and
bounded facet pages from the existing exact full context. Opaque references bind
the draft, hole, context bytes and selected facet; they grant no authority.
The v5 `hole_navigation.rs` adapter exposes these pure projections through
ordinary live-source authentication and detached reads. Full contract, loan and
cleanup contexts retain their existing owners and bytes. See
[Compact Hole Navigation](PROJECT-HOLE-NAVIGATION-V1.md).

`project/candidate/draft_suggestions.rs` derives bounded place and direct-call
proposals from the existing exact hole context, then invokes ordinary fill on
the original draft for each attempt. Type/effect matching prunes enumeration;
full source admission owns ownership, loan, cleanup and profile checks. Preview
drafts are dropped, and their digests confer no registry handle. The v5
`hole_suggestions.rs` adapter and detached reads share the pure query; its
finite output grammar is derived from compiler constructor schemas. See
[Fill Suggestions](PROJECT-HOLE-FILL-SUGGESTIONS-V1.md) for search, replay and
report bounds and the distinction between source admission and intended behavior.

`project/candidate/draft_expression_catalog.rs` reuses the private last-valid
candidate's checked expression catalogue and wraps it with exact draft and
region bindings. The separate last-valid digest fields are descriptive; this
query releases no candidate, source or publication handle. V5 and detached
reads share the projection, and the editor uses it for further hole selection
after fills. See [Draft Expression Catalogue](PROJECT-DRAFT-EXPRESSION-CATALOG-V1.md).

`project/candidate/source_review.rs` independently replays candidate history and
derives a closed source-pair/diff report, lazily retained on that exact candidate.
The v5 `source_review.rs` adapter shares its immutable bytes across bounded
chunks and detached reads; held-source authentication remains in the coordinator.
`editors/vscode` supplies an optional explicit-start MCP client and read-only
virtual diff documents. Its general command allowlist excludes build and source
publication; the separate candidate-test task controller consumes only the four
task methods when they already exist in the startup-selected server catalogue.
Editor
events invalidate UI state but never authorize or replace source authentication.
See [Source Review](PROJECT-CANDIDATE-SOURCE-REVIEW-V1.md) and
[Saved-Source Editor](VSCODE-SAVED-SOURCE-ADAPTER-V1.md).

`editors/vscode/holes.js` coordinates an editor-owned typed draft through the
existing body, expression and contract-hole routes. It validates bound handles
and compact navigation, preserves failed fills, retires superseded draft
handles and requires explicit completion before candidate review. Editor
scratch bindings and source epochs are local freshness checks, not semantic
proof or publication authority; compiler admission remains unchanged.
Checked fill suggestions bind a closed bounded report to the controller's own
current summary. The extension requires an explicit choice before copying only
its expression into ordinary bound scratch; preview digests are never adopted.
Epoch and selection checks surround each asynchronous UI step, and actual fill
remains a separate compiler-validated request.

`editors/vscode/repairs.js` retains rejected attempts separately from the valid
candidate, verifies exact raw diagnostic report bytes and binds compiler repair
catalogues to the predecessor. Explicit selection sends only attempt and repair
IDs; displayed change bodies never become requests. The controller retires old
attempt handles after validated transitions and fails closed on uncertainty.
Diagnostic locations remain descriptive, with no source navigation authority.

An opt-in `open_with_frontend_cache` constructor uses the same authenticated
filesystem loader with `ProjectFrontendCache` as its build strategy. It parses
fresh source directly on first load, then stages exact-source AST reuse during
refresh/preview while rebuilding all checked semantics. Only successful explicit
refresh installs the staged cache alongside the new snapshot/image. Host-policy
v2 selects this strategy through a required boolean; v1 stays closed and cold.
The optional work report records actual frontend calls, not elapsed speed.

`open_with_semantic_cache` additionally retains compiler-created checked module
HIR against exact synthetic AST inputs, including imported declaration stubs.
Host-policy v4 selects this strategy explicitly. Source authentication, HIR
validation, cross-file checks, linking and profile admission remain mandatory;
only eligible resolver work is reused. The cache shares the existing staged
refresh/preview lifecycle and grants no publication authority.

`semantic_cache_store.rs` and its Unix host module own a separate keyed cache
root, exact compiler-file binding and immutable publication. They authenticate
the complete envelope before the private `cache_codec` and `hir/cache_codec`
construct cached AST/HIR. `project/incremental/snapshot.rs` reparses canonical
sources, requires exact synthetic-input reuse, and repeats complete Project
admission and graph comparison before returning an opaque cache. HIR validation
alone does not prove source correspondence: the host must protect the signing
key and keep its static compiler installation immutable from exec. Host-policy
v5 can select one entry before live source authentication; no RPC gains storage
or signing authority. Existing source-backed Image/Revision stores stay cold.

`semantic-cache-lifecycle` composes the existing host-selected cache operations
with authenticated v5 opens and same-revision refresh into one deterministic
compiler-work receipt. It records cold, authenticated restored, refreshed,
evicted and cold-rebuilt stages plus exact retained byte counts and semantic
identity equality. It is an explicit CLI storage effect against an existing
empty private root; it neither changes canonical source nor supplies wall-time,
RSS, cross-process, target-execution or publication evidence.

`image_transport/vnext/read_batch.rs` adds an embedding-host batch API for up to
sixteen selected immutable semantic requests on at most four scoped workers.
Source authentication surrounds the complete joined batch, and rows remain in
input order. Workers receive no registry, snapshot handles, cache, Git host or
test interpreter. Retained reads resolve only request-selected candidate, draft
and attempt references inside authentication, then share pure payload handlers
with sequential dispatch. Diagnostic inventory reads detach a bounded immutable
attempt-reference inventory and retain ordinary matching/error order.
Policy-bearing discovery values are prepared by the serial coordinator inside
authentication, so workers receive no execution policy. Refresh, registry mutations, runtime
tests, builds and publication are excluded. `read_batch_rpc.rs` adds only an
explicit startup-selected `workspace/read-batch` entry point over that same
worker engine. Host-policy v7 configures a fixed worker count independently of
other grants; MCP and generated discovery derive the selected method. The
complete outer response is bounded before final source authentication, including
all-error batches, and keeps the existing 1 MiB cap. The CLI NDJSON loop remains
sequential across outer requests. See [Parallel Read Protocol](IMAGE-READ-BATCH-PROTOCOL-V1.md),
[Frontend Cache](IMAGE-WORKSPACE-FRONTEND-CACHE-V1.md) and
[Parallel Reads](IMAGE-PARALLEL-READS-V1.md) and
[Parallel Retained Reads](IMAGE-PARALLEL-CANDIDATE-READS-V1.md) for bounds and unrun evidence.

`image_transport/vnext/commit.rs` holds separately supplied fixed Git authority
and a startup-only private approval slot. A request cannot choose its repository,
ref, old commit, metadata or approval. A review/export session precedes a new
host-approved commit session that restores the exact candidate. Publication
runs after the initial held-input selection boundary; the Git library owns its
fresh replay, ref pivot and post-pivot uncertainty classification. Compact
receipt handles prevent escaped receipt overflow after a pivot. The original
raw checkout remains unchanged, and consumed or terminal approval is not
refreshed by a source reload. Provider deadlines are never silently reset.

`project/image_targets.rs` and the v5 projection adapter derive actual target
emission facts and replay existing pathless Web/npm carriers. Declared export
and input relationships do not imply runtime coverage or external consumer use.
`candidate/build` needs a host grant, independently restores candidate history,
and returns bounded report chunks; it does not publish filesystem artifacts,
install packages, invoke a native toolchain or execute generated code.

`project/image_coverage.rs` inventories exact retained source/manifest bindings
and interface-import declarations, and explicitly reports deployment,
generated provenance, external behavior, runtime and consumer analysis gaps.
Its closed `blind_spots` ledger binds absent deployment-configuration,
generated-file-provenance and external-API/deployed-runtime evidence to the
exact retained Project/source inventory while explicitly refusing to infer
that the corresponding contract is absent.
The v5 `image/analysis-coverage` read shares ordinary source authentication and
the detached parallel read path. It neither scans external inputs nor mistakes
missing edges for absent external systems. See
[Analysis Coverage](SEMANTIC-IMAGE-ANALYSIS-COVERAGE-V1.md).
`candidate/deployment_contract_evidence.rs` can attach a canonical value-free
configuration declaration to one exact candidate and its complete explicit
manifest-export inventory. It accepts only sorted key/type/required shapes and
changes the deployment row to partial; it never reads environment values,
secrets, paths, URLs or provider locators and does not verify or perform a
deployment. The candidate-only v5 adapter exposes the same pure derivation as a
bounded chunked read and generates its client/MCP surface from the selected
catalogue; declaration bytes remain request data rather than a path or grant.

The v5 discovery catalogue generates its granted methods, schemas, instructions
and typed client helpers together. Opaque nested payload references remain
explicitly incomplete; a generated bundle is not a complete executable response
validator. [Agent Discovery v5](IMAGE-AGENT-DISCOVERY-V5.md) owns the exact
request/envelope schemas, concrete transport payloads and explicitly unbundled
compiler-report schemas; generated clients perform no I/O or execution. See
[Workspace Protocol v5](IMAGE-WORKSPACE-PROTOCOL-V5.md),
[Source Commit v5](IMAGE-SOURCE-COMMIT-PROTOCOL-V5.md),
[Target/Artifact Projections](IMAGE-TARGET-ARTIFACTS-V1.md), and
[Workspace Session CLI](WORKSPACE-SESSION-CLI-V1.md). All evidence for this batch
is authored/unrun; no complete-workflow or current-head promotion follows.

### Project profile and daemon

`project/incremental.rs` owns an opt-in, invocation-local cache of exact-source
canonical ASTs. It can avoid parsing/canonicalization for eligible retained
modules while the ordinary graph, linking and profile gates still revalidate
semantics. Its separate semantic-cache constructor also retains exact synthetic
AST/HIR pairs and conservative reverse-import invalidation. Complete module
hits replay checked-HIR validation and preserve cold builder accounting without
calling the source resolver again. A changed module can reuse exact monomorphic
free-function HIR only under complete non-body environment, signature,
contract, span and selected-AST equality; source verification, declaration
reconstruction, HIR validation and every Project-wide gate still rerun. Only
the separately authenticated snapshot loader can
restore serialized HIR into this cache. `candidate/draft.rs` now carries
disjoint expression holes as well as whole-body holes; completed fills pass
ordinary candidate admission and reauthenticate surviving selections against
the resulting canonical source. Neither cache nor draft owns source authority.

`project/image_reference.rs` exports exact image-bound stable function/facet
selectors and resolves them only through ordinary admitted image provenance.
Its conservative cross-revision rebind authenticates both images and the source
selector, then requires one unique explicit identity and exact source/module
facts before exporting a fresh destination selector. Provenance classification
does not infer revision ancestry, semantic equivalence or compatibility.

`project/target_cache.rs` owns separate caller-scoped exact scalar-Web,
pathless native-C11 and pathless npm target entries. Their keys bind the admitted immutable
Project/workspace/graph identities, canonical manifest, target selectors,
compiler compatibility and byte bound. Every hit independently verifies its
retained carrier; the C lane replays artifact and embedded header-envelope
bindings, while npm replays its closed semantic recipe and ordered file bytes.
Reuse skips only target emission and grants no persistence, filesystem,
package-manager, compilation, execution or publication
authority. See [Project Target Cache v1](PROJECT-TARGET-CACHE-V1.md).

`src/semantic_retention.rs` owns a pure bounded metadata policy for exact image,
candidate and draft identities. It derives canonical chained checkpoints and
separately digest-bound GC plans from caller-supplied inventories. Recovery
authenticates predecessor and policy bytes and returns
`RetentionAuthority::None`; the module never reads storage, deletes an object,
establishes freshness, restores approval, or publishes a generation.
`project/image_store.rs` and `candidate_archive_store.rs` translate only
successful image/candidate/draft publication receipts into that inventory.
Those receipts retain exact logical byte counts and identities but no path or
store handle; heterogeneous planning still performs no discovery, checkpoint
persistence, deletion, replay, approval or publication.
`src/semantic_retention_store.rs` separately persists one already authenticated
checkpoint/plan pair as a fixed immutable envelope through a single no-replace
Unix pivot. Held private root/file identities and independently supplied
checkpoint/predecessor/plan selectors guard ordinary restoration. This store
persists metadata only: it cannot apply the plan, enumerate/delete subjects,
infer newest state or restore any source/image/candidate authority.
`src/semantic_retention_registry.rs` composes successful typed receipts, the
pure planner and that unchanged pair store behind a separate explicit private
registry root. A held `0700` `metadata` child owns immutable pairs; a canonical
`0600` `CURRENT` cursor binds the consecutive sequence, exact pair selectors and
fixed policy. Pair operations stay descriptor-relative to that held child.
Initialization and CAS advancement settle the pair before the cursor pivot.
Recovery authenticates only the pair named by `CURRENT`; exact pre-pivot pairs
may be reused after rederivation, while post-pivot uncertainty requires recovery
from the same explicit root. An interrupted cursor stage must be canonical,
backed by its exact held-directory pair and consecutive with `CURRENT` before
exclusive cleanup; malformed or unrelated stages fail closed. Cursor-stage
cleanup never deletes a retained subject or immutable pair. The registry grants
no source, subject-store, GC, approval or publication authority and has no CLI
or session integration. See [Semantic Retention Registry v1](SEMANTIC-RETENTION-REGISTRY-V1.md).
`src/semantic_retention_lifecycle/automatic.rs` composes one separately held
candidate/draft archive store with that registry. Immutable archive publication
precedes checkpointing. A fresh coordinator can replay an exact already stored
archive into a typed receipt, then either recognize its exact retained subject
or attempt one registry checkpoint, closing the crash window without weakening
ordinary no-adoption publication. Its canonical replay and resume reports are
accountability data only; current source, Git, approval, GC, warm HIR and
publication authority remain outside the module. See [Automatic durable
candidate/draft lifecycle v1](AUTOMATIC-CANDIDATE-DRAFT-LIFECYCLE-V1.md).
`src/cli/retention_metadata.rs` is the narrow command-line adapter. It receives
an explicit bounded canonical declaration inventory whose rows contain only a
closed image/candidate/draft subject and byte accounting. Declaration rows use
visible canonical subject-JSON order; compiler code derives their subject
digests and emits the exact digest-ordered observation inventory consumed by
planning. No caller-supplied subject digest is trusted. The adapter then
receives an explicit bounded canonical observation-inventory file and optional exact
prior-checkpoint file/digest/predecessor tuple for authority-neutral planning.
Planning returns canonical checkpoint and GC-plan metadata without reading a
store or persisting or applying either value. The same adapter receives an
explicit existing store root, explicit bounded checkpoint/plan files for
publication and all exact selectors as positional operands. Load requires the
same selector tuple and returns the authenticated canonical metadata only. The
adapter introduces no receipt trust, inventory or root discovery, freshness
selection, GC execution, subject restoration, approval or publication path. See
[Semantic Retention Metadata CLI v1](SEMANTIC-RETENTION-METADATA-CLI-V1.md).

`candidate/git_publication.rs` authenticates Git object identities and original
Project source before constructing canonical replacement blobs, trees and a
commit. Its explicit host authority owns one expected-old Git ref update. The
Unix process adapter selects a bounded bare SHA1 or SHA256 repository,
executable and lease. It launches from held executable and repository handles,
gives the child only its exact standard-pipe descriptor inventory and a fixed
safety environment with no inherited entries, and settles the owned process
group before ordinary return.
Linux uses descriptor execution and handle-relative cwd selection; macOS derives
a path from the held executable, starts suspended, and attests executable-vnode
and cwd identity before resume, failing closed on disagreement. The
adapter disables inherited config, hooks and Git transports and never rewrites
a checkout. Target OID width must match the held repository format.
SHA1 is legacy Git compatibility: exact original-source comparisons, staged-object
readback and an independent SHA256 content binding do not constitute SHA1
collision detection or a signature. The trusted host owns the executable and
repository. Held execution closes the same-byte pathname-substitution gap, but
the cooperative lease and object scan do not make a same-UID writer to the
repository's contents trustworthy or form an OS network sandbox.
This authority remains separate from managed Workspace `ACTIVE` and ordinary
image/candidate reasoning. V5 can hold it only through an explicit startup host
extension with independently supplied exact candidate approval. `candidate/diagnostic_intent.rs` rederives a selected repair
from a fresh rejected attempt before ordinary candidate admission records its
typed history; no invalid image or submitted replacement becomes trusted state.

`src/project/image_store.rs` binds semantic image receipts to secure persisted
Project source inputs. Loading rebuilds and re-derives the image. The separate
`ImageWorkspace::with_semantic_cache` opt-in uses checked-module reuse for
owned-source refresh; cold and AST-only constructors retain their behavior.
Those source-backed image-store routes do not restore serialized HIR.
`candidate/delta.rs` derives source-bound before/after
semantic facts with exact replay. Diagnostic protocol v4 retains bounded rejected
attempts and verified repair proposals, with test authority selected only by the
host. Store and managed publication remain separate from protocol authority.
`candidate/interface_delta.rs` separately compares complete source-owned protocol
and implementation inventories, actual member-function facts and static call
dependencies. Verification replays the candidate before recomputing the report;
no runtime dispatch facts or graph authority are introduced. V5's
`review_facets.rs` exposes the candidate-bound chunk query. Its sibling
`symbol_diagnostics.rs` joins only retained attempts whose predecessor and intent
target match exactly, derives available repairs through ordinary candidate
admission, and binds chunk continuations to the exact report digest. Rejected
attempts never become checked images or trusted source spans.
`candidate/contract_delta.rs` separately compares whole-candidate ordered
predicates and their static callable dependencies. It derives checked predicate
projections and source-backed dependency facts, then independently replays the
candidate before verifying exact report bytes. V5 exposes a candidate-granted
chunk query; this read performs no target generation or execution and adds no
publication authority.
`candidate/ownership_delta.rs` compares exact checked source-nominal shapes and
supported monomorphic aggregate type facts together with source-function and
retained-instance ownership facts, complete structural inventories, and the
existing ordered loan/cleanup projections. Generic type facts remain explicitly unavailable;
layout keys are compiler facts rather than ABI judgments. Exact report
verification replays the full candidate first. The report neither changes a
plan nor turns its actions into physical ownership authority; v5 exposes it
through candidate-granted chunks.
`candidate/artifact_delta.rs` replays the full candidate before comparing base
and candidate Web/npm carriers through existing independently verified pathless
artifact projections. File-content, stable export, source and carrier bindings
remain distinct. V5 requires the existing build grant and provides report chunks;
neither report generation nor verification installs or publishes artifacts.
`project/image_targets/openapi.rs` extends this pathless review lane with actual
per-source OpenAPI envelopes. The shared `openapi.rs` renderer preserves its
scalar admission rules; the Project carrier rebuilds every canonical input and
regenerates the artifacts before exact replay. Source-selected stable IDs bind
documents to exports, while image summaries and candidate deltas retain their
existing authority boundaries. See [OpenAPI Artifacts](IMAGE-OPENAPI-ARTIFACTS-V1.md).
Its `project/image_targets/c.rs` sibling binds the actual checked native C11
entry projection to source-selected C headers through the existing renderer's
prototype extraction, admission and hygiene rules. The carrier rebuilds all
canonical Project inputs and regenerates every file before exact comparison.
Explicit header exclusions do not create ABI symbols; static linkage and
context/status conventions remain unchanged. See [C Artifacts](IMAGE-C-ARTIFACTS-V1.md).
The [integrated workflow](PROJECT-GRAPH-OPERATIONAL-WORKFLOW-V1.md) is authored,
unrun and publishes managed generations only; canonical Git files stay unchanged.

`src/project/candidate/` owns immutable source-derived candidate overlays and
closed typed intentions. The engine mutates invocation-local ASTs; canonical
source then re-enters complete Project admission and a second source replay.
Candidates retain reviewable replacements/diffs and target projection facts,
never filesystem or publication authority. `src/project/image_facets.rs`
projects actual retained HIR into revision-bound paginated facets without
changing the original image wire. Its `image_facets/instances.rs` child adds
source-template selection and concrete retained-instance pages. It reuses the
seven applicable facet renderers and separately joins exact instance callers
and linked-closure membership, because an instance's internal function ID
still names its template. It creates no new instantiation or execution.
`src/image_transport.rs` exposes these
queries through a separately selected read-only protocol whose one method
catalog also generates discovery/schema/client material. Existing transports
keep their method sets. See [Candidates](PROJECT-CANDIDATES-V1.md),
[facets](SEMANTIC-IMAGE-FACETS-V1.md), and
[Image Agent Protocol](IMAGE-AGENT-PROTOCOL-V1.md).

`src/project/image_dependencies.rs` owns a lazy bounded dependency index retained
by each immutable image. Candidate delta relationships and the read-only v5
`image/dependencies` query share its source-HIR collector. Source-bound access
sites and direct caller closure are structural facts, not runtime or coverage
evidence. Initialization, including failure, is shared across immutable reads;
no index is serialized into Image v1 or given publication authority. See
[Declaration Dependencies](SEMANTIC-IMAGE-DEPENDENCIES-V1.md).

Its `image_dependencies/navigation.rs` child exposes compact summaries and
opaque-reference detail pages over the same retained index. Selection works on
IDs and row ordinals; only requested page rows are projected. It adds no HIR
walker, mutable cursor registry or source authority. See
[Dependency Navigation](SEMANTIC-IMAGE-DEPENDENCY-NAVIGATION-V1.md).

Its `image_dependencies/obligations.rs` child lazily indexes retained cleanup
and loan proof facts by their actual type/case/field identities. Storage shapes
and exact projection paths select original plan coordinates without another
expression-reference scan or changes to plan vectors. The cache stays outside
Image v1 serialization; image verification recomputes it from retained checked
HIR. `candidate/cleanup_dependencies.rs` reuses the collector for target-specific
before/after review, with complete source-history replay on verification. V5
keeps the image query read-only and the candidate query behind its existing
candidate grant. See [Cleanup Dependencies](SEMANTIC-IMAGE-CLEANUP-DEPENDENCIES-V1.md).

The additive [candidate-only protocol](IMAGE-CANDIDATE-PROTOCOL-V2.md) selects
ephemeral candidate/draft registry authority at host startup; it cannot grant
source writes, runtime tests or builds. Responses are bounded and held source
inputs are reauthenticated before registry mutations become visible. Typed
[body-hole drafts](PROJECT-CANDIDATE-HOLES-V1.md) expose context while blocking
materialization until all holes are filled through full candidate admission.
[Ordered signature mapping](PROJECT-SIGNATURE-EVOLUTION-V1.md) stages every
original argument once, left-to-right, before retaining, reordering or removing
Copy parameters; admitted byte, String and resource-free nominal owners must
all be retained once. Source String parameters resolve to owning HIR even
without an explicit source `own` modifier.
An additive borrowed-alias form accepts only an original `borrow str` or
`borrow Slice<u8>` parameter that remains exactly once. It derives the new
parameter type/root from checked HIR and reuses the original staged view;
ordinary post-rebuild loan and provenance validation rejects any unsafe alias.
`candidate/signature_arguments.rs` constructs explicit computed-argument
templates against each caller's bindings and reuses scope-aware substitution
to select staged original values. The signature engine appends typed computed
locals after all original stages; full candidate replay owns semantic admission.
Computed nominal types reuse stable-ID type planning separately for the provider
and each caller annotation. After source rebuild, an additional signature gate
checks exact owner/argument identities and retained Sized Copy, resource-free,
no-drop facts before exposing a candidate, including signatures without calls.
Expression-template preflight without callers is structural only. See
[Argument Expressions](PROJECT-SIGNATURE-ARGUMENT-EXPRESSIONS-V1.md).
Workspace module projections privately retain compiler-checked nominal
parameter and return TypeFacts, including types outside entry/test reachability. The
candidate admission and catalogue share those exact facts for concrete Copy
record/variant eligibility and resource-free owning record/variant eligibility;
they do not infer ownership from source spelling. Primitive String eligibility
uses the checked parameter mode and ordinary compiler TypeFacts.
These facts add no graph wire fields or source authority. These additions are
authored, unrun.

`candidate/expression.rs` joins retained HIR identities to canonical AST
provenance for typed body-expression replacement, preserving the selected
expression's expected type through complete Project rebuilding. Contract
insertion permits exactly one additional predicate while retaining prior
contracts and all other invariant inventories. `candidate/rebase.rs` compares
stable-ID target/dependency facts, normalizes call display names for conflict
selection, binds nominal signatures to retained checked type identities, and
replays supported intentions over an admitted base. Same-root
merge retains both histories and the original source-diff base. These APIs
produce candidates and ancestry reports, never source publication authority.

`candidate/merge_preview.rs` reuses the ordinary merge implementation in both
orders and compares exact accepted manifest/source bytes. It returns only bound
directional facts or bounded diagnostic excerpts, never either temporary
candidate. The v5 adapter uses the same detached two-candidate read subjects in
sequential and parallel paths; no registry mutation or publication host enters
the preview. See [Merge Preview](PROJECT-CANDIDATE-MERGE-PREVIEW-V1.md).

`candidate/rebase_normalize.rs` borrows the existing Operations AST/HIR
occurrence collector through a private Project view. It derives bounded,
collision-checked identity markers for nominal and member references in
comparison-only source copies, retaining checked owner shapes and ancestry.
Canonical formatting of those parse-only copies removes display-name noise
without changing binders, expression order or actual source. Candidate and
draft dependency comparisons normalize only known descriptor display fields;
ordinary source replay and publication boundaries remain unchanged.

The additive contract-expression route in `candidate/expression.rs` reuses exact
HIR/AST joins for existing pre/postcondition subtrees while leaving body-only
APIs unchanged. Its post-admission gate reconstructs the complete requested
canonical source and checks phase/path/type/ownership again. `candidate/draft.rs`
keeps contract holes separate from body regions under one shared budget and
remaps survivors after fills; draft recovery stores only selectors over replayed
valid history. V5 adds candidate-granted discovery and hole opening, with no
build or publication authority. See
[Contract Expression Holes](PROJECT-CANDIDATE-CONTRACT-HOLES-V1.md).

`candidate/builtin.rs`, a child of the intent module, resolves typed
`builtin_call` requests through the compiler-owned byte and string operation inventories.
It rejects source identity/binding collisions and supplies the same descriptors
to discovery and semantic rebase. Its shared operation selector also supplies
the exact HIR occurrence map used by declaration movement, without broadening
movement's checked type or import vocabulary. Ordinary source-call lowering and complete
candidate replay retain ownership, view provenance, cleanup and target
authority; the graph carries no new independent meaning. See
[Builtin Call Constructor](PROJECT-BUILTIN-CALL-CONSTRUCTOR-V1.md).

`candidate/aggregate.rs` resolves typed record/case/field constructors through
retained checked module declarations and existing local/imported type bindings.
The revision-aware expression constructor shares this path across bodies,
expressions, contracts, declarations and hole fills. It preserves requested
initializer order and delegates all semantic admission to the ordinary complete
candidate rebuild. Discovery projects the same checked inventory; rebase binds
referenced aggregate shapes for each original/rebased intermediate revision.
No source spelling or new import authority comes from constructor requests.
Generic construction retains checked template parameter identities and emits
explicit direct-scalar type arguments through the same AST path. The separate
prelude selection authenticates the fixed compiler-owned Option/Result
declaration inventory against retained HIR; it does not relax authored explicit
identity checks. Discovery reports templates and compiler provenance rather
than claiming concrete instantiation admission.
Record-field value projection uses the same checked owner inventory and emits
a hygienic typed local followed by the ordinary field AST. The annotation
forces nominal owner equality through existing source admission, and the base
is evaluated once under normal value-binding ownership/cleanup rules. Rebase
binds the selected field and complete owner descriptor before replay; discovery
does not treat field spelling or a matching result type as owner evidence.
Direct `field_place` construction instead authenticates the lexical root's
nominal type against the selected stable field owner and emits an ordinary
field expression without a temporary. The intent child `candidate/field_place.rs`
owns bounded nominal fact propagation. Constructor scope facts come from
authenticated parameters, selected HIR scope and bounded propagation through
typed constructors; they do not replace the source verifier's loan, move or
cleanup checks. Discovery distinguishes direct places from value projections,
and rebase retains the complete selected-owner dependency. See
[Field Place Constructor](PROJECT-FIELD-PLACE-CONSTRUCTOR-V1.md).
`candidate/aggregate_match.rs` derives whole-variant matching plans and
discovery from the same checked source/prelude identities. The expression
constructor stages the exact nominal scrutinee once and emits ordinary
exhaustive value-match patterns with arm-local payload bindings. Rebase binds
the complete case and payload inventory; ordinary candidate source admission
retains all matching, ownership, cleanup and target checks.
Record updates reuse `candidate/aggregate.rs` owner/member resolution and lower
to the existing update AST after exact-owner staging. The base is evaluated
once before requested replacements, with untouched fields left to ordinary
update semantics. Discovery exposes the complete owner inventory and rebase
binds that inventory even for fields omitted from the replacement subset.

`candidate/declaration.rs` appends a typed function under a selected module
anchor with globally fresh identity and checked namespace/effect budgets.
`candidate/type_declaration.rs` appends explicit monomorphic record/variant
declarations with bounded direct data fields or authenticated existing nominal
selectors under the same function-anchor rule. Field selection reuses the
compiler's stable-ID binding planner; full rebuilding owns data-type admission.
Its complete planned owner/case/field inventory is the only allowed graph
identity extension; exact source reconstruction follows full Project replay.
Rebase checks collisions for all new member IDs, binds nominal field dependencies
through the shared recursive intention walk, and preserves history order
when a later operation evolves or uses the new type.

`candidate/type_rename.rs` delegates source record/variant display changes to a
private owned-source entry point in `semantic_workspace_operations`, reusing
the shared `workspace_graph/operation_sidecar.rs` AST/HIR occurrence collector,
namespace checks and exact normalized replay. The Candidate route reparses the
result, performs ordinary full Project replay and checks its exact source plan.
No new reference index, managed lock or publication authority is introduced;
the public Operations proposal retains its multi-operation minima. Nominal
rebase uses separate source shape/origin/binding facts and test planning falls
back conservatively for non-callable type changes. See
[Nominal Rename](PROJECT-NOMINAL-RENAME-V1.md).

The same adapter and private planner also select explicit source record fields,
variant cases and payload fields. Member sidecar facts bind parent namespaces
and cross-file occurrence paths so constructor, pattern and projection labels
are migrated in consumers while type aliases stay unchanged. Member rebase
binds the complete owner shape and test planning uses a non-callable fallback;
no new public Operations subject or publication route is exposed. See
[Member Rename](PROJECT-MEMBER-RENAME-V1.md).

`candidate/aggregate_nominal.rs` authenticates existing record/variant type
selectors and visible bindings, including direct-scalar generic instances and
the fixed compiler prelude. Selection and catalogue templates are provisional;
after rebuilding, every function addition passes `declaration.rs`'s checked
signature gate. Value nominal parameters require sized Copy/no-drop facts;
explicit owning nominal parameters require sized non-Copy/drop facts. Both
exclude resources, as do nominal results. Bare String parameters retain their
implicit checked ownership. Owning nominal preflight uses the existing bounded
selected-declaration type-facts helper; source/import/target admission is
unchanged. Signature and checked
body-value facts share the existing 4,096-entry per-module table and builder-byte
budget. Rebase binds complete selected type inventories before each replay.
`candidate/extraction.rs` derives immutable scalar or Sized Copy nominal captures
from actual HIR ValueIds, captures complete immutable roots for field reads,
and replaces an authenticated expression in place, rejecting unsafe
boundary relocation. Only the exact declared identity may extend invariant
inventories. Rebase tracks newly introduced identities and rejects collisions.
Checked expression/local/pattern types use retained compiler TypeFacts; nominal
helper signatures use exact stable-ID type planning and the existing post-rebuild
signature gate. A separate nested-block lane retains internal resource-free
owners while keeping captures Copy. It may return one whole checked
resource-free owner through the ordinary provisional-result commit. A distinct
[owning-capture lane](PROJECT-OWNING-CAPTURE-EXTRACTION-V1.md) authenticates one
exact whole body-local Bytes/String owner with one unconditional consuming use,
then transfers it through an owning helper parameter at the original expression
position; parameters, projections, shared/borrowed roots, repeated or
conditional use and external consumers remain closed. The nested-block lane wraps the original
nested block in an empty helper root, preserving lexical cleanup before the
caller resumes and rejecting root-body relocation with owned data. Source/HIR
correspondence in `candidate/extraction_owned.rs` authenticates local values
and stable semantic identities; no
canonical ownership plan is rewritten. Retention keeps its declaration cap and charges bounded traversal
storage. Budget-report changes may change derived graph/image digests without
changing canonical source meaning or granting cache authority.
`candidate/recovery.rs` exports disposable complete histories and restores them
only by replaying against an independently admitted exact source base. It
imports neither serialized HIR nor authority and cannot materialize unresolved
drafts. `candidate/draft_recovery.rs` wraps that valid history with bounded
pending selectors, independently restores the history, and re-creates holes
through ordinary draft APIs before comparing exact draft/capsule identities.
Lineage-bearing v2 drafts additionally bind every successful fill to its exact
checked intention/history ordinal and retain bounded rebase/merge ancestry.
Recovery checks filled events against replayed history and preserves ancestry
as content-bound metadata; it does not prove the contents of parent drafts or
treat lineage as source meaning, approval, or authority. Lineage-free v1 draft and
recovery bytes remain preserved.
`image_transport/vnext/draft_recovery.rs` exposes host-selected chunk export and
transactional draft-only retention; it imports no registry or approval state.
`candidate/draft_archive.rs` composes that capsule with the existing complete
candidate source archive, so rebuilding an unfinished draft no longer requires
its original checkout. It compares the rebuilt last-valid candidate, pending
selectors and complete archive before returning only a draft. The v5
`draft_archive.rs` RPC module retains exact-current-base import; host methods in
`vnext/recovery.rs` separately allow same-manifest historical draft restore only
before the first frame. Both use ordinary authenticated registry admission and
recover no candidate entry or approval. See [Draft Archive](PROJECT-CANDIDATE-DRAFT-ARCHIVE-V1.md).
`candidate/draft_rebase.rs` rebases the private checked history through the
existing candidate owner, then uses shared semantic conflict fingerprints and
authenticated expression-origin remapping to reconstruct pending holes. The
v5 `draft_rebase.rs` adapter installs only the resulting draft after bounded
report preparation and live-source authentication; no candidate or publication
authority is released. See [Draft Rebase](PROJECT-CANDIDATE-DRAFT-REBASE-V1.md).
Its private pending-rebinding helper is also used by `candidate/draft_merge.rs`
after one ordinary candidate history merge. Both parents pass region and
dependency checks; opposing history writes are checked before their pending
maps are reconciled through ordinary hole APIs. Final contexts precede result
publication. Filled events from both branches retain their actual merged
history ordinals, while ancestry records the two exact parent draft digests;
neither fact implicitly completes a pending sibling hole. The v5
`draft_merge.rs` adapter retains only the merged draft under
the same request/registry boundary. See [Draft Merge](PROJECT-CANDIDATE-DRAFT-MERGE-V1.md).
These additions and focused regression cases are authored, unrun.

`candidate/movement.rs` moves eligible functions through stable-ID call/import
bindings. `candidate/movement_types.rs` checks retained resource-free nominal value facts,
plans authenticated type imports and remaps signature/local/aggregate/pattern
type syntax through destination bindings. Rebuilt type identities supplement
the existing call-inventory and exact source-reconstruction checks; source type
imports remain unchanged and ordinary Project admission still rejects cycles.
The movement planner retains exact checked builtin occurrences and reserves
their unchanged spellings during alias migration. Place roots, view operations,
type and ownership identities are compared after source rebuilding. Internal
owned byte work and String signatures add no staging or cleanup-plan edits;
existing owning parameter/type import restrictions remain authoritative.
`candidate/record_field.rs` appends a typed scalar field and migrates
constructors and exact patterns using retained type identities and the owning
compiler type facts. `hir/record_evolution.rs` reconstructs only the bounded
selected declaration closure, including unused records, for the existing HIR
TypeFacts owner; it retains no additional whole-project index.
The operation admits existing checked sized resource-free records while
appending only inert non-droppable fields. Post-admission checks bind the old
field prefix and preserve the record's checked ownership flags; owning match bindings,
loan roots and cleanup order remain under normal source replay. Both reconstruct
the expected canonical source independently after admission; identity guards
permit only the planned function location or new record member. Rebase compares
record shape and relocation facts before full replay. No source authority is
added. `image_facets/relationships.rs` projects bounded data-access and audit
facts from retained HIR with source, expression and evidence provenance; the
existing Project admission remains responsible for excluding unsafe sources.

`candidate/testing.rs` derives test relevance from retained HIR and explicitly
executes the declared interpreter test closure only after exact candidate replay.
Image protocol v3 selects this bounded authority at host startup; policy cannot
be changed by requests. `candidate/diagnostics.rs` retains failed intentions and
diagnostics without exposing invalid source as a checked image, and routes its
bounded literal-retag and direct-field borrow repairs through ordinary complete
candidate admission. Its private `candidate/diagnostic_borrow.rs` helper requires
an actual `SPX-T266` rejection
and only changes typed projections of named roots under byte-view constructors;
source ownership and loan validation remain the admission authority.

`candidate/publication.rs` is a separate host invocation over an existing managed
Workspace. Its in-memory Change-v1 seam acquires existing shared/exclusive
authority before candidate replay, authenticates Project and managed base sources,
and delegates the sole `ACTIVE` publication to the existing Workspace engine.
It grants neither a reusable authority token nor raw Git-source writes.

`src/project/manifest.rs` parses the bounded `semaprax.toml` profiles; its
`tables` submodule parses the extensible `semaprax.manifest.v1` table layout
and lowers it onto the same frozen profile contracts.
`src/project/external_dependencies.rs` replays the exact held Subject-v3
inventory, proves that it is precisely the per-target selected closure, and
lends only embedded canonical source to the linker. Rust dependencies remain
outside that workspace and are rendered only into the authenticated generated
SDK Cargo package.
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

The shared Unix npm publisher in `src/project/npm/publication.rs` writes through
held directories and compares the final reopened parent identity, not only its
canonical pathname, before reporting success. This detects same-path parent
replacement without cleanup or rollback; it is an observation, not atomic
publication. See [Project Manifest v2](PROJECT-MANIFEST-V2.md) for the shared
boundary and authored, unrun regression modules. Windows routes are unchanged.

Project v8 adds one closed `owned-data-api.v1` route. `src/project/public_api.rs`
derives and independently replays the sole semantic API descriptor from the
authenticated linked-HIR subject. Authentic cross-replay regressions distinguish
its retained-signature checks from digest rejection; equal descriptors alone
do not prove function-body equivalence or source provenance.
`src/project/cxx_owned_data.rs` and its bounded renderer compose that same
replayed descriptor with the existing native provider into an authority-free,
pathless C11/C++17 package. The C++ wrapper owns one thread-bound invocation
context, copies owned results before exact handle settlement, and fails stop on
cleanup uncertainty. This is a Project-v8 library surface, not a target,
publisher, loader, package-manager integration, or support promotion.
`src/project/npm/owned_data.rs` and the
owned-data Wasm lowering consume that descriptor for the npm package. The
shared `src/project/npm/owned_data_input_v8.js` admits complete input tuples
before snapshot allocation, using captured brand/buffer intrinsics. Its
historical filename and helper bytes remain unchanged; v9/v10 explicitly
select the same admission without altering v8 JavaScript for that extension.
The subsequent shared [owned npm invocation state](OWNED-NPM-INVOCATION-V1.md)
is composed by `src/project/npm/owned_invocation.rs` from small private arena,
instantiation, invocation, result and facade templates under
`src/project/npm/owned_invocation/`. It reserves busy before preflight,
rejects non-string export identities without caller-controlled coercion,
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

The lower package's private `build_script.rs` renders both owned-data build
script families. It preserves target selection and validates the package path
before line-oriented Cargo output; path text cannot introduce CR/LF directives.
See the [path boundary](PUBLIC-OWNED-DATA-API-V1.md#generated-cargo-build-script-path-boundary)
for the intentional artifact change and authored, unrun regression scope.

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

`platform-tests/owned-data-browser-v1/project` owns a separate fixed direct-Bytes
browser subject. Its Rust fixture test authenticates the Project and inline
carrier; its provisioned browser runner imports the actual generated package
into a test-owned isolated document for hostile-input and lifecycle checks.
It does not authenticate the host's source provenance or replace physical
cleanup evidence. The existing three-engine gate remains authored and unrun.

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
derivation; the Project test owns that rename proof. Selected local passes do
not establish exact-head hosted promotion. Gate selection is
documented in the [web consumer](../examples/frame-payload-web/README.md) and
[browser fixture](../platform-tests/frame-payload-browser-v1/README.md).

`tests/support/owned_frame_artifacts.rs` shares the held-Project descriptor,
provider and published-package replay between that product and the explicitly
provisioned `tests/release_archive_product_v1.rs` archive gate. The archive gate
uses caller-selected unpacked binaries outside the checkout for calculator,
read-only daemon and generated-consumer checks; the existing frame suite keeps
its interpreter/native/raw-Wasm execution lanes. Archive inventory and label
agreement establish self-consistency, not build provenance. The
[release process](RELEASE-PROCESS.md) owns selection and evidence limits.

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

Project v11 keeps nested owned-record results in a separate descriptor and
carrier family. `src/project/nested_owned_record/` owns target-neutral shape
and stable-path authentication; `src/wasm/owned_data_exports/nested_record.rs`
and `src/codegen/native_owned_data_provider/nested_record.rs` independently
replay target layouts. The npm and safe-Rust consumers validate every private
owner before copying any payload, copy the full bounded batch, settle all
owners, and publish the nested value last. No target aggregate layout or raw
owner handle enters the public API. See [Public Nested Owned-Record API
v1](PUBLIC-NESTED-OWNED-RECORD-API-V1.md).

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
read-only Project v8 descriptor/npm carrier surface. Additive Transport v6 is
a separate opt-in read-only surface over exactly Project v8-v11. The retained
manifest profile, never a request field, selects the typed descriptor and npm
carrier schema. V5 and v6 compare each independently replayed carrier's typed
descriptor binding with the separately replayed retained descriptor before
returning either. `src/project_transport/session/owned_data.rs` owns frozen v5;
`src/project_transport/session/public_api.rs` owns v6 dispatch and complete
wrapper budgeting; `src/project/npm/carrier.rs` owns profile-specific carrier
replay and descriptor authentication. These profiles do not add general patch,
filesystem, process, publication, network, persistence, or recovery authority.
The generated v6 codecs remain pure request/response codecs. Their conformance
harness executes Python and provisioned Node/TypeScript adapters as closed-
environment direct children and compiles the exact generated Rust codec into
the owning Rust harness. This is local codec evidence, not a general process,
tool-provenance, registry, or hosted-client authority claim.

## Reports and projections

Read-only commands are implemented in focused modules such as
`src/abi_report.rs`, `src/c_header.rs`, `src/cxx_shim.rs` and its bounded
`src/cxx_shim/package.rs` replay/package child,
`src/capability_manifest.rs`, `src/freestanding_object.rs`, `src/openapi.rs`,
`src/package_report.rs`, `src/plugin_manifest.rs`, `src/region_report.rs`,
`src/simd_report.rs`, and `src/ui_schema.rs`.

`src/doc.rs` is the documentation projection. It builds one model of a checked
module's declarations from the parsed program and its comments (identities,
canonical signatures without bodies, ownership modes, effects, contracts,
members, and leading-comment descriptions) and renders it as Markdown or as a
one-line `semaprax.doc.v1` document, both carrying `graph::revision`. It
reuses the canonical formatter's type, contract, and escaping writers, so a
signature is the formatter's text. `src/cli/doc.rs` owns the closed
`doc <file> [--json]` grammar and verifies before rendering. The projections
harness proves that every documented identity of a graph-carried kind is a
node of `semaprax graph` at the same revision. See
[Documentation Projection v1](DOC-PROJECTION-V1.md).

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

`package_semantic_graph` consumes that same complete capsule replay to retain
coordinate-qualified package, source, interface, import and cross-package call
facts. Its immutable consumer index is independent of a Project image. A v5
host may attach the verified graph before requests and expose read-only package
queries through the selected discovery and parallel-read paths; equal stable
IDs do not establish any Project/package relationship. No source, acquisition,
build or publication authority crosses this boundary. See
[Package Semantic Graph](PACKAGE-SEMANTIC-GRAPH-V1.md).

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
no-replace directory publication. File handles are settled before that rename;
post-publication authentication reopens the fixed inventory through the renamed
held directory and compares it with the retained identity-and-digest proof.
Previsibility cleanup is limited to the
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
- `crates/semaprax-native-rust-interop-platform/src/host_target.rs`: shared
  compile-time native target classification; scalar and owned-data package
  callers retain their narrower publication allowlist, separate from private
  Phase-A target preparation and all held-tool authority;
- `crates/semaprax-native-rust-owned-data-package`: dependency-inverted
  Project-v8 held-tool, archive, deterministic rendering, publication, and
  held-stage verification authority; it receives no source, HIR, or provider
  semantic-authentication authority;
- `src/native_settlement.rs`, `src/arc_zones.rs`, and `src/scoped_tasks.rs`:
  target-neutral proof models rather than wired runtime features;
- `src/agent_definition.rs`: the additive canonical AgentDefinition v1 compiler,
  deterministic AgentGraph v1 projection, and canonical Runtime v1 profile
  compatibility boundary; it has no language parser or execution authority;
- `src/agent_harness.rs`: the authority-free AgentDefinition/Economic Policy
  graph binding and the injected-host Runtime v1 to Economic Agent v1 handoff;
  it owns no provider, journal, approval, custody, chain, or broadcast host;
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
| Verification | `src/verify.rs`, `src/source_verify.rs`, `src/source_verify/` — `declaration/` owns the per-pass declaration checks, `iterative/` the frame machine, `oracle/` the test-only recursive cross-check, `hints.rs` the shared fix hints both verifiers attach to unknown-function, generic-argument, literal-suffix, and borrowed-view diagnostics, and `loans.rs`/`place.rs` the loan lifecycle |
| HIR | `src/hir.rs`, `src/hir/` — `ids.rs`, `nodes.rs`, and `expr_nodes.rs` own the data model; `resolve_*.rs` own AST lowering; `validation.rs` owns core validation |
| Cleanup and layouts | `src/cleanup.rs`, `src/cleanup_plan.rs`, `src/cleanup_plan/`, `src/aggregate_layout.rs`, `src/variant_layout.rs` |
| Graph and read-only analysis | `src/graph.rs`, `src/graph_cleanup.rs`, `src/call_index.rs`, `src/impact.rs`, `src/review.rs`, `src/doc.rs` |
| Semantic retention metadata | `src/semantic_retention.rs`, `src/semantic_retention/`, receipt adapter in `src/candidate_archive_store.rs` |
| Retention metadata persistence | `src/semantic_retention_store.rs`, `src/semantic_retention_store/`, explicit adapter `src/cli/retention_metadata.rs` |
| Retention registry cursor | `src/semantic_retention_registry.rs`, `src/semantic_retention_registry/` |
| Automatic candidate/draft retention lifecycle | `src/semantic_retention_lifecycle.rs`, `src/semantic_retention_lifecycle/automatic.rs`, held replay adapters in `src/candidate_archive_store.rs` |
| Single-file transactions | `src/patch.rs`, `src/patch/`, `src/patch_evidence.rs`, `src/repair.rs` |
| Managed workspace | `src/workspace.rs`, `src/workspace_*`, `src/semantic_workspace*` |
| Project, public descriptor, and daemon | `src/project/`, `src/project/public_api.rs`, `src/project_transport/`, `src/bin/semapraxd.rs` |
| Project v8 promotion observation replay | `src/project/v8_promotion.rs` |
| Exact Project semantic references | `src/project/image_reference.rs` |
| Exact Project target reuse | `src/project/target_cache.rs` |
| Immutable Project revision inputs | `src/project_revision_store.rs`, `src/project_revision_store/unix.rs` |
| Generated Rust package authority | `src/project/native_sdk.rs`, `crates/semaprax-native-rust-owned-data-package/`, `crates/semaprax-native-rust-interop-builder/`; exact Project crate inputs originate in `src/project/manifest/tables.rs` |
| Signed doctor generation store | `crates/semaprax-doctor-release/src/install.rs`, `crates/semaprax-doctor-release/src/install/` |
| Interpreter | `src/interpreter.rs`, `src/interpreter/prepared.rs`, `src/hosted_interpreter.rs`, `src/project/prepared_interpreter/`, `src/project/prepared_interpreter/trace/` |
| Native backend | `src/codegen.rs`, `src/codegen/native_*` |
| WebAssembly backend | `src/wasm.rs`, `src/wasm/` |
| Reports and offline package graph | the focused `*_report`, `package_lock`, `package_resolver`, `package_resolution_snapshot`, schema, manifest, header, and shim modules; candidate replay/conflict projection in `src/project/candidate/package_consumer_replay.rs` |
| Project dependency admission | `src/project/external_dependencies.rs` for exact ordinary Subject-v3 closure replay and `src/project/standard_dependencies.rs` for compiler-bundled packages |
| Effect-free package build and fixed-inventory publication | `src/package_build.rs`, `src/package_build/`, `src/package_build_v2.rs`, `src/package_build_v2/`, `crates/semaprax-offline-wasm-package/` |
| Private host/runtime evidence | `crates/semaprax-native-*`, `platform-tests/` |
| Standard library | `std/<package>/` Project packages, `src/project/standard_dependencies.rs` for closed bundled dependency expansion, `std/packages.json` tier and target metadata, generated `std/catalog.json`; gate `tests/project/standard_library.rs` |
| Executable evidence | `tests/`, crate-local tests, `platform-tests/`, `.github/workflows/` |

This table is the single module-level map. Other contributor documents should
link here instead of copying it.

### Module size

A Rust source file may not exceed 1500 lines unless `tests/module-size-budget.tsv`
records it, and a recorded file may not grow past the size it was recorded at.
`tests/module_size.rs` enforces both, and fails on entries that no longer exceed
the limit so the ledger shrinks with the code.

The constraint is a reading cost, not a style preference. A module that holds
several thousand lines forces every reader — and every agent context window — to
pay for the whole file to reach one item, and it makes unrelated edits collide in
one blast radius. Prefer splitting an oversized module into submodules that each
own one concern. An inherent `impl` may be split across several `impl` blocks in
different modules of the same crate, which is usually the cheapest behaviour-
preserving cut for the largest files here.

Two files exceed the budget because each is a single function whose locals are
shared across every branch — `src/hir/resolve_expr.rs` and
`src/economic_agent/agent_execute.rs`. Cutting those requires extract-method with
new signatures, which is a behavioural change and needs its own review; it is not
a relocation.

### Integration test harnesses

Cargo builds every top-level file in `tests/` as its own binary, and each one
statically links the whole compiler. The test code is a small fraction of that:
a typical file contributed ~1.5 MiB to a ~42 MiB binary. With one file per
subject the suite reached 393 binaries and over 11 GiB of executables, which
exhausted disks and dominated CI time.

Tests are therefore grouped into harnesses. A harness is `tests/<group>.rs`
declaring one module per former file, with the bodies in `tests/<group>/`:

```rust
#[path = "<group>/<name>.rs"]
mod <name>;
```

The `#[path]` is required: in a test crate root, plain `mod foo;` resolves to
`tests/foo.rs`, not to the subdirectory.

**Add a new test as a module of the harness that owns its subject.** A new
top-level file links the compiler again and gives back the saving.

Four things follow from sharing one binary, each enforced or already paid for:

- Relative paths in `include_str!`, `include_bytes!`, and `#[path]` resolve
  against the declaring file, so a body one directory deeper needs `../`.
- Modules share a process id. A temporary fixture root built from a literal
  prefix plus `process::id()` collides if two modules use the same prefix, so
  every prefix in a harness must be distinct. `tests/harness_isolation.rs`
  checks this.
- A test that re-invokes its own binary with `--exact <path>` cannot be a
  module: merging prefixes the path and the selected case silently never runs.
  Such a file stays a top-level target. `tests/harness_isolation.rs` checks this
  too, keyed by path.
- Two modules must not `#[path]`-load the same `tests/support/*.rs`. Inside one
  crate that compiles the file twice and yields unrelated copies of its types;
  `clippy::duplicate_mod` rejects it. Declare the support module once in the
  harness root and refer to it as `crate::<name>`.

A file stays a standalone target when CI or a script selects it with
`--test <name>`, when another test reads it as text, or when it re-invokes its
own binary. Documentation cites tests by path and `tests/documentation.rs` fails
on an unresolvable link, so citations move with the code, and a documented
`--test <old> <filter>` becomes `--test <group> <module>::<filter>` — the
`module::` form matters, since a bare second positional is read as a second
libtest filter.

### Source-locked contracts

Some gates assert over the *text* of a module: they read a `.rs` file with
`include_str!` or a path read, then require substrings, forbid others, count
occurrences, or slice the region between two markers. `tests/economic_agent_v1.rs`
scans for ambient authority this way, `tests/native_rust_interop_ci_contract.rs`
and `crates/semaprax-native-rust-interop-platform-sys/src/tests.rs` bind physical
authority boundaries, and `implementation/tests/ledger_capacity.rs` pins layout
constants.

Splitting a module silently narrows every such gate: the text moves to a sibling
file, the assertions still pass against the smaller root, and the coverage is
gone with nothing failing. When a split moves audited text, join the root and its
submodules — with `concat!(include_str!(..), ..)` or a path-reading helper — so
the contract binds the complete module. Use **original source order**, not the
alphabetical order `mod` declarations are formatted into, whenever the contract
slices a region between two markers. `tests/source_locked_contracts.rs` fails
when a reader binds a module root but not its submodules.
