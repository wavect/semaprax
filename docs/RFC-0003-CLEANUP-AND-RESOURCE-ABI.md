# RFC 0003: Exactly-once cleanup and the resource ABI

Status: accepted; phases 1–2 implemented, phases 3–7 proposed.

This RFC defines the target-neutral destruction, cleanup, and failure contract required before SEMAPRAX may execute resources or records containing resources. Phase 1 implements canonical lifecycle/interface/import declarations and their source/HIR checks. Phase 2 implements a mandatory target-neutral `CleanupPlan` for every resolved function, independently rebuilds it after ordinary HIR and inventory validation, serializes it in Graph v6, and proves its current expression/control-flow surface with focused and hostile-HIR tests. Imported-call execution, target adapters, runtime status ABIs, backend conformance traces, and resource/aggregate execution remain proposed. Native and Wasm continue to reject resource-bearing and record-bearing modules.

## Scope and non-goals

This RFC supplies the cleanup foundation for [RFC 0002](RFC-0002-ALGEBRAIC-DATA.md). It covers uniquely owned opaque resources, aggregates that transitively contain them, transfers, partial initialization, ordinary and checked-failure exits, imported finalizers, explicit fallible close operations, and consistent native/Wasm behavior.

It does not yet define borrowing lifetimes, shared-reference retain/release, regions, foreign unwinding across SEMAPRAX frames, asynchronous cancellation, process termination, signal safety, raw traps, or a stable public binary ABI. C11 and Wasm core are bootstrap lowering contracts; the portable public component boundary remains WIT/WebAssembly Components.

## Safety contract

For every execution in the safe language profile:

1. Each successfully initialized owned resource has exactly one live owner.
2. Transfer clears the source ownership state before an operation that can fail and establishes ownership at the destination specified by the operation.
3. Every live owned resource that is not successfully transferred or committed to another owner is finalized exactly once on every language-level exit, including contract failure, checked arithmetic failure, failed imports, early return, and residual propagation.
4. Uninitialized, transferred, or already-finalized storage is never finalized.
5. Cleanup order is deterministic and independent of target implementation accidents.
6. Automatic finalization is infallible and non-trapping. A resource operation that can fail is an explicit consuming `close`, never an implicit finalizer.
7. Native and Wasm backends execute observably equivalent acquisition, transfer, and destruction traces.
8. The compiler never uses a null or zero payload as the ownership sentinel. Ownership is represented by separate compiler-managed liveness state.

These guarantees cover language-level failures represented by the internal status ABI. OS process termination, fatal signals, raw machine or Wasm traps, and foreign exceptions/unwinds crossing an unadapted boundary are outside the safe contract. An adapter must convert such behavior into status-based failure or isolate it behind an explicit `unsafe` boundary.

## Canonical source

An opaque resource must declare exactly one destruction strategy:

```semaprax
@id("io.file")
resource File {
    @id("io.file.drop")
    drop import "io.file.finalize";
}
```

This block form replaces the pre-alpha `resource Name;` placeholder. The legacy form still parses solely so verification can emit `SPX-O112`; canonical formatting never invents a destruction strategy.

The import string is a target-neutral logical key, not a C symbol, JavaScript property path, JNI name, or framework selector. Target adapters bind that key and are verified separately.

A resource whose representation needs no cleanup states that fact explicitly:

```semaprax
@id("platform.borrowed-token")
resource BorrowedToken {
    @id("platform.borrowed-token.drop")
    drop trivial;
}
```

Every destruction strategy has an explicit persistent lifecycle ID, including `drop trivial`. Omitting a strategy or its ID is an error. `drop trivial` is an audited semantic assertion, not the default and not permission to ignore an owned foreign handle. Canonical formatting preserves the lifecycle ID and renders one destruction declaration in the forms above.

The logical import must be declared by a package interface that supplies its complete authority and failure contract. Phase 1 accepts this declaration grammar:

```semaprax
@id("io.file.host")
interface FileHost
    permits { filesystem.handle.release }
{
    @id("io.file.finalize")
    import fn finalize(file: own File) -> unit
        effects { filesystem.handle.release }
        failure infallible
        consumes file always;

    @id("io.file.close")
    import fn close(file: own File) -> unit
        effects { filesystem.handle.release }
        failure status "io.error.v1"
        consumes file always;
}
```

