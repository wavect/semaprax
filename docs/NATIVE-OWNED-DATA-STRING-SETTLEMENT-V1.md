# Native Owned-Data Internal String Settlement v1

Status: corrective implementation and regressions authored, with focused local
String/Bytes runtime and generated SDK consumer evidence. Remaining validation
limits are recorded below; no package, platform, or production promotion.

Audience: compiler contributors and standalone native SDK reviewers.

## Defect and affected route

The standalone owned-data SDK derives and independently replays its public
descriptor before generating the native provider. That descriptor excludes
public owned String parameters/results, effects, imports, and contracts; it
does not exclude internal String locals or internal String-valued calls.
`with_native_owned_data_sdk_subject` does not activate Project v8 or run its
Wasm admission. The public standalone builder calls the real
`emit_native_owned_data_provider` with that retained HIR and descriptor.

Previously, `OwnedDataProvider` selected terminator-based String helpers and
omitted the inline owner ledger. A String local followed by checked arithmetic
failure could bypass its lexical drop. Context close observes provider handles,
not these internal allocations, so successful close did not prove settlement.
Terminator-based clone and query helpers also lost content after U+0000.

This is not evidence that the example passes activated Project v8/v9 admission:
those routes also apply their existing Wasm restrictions. Neither admission is
widened or silently replaced by the standalone SDK route.

## Correction and ownership

`OwnedDataProvider` now selects the same length-delimited String helpers,
materialized-instance discovery, and per-String-function owner ledger as
ordinary/stdout C generation. That selector change retains its distinct profile
identity and existing Bytes/result lowering. The complete emitted translation
unit uses one String representation, including emitted but unselected functions.

The [inline settlement contract](NATIVE-INLINE-STRING-SETTLEMENT-V1.md) governs
local, temporary, parameter, and provisional-result ownership. Arguments stage
left to right and transfer their complete String owner group at call commit;
checked failure settles all remaining cells without changing the chosen status
or caller output slot. String cleanup and resource CleanupPlan ownership remain
separate. The Bytes cleanup contract and public handle lifetime are unchanged;
the subsequent native lowering correction below realizes existing transfers.

The [contents contract](NATIVE-STRING-CONTENTS-V1.md) governs exact UTF-8 bytes,
including embedded NUL and empty strings. Header-backed String pointers are
internal implementation values, not a new public ABI or permission to adopt
foreign allocations. Allocation exhaustion, runtime invariant failure, signals,
foreign unwind, and `longjmp` remain outside recoverable settlement.

## Explicit compatibility amendment

This supersedes the earlier inline-settlement/contents documents' frozen
`OwnedDataProvider` exclusion. V8/v9 provider C intentionally changes whenever
the complete emitted inventory uses Strings, even if those functions are not
selected exports. Corresponding object/archive bytes and native artifact
lengths, digests, and integrity bindings may therefore change. An older emitter
must not be used to retain stale integrity facts.

For the original String-selector amendment, String-free whole-provider output
and budget accounting remain unchanged. Existing v10 selection/runtime, all
three command-profile selectors, the scalar Rust SDK renderer, and private
callable prelude are not redirected by that amendment. Its public signatures,
descriptor/manifest schemas, HIR, Graph, CleanupPlan, Wasm/npm bytes, and target
admission are unchanged. These preservation statements do not exclude the
separate shared Bytes-lowering correction below: affected native code can
change even without Strings or when an existing command profile uses that path.
This is not a public owned-String API, a v10 backport, or full cross-backend
settlement support.

## Follow-up inventory and native Bytes corrections

Selecting the authored physical fixture exposed two distinct compiler defects.
The structural cleanup inventory walker reserved an extra binding step after
every statement, but only `let` supplied a binding action. Assignment, `while`
and audited `unsafe` statements therefore stopped discovery before later owned
initializers or the block tail. The corrected walker reserves that step only
for `let`, preserving child visitation and structural ownership order. Scalar
loop admission is unchanged; the fixture's scalar-signature helper owns its
internal String separately. Formerly rejected source can now receive complete
inventory, plan and Graph projections; this is not a claim that its defective
metadata was already correct or remains byte-identical.

