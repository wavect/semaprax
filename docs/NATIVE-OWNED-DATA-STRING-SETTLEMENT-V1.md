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

The physical regression suite derives and replays the real standalone
descriptor, generates the real provider, and instruments its allocations with
the existing fixed-table test allocator. Context-close success alone is not
the oracle. Required cases include checked failure before and after String
argument commit, local and mixed Bytes ownership, exact NUL/Unicode values,
legal scalar-loop helpers, poisoned failure outputs, and same-context reuse.
The safe external Rust consumer must use the actual generated package locked
and offline without repository source dependencies. O0/O2 and explicitly
provisioned sanitizer execution remain required, not inferred from static
inspection.

All new executable evidence remains unrun. Existing SDK, Project v8/v9/v10,
frame-payload, and artifact known answers remain required alongside the new
tests; they are not replaced with convenient expected hashes. Ordinary Wasm
String settlement and the remaining frozen command/callable limitations are
separate gaps. No completion-matrix status is promoted.