Phase 1 implements this declaration-only interface subset. Imports are not callable expressions yet. In the v1 source grammar an import's explicit `@id` is also its target-neutral logical key; resolved HIR stores `import_id` and `import_key` separately so later syntax can decouple them. Phase 1 permits one owned opaque-resource parameter, a unit result, one effects clause, one failure clause, and `consumes <parameter> always`. Required authority equals the declared effects, status normalization is `semaprax.status.v1`, and result publication is success-only at the final-zero-status commit.

Automatic `drop` may reference only an infallible, non-trapping, consuming import with one `own` resource parameter and a unit result. APIs that need to report flush, commit, or close failure expose a separate explicit status-returning import such as the one above; its generated safe wrapper presents `close(file: own File) -> Result<unit, E>` once `Result` is available. The first version requires fallible close to consume the resource on both success and failure; an API that returns ownership after failure must do so explicitly in its result type in a later RFC. General source-defined finalizers, finalizer overloading, user-observable field finalization order, and async finalizers require later RFCs.

## Resolved semantic model

Resource declarations resolve destruction independently of display names:

```text
ResourceDrop =
    Imported {
        lifecycle_id: StableId,
        import_id: StableId,
        import_key: LogicalImportKey,
    }
  | Trivial {
        lifecycle_id: StableId,
    }

ResolvedImport {
    id: StableId,
    interface_id: StableId,
    import_key: LogicalImportKey,
    parameters: [ImportParameter {
        type: ResolvedType,
        ownership: Own | Borrow | Shared,
        consumes_on_failure: bool,
    }],
    result: ImportResult {
        type: ResolvedType,
        ownership: Copy | Own | Borrow | Shared,
        producer: Callee,
        out_slot_initialization: SuccessOnly,
        ownership_transfer: FinalZeroStatusCommit,
    },
    effects: EffectSet,
    required_authority: CapabilitySet,
    failure: Infallible | Status {
        domain_id: StableId,
        normalization: StatusSchemaVersion,
    },
}
```

The lifecycle ID, import ID, interface ID, and logical key are serialized semantic data. A target binding selects an implementation without changing program meaning. Its parameter ownership, result/out-slot initialization, effects, required authority, failure domain, normalization version, and consumption-on-failure behavior must match `ResolvedImport` exactly. An `own` parameter with `consumes_on_failure = false` is invalid unless the typed result explicitly returns ownership; that returned-ownership form is outside this RFC's first implementation.

Finalizer effects are part of the resource's resolved lifecycle contract. Verification must establish the authority required to finalize every value that can become owned; a target adapter cannot add ambient authority or widen the declared effects. An imported finalizer is infallible and consumes its payload. An explicit fallible close is a normal consuming call: once its call commit occurs, failure does not restore the source.

`needs_drop` remains a recursive type fact. A resource with either destruction strategy is non-copy. `drop trivial` means destruction performs no external action; it does not make the value copyable. A record needs destruction if any reachable field does, and its cleanup structure derives from resolved field identities and declaration order rather than names or target offsets.

## Cleanup plan

The implemented `semaprax.cleanup-inventory.v1` boundary remains the independent structural input to the plan builder. Every resolved function carries canonical storage candidates for owned non-copy parameters, droppable bindings, owned-producing expression temporaries, and a droppable provisional result. Recursive declaration-ordered shapes assign one distinct flag/lifecycle pair to every resource leaf, and entry state names only owned droppable parameters. `hir::validate` first validates ordinary HIR, then recomputes the complete inventory, then independently rebuilds and exact-compares `semaprax.cleanup-plan.v1`. Any disagreement is `SPX-H006` before either backend's unsupported-feature gate. Inventory discovery order is structural metadata; executable path liveness, transfer timing, cleanup regions, status selection, finalization order, and result publication belong exclusively to the plan serialized by Graph v6.

Phase 2 requires every resolved function to carry a target-neutral `CleanupPlan`, including scalar functions whose slot inventory is empty. That plan will be verified HIR, not backend analysis reconstructed from syntax.