The independent native defect replayed an argument's complete transition group
after its producer had already moved Bytes into the call-argument slot. Other
argument forms needed their move before evaluating a later fallible argument.
Native lowering now authenticates the exact expression, source and destination
against the existing plan immediately after argument evaluation. It carries
the canonical destination identity forward instead of replaying a transfer.
Owned conditional consumers likewise select the exact result transfer, and a
temporary-record Bytes projection returns the plan-owned projected result slot,
not a stale aggregate-field transport expression.

Owned-result scalar and Copy-variant matches remain outside mandatory cleanup
admission. Source typing alone is insufficient: Copy-variant match resolution
rejects the droppable result, while scalar-match mandatory independent cleanup
replay also rejects it during resolution, both with `SPX-H006`. Neither path is
enabled by this correction.

This does not add runtime deduplication, repair liveness flags, sort cleanup
vectors, recompute ownership from C syntax, or change the atomic call commit.
The emitter correction alone changes no source admission, HIR, Graph,
CleanupPlan, Wasm/npm bytes, public signatures or schemas. Affected native C
and its object/archive bytes, lengths, digests and evidence bindings do change;
current production emission must supply those bindings. The owning byte-data
contract remains [Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md).

## Evidence and limits

Focused emitter units cover function-signature/body/contract String discovery,
materialized-instance helper discovery, String-free byte/budget preservation,
and unchanged v10, command, and private callable selectors. Contracts and
generic-instance discovery are whole-emitter evidence, not selected SDK closure
admission.

`tests/native_owned_data_string_settlement_v1.rs` derives and replays the real standalone
descriptor, generates the real provider, and instruments its allocations with
the existing fixed-table test allocator. Context-close success alone is not
the oracle. Required cases include checked failure before and after String
argument commit, local and mixed Bytes ownership, exact NUL/Unicode values,
legal scalar-loop helpers, poisoned failure outputs, and same-context reuse.
The C fixture keeps one native context across 32 rounds for each descriptor
route, checks the v9 scalar carrier as well as its byte handle, and requires
exact allocation/free balance after every call. The clone case checks length
as well as equality so terminated clone/equality cannot jointly hide truncation.
The separate negative fixture retains `SPX-W110` for the existing v8/v9 Wasm
String-literal exclusion.
The separate private-toolchain test
`crates/semaprax-toolchain/tests/native_owned_data_string_sdk_v1.rs` supplies the
replayed descriptor and real emitted provider to the actual lower
`build_and_publish` authority in `StandaloneEvidence` mode, then consumes the
generated package locked and offline without repository source dependencies.
This exercises the renderer/tool/publication path used by the standalone
builder, not its convenience wrapper itself. It adds no private dependency to
the registry compiler. The package authority compiles its archive at O2; the
separate instrumented C fixture covers both O0 and O2. Neither consumer success
nor context close substitutes for physical allocation accounting.
The consumer reuses one safe SDK object, whose context is closed and
reinitialized between calls; this is distinct from the C fixture's reuse of one
initialized native context.

`tests/native_bytes_call_staging_v1.rs` isolates the shared native defect with
eight real standalone-provider call shapes: direct allocation, owned place,
record-field place, temporary-record projection, nested first-argument calls,
block result, conditional result, and multiple owned arguments
with a later place. Each observes checked failure before a later allocation and
successful call commit, including both conditional selections. Literal
allocation/free counts, exact distinct binary payloads, guarded copy bounds,
poisoned failure outputs, double-drop rejection and empty allocation tables
are checked through 32 rounds on one initialized context at O0/O2. The separate
sanitizer selection adds ASan/UBSan; no Project activation or allocator-OOM
recovery follows from these standalone observations.
Separate diagnostic cases retain the closed scalar-match boundary for both
fresh-copy arms and repeated references to the same owned place, and the closed
Copy-variant match boundary; they are not positive native runtime cases.

The lower package's `tests/ffi_boundaries.rs` also strengthens its generated
safe-Rust fail-stop oracle. A flushed `call-completed` witness precedes harness
assertions after any returned result or caught panic. Fatal cases must not
reach it. A separate test-only generated-file mutation keeps the real failing
provider close but deliberately ignores its failure; the same oracle must
reject this control even when a later harness assertion exits unsuccessfully.
This calibration prevents a harness panic from masquerading as runtime
fail-stop. It neither modifies production FFI nor proves containment of
arbitrary malicious native code.

Focused corrective execution gates:

