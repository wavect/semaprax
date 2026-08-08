# RFC 0004: Native call recovery and settlement

- Status: Proposed; target-neutral owner model implemented, physical v3 unwired
- Version: 0.1
- Audience: compiler, native code generator, loader, ownership-host, adapter,
  and conformance-test implementers

## Summary

This RFC defines the target-neutral recovery contract required after a native
owned call crosses its atomic ownership commit. It introduces a host-allocated
linear recovery frame, compiler-certified checkpoints, a one-shot settlement
decision, an idempotent `settle` operation, and an authenticated quiescence
receipt. The purpose is to make normal completion, semantic failure, returned
physical failure, malformed provider output, and host unwinding converge on one
bounded cleanup protocol without guessing which physical resources remain live.

The repository contains an internal, target-neutral model of the bounded
certificate, frame, decision, receipt, and idempotent settlement operation. It
has unit evidence only. There is no v3 descriptor or wire layout, provider
symbol, code generator, loader path, physical finalizer, ownership-host wiring,
or public compiler surface. Callable v2 remains the implemented private
experiment, ordinary native resource builds still fail with `SPX-B104`, and
the model plus this document satisfy no physical-runtime completion gate.

The key rule is:

> After call commit, the host may retire or publish physical ownership only
> after validating one certified quiescent settlement. If certification cannot
> be completed, it must poison and quarantine the exact module instance; it
> must never infer cleanup from a malformed response or retry a finalizer.

## Relationship to existing contracts

This RFC extends, but does not replace, the following contracts:

- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) defines target-neutral
  semantic cleanup order, sticky failure selection, and success-only result
  publication.
- [Host ownership transactions v1](HOST-OWNERSHIP-TRANSACTIONS-V1.md) defines
  preflight, atomic owner commit, and logical completion.
- [Native callable ABI v2](NATIVE-CALLABLE-ABI-V2.md) defines the current
  private request/response and trace-certificate experiment, including its
  unresolved physical-failure boundary.
- [Conformance trace v1](CONFORMANCE-TRACE-V1.md) remains the semantic trace.
  Recovery checkpoints, physical resource state, adapter failure, and
  settlement receipts are adapter evidence and must not be inserted into that
  semantic trace.

Callable v3 is a new ABI. A v2 provider cannot be admitted as v3, and a host
must not negotiate or fall back from v3 to v2 after ownership commit.

## Normative language and goals

The terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

The protocol is designed to establish all of the following for an admitted
call:

1. Every physical ownership transition is represented by a compiler-certified
   checkpoint before provider control may return or report failure.
2. Settlement starts from the exact last certified checkpoint and executes
   only cleanup that remains pending.
3. Repeating the same settlement request has no additional physical effect and
   returns the same receipt.
4. Conflicting settlement decisions, cross-frame data, stale generations, and
   incomplete or reordered recovery paths fail closed.
5. A successful receipt proves call-level quiescence before the ownership
   ledger publishes a result, retires inputs, or permits unload eligibility.
6. All allocation, capacity validation, certificate authentication, invocation
   reservation, and receipt storage occur before ownership commit.

This RFC does not make arbitrary native code safe and does not make process
failure recoverable.

## Layering and trust boundaries

The protocol has four distinct authorities:

1. The compiler derives the cleanup plan, recovery-frame layout, legal
   checkpoints, settlement transition language, and terminal receipt shapes.
2. Descriptor admission authenticates those artifacts and their exact bounds
   before a module instance may accept an owner.
3. The generated provider records physical progress and performs settlement.
4. The host independently validates frame identity, monotonic checkpoint
   progress, the one settlement decision, the certificate path, and the final
   receipt before changing public ownership state.

The provider is still admitted trusted native code. A certificate proves that
reported ordinals form a compiler-approved path; it cannot observe an omitted
physical side effect or make malicious machine code memory-safe. The unsafe
admission caller must continue to establish code and dependency provenance,
exact symbol ownership, ABI compatibility, no-escape behavior, and module
lifetime.

