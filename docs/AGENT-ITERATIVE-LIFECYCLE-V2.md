# Agent iterative lifecycle v2

Audience: runtime integrators and compiler contributors.

This lifecycle runs checked Agent stages in a bounded loop. The reducer alone
chooses whether to continue, complete, suspend, or fail; suspension is data,
not durable restart authority in this version.

Status: **HOSTED GREEN** under the [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
That evidence update does not change the limits below.

`agent_lifecycle::iterative::compile_agent_lifecycle_v2` binds the checked
initialize, observe, authorize and reduce operations from an unchanged
AgentDefinition v1, plus one explicitly selected persistent Step type identity.
It independently checks the whole module and uses the ordinary retained
interpreter preparation and execution path for every deterministic stage.

Step is a monomorphic authored variant with exactly Continue, Complete,
Suspend and Fail cases. Continue and Suspend carry the exact State record's
flat fields in declaration order; Complete carries the Result record's flat
fields in declaration order; Fail carries one i64 code. Field types and every
Step/case/field identity are checked. The admitted leaves are Bytes and the
retained seam's five scalar types. Nested carriers remain outside this profile.
The mapping is derived from checked declarations, never provided by a caller.

Execution initializes once and repeats observe, scripted proposal decoding,
authorize, injected read, and reduce. Only a checked reducer return selects the
next transition. Continue feeds its State to the following turn. Complete,
Suspend and Fail publish their terminal carrier and stop. Suspension is data;
this version does not accept it as durable restart authority.

Every turn runs the authorize stage anew. Its opaque, consumed grant binds the
source-revision-bearing lifecycle digest, turn ordinal, exact State, canonical
proposal, grant case and seal. Cancellation is checked at each deterministic
stage and before dispatch. A caller supplies the only host read implementation;
there is no ambient authority. A failed effect never reaches reduce.

The source-selected `compile_source_agent_lifecycle_v2` bridge derives the
Definition from the same checked module and selected Agent identity.

Caller ceilings bound iterations, deterministic stage count and interpreter
fuel per stage. Hard ceilings of 4096 iterations and 12289 stage records bound
the allocation regardless of caller input. Reducer capacity is reserved before host dispatch. Evidence
binds a length-framed invocation digest of exact task bytes, task budget, all
ordered proposal bytes and all three execution ceilings before any stage
boundary. It records actual stage order, turn and effect counts, authorization bindings and
a terminal-carrier digest, without exposing payloads. Its schema and digest
domain are additive v2; all existing Lifecycle, Definition and Runtime v1
artifacts remain unchanged. This is a retained-interpreter profile.
[Typed operation registries](AGENT-TYPED-EFFECTS-V3.md),
[per-operation durable recovery](AGENT-OPERATION-CHECKPOINT-V2.md), and
[linked Project roles](PROJECT-LINKED-AGENT-LIFECYCLE-V1.md) are implemented
additions with their own contracts and the same hosted-green release baseline;
they do not retroactively widen this v2 wire.

Focused gate: `cargo test --locked -p semaprax --all-features --lib
agent_lifecycle::iterative::tests` (the original six-case focused corpus).
The selector remains the executable reference; its earlier local run is not
the current release's evidence ceiling.

The private frozen-run parity selector
`agent_lifecycle::tests::lifecycle_parity` additionally exercises this same
driver kernel with interpreter, native C11 `-O0`/`-O2`, and Core Wasm stage
dispatch. Its test-only entry supplies the backend explicitly, including the
Wasm source text as data. Both production frozen-run entries (ordinary and
migration-seeded) continue to select the interpreter. The live route does not
gain a backend selector.

The checkpoint route (`agent_lifecycle::iterative::effects::CompiledTypedEffects::run_durable`)
and the migration-seeded checkpoint route (`run_durable_from_seed`) retain their
interpreter default. The additive production `run_durable_with_backend` and
`run_durable_from_seed_with_backend` entries select Interpreter, native C11 or
Core Wasm with an explicit held compiler/runtime capability; the existing
`run_durable_on`/`run_durable_from_seed_on` parity entries remain test-only.
All four use the same persisted,
replay-checked journal driver. Checkpoint identity never depends on which
backend is selected, so the same canonical checkpoint bytes produced under
one backend decode and continue under any other -- including a genuinely
partial checkpoint with real dispatches still outstanding, not only an
idempotent replay of an already-complete run -- with identical status, value,
usage ledger and stage/iteration counts, and identical refusal of a tampered,
foreign-root or stale-ceiling checkpoint on every backend. A selected Wasm
executor that is not handed this exact registry's own retained source is
refused before any identity, decode, store write or handler dispatch, on
both the fresh and the resumed leg. The joined Runtime v2 and checked
migration wrappers also expose the held selector. Their local parity evidence
does not establish hosted deployment, sanitizer coverage or full
instruction/cleanup-event equivalence.

Focused gate: `cargo test --locked -p semaprax --lib
agent_lifecycle::iterative::effects::durable::tests`. It requires an explicit
held `clang` and `node`, like the other cross-backend gates above.

This authored local gate compares proposal admission, fresh authorization
bindings and consumed requests, an injected read operation, continued State,
terminal Result, stage order, turn/effect counters, cancellation and
iteration/stage ceilings. It requires `clang` and `node`; a tool-absent skip
is not execution evidence. Native now additionally settles borrowed stage
arguments and returned `Bytes` at the real boundary, with local allocation,
free, call, cancellation, receipt, and omission/duplication controls. The
reported native cleanup count remains limited to result-copy-out settlement;
it is not full instruction/finalizer parity. Core Wasm now reports that same
event only for a record `Bytes` projection that the replay-verified generated
Node facade returned as an owned `Uint8Array`: that return follows its private
arena's consume and settlement. The stage observer checks this typed result,
and the host requires an exact tagged row at the selected projection before
counting it; missing, extra, malformed and duplicate rows fail closed. This
does not report Wasm memory frees, variant-indexed-`Bytes` cleanup, or full
stage finalizer parity. The public target-stage route instead records one
backend-neutral reservation per settled stage: the checked per-stage cap times
the recorded stage count, bounded by the run-stage cap. Pre-dispatch
cancellation settles before this accounting; otherwise the sealed dispatch
rejects an invalid retained-call stage cap before native/Node admission on
every selector. This is comparable finite admission fuel, not instruction,
full cleanup-event, timing, or byte-identical cross-engine evidence. This private
selector does not extend the released production or hosted support claim.

The canonical v2 document explicitly records initialize-once, the iteration
order, Continue targeting observe, terminal cases, and exact Step case/field
mappings. It does not embed the v1 lifecycle wire or acyclic-only nonclaims.

## Stage semantic work v1

Stage semantic work v1 is the backend-neutral accounting of the work one
checked stage call performs. It adds a metered dispatch to the same sealed
stage executor seam; it does not change any lifecycle wire, reservation or
digest above. The frozen, migration-seeded, live and checkpoint routes keep
their unmetered dispatch and do not gain this selector in this version.

**Semantic fuel.** One unit is charged at each of two checked semantic events,
and at no other point:

1. entering the frame of a source function: the stage entry and every direct
   call to a monomorphic source function, charged in the callee after
   call-depth admission and before its preconditions; and
2. entering a `while` body, charged after its condition evaluated `true`.

Call-depth admission itself is unconditional and backend-neutral, not scoped
to a metered dispatch: the interpreter, native C11, and Core Wasm each refuse
one more frame at the identical fixed ceiling (256) before that frame's own
semantic charge and preconditions, reporting `CallDepthExceeded` rather than
diverging into fuel exhaustion or an uncontrolled host-engine stack trap.
Core Wasm enforces it with an always-on module global incremented at every
function's entry, present in every compiled module whether or not a semantic
meter is selected for that build, across both Wasm emitters: the aggregate
builder (`aggregate::call_admission`) decrements it at a shared exit every
recoverable status already converges on, so a refused frame's decrement runs
unconditionally alongside its increment; the legacy scalar-core emitter
(`scalar_call_admission`), reached by a plain scalar or owned-Bytes/String
program with no aggregate lowering, instead reports a refused frame through
its existing `spx_contract_fail`-plus-`unreachable` failure channel, which
traps the whole call activation rather than returning through it, so only
its one normal-return path decrements. Because that trap does not discard
the module instance, every genuine external entry the legacy emitter
produces also resets the counter to zero as the first thing it does, so a
trapped call cannot leave a later call on the same instance refused at a
phantom depth.

A dispatch is admitted with a limit in `1..=1_000_000`. A charge made while the
charged count equals the limit is refused and not counted. Refusal is sticky:
the call stops at that exact semantic event with `FuelExhausted`, selects no
other status, and settles every live compiler-owned value through the
backend's ordinary failure path. The reported `SemanticWork` carries the
charged count, the admitted limit and whether the limit stopped the call.
Every backend that reports it must report the same count at the same event.

**Metered profile.** Before any compiler or Node process exists, the metered
dispatch admits the stage entry's reachable direct-call closure. Direct calls
to monomorphic source functions and `while` loops are the only metered
constructs. Function values, closures, generic instances, host or native
imports and yields are refused with a stable `semantic_work.profile.*`
diagnostic, as is a limit outside the interval. Cancellation keeps its
precedence over both. There is no fallback to an unmetered or interpreter
execution, and a backend that returns without reporting the admitted limit is
refused.

**Backend instruction counts are separate.** The interpreter's per-node
`steps_used` stays a backend-specific instruction count under its own
`max_steps` budget; native C11 and Core Wasm report `0`. Instruction counts are
never compared and are not semantic work. If the interpreter's step budget
stops a call first, the outcome is `FuelExhausted` with
`SemanticWork::exhausted = false`.

**Cleanup events.** A cleanup event is one canonical cleanup-plan finalizer a
backend actually performed: the owning function and the plan's liveness-flag
identity of the finalized compiler-owned slot. Events are reported in execution
order and are never sorted. Native C11 records an event inside the finalizer's
own liveness guard in its shared epilogue and plan scope exits; Core Wasm
records it inside the same guard of each plan finalize action. A refused charge
settles the frame's live slots in the canonical union order of all terminal
exits: the deterministic precedence-preserving order that native C11 already
uses for every failure, ties broken by cleanup place. Core Wasm uses that same
order at a refused charge and its planned exits everywhere else. The
interpreter's value model performs no plan finalizer; it reports no event
sequence (`None`) and is never compared on this axis. Its result copy-out
events remain the existing boundary `cleanup_events`, compared on every
backend.

**Transport.** Native C11 selects the private status domain
`semaprax.agent-stage-semantic-fuel.v1` code 1 and prints one
`SEMANTIC-WORK v1 <entry> <fuel> <exhausted> <overflow> <domain> <code>
<count> <events...>` row after the existing result and settlement rows. Core
Wasm propagates private raw status 12, outside the public `1..=10` facade
range, and exports its meter globals (`spx_semantic_*`); the executor's
observer captures the one instance the package facade creates, resets the
meter before every projection call and writes one
`semaprax.agent-wasm-stage-semantic-work.v1` row per executed call. Both
parsers are strict: omitted, extra, malformed, contradictory or divergent rows
fail closed. The event capacity is 256 on native C11 and 64 on Core Wasm, and
overflow refuses the observation rather than truncating it.

Focused gate: `cargo test --locked -p semaprax --lib
agent_lifecycle::tests::semantic_work_parity`. It runs one stage on the
interpreter, native C11 `-O0` and `-O2`, and Core Wasm, and compares the
outcome, copy-out events and semantic fuel on success, at the exact limit, on
a checked failure before and inside the loop, and on exhaustion at a
helper entry with a live owned argument, at a mid-loop call entry, at a
mid-loop body entry and at the last call entry; it compares the compiled
backends' performed-finalizer sequences on each path. It requires the held
`clang` and `node` fixtures. This is local evidence for the private seam
only: it is not public-route, hosted, sanitizer or instruction-count evidence.

## Canonical retained context for explicit hosts

`agent_lifecycle::canonical_retained_value_json` exposes the lifecycle's
existing canonical retained-value encoding as read-only host context. It does
not grant a capability, decode a proposal, modify a stage binding, or change
any lifecycle wire. An explicit host may carry those bytes into a provider
request only after it has separately acquired the host capability; the source
feedback driver remains the proposal decoder and bounded retry owner.
