# RFC 0004: Native call recovery and settlement

- Status: Proposed; private phase model/proof and v3 metadata contract exist,
  physical v3 unwired
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
certificate, progress graph, frame, phase-aware transaction, decision,
candidate/committed receipt evidence, and idempotent settlement operation plus
private compiler derivation from validated cleanup HIR. The authority-free
[settlement-proof v1](NATIVE-CALLABLE-SETTLEMENT-PROOF-V1.md)
format now embeds the exact callable-v2 descriptor and a canonical binary graph;
an independent host parser validates its bounds, hashes, topology, transitions,
and cross-artifact bindings. The separate metadata-only [native callable ABI
v3](NATIVE-CALLABLE-ABI-V3.md) now fixes the current private `SPXNABI3`
descriptor projection, capacities, linkage metadata, hash DAG, and graph. Its
seven future runtime strings/fingerprints are provisional bounded role/schema
reservations that omit full byte/tag/digest/host-HMAC transcripts; they are not
frozen wire codecs. There is no v3 provider code, loader/static
admission, physical finalizer, ownership-host wiring, or public callable-v3
compiler surface. Callable v2 has an independent public
build-only bundle surface plus a feature-gated execution experiment; ordinary
native resource execution still fails with `SPX-B104`, and
the model plus this document satisfy no physical-runtime completion gate.

The key rule is:

> After call commit, the host may retire or publish physical ownership only
> after validating one certified quiescent settlement. If certification cannot
> be completed, it must poison and quarantine the exact module instance; it
> must never infer cleanup from a malformed response or retry a finalizer.

Callable v3 has three ordered irreversible boundaries, and none may be
collapsed into another:

```text
CallCommit -> SettlementDecisionCommit -> host ReceiptCommit
```

`CallCommit` transfers the staged owners into the exact call frame.
`SettlementDecisionCommit` locks one exact accept-or-abort decision for that
frame. Host `ReceiptCommit` independently validates and authenticates the
provider's candidate receipt before publishing one ledger outcome. A provider
terminal disposition, including its internal `Published` state, is evidence
for that last decision and is never itself public host publication.

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
- [Native callable ABI v3](NATIVE-CALLABLE-ABI-V3.md) fixes the current private
  metadata descriptor and capacities while reserving provisional future wire
  roles without granting execution or settlement authority.
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
3. Repeating the same locked settlement decision has no additional physical
   effect and returns the same candidate receipt.
4. Conflicting settlement decisions, cross-frame data, stale generations, and
   incomplete or reordered recovery paths fail closed.
5. Only a host-validated and host-authenticated receipt may commit one ledger
   publication, input retirement, or unload-eligibility transition.
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

The implemented foundation models a frame and a separate phase-aware linear
transaction prepared from one authenticated certificate, one nonzero
invocation, and one dense checkpoint. They store
the function identity, nonzero recovery-contract fingerprint, certificate
fingerprint, invocation, checkpoint ordinal, one owner-level state per resource,
and an optional cached terminal settlement. It contains no raw pointer, target
handle, loader lease, authority, physical payload, or host ledger.

The Rust frame type is deliberately neither cloneable nor formattable, but that
negative API fact is not a uniqueness proof. Test-only snapshot preparation is
deterministic, while production proof consumers can prepare only the certified
post-commit start and walk exact progress edges. The model performs no invocation
reservation and binds neither a process-local module instance nor a nonreused
frame generation. Consequently its action vectors are proof data, not physical
finalizer capabilities. Only future host-ledger admission may create the one
linear physical frame, and it MUST reject duplicate invocation/frame-generation
reservations before ownership commit.

The future physical host MUST allocate every frame, action buffer, and receipt
buffer before owner commit and treat the frame as linear. Module-instance and
frame-generation binding, exact loader retention, thread policy, and physical
sidecars are mandatory future wiring; they are not properties of the current
model and must not be inferred from a recovery-contract fingerprint.

### Implemented model bounds

`semaprax.native-settlement-certificate.v2` enforces:

| Quantity | Maximum |
| --- | ---: |
| Owner-level resource entries per frame | 4,096 |
| Dense checkpoints per certificate | 65,536 |
| `resource_count * checkpoint_count` validation work units | 1,000,000 |

The resource and checkpoint counts must also be nonzero, checkpoint ordinals
must be exactly dense `1..=N`, and every size multiplication is checked. These
bounds make model validation finite; they are not an implemented physical ABI
or byte-capacity guarantee.