The following are inside the intended v3 failure model:

- ordinary scalar or owned success;
- normalized semantic failure;
- a provider-reported physical failure at a certified checkpoint;
- a malformed or semantically inconsistent candidate response;
- duplicate identical settlement requests;
- host-language unwinding after ownership commit, when the host settlement
  guard remains able to call `settle`;
- draining initiated while frames or returned owners remain live.

The following remain outside the safe contract: hostile native code, undefined
behavior, arbitrary memory corruption, process termination, fatal signals,
power loss, an uncontained foreign unwind or `longjmp`, a trapping or
non-idempotent automatic finalizer, kernel or device side effects absent from
the declared adapter contract, and destruction of the module image while its
code may still execute. Encountering evidence of one of these conditions MUST
quarantine rather than trigger speculative cleanup.

## Bounded target-neutral recovery frame

The implemented foundation models a frame prepared from one authenticated
certificate, one nonzero invocation, and one dense checkpoint. The frame stores
the function identity, nonzero call-contract fingerprint, certificate
fingerprint, invocation, checkpoint ordinal, one owner-level state per resource,
and an optional cached terminal settlement. It contains no raw pointer, target
handle, loader lease, authority, physical payload, or host ledger.

The Rust frame type is deliberately neither cloneable nor formattable, but that
negative API fact is not a uniqueness proof. `prepare_frame` is a deterministic
model constructor: calling it twice with the same certificate, invocation, and
checkpoint produces two equal model states. The model performs no invocation
reservation and binds neither a process-local module instance nor a nonreused
frame generation. Consequently its action vectors are proof data, not physical
finalizer capabilities. Only future host-ledger admission may create the one
linear physical frame, and it MUST reject duplicate invocation/frame-generation
reservations before ownership commit.

The future physical host MUST allocate every frame, action buffer, and receipt
buffer before owner commit and treat the frame as linear. Module-instance and
frame-generation binding, exact loader retention, thread policy, and physical
sidecars are mandatory future wiring; they are not properties of the current
model and must not be inferred from a function or call-contract fingerprint.

### Implemented model bounds

`semaprax.native-settlement-certificate.v1` enforces:

| Quantity | Maximum |
| --- | ---: |
| Owner-level resource entries per frame | 4,096 |
| Dense checkpoints per certificate | 65,536 |
| `resource_count * checkpoint_count` validation work units | 1,000,000 |

The resource and checkpoint counts must also be nonzero, checkpoint ordinals
must be exactly dense `1..=N`, and every size multiplication is checked. These
bounds make model validation finite; they are not an implemented physical ABI
or byte-capacity guarantee.

A future descriptor and wire schema MUST authenticate exact frame,
certificate, action, and receipt byte capacities and MUST set a versioned hard
byte ceiling before wiring. Every alignment, addition, multiplication,
host-size conversion, and allocation must be checked before commit. Exceeding a
bound is a precommit rejection, never postcommit truncation. The present model's
canonical JSON is test evidence, not a stable native wire layout.

## Owner-level resource states

The foundation deliberately uses these five closed states:

| State | Meaning |
| --- | --- |
| `Live` | The call owns this admitted input and settlement must finalize it |
| `ProvisionalResult` | The call owns the one possible unpublished result; accept-owned may publish it, while abort must finalize it |
| `Finalizing` | A physical finalizer is active; this is transient and never an admissible recovery checkpoint |
| `Dead` | No call-owned value remains in this ordinal |
| `Published` | The owned result was selected for publication; this is terminal and never an admissible recovery checkpoint |

`Dead` is intentionally an owner-level disposition. It does not distinguish
never initialized, previously finalized, or transferred ownership. The current
direct-input tranche has no acquisition, nested call, local resource creation,
or general transfer checkpoint. Those shapes require an extended, independently
validated state model before admission; a future implementation must not guess
them from `Dead`.