```sh
cargo test --locked -p semaprax --lib codegen::native_emit::owned_strings::tests
cargo test --locked -p semaprax --test native_owned_data_string_settlement_v1
cargo test --locked -p semaprax --test native_owned_data_string_settlement_v1 standalone_owned_data_strings_settle_at_o0_and_o2 -- --exact
cargo test --locked -p semaprax --test native_bytes_call_staging_v1 bytes_call_arguments_settle_once_at_o0_and_o2 -- --exact
cargo test --locked -p semaprax-toolchain --test native_owned_data_string_sdk_v1 provisioned_standalone_owned_data_string_sdk_consumer -- --ignored --exact
```

The ordinary String regression is no longer ignored: its historical H006
blocker was corrected and physical execution passed. The new Bytes fixture retains
its printed scratch directory and has no intrinsic process deadline. Execute
these trusted compiler/target fixtures under independently bounded process and
memory limits; do not infer descendant settlement from direct-child completion.

The C fixture requires `CLANG` or `clang` and does not silently skip when it is
absent. The explicitly ignored SDK gate must be deliberately selected after
provisioning absolute `CLANG` and `SEMAPRAX_ARCHIVER` paths admitted by the
existing held-tool rules. Windows additionally needs `SEMAPRAX_VCTOOLS`,
`SEMAPRAX_LINKER`, `INCLUDE`, and `LIB` for the existing MSVC toolchain contract.
These are invocation prerequisites, not newly granted tool authority. A default
run that leaves the SDK gate ignored does not establish consumer evidence.
Explicit sanitizer execution remains independently required; static inspection
does not substitute for either gate.

For the ignored sanitizer gate, provision an absolute
`SEMAPRAX_STRING_SANITIZER_CLANG` with ASan/UBSan runtimes and run:

```sh
cargo test --locked -p semaprax --test native_owned_data_string_settlement_v1 provisioned_owned_data_strings_asan_ubsan -- --ignored --exact
cargo test --locked -p semaprax --test native_bytes_call_staging_v1 provisioned_bytes_call_arguments_asan_ubsan -- --ignored --exact
```

The fixed allocation table supplies the leak oracle even where LeakSanitizer is
unavailable. ASan/UBSan are additional memory/undefined-behavior checks, not a
claim of LeakSanitizer coverage.

Local corrective validation has passed the original String fixture's ordinary
O0/O2 execution, separately selected ASan/UBSan O0/O2 execution, and `SPX-W110`
negative case on Linux AArch64 with Rust 1.88 and Clang 14. On macOS with Rust
1.98, 57 cleanup/loop tests, eight String-emitter units, 16 preservation tests,
the frozen scalar-C known answer, and 46 lower-package tests pass. These are
local, focused observations, not hosted or Windows evidence.

The eight-shape Bytes corpus passes ordinary O0/O2 and separately selected
ASan/UBSan O0/O2 execution on Linux AArch64 with Rust 1.88 and Clang 14.
The two new closed-match diagnostic tests also pass, completing all four tests
in that fixture. Three existing owned-byte record tests and two owned-byte
variant tests pass there, as do the existing frame corpus's ordinary native
gate and calibrated isolated/retained ASan/UBSan O0/O2 gate. These selected
checks do not establish the full suite or Windows support.

The actual generated String SDK consumer also passes on Linux AArch64 with
Rust 1.88, Clang 14 and the held archiver `/usr/bin/aarch64-linux-gnu-ar`.
The real package is compiled at O2 and consumed locked/offline in a fresh nested
target directory, with publication on the container's `/tmp` tmpfs. The same
test binary and tools failed with `PackageError::Publication` when its empty
fixture was placed on the Docker Desktop `/target` bind mount. The precise
failing filesystem operation has not been established; this is a
filesystem-dependent validation limit, not evidence of universal filesystem
support.

The initial focused Clippy attempt was blocked by an unrelated `field_place`
lint. After integrating the CI owner's correction, strict compiler-library
Clippy passes on macOS. No successful full quality profile or hosted run is
claimed by this corrective evidence.

Existing SDK, Project v8/v9/v10,
frame-payload, and artifact known answers remain required alongside the new
tests; they are not replaced with convenient expected hashes. Ordinary Wasm
String settlement and the remaining frozen command/callable limitations are
separate gaps. No completion-matrix status is promoted.