```text
BlockId | EdgeId | CleanupRegionId | ExitTargetId | LivenessFlagId

StatusSourceId {
    expression: ExpressionId,
    lane: OperationFailure | ContractFalse,
}

StorageId =
    Value(ValueId)
  | Temporary(ExpressionId)
  | CallArgument {
        call: ExpressionId,
        parameter_index: u32,
        value_expression: ExpressionId,
    }
  | ProvisionalResult

FieldLivenessShape =
    NoDrop
  | Leaf {
        flag: LivenessFlagId,
        lifecycle_id: DeclarationId,
    }
  | Record {
        type_id: DeclarationId,
        fields: [FieldLiveness {
            field_id: DeclarationId,
            field_index: u32,
            shape: FieldLivenessShape,
        }],
    }

CleanupPlace {
    storage: StorageId,
    projections: [DeclarationId],
}

CleanupSlot {
    id: u32,
    storage: StorageId,
    type: ResolvedType,
    storage_index: u32,
    field_liveness_shape: FieldLivenessShape,
}

CleanupResultSource =
    Scalar(ExpressionId)
  | Owned(CleanupPlace)

CleanupEntryState {
    live_owned_parameters: [CleanupPlace],
}

CleanupTransition =
    Initialize { at: ExpressionId, destination: CleanupPlace }
  | Transfer { at: ExpressionId, source: CleanupPlace, destination: CleanupPlace }
  | CallCommit {
        call: ExpressionId,
        arguments: [CallArgumentTransfer {
            parameter_index: u32,
            source: CleanupPlace,
        }],
    }
  | SelectFailure { source: StatusSourceId }

StatusSource {
    id: StatusSourceId,
    producer:
        PropagatedCall { callee: DeclarationId }
      | CheckedArithmetic { operation: CheckedOperation, normalized_cases: [StatusCase] }
      | ContractFalse { phase: Requires | Ensures, ordinal: u32 },
}

CleanupBlock {
    id: BlockId,
    region: CleanupRegionId,
    transitions: [CleanupTransition],
    terminator: Goto(EdgeId) | Branch([EdgeId]) | Exit(ExitTargetId),
}

CleanupEdge {
    id: EdgeId,
    from: BlockId,
    to: BlockId,
    condition:
        Always
      | BooleanResult(ExpressionId, bool)
      | StatusZero(StatusSourceId)
      | StatusNonzero(StatusSourceId),
}

CleanupRegion {
    id: CleanupRegionId,
    parent: CleanupRegionId | None,
    slots: [StorageId],
    normal_scope_end: ExitTargetId,
}

FinalizeAction {
    source: CleanupPlace,
    lifecycle_id: DeclarationId,
    guard_flag: LivenessFlagId,
}

ExitTarget {
    id: ExitTargetId,
    from: BlockId,
    leaves_regions: [CleanupRegionId],
    finalize_in_order: [FinalizeAction],
    continuation:
        Continue(EdgeId)
      | CommitResult { source: CleanupResultSource }
      | ReturnFailure { source: StatusSourceId }
      | ReturnUnit,
}

CleanupPlan {
    entry: BlockId,
    entry_state: CleanupEntryState,
    slots: [CleanupSlot],
    status_sources: [StatusSource],
    blocks: [CleanupBlock],
    edges: [CleanupEdge],
    regions: [CleanupRegion],
    exits: [ExitTarget],
}
```

`CheckedArithmetic` uses the compiler-owned `semaprax.status.v1` arithmetic cases below. The numeric values are part of cleanup-plan v1 and must match on every backend: `1 add_overflow`, `2 sub_overflow`, `3 mul_overflow`, `4 division_by_zero`, `5 division_overflow`, `6 remainder_by_zero`, `7 remainder_overflow`, and `8 negation_overflow`. Division and remainder test a zero divisor before the signed `i64::MIN / -1` or `i64::MIN % -1` overflow case.

`CleanupPlace` addresses a whole slot or an exact nested field path, so partially initialized temporaries and provisional aggregate results never require a backend to reconstruct field ownership. A scalar result has no cleanup slot, but still has an explicit `Scalar` publication source and success commit. `NoDrop` carries no flag. Every `Leaf`, including `drop trivial`, has one distinct liveness flag and lifecycle ID; a finalizer has exactly one guard, never an ambiguous all/any flag list. Record fields are stored in declaration order; transitions establish actual initialization order on each executable path, and `finalize_in_order` records its reverse explicitly. Slot and flag IDs are contiguous in canonical structural order, but mutually exclusive branches do not pretend to share one runtime initialization sequence. Expression transitions are keyed by revision-scoped expression IDs; they may not depend on source spans or backend-generated temporaries. Entry state seeds every `own` droppable parameter live in signature order without inventing an expression ID. Borrowed and shared parameters never enter the unique-owner cleanup plan.