At a recoverable checkpoint, every entry must be `Live`,
`ProvisionalResult`, or `Dead`, and there may be at most one provisional result.
`Finalizing` is rejected because a synchronous total automatic finalizer must
return and record `Dead` before another failure boundary. A trap, unwind,
`longjmp`, process failure, or non-idempotent side effect during finalization is
outside the admitted recovery model. The host must quarantine and must not
retry such a finalizer. `Published` is rejected at checkpoint construction
because publication is a terminal settlement action, not an executable
provider claim.

The only foundation transitions are:

```text
Live -> Finalizing -> Dead
ProvisionalResult -> Finalizing -> Dead       (abort)
ProvisionalResult -> Published                (accepted owned success)
Dead -> Dead
```

A null or zero payload is never an ownership state.

## Dense certified checkpoints

Each `SettlementCheckpointSpec` contains:

- its dense one-based checkpoint ordinal;
- the complete owner-state vector;
- zero or one admitted normal outcome;
- the exact abort cleanup order; and
- the exact accept cleanup order.

The certificate constructor rejects a vector whose length differs from the
resource count, any checkpoint containing `Finalizing` or `Published`, more
than one provisional result, an empty or NUL-bearing function identity, a zero
call contract, non-dense ordinals, and every cleanup list that is not an exact
duplicate-free permutation of its required owner ordinals.

Abort cleanup is an exact ordered permutation of every non-`Dead` owner,
including a provisional result. For accepted scalar success or semantic
failure, no provisional result may exist and accept cleanup is an exact ordered
permutation of every `Live` owner. For accepted owned success, exactly the named
ordinal must be `ProvisionalResult`; accept cleanup is an exact ordered
permutation of every `Live` owner and publication of that provisional result is
the final action. A checkpoint with no normal outcome admits only abort and has
an empty accept-cleanup list.

The model authenticates settlement from a returned dense checkpoint. It does
not yet record or validate a sequence of physical progress transitions between
checkpoints. Future compiler derivation MUST place recoverable checkpoints only
after an atomic physical action has completed and MUST prove the checkpoint
vector and both cleanup permutations from independently validated cleanup HIR.
The provider may report failure only at such a returnable checkpoint. Missing,
skipped, forged, or physically inconsistent progress is a host/adapter contract
violation and must quarantine.

Recovery checkpoints are adapter evidence, not semantic trace events. The
semantic trace-path certificate remains independent. Future code generation
must derive both from the same validated cleanup plan and the host must validate
both; neither projection may repair the other.

## Decisions and settlement

The closed decision is either:

- `Accept(ScalarSuccess)`;
- `Accept(SemanticFailure)`;
- `Accept(OwnedSuccess { owner_ordinal })`; or
- `Abort(PhysicalResult(nonzero) | MalformedResponse | TraceRejected |
  HostUnwind)`.

An `Accept` decision must equal the checkpoint's admitted normal outcome.
`PhysicalResult(0)` is invalid. `Abort` is always governed by the exact abort
cleanup permutation; malformed output never authorizes the host to discard the
frame or invent liveness.

The target-neutral `settle` operation authenticates the frame against the
certificate, revalidates its checkpoint state, derives actions only from the
selected exact permutation, applies each action to its required state, computes
terminal dispositions, validates the receipt, and caches the terminal result.
It returns both the receipt and the actions that a future physical settlement
provider must perform.

Settlement is idempotent for the same decision. The first call returns the
certified actions; every later call with the identical decision returns the
byte-identical cached receipt and an empty performed-action list. A different
decision on a terminal frame is rejected as `ConflictingTerminalDecision`.
This is model-level proof that a host can avoid retrying a finalizer. It is not
yet proof that any physical finalizer ran once.