The metadata-only `SPXNABI3` descriptor authenticates exact request, response,
frame, decision, action, candidate-receipt, count, and instance-reservation
capacities under versioned hard ceilings. The corresponding runtime wire codecs
are still future work; the current role statements are incomplete and may
change private v3 fingerprints/symbols/known answers when complete independently
tested codecs are frozen. Every alignment, addition, multiplication, host-size
conversion, and allocation must be checked before commit. Exceeding a bound is
a precommit rejection, never postcommit truncation. The present model's
canonical JSON remains test evidence, not a native runtime wire layout.

## Owner-level resource states

The foundation deliberately uses these five closed states:

| State | Meaning |
| --- | --- |
| `Live` | The call owns this admitted input and settlement must finalize it |
| `ProvisionalResult` | The call owns the one possible unpublished result; accept-owned may publish it, while abort must finalize it |
| `Finalizing` | A physical finalizer action has started; this is transient and never an admissible recovery checkpoint |
| `Dead` | No call-owned value remains in this ordinal |
| `Published` | Provider/model evidence says that the owned result was selected for publication; this is terminal, never an admissible recovery checkpoint, and not public host publication |

`Dead` is intentionally an owner-level disposition. It does not distinguish
never initialized, previously finalized, or transferred ownership. The current
direct-input tranche has no acquisition, nested call, local resource creation,
or general transfer checkpoint. Those shapes require an extended, independently
validated state model before admission; a future implementation must not guess
them from `Dead`.

At a recoverable checkpoint, every entry must be `Live`,
`ProvisionalResult`, or `Dead`, and there may be at most one provisional result.
`Finalizing` is rejected because settlement must record it before entering the
physical finalizer, and may record `Dead` only after the finalizer returns
normally. A trap, unwind, `longjmp`, process failure, or uncertain side effect
while `Finalizing` is therefore not retryable: the host must quarantine the
exact frame and module instance and preserve the evidence. `Published` is
rejected at checkpoint construction because it is a terminal provider/model
selection, not a recoverable provider claim and not authority to mutate the
public ledger.

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
recovery contract, non-dense ordinals, and every cleanup list that is not an exact
duplicate-free permutation of its required owner ordinals.

Abort cleanup is an exact ordered permutation of every non-`Dead` owner,
including a provisional result. For accepted scalar success or semantic
failure, no provisional result may exist and accept cleanup is an exact ordered
permutation of every `Live` owner. For accepted owned success, exactly the named
ordinal must be `ProvisionalResult`; accept cleanup is an exact ordered
permutation of every `Live` owner and publication of that provisional result is
the final action. A checkpoint with no normal outcome admits only abort and has
an empty accept-cleanup list.

The model authenticates one all-`Live` post-commit start and a bounded acyclic
sequence of typed progress transitions between dense checkpoints. Private
compiler derivation places checkpoints after each complete physical ownership
action and binds terminal certification to an independently accepted semantic
trace path. The checkpoint vector and both cleanup permutations come only from
validated cleanup HIR.
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

### Ordered commit protocol

The physical protocol MUST represent these three ordered boundaries explicitly:

1. `CallCommit` atomically transfers the staged owners to one exact-instance,
   nonreused frame generation.
2. `SettlementDecisionCommit` locks one exact decision before the first
   settlement action. Recommitting the identical decision is idempotent;
   proposing a different decision poisons and quarantines the frame.
3. Host `ReceiptCommit` independently parses, replays, validates, and
   authenticates the provider's candidate receipt, then changes public ledger
   state at most once. A stale, cross-bound, conflicting, malformed, skipped,
   duplicated, or reordered candidate quarantines without publication.

Host unwind handling is phase-aware. After `CallCommit` but before a known
`SettlementDecisionCommit`, the guard selects and locks
`Abort(HostUnwind)`. After the decision is locked, the guard MUST resume that
exact decision and MUST NOT replace an in-progress `Accept` with an abort. If
the host cannot prove which phase or decision was committed, or observes a
conflicting decision, it must poison and quarantine instead of guessing.

Each finalizer action has its own start/completion boundary. The frame records
the resource as `Finalizing` before invoking the physical effect and records it
as `Dead` only after normal return. Unwind or interruption while `Finalizing`
quarantines the exact instance; neither provider nor host may retry that action.
Quarantine is terminal protocol evidence, not a settlement decision or a
receipt commit.

The existing target-neutral `settle` operation authenticates the frame against
the certificate, revalidates its checkpoint state, derives actions only from the
selected exact permutation, applies each action to its required state, computes
terminal dispositions, validates the receipt, and caches the terminal result.
It returns both a model receipt and the actions that a future physical
settlement provider must perform. Its atomic proof step is not evidence that a
physical provider exposes the ordered commit protocol above.