Blocks, edges, lexical cleanup regions, normal scope ends, exceptional exits, and their continuations are all explicit. The plan therefore tells a backend where ownership state changes, which regions an edge leaves, which guarded places to finalize, where cleanup continues, and whether the exit commits a result or returns failure. A backend may optimize a validated plan but must not reconstruct scope or cleanup behavior from AST shape, source spans, C blocks, or Wasm stack layout.

Plan construction follows these rules:

- A successful constructor or imported acquisition initializes its result slot. A failed operation does not.
- Moving a value clears the source place before entering a potentially failing operation and initializes a non-call destination according to the transfer.
- Arguments evaluate left-to-right into caller-owned temporaries. One atomic `CallCommit` contains every and only `own` argument in parameter order and transfers them into callee-owned ABI slots before entry. A failure before that point remains caller cleanup; after it, the callee owns the arguments even when the call reports failure, and the caller must not reacquire them. Backends may not reconstruct the atomic group from unrelated transfer records.
- A return expression initializes a callee-owned `ProvisionalResult`; it never writes the caller's out-slot directly.
- Copy operations do not change ownership liveness.
- In cleanup-plan v1, an unnamed owned temporary that is not transferred remains live until the end of its enclosing lexical cleanup region; v1 does not introduce a separate C++/Rust-style full-expression destruction boundary. Conditional paths retain guarded liveness for such temporaries through their join, so only the path that initialized a leaf finalizes it. A later schema may shorten these lifetimes as an observable optimization only with target-neutral trace evidence.
- Partial aggregate construction marks each field live only after that field initializes. Failure destroys the live prefix in reverse initialization order.
- Whole-aggregate ownership boundaries normalize cleanup history. Once an aggregate is fully initialized and is transferred as a whole, committed as a call argument, bound as a complete value, or published as the provisional result, its destination begins a new semantic initialization epoch whose droppable leaves are live in recursive declaration order. Cleanup of that destination is therefore reverse recursive declaration order. This rule does not rewrite an in-progress constructor: failure during construction still cleans only successfully initialized leaves in reverse actual completion order. The normalization is valid while field-finalization order remains non-user-observable; any future feature that exposes that order must revise the cleanup schema rather than infer hidden source history across boundaries.
- Moving one field clears only that field. Siblings remain independently live, while any operation requiring the complete parent remains invalid under RFC 0002 place rules.
- Nested records recursively expose field liveness. Implementations may use bitsets or equivalent explicit flags, but not payload sentinels.

HIR validation must replay the complete plan against typed control flow and reject duplicate initialization, transfer from a non-live place, conflicting storage, missing cleanup coverage, non-atomic call commits, non-deterministic ordering, invalid field paths, missing or contradictory blocks/edges/regions/exits, and disagreement with ownership facts. Failure selection is write-once: dataflow begins with no selected status, `SelectFailure` chooses one canonical expression/lane source, cleanup cannot replace or clear it, `ReturnFailure` must return that exact dominating source, and a success/result-commit path must have none. Separate lanes distinguish an operation failure from a false contract result at the same expression. `CommitResult { source }` is a semantic exit action rather than an expression transition: it may occur only after postconditions and every non-result cleanup action, and no failure-reachable edge may execute it. `SPX-H006` remains the existing generic malformed-HIR trust-boundary diagnostic; cleanup-plan failures use it with a deterministic cleanup-specific reason and semantic IDs. A malformed plan is a compiler-input error even when source verification previously succeeded.

## Conformance trace

Backend equivalence tests use the versioned [conformance trace v1](CONFORMANCE-TRACE-V1.md): a target-neutral event trace containing stable import/lifecycle IDs, semantic storage/place IDs, expression IDs for transitions, event kind (`initialize`, `transfer`, `call_commit`, `import_begin`, `import_end`, `select_failure`, `finalize_begin`, `finalize_end`, or `result_commit`), and normalized status. `select_failure` exposes the exact write-once source selected by every nested frame as well as the root. Callable imports may complete with a normalized failure; automatic-finalizer import completion is success-only in the type system. A trivial strategy emits both finalization events with success even though it performs no host call. Physical pointers, Wasm handles, status tokens, stack offsets, and host exception objects are excluded. Two backends conform only when they produce the same ordered semantic events and normalized final status for the same verified program and injected import outcomes.

## Unified epilogue