Future callable v3 requires separate generated `execute` and `settle`
operations. After atomic owner commit, every `execute` return—including normal
success—must lead to one settlement decision and validated receipt. All
physical buffers and the settlement guard must be prepared before commit.
Physical `settle` must be allocation-free, non-panicking, non-unwinding,
non-trapping, and must perform the returned actions exactly once. Host unwind
after commit must be converted to `Abort(HostUnwind)` by a combined
frame/lease/ledger guard. None of this provider or host wiring exists yet.

## Settlement certificate

The implemented `NativeSettlementCertificate` binds the schema, stable
function declaration ID, nonzero 32-byte call contract, resource count, and
dense checkpoint specs. It emits deterministic canonical JSON and a
domain-separated SHA-256 fingerprint. Construction performs all structural,
state, permutation, outcome, and work-budget checks described above.

This certificate is a bounded target-neutral decision table, not yet a compiled
physical-path DFA. The future compiler must derive it from independently
validated HIR, cleanup inventory, cleanup plan, physical recovery layout, and
result contract. A future descriptor must fingerprint it separately from the
semantic event dictionary and trace-path certificate, and an independently
implemented host parser must reject every noncanonical byte, identity, count,
bound, or fingerprint mismatch before commit.

## Quiescence receipt

The implemented `semaprax.native-settlement-receipt.v1` model binds:

- schema, function, call contract, certificate fingerprint, and invocation;
- the exact checkpoint and decision;
- the exact derived ordered `Finalize`/`Publish` actions;
- one terminal `Dead` or `Published` disposition per owner ordinal; and
- `active_finalizers`, which must be zero.

It has deterministic canonical JSON and a separately domain-separated
fingerprint. Validation reconstructs the expected actions from the certificate,
replays them from the checkpoint, checks every terminal disposition, and
rejects any live, provisional, or finalizing terminal state. Owned success has
exactly one published disposition; every other admitted outcome has none.

This proves **model quiescence** only: the certified action list is exhausted,
every owner-level disposition is terminal, and no modeled finalizer is active.
It does not prove that native code performed the action, that callbacks are
idle, that a loader lease is retained, or that a module may be unmapped.

A future wire receipt must additionally bind the exact physical module
instance, frame generation, recovery-layout identity, response digest, semantic
trace digest, adapter-evidence digest, and exact byte capacities. Only the host
authority may authenticate an accepted physical receipt after independent
parsing and replay. Only then may the ledger publish the one owned result or
retire committed inputs.

Call-level physical quiescence is not module-level quiescence and never promises
immediate unmapping. A module becomes merely unload-eligible after draining,
all frames are settled and released, all owners/results/credentials/callbacks
and finalizer pins are gone, and the platform loader policy permits release.

## Poisoning and quarantine

Poisoning is monotonic. It stops new calls and prevents publication or ordinary
ledger completion for the affected frame. Quarantine retains the exact module
instance, recovery frame, authority context, and diagnostic receipt storage for
at least the process lifetime unless a separately proven platform isolation
mechanism can release them safely.

Quarantine deliberately prefers a bounded leak over use-after-free,
double-finalization, executing unmapped code, or publishing uncertain
ownership. It is a response to violation of the admitted protocol, not a
successful settlement and not evidence that safe-language exactly-once cleanup
was preserved. Production admission must define observable, stable adapter
failure diagnostics for quarantine without exposing secrets, pointers, bearer
credentials, or target-private payloads.

## Required executable evidence

No implementation may claim this RFC, alter `SPX-B104`, or expose callable v3
publicly until all applicable gates are green together.

### Current model evidence

The internal model's focused unit suite currently covers:

- exhaustive abort settlement for all owner-state combinations up to six
  entries and every closed abort reason;
- exhaustive accepted scalar, semantic-failure, and owned-result combinations
  up to six entries;
- exact ordered finalization, unique publication, same-decision idempotence,
  conflicting-decision nonmutation, and zero physical-result rejection;
- structural certificate hostility for invalid identity, zero contract,
  noncanonical checkpoint, duplicate cleanup ordinal, terminal checkpoint
  state, and multiple provisional results;