Settlement is idempotent for the same decision. The first call returns the
certified actions; every later call with the identical decision returns the
byte-identical cached receipt and an empty performed-action list. A different
decision on a terminal frame is rejected as `ConflictingTerminalDecision`.
This is model-level proof that a host can avoid retrying a finalizer. It is not
yet proof that any physical finalizer ran once.

The private `NativeSettlementTransaction` additionally makes the protocol
phases executable as `Executing`, `DecisionLocked`, `ActionInProgress`,
`ProviderSettled`, model `ReceiptCommitted`, and `Quarantined`. It advances only
from the authenticated start; locks one exact decision; records `Finalizing`
before returning an opaque linear finalizer ticket; requires exact ticket
completion before recording `Dead`; and separately caches/replays provider
candidate and model-committed receipt evidence. Every conflict, cross-binding,
skipped action, stale ticket, malformed candidate, or uncertain in-progress
finalizer monotonically quarantines. This Rust model allocates vectors and its
`ReceiptCommitted` phase proves only validation/commit eligibility: it has no
exact-instance reservation, host secret, ledger, or physical effect.

Future callable v3 requires separate generated `execute` and `settle`
operations. After atomic owner commit, every `execute` return—including normal
success—must lead to one settlement decision and validated receipt. All
physical buffers and the settlement guard must be prepared before commit.
Physical `settle` must be allocation-free, non-panicking, non-unwinding,
non-trapping, and must perform the returned actions exactly once. A combined
frame/lease/ledger guard must apply the phase-aware unwind rule above and retain
the exact image through every in-progress action and quarantine. None of this
provider or host wiring exists yet.

## Settlement certificate

The implemented `NativeSettlementCertificate` binds the schema, stable
function declaration ID, nonzero 32-byte recovery contract, resource count,
dense checkpoint specs, sole start, and typed progress edges. It emits deterministic canonical JSON and a
domain-separated SHA-256 fingerprint. Construction performs all structural,
state, permutation, outcome, and work-budget checks described above.

This certificate is a bounded target-neutral decision table and progress graph,
not yet a physical provider protocol. The private compiler derives it from
independently validated HIR, cleanup inventory, cleanup plan, direct-owner
recovery layout, result meaning, and the semantic trace certificate. The
metadata-only v3 descriptor fingerprints it separately from the semantic event
dictionary and trace-path certificate. A `CertifyOutcome` edge additionally
carries its ordinal/outcome witness and a nonzero digest computed over that
transcript plus the trace-certificate fingerprint. The host recomputes the
exact `semaprax.native-recovery-trace-evidence.v1` digest and rejects resealed
witness/digest mutations. This proves only the witness binding; the v3 metadata
parser does not independently accept, reconstruct, or walk the trace-path DFA
certificate. It must still reject every noncanonical byte, identity, count,
bound, or fingerprint mismatch; physical commit and provider admission remain
unwired.

## Candidate and committed receipts

The implemented `semaprax.native-settlement-receipt.v2` model binds:

- schema, function, recovery contract, certificate fingerprint, and invocation;
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

For callable v3, the provider emits only a **candidate receipt**. Candidate
bytes, even when structurally valid, have no ledger authority. The host must
independently bind them to the exact module instance and frame generation,
replay their decision and ordered actions, require zero active finalizers,
validate all terminal dispositions and evidence digests, and authenticate the
accepted receipt with host-only authority. Only the resulting host
`ReceiptCommit` may publish the selected owner or retire inputs, and identical
replay returns the already committed ledger result without republishing it.

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
instance, recovery frame, authority context, locked-decision and action-phase
evidence, candidate/diagnostic receipt storage, and every required code or
finalizer pin for at least the process lifetime unless a separately proven
platform isolation mechanism can release them safely.

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
- one all-live start, executable typed progress, exact cross-edge cleanup-order
  continuity, trace-bound terminal outcomes, and exact corpus graph snapshots;
- structural certificate hostility for invalid identity, zero recovery contract,
  noncanonical checkpoint, invalid/reordered/duplicate progress, duplicate
  cleanup ordinal, terminal checkpoint state, and multiple provisional results;
- exact minimum/maximum/work-budget boundaries and fixed certificate/receipt
  known-answer projections;
- independent receipt-field mutation rejection; and
- deterministic, domain-separated certificate and receipt projections;
- the complete phase-aware transaction path for every closed decision, every
  certified checkpoint and abort reason, exact legacy receipt known answers,
  identical decision/candidate/committed replay, and independent candidate
  validation before model receipt commit;