All language-level exits branch to one semantic cleanup epilogue. Backends may share blocks or specialize them only when equivalence is proven.

The epilogue and result commit protocol:

1. evaluates the body result into a callee-owned provisional result slot;
2. evaluates postconditions by borrowing that provisional result;
3. on a still-successful path, finalizes every live non-result slot in the explicit plan order;
4. only after postconditions and non-result cleanup complete, transfers the provisional result to the caller out-slot as the final zero-status commit;
5. clears the provisional liveness flag and returns status zero;
6. on any failure before the commit, leaves the caller out-slot uninitialized, finalizes the provisional result if live, then finalizes every other live slot and returns the original nonzero status;
7. for every finalization, clears the liveness flag before invoking the trivial or imported finalizer and continues until all selected live slots are clear.

Automatic finalizers are infallible and non-trapping, so cleanup cannot replace the status already selected by normal execution or an explicit fallible operation. Clearing before invocation prevents reentrant double finalization. Reverse initialization order applies to the semantic order recorded in the plan, including conditional and partial construction; it is not inferred from stack addresses or structure layout. Preconditions run before body-owned locals exist. The caller may mark its out-slot initialized only after observing status zero; a nonzero return is proof that no result ownership was published.

## Status schema and normalization

The ABI status is a context-local `u32` token, not a raw operating-system error:

- token `0` is success and has no status record;
- every nonzero token indexes an immutable `NormalizedStatus` in the active context's status arena;
- tokens are meaningful only with that context and are excluded from deterministic traces and public interfaces.

```text
NormalizedStatus {
    schema: "semaprax.status.v1",
    domain_id: StableId, // 1..=255 UTF-8 bytes, no NUL
    code: nonzero u32,
    class: Contract | Arithmetic | Import | ExplicitClose | Adapter,
    retryable: bool | Unknown,
}
```

Failure code zero is forbidden in status v1 independently of the ABI token rule: a normalized record always denotes failure, while ABI token zero always denotes success and has no record. A domain identity is 1–255 UTF-8 bytes and contains no NUL; byte length, not character count, is normative across source, HIR, native, Wasm, and adapters. Opaque diagnostic detail is a target-private sidecar associated with an arena record rather than a field of `NormalizedStatus`. Program behavior and conformance compare `(schema, domain_id, code, class, retryable)`; localized messages, backtraces, host objects, the sidecar, and the context-local token are non-semantic and absent from conformance JSON.

Bindings normalize failures before returning:

- POSIX `errno` values map by symbolic condition through the versioned `posix.errno.v1` table; platform numeric values never become semantic codes directly.
- Windows `HRESULT` values map to `windows.hresult.v1` with the canonical unsigned 32-bit value and a declared class mapping.
- Objective-C, Swift, JVM, and JavaScript exception types require an adapter-manifest mapping to declared domain/code pairs. An undeclared exception becomes deterministic `semaprax.adapter.unexpected.v1`, never a class name or message derived at runtime.
- Import-defined status or error enums use the domain and code table declared by their package interface. A binding may not invent, merge, or renumber codes.
- Contracts and checked arithmetic use compiler-owned versioned domains shared by every backend.

The compiler-owned contract domain is `semaprax.contract.v1`: code `1` is `requires_false` and code `2` is `ensures_false`. Both have class `Contract` and `retryable = false`. Contract ordinal and expression identity remain in the selected `StatusSourceId`; they never create target-specific status codes. The compiler-owned arithmetic domain is `semaprax.arithmetic.v1` and uses the exact operation-specific codes defined by `StatusCase` above, class `Arithmetic`, and `retryable = false`.

Nested calls allocate records in the same invocation-scoped arena or explicitly re-home them into the caller context before returning. Public wrappers resolve the normalized record before releasing the context and then map it to the platform result, exception, rejected promise, log, or process status.

## Internal call ABI

Safe internal calls are non-trapping and status-based:

```text
(context, parameters..., result_out) -> status_token: u32
```

The caller supplies valid, correctly aligned, non-aliasing storage whose lifetime covers the call, but that storage is logically uninitialized on entry. The callee builds and owns its result in separate provisional storage, evaluates postconditions against that value, and finalizes all live non-result slots. Only then may it transfer/copy the provisional value into `result_out` and return token zero; that return is the single publication commit. Every pre-commit failure finalizes the provisional value if live, performs the planned cleanup, returns a nonzero token, and performs no write that gives the caller result ownership. The caller marks the out-slot live only after zero and must treat it as uninitialized after every nonzero token.