- independent receipt-field mutation rejection; and
- deterministic, domain-separated certificate and receipt projections.

This evidence proves the bounded owner-level model, not physical settlement.
Before any wire or host connection, the model gate must additionally include
exact minimum/maximum/work-budget boundary cases, fixed known-answer hashes,
broader property generation, and hostile tests for every closed enum and count.
A future wire then requires every-byte, truncation, trailing-data,
reserved-field, capacity, arithmetic-overflow, stale-generation, and
cross-instance mutation evidence through an independent parser.

### Generated provider and host

- failure injection immediately before and after every compiler checkpoint for
  every admitted corpus path;
- scalar success, owned success, semantic failure, malformed response, returned
  physical failure, staged-result corruption, partial cleanup, and host unwind;
- exact physical finalizer counters and order using unique payloads, proving no
  leak or duplicate finalization inside the admitted failure model;
- duplicate identical settlement returning byte-identical receipts with no
  effects, plus conflicting, stale, cross-frame, cross-module, cross-thread,
  incomplete, duplicate, skipped, and reordered settlement rejection;
- panic injection after ledger commit at every host boundary, proving the
  combined frame/lease/ledger guard settles or quarantines without publication;
- draining, active-call, owner/result retention, callback/finalizer pins,
  last-reference release, and unload-eligibility races; and
- O0/O2 equivalence and strict generated C/C++ warnings.

### Platform and safety matrix

- ASan and UBSan on every generated native provider path;
- sanitizer instrumentation of the Rust loader and ownership host, not merely
  linkage of sanitizer runtimes;
- relevant Miri and concurrency-model evidence for safe Rust bookkeeping;
- real Linux, macOS, and Windows dynamically loaded execution, including
  hardened dependency-search collision fixtures;
- Android runtime/device admission and an iOS-compatible static-link profile
  with representative device or simulator execution;
- MSRV, formatting, strict Clippy, docs-with-warnings-denied, package,
  dependency-policy, examples, and full test gates; and
- exact reference/native/Wasm semantic trace, outcome, publication, and final
  liveness equivalence for every source shape opened by the public gate.

## Implementation sequence

1. Complete the partially implemented target-neutral frame and settlement
   model gate with boundary, known-answer, and property evidence.
2. Derive and independently validate the settlement certificate from cleanup
   HIR while callable
   v3 remains unreachable from compiler preflight.
3. Add descriptor-v3 and generated `execute`/`settle` artifacts behind the
   private feature; keep v2 tests unchanged as compatibility evidence.
4. Connect the exact-instance loader and host with the combined settlement guard,
   receipt authentication, poison, draining, and quarantine.
5. Run the complete failure-injection, sanitizer, unload, and platform matrix.
6. Connect ordinary compiler build/preflight only after the full admitted slice
   is proven. Every excluded shape retains its stable fail-closed diagnostic.

Each step must update the completion matrix, architecture, quality gates,
roadmap, migrations, and changelog honestly. An earlier step is enabling
infrastructure, not evidence for a later one.

## Explicit nonclaims and current status

This RFC specifies no stable public C ABI, public Rust API, byte layout, symbol,
descriptor magic, capability token, or loader constructor. It does not implement
imports or finalizers, aggregates, callbacks, async, concurrency, fork recovery,
hot reload, signed code, code-provenance authentication, Android/iOS hosts,
WebAssembly Components, or ecosystem adapters. It does not turn quarantine into
successful cleanup and does not recover from interruption inside a finalizer.

As of this RFC's publication, only the hidden target-neutral owner-state model
is implemented; none of its physical runtime pieces are wired. Callable v2
continues to retire logical ledger state after physical failure without proving
general physical fallback cleanup or quiescence. Therefore the completion
matrix remains Partial, callable v3 has no native execution evidence, and
`SPX-B104` remains closed without exception.