- unwind before decision lock, after each locked decision, at every finalizer
  index, after provider settlement, and after model receipt commit, including
  absorbing quarantine with exact resource/evidence preservation; and
- hostile internal-state, ticket, decision, action-order, receipt, invocation,
  and certificate-binding mutations across every irreversible phase, plus
  non-`Clone` and non-formatting transaction/ticket gates.

These cases are part of the module's 29 focused tests. This evidence proves the
bounded owner-level and phase-aware model, not physical settlement.
The private compiler derivation separately proves the current direct-trivial
corpus against independently validated cleanup HIR and semantic trace paths.
The private proof envelope adds every-prefix, trailing-data, every-byte,
authenticated hostile-graph, exact-cap, cross-module, changed-trace, and
independent-parser evidence. It binds the exact v2 call contract and trace
certificate but deliberately carries no instance, generation, capability,
finalizer, frame, action, or receipt authority. Those physical-runtime wires
still require stale-generation, cross-instance, failure-injection, and
quiescence evidence.

The pure model now starts from the sole authenticated post-`CallCommit` state
and exercises `SettlementDecisionCommit`, provider-settled, and
model-`ReceiptCommitted` evidence phases, phase-aware unwind recovery, and
persistent `Finalizing` uncertainty. Physical exact-instance reservation and
quarantine, allocation-free provider execution,
host receipt authentication, atomic ledger publication, and loader retention
remain normative v3 requirements below. They must not be inferred from the
model, atomic `settle` helper, or proof envelope.

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
- unwind injection before decision lock, after each exact decision lock, during
  every finalizer, after provider settlement, and around host receipt commit;
  these cases must prove abort selection only before decision lock, exact
  decision resumption afterward, no retry from `Finalizing`, and exactly one
  authenticated ledger publication;
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

1. Maintain the implemented target-neutral frame and phase-aware transaction
   model gate with boundary, known-answer, hostility, and property evidence.
2. Derive and independently validate the settlement certificate from cleanup
   HIR while callable v3 remains unreachable from compiler preflight.
3. Serialize the derived proof through one bounded authority-free envelope and
   parse it independently from callable v3.
4. Maintain the current private descriptor-v3 metadata contract, build-target-
   bound compiler encoder, and
   independent host parser behind the private feature; keep v2/proof known
   answers unchanged as compatibility evidence.
5. Replace all seven provisional role statements with complete independently
   encoded/parsed byte, tag, digest, and host-HMAC transcripts; freeze new
   private v3 known answers; then add generated `execute`/`settle` provider
   artifacts behind the private feature.
6. Connect the exact-instance loader and host with the combined settlement guard,
   receipt authentication, poison, draining, and quarantine.
7. Run the complete failure-injection, sanitizer, unload, and platform matrix.
8. Connect callable-v3 compiler execution/admission only after the full admitted
   slice is proven. The independent callable-v2 build-only bundle remains
   non-executing, and every excluded shape retains its stable fail-closed
   diagnostic.

Each step must update the completion matrix, architecture, quality gates,
roadmap, migrations, and changelog honestly. An earlier step is enabling
infrastructure, not evidence for a later one.

## Explicit nonclaims and current status

This RFC specifies no stable public C ABI, public Rust API, capability token,
or loader/static-registration constructor. The private metadata-only callable
v3 document fixes its descriptor bytes, derived symbols, capacities, and future
wire-role reservations, but those role statements are not complete/frozen
runtime codecs and provider symbols are not implemented or admitted. The
emitter derives only its compiler build target and has no cross-target
configuration. Android/iOS/Windows cross-emission and runtime evidence are
absent; future iOS device, simulator, and Mac Catalyst/macabi targets remain
distinct. The private
`SPXNPRF1` proof format is versioned separately and cannot be executed or loaded.
It does not implement
imports or finalizers, aggregates, callbacks, async, concurrency, fork recovery,
hot reload, signed code, code-provenance authentication, Android/iOS hosts,
WebAssembly Components, or ecosystem adapters. It does not turn quarantine into
successful cleanup and does not recover from interruption inside a finalizer.

As of this revision, the hidden target-neutral owner-state/progress model,
phase-aware linear transaction and its 29 focused tests, private compiler
derivation, bounded binary proof encoder, independent proof parser, and v3
metadata contract exist; the existing loader rejects v3 before path/image
access, and none of the physical v3 runtime pieces are wired.
Callable v2
continues to retire logical ledger state after physical failure without proving
general physical fallback cleanup or quiescence. Therefore the completion
matrix remains Partial, callable v3 has no native execution evidence, and
`SPX-B104` remains closed without exception.