Parameters marked `own` transfer before callee entry; borrowed parameters remain caller-owned. The context carries capability/import bindings, the normalized-status arena, and target runtime state, not ambient authority.

Public entry wrappers may translate the final status into a process exit, host exception, rejected promise, or trap only **after** the SEMAPRAX cleanup epilogue has completed. Internal checked arithmetic and contract failure must branch to cleanup rather than trap directly.

## C11/native bootstrap adapter

The bootstrap C ABI has this conceptual shape:

```c
uint32_t spx_function(
    struct spx_context *context,
    /* parameters in semantic order */,
    struct spx_result_type *result_out
);
```

Resource payload types are stable-ID-derived wrappers around `uintptr_t`, not bare `void *` and not nullable ownership indicators. Aggregates use deterministic, target-validated layouts. Each cleanup slot or droppable field has an independent flag or bit. The lowering clears the flag before calling an infallible bound finalizer and continues cleanup. A fallible explicit close is lowered as an ordinary status/out call, never emitted from the epilogue.

An imported C finalizer has conceptual type `void (*)(struct spx_context *, spx_resource)` and may neither unwind nor return status. An explicit close uses the ordinary `uint32_t` status/out ABI. Binding validation rejects a finalizer with a fallible signature; conformance validation rejects an adapter whose implementation unwinds or traps. Platform shims must contain foreign exceptions at the boundary, and any non-semantic telemetry they record cannot alter cleanup or status.

Foreign C/Objective-C operations that can longjmp, unwind, invoke undefined behavior, or terminate cannot be exposed as safe imports directly. Generated shims must catch/translate supported foreign failures into status values; otherwise the operation is isolated behind explicit `unsafe` interop. Sanitizer-clean execution is required evidence, not an ABI promise.

## WebAssembly core bootstrap adapter

The internal Wasm function ABI is:

```text
(context_pointer: i32, parameters..., result_pointer: i32) -> status_token: i32
```

Opaque resources are `i32` indices into a host-managed resource table. The context pointer addresses a validated, instance-local linear-memory record containing an instance nonce, import/capability-table reference, normalized-status arena reference, and shadow-stack base, limit, and top. Aggregates and result slots live in linear memory with deterministic offsets. A compiler-managed shadow stack holds aggregate temporaries and explicit liveness bits; every exit restores the saved shadow-stack top after cleanup. No handle value, including zero, substitutes for a liveness bit.

There is no mutable process-global "current context." Each root invocation receives a distinct context and status arena. A reentrant callback pushes a distinct frame, saves its predecessor's top, and restores it on every return; it may share immutable import/capability bindings but not frame liveness. Each worker, Wasm store, or native thread owns its context and resource-handle namespace unless a later `Sendable`/`Shareable` contract explicitly authorizes sharing. Shared-memory implementations must synchronize their host tables and status arenas, but may not share shadow-stack tops. Context nonce, bounds, alignment, handle namespace, and frame restoration are validated at every generated host boundary.

Imported JavaScript shims receive or close over the validated context, catch host exceptions, normalize them into that context's status arena, and return nonzero tokens. Promise rejection is only a public-host mapping after cleanup; synchronous core imports may not leak JavaScript exceptions through Wasm frames. Checked arithmetic and contracts branch to the unified epilogue rather than issue a raw trap. Component Model resource handles and canonical ABI lowering may replace the bootstrap representation later while preserving the same semantic trace.

A Wasm finalizer import has conceptual type `(context_pointer: i32, resource_handle: i32) -> ()`; it cannot return a status. Explicit close uses the ordinary status/out convention. A declared fallible finalizer binding is rejected, while hostile throwing/trapping finalizer shims fail adapter conformance and must be contained by the generated host boundary.

## Interoperability and platform hosts

One logical import ID must retain the same ownership, effect, success, and failure meaning across all adapters:

| Boundary | Resource and failure mapping |
| --- | --- |
| C / Objective-C | Generated stable wrapper type plus ownership annotations; automatic finalizer is `noexcept`/non-trapping. An explicit fallible close shim consumes its argument, returns status, and initializes its result out-parameter only on success. |
| WIT / WebAssembly Components | WIT `resource.drop` maps only to the infallible finalizer. A fallible close is a separate interface function returning `result`; canonical ABI errors normalize before entering core cleanup. |
| Swift / iOS / macOS | Generated owning wrapper has a non-throwing `deinit` fallback. A consuming explicit close surfaces `Result`/`throws` while the wrapper is reachable; public failure appears only after SEMAPRAX cleanup. |
| JNI / Kotlin / Android | Native handle table entry has a non-throwing finalizer/Cleaner path. A separate consuming close normalizes JNI exceptions into status; JVM cleanup itself never reports failure. |
| JavaScript / TypeScript / web | Host resource-table entry has an infallible explicit drop import; a separate consuming close returns a synchronous status/result in this core ABI. Promise-based close belongs to the later async ABI or public orchestration and cannot suspend this cleanup frame. |
| Windows native | C-compatible status/out shim over Win32, COM, or WinRT adapters; HRESULT/last-error translation occurs without skipping SEMAPRAX cleanup. |
| Linux native/server | C-compatible status/out shim over system libraries; errno or library errors become deterministic status data before return. |

Generated adapters are part of the trusted boundary and require bidirectional conformance tests. Platform lifecycle hooks may trigger higher-level cancellation later, but cannot silently revoke or double-drop live resources.

## Diagnostics

The following stable codes are reserved for this tranche:

- `SPX-O112`: owned resource declaration has no destruction strategy.
- `SPX-O113`: lifecycle declaration is missing its stable ID, duplicated, fallible, or otherwise incompatible with automatic finalization.
- `SPX-O114`: safe-profile import or finalizer can trap or unwind instead of satisfying its declared status/finality contract.
- `SPX-I403`: interface/import declaration has a missing or empty persistent ID, duplicate interface/import name, logical key, permit, or effect, or an empty status domain. Grammar-shape errors remain parser diagnostics.
- `SPX-I404`: interface/import ownership, authority, consumption, result, or failure contract is internally inconsistent.
- `SPX-E103`: a function can own a resource but omits an effect required by its automatic lifecycle.
- `SPX-I401`: target adapter has no binding for a required logical import key.
- `SPX-I402`: target binding ABI, ownership, effects, or failure contract does not match the resolved import.
- `SPX-H006`: existing generic malformed-HIR boundary; cleanup inconsistencies carry a deterministic cleanup-specific reason.
- `SPX-B104`: native lowering cannot preserve a validated cleanup invariant.
- `SPX-W111`: Wasm lowering cannot preserve a validated cleanup invariant.

Frontend diagnostics point at authored declarations or expressions. HIR/backend diagnostics identify stable declaration IDs, expression IDs, storage places, and the violated invariant. A backend must fail closed rather than repair or omit a cleanup transition.

## Phased implementation and evidence gates

Each phase is incomplete until its executable evidence passes. An RFC, type definition, generated text, or successful scalar build does not satisfy a phase.

