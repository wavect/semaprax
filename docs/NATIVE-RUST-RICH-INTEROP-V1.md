# Native Rust Rich Interoperability v1

Status: local additive bootstrap for [RI-01](https://github.com/wavect/semaprax/issues/359).
It defines compiler input and generated artifacts. The checked-in scalar
fixture proves one generated native round trip; it does not claim public Rust
support or completion of this contract's broader evidence gates.

## Scope and relationship to Native Rust Interoperability v1

This contract is a separate profile from
[Native Rust Interoperability v1](NATIVE-RUST-INTEROP-V1.md). The existing
profile continues to own its scalar SDK, descriptor schemas, same-thread and
non-reentrant bridge, and its published local evidence. A rich-binding plan
must never be accepted as a v1 Spec, descriptor, bundle, or Project subject.

RI-01 starts with a closed bootstrap: one selected Rust package exposes
ordinary Rust functions, an internal plan names those functions, and generated
code provides the only C-compatible boundary. The Rust package does not carry
SEMAPRAX annotations, FFI exports, or a guessed Rust symbol ABI. General Cargo
discovery, arbitrary crates, retained references, traits, async, resources,
aggregate values, dynamic loading, and registry publication are outside this
version.

## Names and identities

The canonical document schemas are:

| Document | Schema | Purpose |
| --- | --- | --- |
| rich interop Spec | `semaprax.native-rust-rich-interop-spec.v1` | selected target, package, bindings, and generated declarations |
| BindingPlan | `semaprax.native-rust-rich-binding-plan.v1` | internal typed compiler input after the Spec is authenticated |
| descriptor | `semaprax.native-rust-rich-interop-descriptor.v1` | replayable generated-boundary facts |
| bundle | `semaprax.native-rust-rich-interop-bundle.v1` | exact generated-file inventory |

Their SHA-256 digest domains are, in the same order,
`semaprax.native-rust-rich-interop.spec.v1\0`,
`semaprax.native-rust-rich-interop.plan.v1\0`,
`semaprax.native-rust-rich-interop.descriptor.v1\0`, and
`semaprax.native-rust-rich-interop.bundle.v1\0`. A renderer writes compact
JSON with one trailing LF, uses the ordered keys in this document, and emits
arrays in declared plan order. It rejects duplicate keys and values outside a
closed enum before allocating a generated artifact.

Every generated Semaprax declaration has a persistent `@id` from
`binding.semaprax_id`; the plan rejects duplicate IDs. A Rust path is data,
not an identity: renaming a Rust path changes the package and plan digests but
does not silently allocate a new Semaprax API identity. A future automatic
indexer must reproduce this identity selection or require an explicit migration.

## BindingPlan

`BindingPlan` is compiler input, not a capability or an instruction to execute
a process or foreign function. It is constructed only after the selected
package bytes, target profile, and authored Semaprax source are held and
authenticated. The implementation then validates it against those held inputs,
renders all artifacts from it, independently replays its descriptor and file
inventory, and only then uses ordinary build authority.

The canonical plan has these ordered top-level keys:

```text
schema, package, target, bindings, limits, nonclaims
```

`package` contains `name`, `version`, `source_digest`, and
`cargo_lock_digest`. `target` contains one admitted `triple` and
`backend: "native-c11-static"`. A plan never inherits a host target, Cargo
configuration, filesystem path, environment variable, or tool from ambient
state.

Each `bindings` item has these ordered keys:

```text
semaprax_id, semaprax_name, rust_path, receiver, arguments, result,
substitutions, effects, failure
```

The fields have the following closed meanings:

| Field | Required meaning in v1 |
| --- | --- |
| `semaprax_id` | persistent public declaration identity |
| `semaprax_name` | generated declaration spelling; unique in its generated module |
| `rust_path` | fully selected ordinary Rust item path within the exact package |
| `receiver` | `none`; methods are deferred |
| `arguments` | authored positional order; each row has `name`, `type`, and `mode` |
| `result` | one `type` and `mode` |
| `substitutions` | explicit ordered concrete type substitutions; `[]` for nongeneric calls |
| `effects` | sorted declared Semaprax effects required by the foreign action; `[]` grants none |
| `failure` | one closed mapping for semantic division failure, Rust `Result::Err`, and panic |

For the bootstrap, `type` is one of `i64` or `bool`; `mode` is `copy` for every
argument and result. The plan rejects every receiver, borrowed, owned,
aggregate, raw-pointer, resource, trait-object, `async`, inferred generic, or
unlisted substitution shape. This makes the first fixture's `add(i64, i64)`
and `checked_div(i64, i64)` precise without implying an aggregate or ownership
ABI.

Arguments are evaluated and staged left to right. They transfer together only
at the generated call boundary. A generated thunk may not call Rust until all
validation, conversion, required-effect admission, and failure output staging
have succeeded. Its cleanup follows the existing compiler-owned ordered cleanup
plan; a cleanup error cannot replace a previously selected failure.

## Failure and effect boundary

`failure` records three distinct outcomes:

| Case | Required result |
| --- | --- |
| Semaprax checked division failure | the selected Semaprax semantic failure; no success result is written |
| Rust `Result::Err` | the plan's named foreign-error status; no success result is written |
| Rust panic while inside the generated Rust adapter | the plan's named panic status after the adapter catches the unwind; no success result is written |

The bootstrap fixture must make these cases observably distinct. Panic payloads,
Rust error payloads, paths, pointers, and tool output are not diagnostic data.
An unwind crossing the C-compatible thunk is forbidden. Abort, OOM, signal and
process failure remain outside the recovery claim.

Effects name an already-declared Semaprax effect and are checked at the selected
generated declaration and every caller. Listing an effect in a plan does not
grant it to the adapter, Cargo, generated code, or a Rust crate. The plan also
does not grant filesystem, network, process, home, secret, key, wallet, or
signing authority.

## Generation and target selection

One accepted BindingPlan generates both the Semaprax declaration projection and
the Rust adapter. The declaration projection retains the plan's stable IDs,
parameter order, copy modes, declared effects, selected target, and failure
facts. The adapter calls selected monomorphized Rust items and exposes only
generated C-compatible thunks. The accompanying C11 artifact calls those
thunks through an exact generated header. Neither artifact resolves a guessed
Rust symbol or exposes a `repr(Rust)` value.

The native backend may select `native-c11-static` only after the exact plan,
package, generated adapter, C header, C artifact, descriptor and bundle all
replay. Ordinary interpreter and Core Wasm routes reject a rich binding before
foreign execution and before incrementing any fixture counter. They do not
simulate a result. A plan that selects any other backend is invalid in this
version.

The initial fixture package is checked in with `add` and `checked_div` only.
It is an executable bootstrap input for RI-01, not an automatic-indexing
interface or a supported general package workflow.

## Diagnostics

These identifiers are reserved after checking the current registry; their
implementations must use the exact messages chosen by the owning harness.

| Code | Condition |
| --- | --- |
| `SPX-B117` | rich BindingPlan is malformed, noncanonical, unauthenticated, or disagrees with held Spec/source/package facts |
| `SPX-B118` | selected rich target, value class, receiver, substitution, effect, or backend is outside this profile; refusal occurs before foreign invocation |
| `SPX-B119` | generated rich adapter selects the declared Rust-error or caught-panic status and suppresses success publication |
| `SPX-B120` | generated declaration, adapter, descriptor, or bundle fails independent replay |

Existing v1 diagnostic identifiers retain their current meanings. This version
does not reclassify `SPX-W114` or v1 bridge failures.

## Required evidence before implementation status changes

The owning native-Rust harness must retain all of the following at the same
revision:

1. A fresh Rust consumer executes Rust → Semaprax → Rust → Semaprax → Rust
   from authored sources and generated artifacts, with no handwritten adapter.
2. `add`, zero and negative arguments, semantic division failure, Rust
   `Result::Err`, and panic take their distinct selected paths; all failures
   prove that no success output was published.
3. Canonical source round-trip, explicit import identities, selected HIR facts,
   descriptor generation, and injected descriptor/plan disagreement are
   covered by stable tests.
4. Unsupported target and signature cases reject before foreign invocation; a
   fixture counter remains zero. Existing scalar SDK and owned-data bytes and
   refusal tests remain unchanged.
5. Native, interpreter, and Wasm target selection is asserted: native executes
   only the admitted profile, while interpreter and Wasm refuse before the
   foreign call.

The bootstrap physical test is
`generated_rich_fixture_adapter_round_trips_without_a_handwritten_host` in the
native-Rust builder harness. It compiles the ordinary fixture crate, generated
adapter, generated C11 bundle, and a fresh Rust consumer; that consumer proves
positive, zero, and negative `add` calls through Rust → Semaprax → generated
adapter → Rust. The same generated adapter maps the fixture's selected
`checked_div` `Result::Err` to the declared import status, lets the v1 bridge
report a caught adapter panic, and keeps semantic division inside Semaprax.
`rich_fixture_plan_is_canonical_and_rejects_a_generated_method_injection` and
`rich_fixture_preserves_canonical_source_and_selected_stable_id_facts` cover
the canonical plan/source, declaration identities, descriptor disagreement,
unsupported target/signature, and pre-callback refusal rows. These are local
bootstrap gates; automatic indexing and any general Rust package workflow
remain outside this profile.

This does not make any target, generated package, Rust ABI, Cargo integration,
or ecosystem binding supported.

## Experimental opaque-owner rendering (RI-05)

The unpublished builder's `prepare_opaque_owner_native` is a separate pure
rendering seam. It accepts checked source and one function identity, validates
HIR and independently replays the attached cleanup plan, then returns a C11
header, C11 function implementation, and Rust adapter. It grants no build or
publication authority and is not an extension of the scalar v1 SDK wire format.

This closed profile admits one opaque resource with a real imported,
infallible destructor; one ordinary Rust associated constructor `(i64) -> Self`;
and one consuming method `(Self, i64) -> bool`. Both Rust declarations have
explicit paths and `failure infallible`, with no effects. The destructor's
logical import is bound to Rust `Drop`; `drop trivial` is refused. The selected
Semaprax function has at most eight `i64` parameters and an `i64` or `bool`
result. Its body admits literals, immutable bindings, whole places, these two
native calls, `if`, checked `i64` division, and a bounded acyclic closure of
ordinary Semaprax helpers. Helpers may take `i64` values or own the same resource
and may return that resource, `i64`, or `bool`. Public SDK exports retain their
scalar signature. Other expressions, contracts, resource projections, borrowing,
nested statement scopes, and recursive or generic helpers are refused.
The renderer bounds the closure to 32 functions and each body to 256 expressions,
eight parameters, and 64 cleanup slots.

The C carrier consists of three `uint64_t` fields in order: context identity,
slot generation, and slot index. Rust retains the actual object in a table;
no `repr(Rust)` object or allocation crosses into C. Contexts belong to one
thread, have process-unique non-repeating identities, and hold at most 32 slots.
A slot is reused only with a fresh generation. Wrong-context, absent-slot,
stale-generation, and already-consumed carriers fail before the target method
or destructor. These are bearer tokens, not cryptographic capabilities. Raw C
callers retain the ordinary validity obligations for pointer arguments.

Constructor output is written only after the Rust value has entered the table.
A consuming method validates and removes its slot once before calling Rust.
Constructor, method, and destructor unwinds are caught within Rust; panic
payloads are forgotten and never become diagnostics. Abort and allocation
failure are outside recovery. A generated safe Rust caller creates a context,
calls the generated Semaprax function, and checks that context closure finds no
live owner. A leaked owner causes closure to refuse; closure never substitutes
an invented finalizer order for the compiler plan.

Source verification, HIR validation, and their recursive test oracles preserve
owned constructor results and reject reuse after a consuming native argument.
Only ownership-bearing native calls use the existing atomic cleanup-call
protocol; old scalar imports retain their prior cleanup projection. Arguments
stage left to right, the declared call commit clears all staged argument
liveness; owned native parameters retain `consumes_on_failure: true`, and successful constructors initialize their result epoch. Failure
while evaluating a later argument finalizes the earlier staged owner. The C
implementation follows the validated cleanup CFG and emits each
`finalize_in_order` vector as received. A cleanup failure cannot replace a
previously selected status; result publication occurs after successful cleanup.

Ordinary helper calls use the same checked argument commit boundary. Each
helper initializes owned parameters from the canonical cleanup entry state and
executes its independently validated cleanup CFG. An owned result stays in the
plan's provisional-result slot until all non-result cleanup succeeds. A caught
destructor failure prevents publication and disposes the provisional result
last without replacing the selected failure. No downstream sorting or inferred
cleanup order is introduced.

This experimental ABI uses closed statuses: zero success, 2 caught panic,
3 carrier or pointer-shape refusal, 4 capacity, 5 live owner at context close,
6 table re-entry refusal, 7 internal liveness invariant, and 8 checked division
failure. These are not scalar v1 status-domain ordinals or Rust domain errors.

The physical gate is builder test
`generated_opaque_owner_source_and_cleanup_execute_physically`. It compiles
actual generated Semaprax C11 and Rust glue at C `-O0` and `-O2`, checks counted
Rust destruction in canonical order on success, late argument failure,
constructor panic and method panic, and retains poisoned result slots on
failure. Stale and foreign-context carriers are rejected without taking the
live value. Compiled missing-drop and flipped-method controls must fail the
same consumer assertions. The source/HIR harness retains move and tampered
call-commit regressions. The pure renderer alone does not establish Project
publication; the indexed package route below adds that separate boundary.
Standard Rust crate coverage, borrowed owners, general resource methods, hosted coverage, full quality-gate
passage remain outside this local bounded acceptance.

The focused `owner_return_` selector additionally covers constructor → checked
owner-returning helper → forwarding helper → consuming import, both in the pure
renderer and a generated indexed Project package. Its counted Rust fixture
checks exact cleanup order at C `-O0` and `-O2`, constructor/method/destructor
panics, sticky division failure, and poisoned result preservation. Compiled
missing-drop and premature-publication controls must fail the same assertions.
Canonical source/graph round trips retain owned commits and source move
diagnostics. These are local focused gates; owner-valued public SDK exports and
general rich conversions are not admitted by this helper extension.

Graph v56 is selected by an ownership-bearing native Rust import. Its import
result `type` is the resource's persistent declaration ID and its
`ownership_mode` is `own`. Native call expressions retain the same resource
identity in `result`, alongside the ordinary nominal `type_id`. It includes
v52-v55's native-path and selected-index/receiver fields (null when absent).
Scalar-only modules keep their prior schema and bytes. Graph evidence routes
that do not admit this projection refuse v56 explicitly.

### Indexed owner Project package (experimental)

The existing explicit `indexed-project` builder/CLI route also admits the closed
owner profile above. Two short `import rust selected` declarations name a
public nongeneric Rust struct's `(i64) -> Self` constructor and consuming
`(self, i64) -> bool` method. The source declares a resource with that Rust
struct's final name and an imported infallible destructor. Both imports must
select the same exact package, Cargo alias, index, target, source bytes, and
stable compiler. The index must include the public struct in each method's
complete type closure. The linked resource ID and full Rust type path must
agree across both imports. Other owner signatures still refuse.

Binding facts enter ordinary Project source checking, HIR, cleanup replay,
and graphing before generation. The linker retains the resource and lifecycle
declaration facts and admits only the destructor attached to the matching
selected constructor/method pair; unrelated ordinary imports still refuse.
Resource retention follows the exact selected function closure and leaves the
record/variant-only owned-data and Wasm type closure unchanged. Package
preparation checks the exact selected signatures against HIR again. Stable
rustc then checks explicit function types against the unchanged selected Rust source; matching metadata alone cannot
admit a method with a different actual result type. The source remains within
the existing bounded self-contained single-file Rust profile, so this is not
general Cargo dependency compilation or a claim of real `regex` crate support.

The additive package schema is
`semaprax.native-rust-opaque-owner-sdk.experimental.v1`; scalar SDK and
owned-data v1 formats are unchanged. A package contains `Cargo.toml`, `build.rs`,
`lib.rs`, `owner.h`, the current-target native archive, `descriptor.json`, and
`semaprax.native-rust-sdk.json`. The descriptor binds the authenticated Project
subject, resource and Rust type identities, selected package/index/imports,
and exact generated C, header and Rust hashes. The manifest binds all six other
files. `spx_owner_call` is the generated safe call entry for the single selected
export. No user-written Rust wrapper implements constructor or method glue.

The publisher acquires explicit compiler and archiver authority, verifies the
selected stable compiler through held direct-rustc discovery, compiles the
Rust signatures before publication, and reuses the existing held C archive
and seven-file no-clobber publisher. It verifies the complete published
inventory through the original parent handle. Uncertain compiler output
retains its private stage for reconciliation. The authenticated Project route
performs its final source recheck on success and refusal.

The focused `indexed_owner_` selector exercises an actual generated Project
package and Rust consumer, a freshly rebound flipped Rust implementation that
must fail the same consumer, stale package bytes refused before publication,
and an indexed/actual signature mismatch refused by rustc. Source/graph tests
retain repeatability and a stable use-after-move diagnostic. This extends the
experimental Project and CLI seam; arbitrary owner signatures, real-world
crate families, borrowed methods and hosted coverage are separate claims.

### Bounded native String rendering (experimental)

`prepare_owned_string_native` is a separate pure renderer for a checked
`(i64) -> string` native constructor and consuming `(own string, i64) -> bool`
native function. The Semaprax spelling is `string`; the Rust value is exactly
`std::string::String`. Ordinary helper parameters keep the existing implicit
owned normalization (`value: string`). Selected metadata may bind the exact
`alloc::string::String` signature; unsupported argument widths are refused.
The generated Rust is type checked against the actual target functions when
compiled. Metadata alone grants neither execution nor publication authority.

The actual Rust String stays behind the context/generation/slot carrier and is
moved into and out of its table without copying its allocation. Owning Semaprax
string place reads create the fresh allocation required by core String semantics;
the original owner remains live until its canonical cleanup. Clone allocation
failure aborts, matching the core runtime, rather than inventing an unplanned
recoverable failure edge. No Rust object,
allocation layout, or arbitrary UTF-8 pointer crosses C. Rust's String type
provides valid UTF-8; this route does not decode raw caller-provided byte spans.
Length and capacity are each bounded to 4096 bytes after construction. This is
a retained-value bound, not a sandbox on allocations within trusted Rust code.
Bridge context and slot storage use fallible reservation; slot reservation
occurs before constructor effects. A rejected constructed value is dropped
before returning refusal and leaves the result slot unchanged.

The source/HIR ownership checks, canonical `core.string.drop` lifecycle, helper
cleanup plans, and provisional result publication rules remain authoritative.
The two `owned_string_` tests cover canonical source/graph replay, selected
signature binding, width and use-after-move refusals, generated C at `-O0` and
`-O2`, pointer preservation, and real deallocation observed by an allocator.
They exercise late argument failure, constructor/consumer panic, size refusal,
stale/foreign carriers, and injected bridge-reservation/admission failure. An
isolated clone-allocation control must terminate with an abort signal.
A compiled missing-cleanup control must fail the same consumer assertions.

These programs select graph v57, which adds native `string` result/parameter
meaning to v56. Older programs retain their prior schema and bytes; evidence
routes without this admission refuse v57. This String renderer does not publish
a Project/CLI package. Later sections add owned Option/Result payload conversion
and focused failure injection. General public owner-valued exports and graceful
allocator-abort recovery are outside this bounded profile. Existing scalar and
owned-data v1 contracts are unchanged.

### Scoped generated borrowed views (private RI-06 seam)

The private borrowed-input fragment has an additive `with_str_view` operation
for the exact `Regex::as_str`/byte-Regex `as_str` relation. A sibling fragment
inspects an ordinary owned `url::Url` through `Url::as_str`. A typed
`for<'owner> fn(&'owner Owner) -> &'owner str` method pointer verifies each
library relation. Neither renderer accepts arbitrary reference signatures or
constructs a reference from a C carrier.

The callback receives a typed view whose lifetime cannot escape its invocation.
Its higher-ranked bound prevents returning or storing the view, including in an
escaping async future. Rust rejects owner moves/drops, replacement and exclusive
mutation overlapping the invocation. These are Rust compiler checks on the
private generated fragment, not newly admitted Semaprax source loan rules or
checkpoint diagnostics. Owner extraction and mutable access resume after the
callback. The receiver remains owned by its adapter throughout the callback.

A separate guard belongs to the invocation rather than the view. Forgetting the
view cannot release that guard. All same-owner re-entry is refused before the
selected library method is called; an independent owner is unaffected. Callback
unwinding releases the guard. `with_exclusive` requires an exclusive Rust borrow
of the adapter; no alias or interior-mutability exception is inferred.

The focused `borrowed_view_generated_regex_url_scope_and_negative_controls`
gate compiles and executes generated fragments against pinned real Regex and
Url packages at Rust `-O0` and `-O2`. It compares returned pointers with the
original owner's storage, counts zero allocator calls during a Url view, and
checks target counters before and after rejected re-entry. It compiles deliberate
removed-guard and introduced-copy controls and requires their consumer
assertions to fail. Separate compiler fixtures cover drop, consume, replacement,
exclusive mutation, temporary escape, stored escape and async escape. These
measurements are local and noncanonical; they do not promise zero allocations
inside every Regex operation.

This remains private generated Rust evidence. It does not add a borrowed C ABI,
selected Project publication, native Semaprax loan lowering, durable checkpoint
admission, carrier fuzzing, Miri, sanitizer or hosted evidence. The existing v1
non-reentrant policy, scalar profiles and owned-data snapshots remain unchanged.
RI-06 stays open until those integration and acceptance gaps are closed.

### Closed owned Option/Result rendering (experimental)

`prepare_owned_container_native` is an additive pure renderer for exactly
`Option<string>` and `Result<string, i64>` native results. It accepts one
`(i64) -> container` constructor and one consuming `(own container, i64) -> bool`
import, plus the existing bounded acyclic Semaprax helper closure and scalar
public entry. Source and HIR admission require a matching native constructor
import; ordinary generic String programs retain their prior refusal. The generated Rust function pointers require the corresponding
standard Rust enum and actual String type. Nested containers, different payload
or error types, and selected-index/Project publication are outside this route.

The closed C carrier has a byte tag, seven zero reserved bytes, an i64 domain
error and an opaque String owner token. Option uses None=0/Some=1; Result uses
Ok=0/Err=1. Inactive owner fields must be zero. Some/Ok require error=0 and a
live context/generation/slot token. Rust matches real enum values to construct
or consume this carrier; it never reinterprets a Rust enum layout. None and
Err retain no owner-table entry. Domain outcomes return bridge status zero;
allocation, panic, stale carrier and protocol refusal remain separate statuses.
Strings retain the existing 4096-byte length/capacity bounds and owning-side
allocation/deallocation. Moving a container does not clone its String payload.

The compiler's existing conditional cleanup inventory and plan remain
canonical. Generated code authenticates a complete tag before conditional
entry or initialization, performs TransferVariant and CallCommit in their
existing order, and executes only the named active-case finalizer. Its whole
carrier initialization bit records storage availability, not an invented
payload obligation for None/Err. Helper results stay provisional until all
non-result cleanup succeeds; a cleanup failure disposes the active provisional
payload once without overwriting an earlier selected failure.

The two `owned_container_` selectors cover canonical source/graph replay,
use-after-move and closed-type refusals, checked helpers, real generated C at
O0/O2, empty/bounded Strings, Some/None/Ok/domain Err, signed error extremes,
raw pointer preservation, stale/forged/cross-context carrier refusals before
target effects, constructor/consumer panic and poisoned output preservation.
Injected pre-target reservation, result admission and cleanup failures retain
exact deallocation traces. A separate late-argument failure exercises cleanup
before CallCommit. Compiled missing-drop and domain-as-status controls must
fail the same assertions; a different actual Rust parameter width must fail
Rust type checking. Fixture allocation instrumentation is local evidence and
not a replacement for sanitizer or Miri coverage.

These imports select graph v58, adding owned container results to v57. Prior
scalar, opaque-resource, String and owned-data v1 schemas and runtime templates
remain separate. This does not complete RI-05: selected container publication,
general/nested conversions, full primitive-width and failure-injection matrices,
public owner-valued exports, fuzzing, sanitizers, Miri and full-profile evidence
remain outstanding.

#### Focused container safety evidence

The two `owned_container_safety_` tests separate Rust-only carrier validation
from the generated C boundary. A real fault allocator refuses context/table
reservation before target effects or output publication. The owner table is
filled to its bound, a slot is reclaimed with a new generation, and live-owner
context closure is refused without finalization. Each closed type exercises
1536 initialized hostile carriers, unchanged target counters/output, exact
payload deallocation counts, and five incompatible actual Rust integer widths.
This is bounded hostile-input coverage, not a broad fuzz campaign.

The physical Semaprax fixture runs with generated C instrumented by the
configured Clang's AddressSanitizer and UndefinedBehaviorSanitizer at O1.
An isolated heap-use-after-free child must be detected, proving the sanitizer
runtime is active. This gate explicitly links the runtime reported by that
same Clang because rustc uses `-nodefaultlibs`. Rust code and its standard
library are not sanitizer-instrumented. LeakSanitizer is disabled; existing
exact allocation/deallocation assertions remain active. Local Apple Clang 21
and Homebrew rustc 1.98 evidence passed 2/2 without ignored cases. Miri is unavailable
in the configured toolchain and no Miri result is claimed.

### Closed nested domain result (experimental)

The native container renderer additionally admits exactly
`Result<string, Option<i64>>` when a matching native constructor import is
present. This is an owned outer Result with a nested drop-free domain error.
It does not admit a String owner below multiple active-case guards, or general
recursive container types.

The C carrier remains 40 bytes and explicitly names an outer tag and a domain
tag, followed by six zero reserved bytes, an i64 error value, and the opaque
String owner token. Outer Ok=0 requires domain_tag=0/error=0 and a live payload.
Outer Err=1 requires a zero payload; domain None=0 requires error=0, and domain
Some=1 carries the full i64. Other tags and noncanonical inactive fields refuse
before target effects. The Rust adapter reconstructs the actual nested enum
with typed matches. No Rust enum layout crosses the C boundary.

Canonical cleanup still has exactly the outer Ok String obligation. Err(None)
and Err(Some(value)) allocate no owner-table payload. The nested error changes
neither transfer order nor result publication, and never becomes a bridge
status. These source programs select graph v59. Earlier flat carrier generation
and all frozen v1 package contracts retain their existing paths.

The `owned_nested_` source/graph and physical gates cover roundtrip, move refusal,
unsupported shapes, Ok(String), Err(None), Err(Some), signed error extremes,
invalid nested tags and noncanonical None, real generated C at O0/O2, exact
payload deallocation traces, and a late argument failure. A compiled control
that collapses Some(error) into None must fail the same assertions. The older
container fixture also retains empty/bound checks and constructor/consumer
panic paths. This remains an experimental renderer, not selected container
Project publication or complete RI-05 acceptance.

### Frozen owned-data v1 compatibility evidence

The owning package test `owned_v1_frozen_bytes_and_physical_output_copy` pins
complete generated text, descriptor, and manifest lengths/SHA-256 digests to
pre-RI-05 commit `4dd73795021564dcc6d90ac6505b2814c1818cd1`. The checked-in fixture
was captured by compiling that commit's unchanged renderer sources, not by
recording the current renderer's output. It covers five declared target
renderings and the owned-bytes, Option-owned-bytes, and Result-owned-bytes-i64
v1 results. The manifest uses fixed archive input; these checks do not establish
reproducible native archive machine code or physical execution on five targets.

The same focused test physically runs the local generated SDK at O0/O2 with a
provider owning real Vec allocations. The copy destination must differ from the
provider allocation. The provider overwrites and frees its storage before the
host observes success; the host's returned bytes must remain intact. Empty and
65536-byte payloads, non-UTF-8 bytes, None, and domain Err are exercised with copy,
drop, and close counts. A successfully compiled skipped-copy control must fail
the copy-count assertion. Local rustc 1.98 evidence passed 1/1, zero ignored.
This adds compatibility evidence without changing v1 renderer/runtime bytes.

### Authenticated owner admission before transfer

Calls carrying a native opaque owner, String, or an admitted owned container
stage every argument left to right. CleanupPlan v14 adds a distinct
`owner_admission` status lane for the exact checked call, before its existing
atomic `CallCommit`. Admission performs only immutable carrier validation; it
cannot invoke target code, allocate a result, or consume an owner. A refusal
selects adapter status 7 in `semaprax.native-rust-owner-admission.v1`, with
retryability false, and follows the ordinary planner exit from the still-owned
argument epochs. The exact canonical finalizer vector is retained and replayed;
the C renderer neither reconstructs nor reorders it. A cleanup error cannot
replace that selected refusal. No result is published on this path.

On admission success, the original atomic commit and target operation status
remain separate. A target that consumes its arguments and then panics does not
restore their ownership to the caller. Defensive checks remaining at commit
protect compiler invariants; no target code or state mutation runs between
admission success and the all-at-once live-bit transition.

Independent HIR replay authenticates the admission source, callee, branch,
precommit position, and rollback finalizers. Programs with this protocol select
Graph v60; constructor-only programs and profiles without owner admission keep
their prior graph/cleanup identities. Frozen owned-data v1 artifacts and closed
trace/evidence routes do not gain implicit admission of the new lane.

`owned_transfer_two_arguments_staging_atomicity_and_failures` executes checked
Semaprax helpers with two owned Option<String> or Result<String,i64> arguments.
The source roundtrip preserves the graph and one commit names both source
slots. Physical O0/O2 cases count consumption and finalization for success,
inactive payloads, first/second constructor panic or reservation failure,
oversized second output, and a late scalar argument failure.

Injected refusal at either carrier validation must produce the canonical
`[second, first]` drop order, zero target calls, unchanged output, and successful
context closure without test-side token release. An injected cleanup status
must leave refusal status 7 selected. A compiled premature-transfer control
must fail this finalizer oracle. Forged status removal, branch replacement,
and swapped rollback finalizers are rejected by independent HIR validation.
This bounded gate does not establish full RI-05 acceptance.

Local focused evidence: 1 passed, 0 failed/ignored, 191 filtered, on arm64 macOS
with Rust 1.98.0 and Apple Clang 21. The exact builder lib selector above used
one Cargo job, debug info disabled, incremental compilation disabled, offline
locked resolution, and `SEMAPRAX_ARCHIVER=/usr/bin/libtool`. The first compile
identified a missing cache-codec lane arm; the additive tag-2 correction was
included in the passing retry. No hosted, full-profile, Rust sanitizer, or Miri
coverage is claimed by this gate.

### Generated safe context lifetime and real archive compatibility gates

The experimental opaque renderer exports `SpxOwnerContext` and affine
`SpxOwned<'context>` in its generated Rust adapter. Owners borrow the context;
`close(self)` consumes it. Downstream Rust cannot close a context with a live
owner that is subsequently used, reuse a closed context or consumed owner,
implicitly copy an owner, escape its context, or move the context across a
thread. The facade adds no ambient authority and exposes no raw-handle getter.
The raw C interface remains separately checked at runtime. This is a public
generated Rust API guarantee; it does not introduce Semaprax context syntax.

`owned_context_public_lifetimes_and_isolated_panic_policy` passed 1/1 locally,
zero failed/ignored, 193 filtered. It builds a separate consumer crate, observes
counted physical construction/consumption/drop, and checks six rustc refusal
cases (E0505/E0382/E0597/E0277). An isolated unwind child reports one Drop panic,
returns status 2, and closes the context. A separately compiled `panic=abort`
child terminates with SIGABRT and publishes no success output. A possibly
executed finalizer is never retried; a refused or uncertain raw state remains
quarantined. These guarantees do not contain foreign UB or process-global
panic hooks.

`owned_v1_historical_archive_bytes_and_linked_copy` passed 1/1 locally, zero
failed/ignored, 60 filtered. Immutable pre-RI05 package production source at
`4dd73795021564dcc6d90ac6505b2814c1818cd1` produced the pinned expected digests;
current production produced identical bytes for all seven package files,
including the 3048-byte native archive. Both platform crate source trees and
manifests were verified unchanged from that baseline. The comparison shares
current locked dependencies, Apple Clang 21/libtool, and arm64 macOS; it is not a
claim about historical toolchain binaries or other native targets.

The actual archived C provider allocates, copies to distinct host storage,
overwrites/frees its allocation, and counts close/drop/copy. A linked Rust
consumer passes empty, three-byte and 65536-byte results. An independently
compiled flipped provider changes the archive and fails the payload oracle.
The earlier five-target generated-text fixture remains unchanged. This new
archive fixture is selected only on arm64 macOS; it adds no runtime v1 changes.
Historical direct-rustc capture also passed 1/1, zero ignored, 60 filtered.

### Bounded native UTF-8 ingress

The experimental generated String adapter exports
`spx_owner_string_from_utf8(context, data, u64_length, out_owner)` and a signed
length entry. These are explicit native C conversion services, not additional
Semaprax syntax or a general Rust layout projection. The caller supplies valid
input/output ranges under the documented unsafe pointer contract. Nonempty
null input and malformed UTF-8 refuse with status 3. Negative signed lengths,
lengths above 4096, failed host-width conversion, full owner tables and fallible
allocation refuse with status 4, without publishing an output owner.

Admitted input is copied into a Rust-owned String allocation. Table reservation
and String allocation are fallible; the slot is initialized only after complete
validation and copying. The original C input can then be overwritten. Allocation
and deallocation remain on Rust's side, using the same context/generation and
consuming-method protocol as existing String owners. No UTF-8 transmute or
unchecked decoding is used.

`owned_utf8_native_ingress_widths_allocation_and_hostile_controls` passed 1/1,
zero failed/ignored, 194 filtered, on arm64 macOS with the configured Rust
1.98.0/Apple Clang 21 tools. O0/O2 C callers exercise empty input, a multibyte
scalar, the 4096-byte limit, overlong/surrogate/out-of-range/truncated UTF-8,
null input, i64 signed extrema and u64 upper limits. The consuming native Rust
method observes the original content after input overwrite and a distinct
allocation; the large allocation's deallocation is counted once. The gate also
executes 512 deterministic bounded byte samples, full-table refusal/reclamation,
injected allocation failure and a compiled accepting-invalid-input control.
Every refusal retains the output sentinel and makes no target constructor call.
This is native C ingress evidence; 32-bit execution, Rust sanitizer instrumentation
and Miri execution are not claimed.

### RI-05 acceptance record

Issue #363 is accepted for the explicit bounded native owner profile. Pushed
implementation/evidence includes selected opaque Project publication
(`af647c97a`), owner-returning checked helpers (`319c97160`), flat and nested
owned containers (`728233398`, `4d9f6b49b`), hostile/sanitizer controls
(`cf38444a3`), canonical admission rollback (`efe569912`), public Rust context
lifetimes and real archive compatibility (`6c54dfd94`), and native UTF-8 ingress
(`d14060e4a`). Each section records its actual executable scope.

The user authorized deferring the full quality profile as a closure blocker.
Miri is unavailable on the configured toolchain; prior sanitizer evidence is C
ASan/UBSan only, with Rust/std uninstrumented and leak sanitizer disabled. No
hosted, general Rust ABI, arbitrary container instantiation, new Semaprax
context-close syntax, or cross-target execution claim follows from this closure.


## RI-07 concrete demand routing and fallible Serde wire conversion

The additive `prepare_demanded_native_rust` API renders an inert binding plan,
ordinary checked scalar C, and a generated Rust adapter. Explicit requests bind
existing monomorphic Semaprax import IDs to an exactly replayed API index,
package/source identity, Cargo alias, target, features, and selected stable
compiler. Concrete type/const arguments follow declaration order. Equal
requests and carrier signatures share one wrapper; different instantiations
retain distinct physical identities. Public nongeneric nominal type arguments
and the listed scalar primitives are admitted. Private, sealed, unsafe,
ambiguous, nested, or noncanonical requests fail closed.

The generated function-pointer assignment instantiates the exact Rust function.
An associated-result annotation additionally requires rustc to prove the trait
implementation and equality to the declared Semaprax scalar result. The bounded
mapper attaches captured primary wrapper errors to checked Semaprax call spans,
retaining the Rust item, indexed signature/bound, and compiler code/message.
Captured diagnostics carry no compilation or execution authority.

This generation API does not execute tools or publish a Project/CLI package.
The caller that subsequently compiles and links must hold the selected Rust
crate bytes. The physical owning gate compiles the unchanged selected fixture
crate and runs the generated Semaprax body through C O0/O2. It executes two
const specializations and an associated result, repeated-request reuse, a
compiled flipped-specialization control, an actual `Iterator` bound refusal
mapped to its source call, valid-index private/sealed/unsafe refusals, and
malformed-const/nested-request/package-drift controls. Existing scalar and
public-generic descriptor/admission code is unchanged.

The local Serde record renderer also emits an owned wire type with explicit
boolean tags and UTF-8 byte fields. Its fallible field-wise conversion publishes
`Ok(record)` only after all fields validate. Late refusal drops both converted
locals and untouched wire fields through normal Rust ownership. Wire and mirror
identity imply no storage layout compatibility. Reverse conversion reports
explicitly copied owned payload bytes; this count excludes scalar stores, Serde
parsing, other allocator activity, and JSON output.

The two `demanded_` tests passed 2/2, zero failed/ignored, 197 filtered, on arm64
macOS using Rust 1.98.0 and Apple Clang 21. In addition to the native binding
cases, the offline pinned Serde consumer counted both buffer frees for invalid
UTF-8 and a late invalid boolean, retained no partial record, and rejected a
compiled accepting-invalid control. Its 1,000-iteration two-String conversion
reported 12,000 explicit payload-copy bytes and 765,209 ns for this local run;
this is an observation, not a performance guarantee. The owning command was:

```sh
env RUSTC=/opt/homebrew/bin/rustc CLANG=/usr/bin/clang \
  SEMAPRAX_ARCHIVER=/usr/bin/libtool CARGO_TARGET_DIR=target/ri05-owner \
  CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  cargo test --offline --locked -p semaprax-native-rust-interop --lib \
  demanded_ -- --nocapture --test-threads=1
```

Prior JSON/Vec evidence remains attributed to `56ba81928`, and the index-level
expansion/const/associated evidence to `43d060b9c`.

### RI-07 focused acceptance completion

The real JSON/Vec mirror test now runs without an environment opt-in. It starts
from fully checked Semaprax records, pins canonical source/graph round-trip,
executes serialization/deserialization and an actual collection operation, and
retains malformed JSON as `serde_json::Error`. A legal field named `value` is
covered: generated conversion locals use numbered names so they cannot shadow
the owned wire input. Success, invalid UTF-8, and a late invalid boolean execute
through that generated conversion.

The same isolated offline consumer then asks rustc to compile deliberate
foreign-target and overlapping-local Serde implementations. It requires actual
`E0117` and `E0119` errors respectively. These are downstream refusal controls;
the generator emits derives only for its local mirror and exposes no arbitrary
trait-implementation facility. The exact physical test passed 1/1, zero failed
or ignored, 198 filtered, in 9.99 seconds on arm64 macOS with Rust 1.98.0.

The existing public-generic ownership separation module passed 5/5, zero failed
or ignored. It checks admitted source, exact ABI report and C-header exclusions,
Wasm scalar rejection, and the conservative unsupported/unpublished charter.
This complements the native calls and owner controls above; a descriptor alone
does not establish callable generic support.

Both commands used the same compiler/tool paths, private target, single job,
zero debug information, disabled incremental compilation, and offline/locked
settings shown above, serially:

```sh
cargo test --offline --locked -p semaprax-native-rust-interop --lib \
  public_sdk::serde_projection::tests::generated_mirror_round_trips_with_real_serde_json_and_vec \
  -- --exact --nocapture --test-threads=1
cargo test --offline --locked -p semaprax --test projections \
  public_generic_ownership_milestone:: -- --nocapture --test-threads=1
```

Together with `f1d61d0ac` and the prior index-demand evidence at `43d060b9c`,
these gates complete the bounded RI-07 acceptance. Nested type syntax refuses
before compilation, and the fixed 64-distinct-demand budget refuses expansion;
general recursive generic expansion is not admitted. No Project/CLI generic
publication, arbitrary trait implementation, inference, HRTB/GAT/unsized
obligation, complete Rust solver, storage-layout identity, or hosted support is
claimed. The full quality profile was deferred under the user's explicit
instruction; no full-profile pass is claimed.

## RI-08 checked callbacks and source-driven registry evidence

The additive [retained affine callback profile](AFFINE-CALLBACK-V1.md) now
executes a real source-owned `FnOnce() -> i64` carrier with one Bytes capture.
It moves through checked source helper results and consuming parameters. Its
inert generated Rust owner retains the native environment after factory return;
physical iterator/consuming-trait use, unused drop, changed-body controls,
postcondition failure cleanup and cross-crate ownership refusals are covered.
This is a separate closed profile from the scalar state adapters below. Mutable
or borrowed source captures and affine foreign registration remain open.

The additive `prepare_native_rust_callbacks` renderer selects a checked
`fn(i64) -> fn(i64) -> i64` factory with one immutable scalar snapshot capture,
plus an explicit `fn(i64, i64) -> i64` next-state function. It authenticates the
closure's independent HIR cleanup/loan body product, lifts the identical body
into ordinary canonical source, rechecks it, and uses the existing scalar C
lowering. It never changes ordinary closure syntax or scalar-v1 re-entry rules.
The original closure identity and source revision remain separate projection
facts; no public callable or Rust trait-object ABI is introduced.

Generated Rust uses normal closure expressions for `Fn`/`FnOnce`, a local safe
trait impl, and a private owned environment. The mutable adapter is explicitly
a next-state protocol: state commits only after the checked call succeeds.
It does not redefine the source closure's immutable capture as mutable.
The configured trait path/method/associated error type are bounded Rust tokens;
compiling the generated safe impl must establish the exact trait obligation.
This is explicit projection, not automatic API-index trait selection.

`prepare_registered_native_rust_callbacks` additionally selects exact authored
install/apply/close exports and corresponding declared Rust registry imports.
Their sole effect is `callback.registry`, and their fixed failure domain is
`semaprax.rich-callback-registry.v1`. Each export must call its matching import
once with its unchanged scalar parameter. Registration and callback bodies use
separate scalar bridge instances. A shared same-thread domain enforces a maximum
32-level callback depth and 64 live environments without permitting re-entry on
an active receiver. Normal registration enters from a Semaprax export; dispatch
runs Rust -> outer Semaprax C -> Rust Registry -> generated local trait proxy ->
inner Semaprax C. No bridge instance becomes reentrant to achieve that route.

The registry host retains an independent environment lease before handing its
proxy to the foreign registry. Uncertain foreign register/dispatch/unregister
outcomes stop future calls and quarantine that environment; guessed registry
state never authorizes its destruction. Ordinary teardown invokes the authored
close export and removes the proxy before releasing the retained lease. The
outer boundary maps only an invocation's exact fixed failure status back to its
private pending typed error. Contract failure, checked arithmetic failure,
callback-domain refusal, guard refusal and caught Rust panic remain distinct;
no failing path manufactures a successful result. Existing Rust trampolines
catch panic before any C return and forget panic payloads.

Both renderers produce inert source. They invoke no compiler, grant no tool or
host authority, and publish no Project/Cargo package. The executing caller must
hold the ordinary native build/execution authority. The same-thread safe API
is !Send/!Sync; compile-fail transfer evidence is not a forged-pointer runtime
wrong-thread claim. Abort, OOM and process crashes are not caught.

Focused physical execution passed on arm64 macOS with Rust 1.98.0 and Apple
Clang 21.0.0: `rich_callback_` passed 2/2 (200 filtered), and the exact registered
callback selector passed 1/1 (201 filtered), zero failed or ignored. The owning
tests compile a normal
Rust trait/Registry fixture, a separate generated Rust rlib, and normal Rust
consumers with no handwritten FFI adapters. C O0/O2 runs the real iterator and
nested registration paths. Controls include cross-crate duplicate FnOnce,
borrowed Rust callback escape, receiver conflict and thread-transfer refusal;
pre-call re-entry/depth/teardown guards; source contract and arithmetic errors;
foreign registry panics; zero live environments after normal close/Drop;
retained environments after uncertain teardown; and compiled teardown/depth
mutants. Canonical source/graph and unsupported selection diagnostics remain
in the owning harness.

### Checked source Result callbacks

`prepare_native_rust_result_callback` selects a pure named
`fn(i64, i64) -> Result<i64, i64>` declaration. The first scalar parameter is
an explicit environment value; this is not new closure capture syntax. It
checks the original source and independently validates its HIR before deriving
one ordinary scalar entry. Only bounded terminal `Result::Ok` / `Result::Err`
constructors, blocks and branches are translated. Payloads, conditions and
preceding statements retain their execution order and execute once. The
original preconditions are retained; postconditions are restricted to scalar
parameter expressions, so a Result-dependent contract cannot be reinterpreted
against a scalar acknowledgement. Unsupported shapes refuse with `SPX-B154`.

A private generated scalar import stages the actual tag and full signed i64
payload. Its `callback.result` effect is internal result staging, with no
filesystem, process or network authority. The ordinary scalar boundary and
postconditions must succeed before generated Rust publishes the staged value.
`SpxCallbackError::SourceDomain(i64)` carries an authored Err (including zero,
i64::MIN and i64::MAX), separately from checked contract/semantic errors and
caught Rust panic. Missing, malformed or duplicate publication refuses; no
failure becomes a default successful return. The existing explicit next-state
proxy commits state only after Ok, and clears staged results on all ordinary
failure returns. Scalar-v1 admission remains unchanged: aggregate Result values
never cross that ABI or survive in the projected scalar closure.

The `rich_result_callback_` owning selector passed locally: 2 passed, 0
failed/ignored, 210 filtered, 2.37s runtime. It compiles a separate generated Rust
rlib, real safe-trait fixture and ordinary Rust iterator/trait consumers with
C O0/O2. It checks domain/precondition/postcondition/arithmetic/panic separation,
state rollback, re-entry and teardown, zero live environments, authored-body
changes and an Err-to-invented-success mutant. Canonical source replay and
unsupported selection controls are included. The shared runtime-template change
also passed the existing `rich_callback_` preservation selector (2 passed, 0
failed/ignored, 210 filtered; 1.75s). Tools were Rust 1.98.0 and Apple Clang
21.0.0 on aarch64 macOS, offline/locked Cargo, one job, debug0 and incremental0,
with a private target. No hosted or whole-repository full-profile run is claimed.

RI-08 remains open for source-level mutable/once receiver modes, RI-06-proven
borrowed Semaprax capture scopes and retained affine owned captures, broader
callback Result shapes. Bounded selected-index trait admission is described below. Explicit
next-state functions and a borrowed generated Rust closure do not establish
those broader source semantics.

The gates ran serially in the builder library harness, with
`RUSTC=/opt/homebrew/bin/rustc`, `CLANG=/usr/bin/clang`,
`SEMAPRAX_ARCHIVER=/usr/bin/libtool`, `CARGO_TARGET_DIR=target/ri05-owner`,
`CARGO_BUILD_JOBS=1`, `CARGO_INCREMENTAL=0`, and both development/test profile
debug settings zero:

```sh
cargo test --offline --locked -p semaprax-native-rust-interop --lib \
  rich_callback_ -- --nocapture --test-threads=1
cargo test --offline --locked -p semaprax-native-rust-interop --lib \
  public_sdk::registered_callback::tests::registered_callback_nested_c_rust_round_trip_and_uncertain_teardown \
  -- --exact --nocapture --test-threads=1
```


### Authenticated Project Regex body execution (RI-06 partial)

`prepare_indexed_regex_project_package` returns inert Cargo, Rust, C, header,
BindingPlan and descriptor bytes for one exact indexed `regex =1.13.1` closure.
It authenticates both selected imports, the Project source, registry checksum,
compiler/target identity and byte-exact committed Cargo lock. The caller owns
file creation, Clang/Cargo execution and any later publication.

The closed export is an effect-free `fn() -> i64` using bounded literal String
bindings, `Regex::new`, borrowed Result matching, `Regex::is_match` and scalar
conditionals. C executes the checked expression body and the canonical cleanup
vectors; the Rust wrapper does not substitute a separate implementation for
that body. The selected Result owner retains its resource/drop identities in
source, HIR, linked declaration facts and cleanup replay. Literal Strings have
ordinary allocated storage and canonical finalizers. Lending a named String to
a native borrowed parameter does not create an owned-value clone. Allocation
failure for literal construction remains fail-stop.

Focused local evidence on aarch64 macOS, Rust 1.98.0 and Apple Clang 21:

```sh
RUSTC=/opt/homebrew/bin/rustc CLANG=/usr/bin/clang \
SEMAPRAX_ARCHIVER=/usr/bin/libtool CARGO_TARGET_DIR=target/ri05-owner \
CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_PROFILE_TEST_DEBUG=0 cargo test --offline --locked \
-p semaprax-native-rust-interop --lib \
public_sdk::indexed_tests::indexed_project::indexed_real_regex_project_generates_and_executes_locked_offline_owner_loan \
-- --exact --nocapture --test-threads=1
```

The owning selector passed 1/1, zero failed/ignored (204 filtered). Real pinned
Regex consumers built and ran with locked/offline Cargo and C at O0/O2. Controls
cover a checked authored result changing 41 to 42, invalid-pattern domain Err
returning 9, pointer/length identity across the borrow, zero adapter copies,
exactly two source String constructions with no live String/Regex owners after
success, removed finalization failing the unchanged consumer, and stale source
or changed lock refusal.

This is a local bounded native route, not general borrowed ABI support, CLI
publication or a hosted gate. Url inspection/owner-tied returned-view integration,
remaining source escape/exclusivity controls and complete RI-06 safety acceptance
remain open. This selector does not add new Miri or sanitizer evidence, and the
full quality profile was not rerun for this slice.

### Selected-index Result callback trait adapter

`prepare_indexed_native_rust_result_callback` adds exact index selection for the
named Result callback route. The caller supplies the checked source identity,
method and associated-error paths, expected canonical index digest, and exact
package/alias/target/features/stable-compiler identity. Replay precedes trait
selection. Both selected members must belong to one public monomorphic trait;
the method must have `&mut self`, one i64 argument, and `Result<i64, Self::Error>`.
The sole associated type must have no bounds or default. Known extra members,
generic obligations, private/sealed items, unknown incomplete closure, incompatible
receivers and signature shapes fail with source-located `SPX-B154`.

The general index still marks external standard `Result` incomplete. This route
admits only the exact `Result<i64, Self::Error>` signature with the retained
trait/error graph and one standard Result leaf; it does not rewrite index facts
or widen ordinary index support.

The adapter derives its Rust impl tokens from those selected members and binds
the source revision plus selected index/member identities into a deterministic
binding digest. This is inert metadata and generated source, with no loader,
compiler, publication or host authority. The selected index can be partial and
does not encode every Rust trait property: the real stable compiler must still
prove the local **safe** impl against the actual crate. An unsafe trait, sealed
supertrait or missing method therefore cannot acquire an implementation merely
by presenting misleading metadata. Scalar ABI admission is unchanged.

The owning `indexed_trait_callback_` gate uses a genuine pinned nightly rustdoc
capture of the unchanged safe-trait fixture, verifies its source digest, then
compiles the actual fixture and generated adapter as separate Rust crates.
Its C O2 consumer retains a non-zero-sized proxy beyond the installer return,
invokes it through a real iterator and trait object, preserves a domain failure
without committing state, and returns the live environment count to zero on
teardown. Exact identity/shape refusals and real unsafe/sealed/extra-method/wrong
signature compiler controls accompany affine once, mutable-borrow and thread
transfer compile failures. This adds selected-trait admission; it does not
reinterpret scalar state transitions as source mutable/owned captures. Source
borrowed and retained affine capture modes remain open under RI-08.

Local evidence: the physical safe-impl/retention case passed in the two-case
selector; after correcting only canonical JSON in a negative metadata fixture,
the exact `indexed_trait_callback_exact_identity_and_shape_refusals` retry
passed 1/1 (0 failed/ignored, 213 filtered; 0.30s). Production code and the
physical fixture were unchanged for that retry. Tools were stable Rust 1.98.0,
Apple Clang 21.0.0 and pinned rustdoc 1.101.0-nightly (2026-10-01 commit,
nightly-2026-10-02 toolchain; JSON format 61). Cargo was offline/locked, jobs1,
debug0/incremental0, with a private target. No full-profile or hosted gate is
claimed.
