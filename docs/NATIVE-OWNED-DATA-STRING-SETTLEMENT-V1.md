# Native Owned-Data Internal String Settlement v1

Status: corrective implementation and regression evidence authored but unrun;
no package, platform, or production promotion.

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
ordinary/stdout C generation. It retains its distinct profile identity and all
existing Bytes/result lowering. The complete emitted translation unit uses one
String representation, including emitted but unselected functions.

The [inline settlement contract](NATIVE-INLINE-STRING-SETTLEMENT-V1.md) governs
local, temporary, parameter, and provisional-result ownership. Arguments stage
left to right and transfer their complete String owner group at call commit;
checked failure settles all remaining cells without changing the chosen status
or caller output slot. String cleanup and resource CleanupPlan ownership remain
separate. The existing Bytes cleanup and public handle lifetime are unchanged.

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

String-free whole-provider output and budget accounting remain unchanged.
Existing v10 selection/runtime, all three command profiles, the scalar Rust SDK
renderer, and private callable prelude remain unchanged. Public signatures,
descriptor/manifest schemas, HIR, Graph, CleanupPlan, Wasm/npm bytes, and target
admission are not changed by this correction. This is not a public owned-String
API, a v10 backport, or full cross-backend settlement support.

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

Focused future execution gates (not run in this batch):

```sh
cargo test --locked -p semaprax --lib codegen::native_emit::owned_strings::tests
cargo test --locked -p semaprax --test native_owned_data_string_settlement_v1
cargo test --locked -p semaprax-toolchain --test native_owned_data_string_sdk_v1 provisioned_standalone_owned_data_string_sdk_consumer -- --ignored --exact
```

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
```

The fixed allocation table supplies the leak oracle even where LeakSanitizer is
unavailable. ASan/UBSan are additional memory/undefined-behavior checks, not a
claim of LeakSanitizer coverage.

All new executable evidence remains unrun. Existing SDK, Project v8/v9/v10,
frame-payload, and artifact known answers remain required alongside the new
tests; they are not replaced with convenient expected hashes. Ordinary Wasm
String settlement and the remaining frozen command/callable limitations are
separate gaps. No completion-matrix status is promoted.