1. **Source and resolution — implemented.** Parse and canonically format both finalization forms with lifecycle IDs and the declaration-only interface/import contract; reject missing, duplicate, malformed, fallible-drop, authority, ownership, consumption, result-initialization, or failure-domain conflicts with stable diagnostics. Resolve lifecycle/interface/import IDs, logical keys, parameter and result ownership, effects/authority, normalized failure contracts, and recursive `needs_drop` facts. Source verification and hostile-HIR replay enforce recursive lifecycle effects. Graph v6 and semantic resource renames preserve lifecycle/import identities and context closure. This phase does not execute imports or cleanup.
2. **Verified cleanup HIR — implemented for the current HIR surface.** Every function carries a deterministic `CleanupPlan` with typed CFG blocks/edges, lexical regions and scope exits, entry liveness, exact leaf flags/lifecycles, left-to-right caller-owned argument epochs, one atomic call commit, checked-operation and contract status sources, sticky failure exits, guarded reverse finalization, partial-record history, whole-value normalization, provisional owned results, and scalar/owned publication commits. Validation rebuilds the plan solely from already-validated core HIR and inventory and rejects any component mismatch with `SPX-H006`; native and Wasm consumers inherit this gate. Graph v6 exact snapshots, deterministic rename evidence, focused semantic tests, and hostile plan mutations cover the boundary. This phase is target-neutral metadata only and does not claim resource execution or backend trace conformance.
3. **Native scalar-resource execution.** Target-neutral groundwork is implemented: versioned normalized status/trace types, context/arena-safe status tokens, independent inventory/HIR coverage and exhaustive current-CFG plan replay, and a scenario-driven single-frame reference executor with explicit result-publication state. The scalar native status/out slice is also implemented: nested generated calls share one context, propagate the original nonzero token, distinguish every compiler-owned contract/arithmetic code, and leave caller output untouched until the postcondition-checked success commit. Unreachable first-slice scaffolding now derives deterministic strongly typed resource wrapper/finalizer symbols, stages resource-aware signatures, indexes direct trivial-resource cleanup without reconstructing it, computes a checked conservative max-path trace capacity, emits guarded plan-driven cleanup fragments and actual root-frame semantic events from the classifier-owned function identity, and attaches one-shot trace storage through an owner/generation-checked pre-ownership capacity gate. Persistent identities are rejected if they contain NUL, generated C literals preserve hostile trigraph-like, UTF-8, and full signed-`i64` values, and every direct cleanup slot/transfer proves exact lifecycle/type coherence. A typed value plan now places real Boolean contracts, `i64 >=` comparison, checked addition, scalar result aliases, and owned transfers at the exact validated cleanup blocks without generating control flow. The test-only native conformance lane executes that real planner for discard/reverse cleanup, requires true/false, checked-add success/overflow/precondition failure, owned identity, and failed owned postcondition cases with zero/max payloads. Its versioned length-framed decoder and independent program-bound materializer must match the reference executor as typed values and canonical JSON; O0/O2 outputs are byte-identical, ASan/UBSan are required on Linux CI, undersized capacity fails before ownership, and scalar/owned poison slots prove publication timing. The scaffold still rejects records, imported lifecycles, projections, generics, calls, source conditionals, lazy Boolean flow, and payload-source-less initialization. The remaining native backend blocker is a real public resource ingress/export with an explicit ownership and trace-capacity contract; no source constructor, callable import, or resource-valued `main` exists today. After that boundary is designed and proven, extend acquisition, calls/imports, explicit close, every checked-arithmetic case, and broader control flow. Specifically prove explicit close/import failure consumes exactly once and other resources still finalize. Negative fixtures must prove that fallible or trapping finalizer bindings are rejected or fail adapter conformance before publication. Run the cross-platform native CI matrix for every expansion.
4. **Wasm scalar-resource execution.** Run the same trace corpus in a real Wasm host. Prove exception-to-status normalization, provisional-result commit, context nonce/bounds validation, nested reentrant frames, per-worker/store isolation, shadow-stack restoration, and byte-for-byte deterministic modules where already required.
5. **Aggregate cleanup.** Prove partial and nested construction, field moves, sibling survival, aggregate return, failed postconditions, and reverse field cleanup for resource-containing records. Enable record backend lowering only after native/Wasm trace equivalence passes.
6. **Interop adapters.** Bind the same logical fixtures through C, WIT, Swift/Apple, JNI/Kotlin/Android, and JavaScript/TypeScript. Prove ownership, out-slot initialization, status normalization, infallible automatic finalization, and separate fallible close mapping in both directions on representative hosts, simulators, or devices.
7. **Broader control flow.** Extend the plan and trace suite before enabling loops, variants/matching, `?`, closures, regions, concurrency, cancellation, or async resources.

At every phase, source-verifier and hostile-HIR replay diagnostics must agree, failed imports and failed postconditions must not initialize caller result storage, automatic finalizers must remain infallible and non-trapping, explicit close failure must obey its declared consumption contract, and native/Wasm normalized event traces must match. The rows in the [completion matrix](COMPLETION-MATRIX.md) remain Partial or Missing until their complete gates pass.

## Rejected shortcuts

- Treating all resources as `void *` or `i32` without a destruction contract.
- Making destruction optional or silently equivalent to `drop trivial`.
- Using zero/null payloads as moved or uninitialized markers.
- Reconstructing ownership from backend control flow instead of validating one shared cleanup plan.
- Allowing an automatic finalizer, WIT `resource.drop`, Swift `deinit`, or JVM cleanup hook to report a recoverable failure; fallible cleanup belongs in explicit `close`.
- Trapping immediately on contracts, arithmetic, host exceptions, or explicit close failure.
- Enabling scalar-only record lowering while resource-containing records use a different ownership model.
- Claiming platform interoperability from generated wrappers that have not executed conformance fixtures.

These shortcuts weaken exactly the safety and cross-target equivalence that SEMAPRAX is intended to make verifiable.
