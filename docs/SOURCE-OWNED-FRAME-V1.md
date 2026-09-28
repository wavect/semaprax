# Source owned frame v1 — bounded contract

Status: **reviewed design; implementation in progress; no completion claim**.
Audience: compiler, interpreter and durable-runtime implementers and reviewers.
Base: `5ae54dd4`. This is a proposed additive R20/#296 dependency slice, not
approval to close R20 or widen any backend's support policy.

Owners: [Resumable Effects](RESUMABLE-EFFECTS-V1.md),
[Source Continuations](RESUMABLE-EFFECTS-CONTINUATION-V1.md), especially
sections 11.6 and 12.2, and [Source Model Wait](SOURCE-MODEL-WAIT-V1.md).
The existing Agent typed-carrier contract remains authoritative for deployed
role bindings. This proposal does not change those bindings.

## 1. Exact first profile

An ordinary checked free function has exactly one owned parameter, a flat,
non-generic nominal record, and returns that same nominal record. The record
has 1–8 fields in declaration order, with at least one direct `Bytes` field;
all remaining fields are the existing admitted Copy scalars. No nested
record, variant, String, borrow, shared owner, resource or handle is admitted.
Every declaration and field has an explicit persistent identity.

There is exactly one direct, top-level sequential yield with Copy-scalar
request and response. The body consists of Copy-only prefix statements, one
`let answer = yield request`, Copy-only suffix statements, and a final whole
move of the original parameter. Copy field reads do not move owned leaves.
No field/whole reassignment, record update, partial move, additional owned
local, branch, loop, nested yield, call, allocation, ordinary effect or
reachable yielding callee is admitted. Contracts are literal booleans or
call-free Copy expressions over admitted scalar facts; they cannot move or
borrow Bytes. Preconditions belong to start; postconditions belong to resume.

This first slice parks and returns State intact. It does not yet change State
across a wait or consume an owned answer. Unsupported shapes retain their
existing stable refusals: `SPX-T302` effects, `SPX-T305` borrows, `SPX-T306`
resources, and shape-aware `SPX-T303`/`SPX-T307` owned/aggregate exclusions.
A supported shape must not reach lowering only to fail with generic H006.

The concrete dependency shape for FixtureAgent is:

```spx
@id("fixture.agent.park-state")
fn park_state(state: own State) -> State
    yields i64 -> i64
{
    let answer = yield state.budget;
    state
}
```

Here the existing State has `objective: Bytes`, `budget: i64`, and
`epoch: i64`. This example is a proposed ordinary helper, not a modification
to Agent role declarations. It must be compiler-checked in the owning gate
before being presented as supported source. The existing Copy
Observation-to-Proposal bridge remains separate: integrating this State
owner into that Agent session is subsequent reviewed work.

## 2. Compiler proof, not carrier resemblance

The compiler resolves the exact function and independently validates HIR,
loan metadata and its canonical cleanup plan. At the suspension it proves:

- exactly the parameter's whole storage is live;
- its nominal identity, ordered fields, owned-leaf storage paths and live
  flags match the declaration and cleanup inventory;
- every owned leaf is active, and none was transferred, partially moved,
  renewed, dropped or borrowed before the site;
- the suffix transfers the one whole root to the result, with no other owned
  storage becoming live;
- suspension abandonment and terminal failure have exact compiler-derived
  cleanup vectors; success has the correct result-transfer exclusion.

The liveness query must handle **actual record storage and field liveness**.
Extending a predicate that currently accepts only `StorageId::Value` Bytes
leaves, flattening the record into unrelated byte vectors, checking a host
counter, or applying a helper model is insufficient evidence. The executable
query consumes the checked plan's transitions and identities. Inventory
order stays structural; cleanup vectors stay in canonical runtime order.
Neither codec nor driver sorts, repairs or recomputes that order.

The lowering binding commits to the exact checked program, function, site,
parameter/result/channel shapes, storage and leaf identities, live flags,
cleanup vectors, and profile version. Mutating any of those invalidates the
binding even if payload bytes still look identical.

## 3. Consuming opaque API and argument commit

The following sealed API roles are fixed by this contract (Rust modules may
re-export these names without exposing their representations):

- `CheckedOwnedFramePlan`: opaque immutable compiler derivation.
- `OwnedFrameArgument`: non-Clone opaque owner, created by checked carrier
  admission; no public fields or unchecked byte constructor.
- `OwnedFrameInvocation`: non-Clone active owner plus exclusive journal lease.
- `InertOwnedFrameCheckpoint`: authenticated data, never an active owner.
- `OwnedFrameResult`: non-Clone owned result, available only at result delivery.

Carrier bytes may be inspected or copied as data. That cannot clone a language
owner. Public start consumes the argument by value; returning a borrowed
slice of clonable `ResumableChannelValue` is not an owning entry point.

Admission, capacity checks and staging happen before argument commit. General staging order remains left to right; with one
argument it has exactly one staged root. A precommit failure returns the
untouched opaque argument together with the diagnostic. The durable
`ArgumentCommitted` record is appended and synchronized before the invocation
may evaluate preconditions or execute the prefix. A false precondition is a
postcommit terminal failure with the parameter cleanup obligation; it does not
return caller ownership. After this boundary no error returns an argument
owner to the caller. A crash after the append leaves recovery, not the caller,
responsible for that owner. An ambiguous commit append returns `StartUncertain(PoisonedOwnedFrameInvocation)`,
which retains the argument internally and exposes no argument/result extraction.
It never returns `OwnedFrameArgument`, even when no complete row was observed
by that write attempt. Only an authenticated re-open under the same pinned
authority can decide the committed tail. A proven precommit rejection is the
separate `StartRejected { argument, diagnostic }` case.

Prepare constructs the plan and pure carrier data only. It grants no key,
filesystem, host-call, answer, cleanup or publication authority.

Immediately before any Created/ArgumentCommitted write, start borrows and
resnapshots the actual supplied opaque argument. It compares the plan binding,
nominal and ordered field identities/types/count, every scalar bit and Bytes
value, exclusive backing, argument digest, and all prepared admission, fuel
and scope facts. A prewrite mismatch returns that original argument unchanged.
Prepared facts for another owner cannot justify commit. Once a commit syscall
is attempted, an ambiguous outcome cannot return the argument.

### 3.1 Owner lifetime and aliases

There is one logical root credential per committed invocation, regardless of
how many inert byte snapshots or internal reference-counted aliases exist.
`Value::Record` and Bytes backing currently use Arc; ordinary
`clone_value` also aliases the result for postconditions (`interpreter.rs`
`call_frame_inner`). The new mode uses borrowed Copy-only contract projections, retaining the exact
pending root rather than invoking ordinary `clone_value` for ensures. The old
scalar `evaluate_entry`, `call_frame_inner` automatic Drop and ensures-clone
paths remain unchanged. It drains all evaluator environment, result-binding,
borrow-view and parked-frame aliases on both success and failure. It cannot
settle through a newly decoded look-alike record. Before last-owner settlement
or outward transfer, audit the actual root and owned-leaf backing references:
no unaccounted Arc alias may survive. An alias mismatch fails closed with the
root retained as unsettled, never a fabricated successful cleanup receipt.

| Action | Ownership and physical meaning |
| --- | --- |
| Park | Moves the root into the invocation; drops only non-owning evaluator aliases. No semantic cleanup. |
| Close live invocation | Explicit abandonment under current caller authority, then the normal durable terminal/cleanup protocol; no implicit success. |
| Drop or unwind before commit | Disposes the opaque argument through its checked argument-disposal plan, once; no durable owner exists. A normal StartRejected instead returns it untouched. |
| Drop/unwind after commit or uncertain commit | Drops process backing references only; performs no source finalizer, host cleanup, answer or result claim. Durable logical obligation remains unresolved and exclusively recoverable. |
| Drop inert decoded checkpoint | Disposes data backing only, with no owner credential or semantic cleanup. |
| Explicit failure settlement | Drains aliases and performs the real root's ordered checked leaf cleanup once, inside the recorded cleanup window. |
| Successful result claim | Moves the root credential to the one non-Clone result and removes it from invocation cleanup authority. |
| Result Drop | After transfer to caller, drains its private aliases and disposes that result through its checked result-disposal plan once. It does not alter/repeat invocation cleanup. |
| Consuming result handoff | `OwnedFrameResult::into_argument(self, checked_plan)` transfers the same whole owner to a newly admitted opaque argument. Rejection returns the untouched result; no clone, naked Value, byte-export constructor or second credential. |

Non-owning backing disposal may free allocations after the last process Arc
is gone; it is not evidence that a durable language cleanup obligation was
observed or settled. Forgotten/drop-after-commit sessions must report unresolved
obligations in recovery, not zero-owner success based on freed backing. Rust
unwinding cannot hide an uncertain commit by running an argument-return path.
The compiler/evaluator foundation exposes explicit park/resume/settle and
sealed result consumption; it makes no durable recovery claim until section 4's
store authority and wire packet have passed their own gates. Its consuming
interface is fixed as follows (all named owner-bearing types are opaque and
non-Clone; outcome accessors expose data/status only):

```rust
compile_owned_frame_plan(program: &ResolvedProgram, function: &DeclarationId)
    -> Result<CheckedOwnedFramePlan, Diagnostic>;
// Inert data, not an owner or a restoration credential.
pub struct OwnedFrameInput {
    pub declaration: DeclarationId,
    pub fields: Vec<OwnedFrameInputField>,
}
pub struct OwnedFrameInputField {
    pub identity: DeclarationId,
    pub value: OwnedFrameInputValue,
}
pub enum OwnedFrameInputValue {
    Bytes(Vec<u8>),
    Scalar(ArgumentValue),
}
admit_owned_frame_input(plan: &CheckedOwnedFramePlan, input: OwnedFrameInput)
    -> Result<OwnedFrameArgument, OwnedFrameInputRejection>;
admit_owned_frame_argument(plan: &CheckedOwnedFramePlan, input: RetainedValue)
    -> Result<OwnedFrameArgument, OwnedFrameArgumentRejection>;
start_owned_frame(plan: &CheckedOwnedFramePlan, argument: OwnedFrameArgument,
                  budget: &mut OwnedFrameBudget) -> OwnedFrameFoundationStep;
resume_owned_frame(parked: OwnedFrameParked, answer: ArgumentValue,
                   budget: &mut OwnedFrameBudget) -> OwnedFrameFoundationStep;
settle_owned_frame(terminal: OwnedFrameStagedTerminal)
    -> Result<OwnedFrameSettledOutcome, OwnedFrameSettlementRejection>;
```

OwnedFrameInputRejection owns the original unchanged OwnedFrameInput and the
diagnostic. Admission borrows the input to validate the nominal declaration,
exact declaration order, field identities/count/types, payload bounds and all
eight admitted scalar types before creating any fresh private Arc owner. The
scalar types are i64, i32, u8, usize, char, bool, f32 and f64; Unicode scalar
validity and the profile's usize u32 range are checked. Float32/Float64 retain
their exact bits, including NaN payloads and negative zero. Borrowed scalar
variants are refused. The input contains no semantic cleanup obligation or
restoration credential; dropping rejected or otherwise inert input only
disposes ordinary host data.

The existing admit_owned_frame_argument(RetainedValue) is a convenience subset
using the same pure borrowed validator, not a limit on this approved profile.
Its OwnedFrameArgumentRejection returns the original unchanged RetainedValue,
including its original variant, and the diagnostic; the old enum remains
unchanged. Neither admission path constructs a private owner before validation
succeeds. Private field-substitution and float-bit tests observe the real
interpreter backing without adding a public result-payload projection.
FoundationStep is exactly Parked(OwnedFrameParked) or
Terminal(OwnedFrameStagedTerminal). Fuel/cancellation/guard/answer failures
retain the pending root in Terminal, never a bare error that silently disposes
it. The budget is caller-owned metering/cancellation data, not host authority.
SettledOutcome is exactly Completed(OwnedFrameResult, release_receipt) or
Failed(primary_status, release_receipt). SettlementRejection retains the
unsettled terminal root and reason. None expose private interpreter Value.
Result offers consuming `dispose(self)` and `into_argument(self, checked_plan)`;
no result clone or borrowed owning-payload projection exists.

Foundation park/terminal Drop or unwind disposes process backing but exposes no
semantic release receipt, successful terminal status or durable recoverability.
Tests distinguish it from explicit settle. Once a durable session owns these
values, the authoritative journal retains the unresolved logical obligation
as specified above. Public APIs must document this distinction; a freed Weak
pointer is not itself a semantic settlement receipt.

Observe actual two-Bytes record backing, including empty and embedded-zero
payloads, using private Weak references and strong-count assertions around
park, resume, staged terminal, failure and final owner disposal. No test adds
an externally retained strong alias to simulate the successful path. A hostile
alias test instead proves retained/unsettled refusal. Declaration order and
compiler canonical vectors govern serialization and release: the current
interpreter's BTreeMap field-key order never supplies cleanup order.


## 4. Replay and restoration

Fresh evaluation binds the consumed record root once. At park the evaluator
extracts the compiler-proven whole record into the owned frame. Resume binds
that frame root directly, without recreating an owned argument or evaluating
an owning prefix. The pure Copy prefix is replayed only to recheck request,
site, argument facts and binding; all that work is metered.

Restoration is scoped to one caller-authorized **authoritative** directory and
journal file. The caller supplies held directory/file identities (device/inode
or equivalent platform identity), and operations remain relative to those held
handles with no symlink traversal. Fresh creation is exclusive. Every reopen
checks those pinned identities before authentication or ownership restoration.
Replacement, copied journals/snapshots, hardlink aliases under another
registration, and rebinding the invocation to another directory/file are
outside this authority and rejected. Exclusive locking is on that registered
identity, not merely a pathname that could name a different file.

The caller must protect the authoritative store from full authenticated-tail
rollback, copying and replacement for the invocation lifetime, including after
ResultClaimed. Ordinary HMAC chaining/flock do not detect an attacker restoring
an earlier complete valid file or taking a copied file to another machine.
This contract provides no global uniqueness, anti-rollback hardware, arbitrary
backup restore or cross-directory/fork recovery. The store registration denies
those operations; if its monotonic authoritative history cannot be assured,
restoration fails closed. A torn-tail policy cannot justify deleting a complete
valid record. This same trust scope applies to argument, cleanup and result
claims, not only to dispatch.

Restore requires, in this order:

1. caller-supplied checked program, current expected scope and checkpoint key;
2. caller-authorized journal directory and its exclusive invocation lock;
3. authenticated canonical journal replay, including observed-tail policy;
4. exact committed argument, plan, checkpoint, generation and frame agreement;
5. structural validation of the full live record and canonical cleanup plan;
6. construction of one opaque restored owner bound to that lease.

Decode verifies data and returns only an inert checkpoint. It never inserts
an owner into an evaluator, dispatches, cleans up, delivers a result or grants
answer authority. No public conversion from inert checkpoint to argument or
result exists. Created-only recovery returns `UncommittedStart` evidence with no owner,
argument token, result or disposal obligation reconstructed from Created data.
Before a durable ArgumentCommitted record, those bytes are inert input facts.
Recovery never accepts an additional caller argument owner to replace the
committed root. Independent expected scope/key facts are not
read from the untrusted checkpoint to justify its own admission.

## 5. Durable states and authority

One journal owns argument commit, frame, dispatch, answer, settlement and
result delivery. It is not accompanied by a second authority-bearing store.
The proposed state sequence is:

```text
Created -> ArgumentCommitted -> StartReserved -> Yielded
        -> Dispatched -> Answered -> ResumeReserved
        -> Completed | Failed
        -> CleanupStarted -> CleanupSettled
        -> ResultClaimed (success only)
```

### 5.1 Phase-specific replay grammar

Structural fold performs no source evaluation. A Created-only tail is inert
and cannot reserve/evaluate start. ArgumentCommitted owns the root even before
StartReserved exists. The pre-yield root is exactly its immutable argument
payload plus committed storage/leaf flags: this profile does not mutate it.
Recovery may allocate fresh process backing for that **same committed logical
owner** under its exclusive lease; it never reprojects a fresh argument token
or transfers caller ownership a second time. Live interrupted evaluation
retains the existing root instead of allocating a replacement.

ReplayReserved has a closed `basis` object; variants are mutually exclusive:

| basis.kind | Exact additional fields | Allowed next work |
| --- | --- | --- |
| `pre_yield` | `created_sequence`, `argument_committed_sequence`, `argument_digest` | Recheck or retry start on the existing committed root. No checkpoint digest exists yet. |
| `yielded` | `yielded_sequence`, `checkpoint_digest` | Recheck parked state/request; preserve dispatch phase. |
| `answered` | `yielded_sequence`, `checkpoint_digest`, `answered_sequence`, `answer_digest` | Recheck recorded answer or retry pure resume; no host entry. |
| `terminal_completed` | `completed_sequence`, `result_digest`, `cleanup_digest` | Structural terminal restoration only; no source replay or reservation. |
| `terminal_failed` | `failed_sequence`, `argument_digest`, `cleanup_digest` | Structural restoration of pending obligation only; no source replay or reservation. |

The two terminal bases are used by the internal restore result only, never
ReplayReserved/ReplayValidated. They do not pretend a failed-before-yield
terminal owns a Yielded checkpoint. Completed restores one unpublished result
only before ResultClaimed, as permitted by the authoritative store history;
Failed restores only its pending obligation before CleanupStarted. Settled,
CleanupInDoubt and ResultDeliveryInDoubt tails expose evidence/status and
never re-run source or remint a result. CleanupInDoubt retains the unresolved
logical obligation; it does not create another callable cleanup owner.

A StartReserved/ResumeReserved row proves budget spent, not evaluation success.
An interrupted start/resume retries with a new respective reservation after a
charged replay reservation and ReplayValidated for its causal basis. A prior
StartReserved/ResumeReserved is then closed as interrupted in the structural
fold; it is never closed a second time by the new evaluation. A first
start/resume uses its first reservation directly; recovery never reuses an
old reservation as fresh fuel. Repeated interrupted ReplayReserved rows are
allowed for the same original causal basis and remain spent. ReplayValidated
references the latest outstanding replay reservation, repeats its exact basis,
and reports consumed steps; only then may another phase transition occur.
A pre-yield validation refers to argument facts, not an invented null or future
checkpoint digest. Successful start closes its current StartReserved with
Yielded/Failed; resume closes ResumeReserved with Completed/Failed. A replay
validation does not itself close an interrupted start/resume or authorize
Yielded/dispatch. Each source-evaluating retry requires that fresh reservation.

Fuel exhaustion can select Failed without another evaluation/reservation and
retain the root for cleanup. After terminal selection, structural recovery
needs no replay fuel. Fold rejects stale causal references, wrong basis,
missing validation, unused or multiply closed reservations, cross-phase rows,
and any phase change after result claim. Cleanup and publication authority do
not arise from replay validation.

`Dispatched` is synchronized before the injected host operation. A recovered
Dispatched-without-Answered tail is in doubt: never redispatch. A genuine
checkpoint does not authorize an answer; the existing explicit current
capability policy and checked signature govern both dispatch and answer.
An authenticated Answered row replays its exact scalar answer, without host
entry. Wrong-type answers are rejected before append and select the existing
sticky failure class when the host boundary has consumed the request.

The new owned evaluator returns a staged terminal candidate plus the exact
pending owned obligations. It must not physically drop the State root on an
ordinary evaluator return before the durable cleanup boundary. Existing modes
that automatically drop owned locals at exit cannot be reused unchanged.
Checked postcondition failure retains that candidate root as a pending failure
obligation. Source transfer/cleanup facts are still compiler-owned; only their
physical settlement is deferred to the journal-authorized boundary. A terminal
append failure keeps the candidate owner in the poisoned live session, or
requires exclusive authenticated recovery after a process crash. No driver
callback cleans up a root the evaluator has already settled.

Existing `CapabilityPolicy::allows(id)` authorizes only an ID. Each dispatch,
answer, abandonment, cleanup and result action also checks the independent live
expected scope/epoch, registered directory/file identities, generation and
legal phase/causal references. Current policy must allow the checked function
ID. Replayed bytes do not supply these expected facts. Policy denial leaves the
logical obligation unsettled and grants no cleanup or publication authority.

Terminal failure selects one primary status; cleanup cannot replace it.
Abandonment after argument commit consumes the parked owner through the
failure cleanup path, including a recovered in-doubt dispatch after an
explicit caller decision to abandon. No automatic retry follows.

CleanupStarted is synchronized before invoking the owned settlement sink.
The sink receives the compiler-derived whole-record obligation and its exact
ordered leaf operations. It attempts every required operation even when an
earlier operation fails, recording the per-operation outcome in the bounded
CleanupSettled receipt. A default/no-op sink cannot claim owned settlement.
Callbacks observe the real evaluator's Bytes owners, not fabricated values
or test-only ledger entries. A recovered CleanupStarted-without-CleanupSettled
tail is CleanupInDoubt and never reruns settlement. `confirm_failed_cleanup` requires the caller to supply an explicit
`CleanupConfirmation` grant under the pinned live lease and current capability
policy. It is permitted only for CleanupInDoubt with a Failed terminal. The
grant binds scope, function, generation, terminal sequence, CleanupStarted
sequence and exact cleanup digest; a genuine checkpoint, HMAC key or lock is
insufficient. The
CleanupSettled row with `receipt.kind = "host_confirmed"` records that grant
digest, preserves the primary failure,
and reports no observed physical receipt. Completed terminals cannot use this
route; HostConfirmed never permits ResultClaimed. No no-argument confirmation
method exists.

Success retains the root as the unpublished result owner while non-result
cleanup finishes. Failed postconditions settle that root instead. Only
Completed plus successful required non-result cleanup can reach result claim.
`ResultClaimed` is synchronized before handing the opaque result to the
caller. Recovery of that row never hands out a second owner. A crash between
claim append and delivery is ResultDeliveryInDoubt; it returns evidence, not a
new result. This conservative loss window must be explicit in the public API.
A settled failure has no result claim.

The interpreter performs each compiler-ordered real leaf release; the host
callback observes that completed physical boundary and cannot substitute for
it. Catch callback unwind separately for each operation, record its observation
outcome as `failed`, and continue all remaining real releases and callbacks.
Returned callback errors have the same outcome. These outcomes describe
observation success/failure, not a newly fallible Bytes deallocation. Preserve
the selected source failure. A successful callback cannot create a receipt for
an operation that was not physically released, and a no-op observer cannot
replace finalization. Process abort/crash or uncertain CleanupSettled append
leaves CleanupInDoubt; recovery never repeats release or observation.

## 6. New encodings and frozen preservation

Use new independent identities; do not reinterpret any current scalar,
control-owned, aggregate-channel, whole-function Copy, or Source Live codec:

| Item | Proposed identity/domain |
| --- | --- |
| Checked plan identity | `semaprax.source-owned-frame-plan.v1` |
| Outer plan digest | `semaprax.source-owned-frame-plan.v1\0` |
| Cleanup-plan digest | `semaprax.source-owned-frame-cleanup-plan.v1\0` |
| Checkpoint schema | `semaprax.source-owned-frame-checkpoint.v1` |
| Checkpoint MAC | `semaprax.source-owned-frame-checkpoint-authentication.v1\0` |
| Journal schema | `semaprax.source-owned-frame-journal.v1` |
| Journal record MAC | `semaprax.source-owned-frame-journal-record.v1\0` |
| Journal name hash | `semaprax.source-owned-frame-journal-name.v1\0` |
| Argument digest | `semaprax.source-owned-frame-arguments.v1\0` |
| Frame digest | `semaprax.source-owned-frame-payload.v1\0` |
| Generation digest | `semaprax.source-owned-frame-generation.v1\0` |
| Checkpoint digest | `semaprax.source-owned-frame-checkpoint.v1\0` |
| Request digest | `semaprax.source-owned-frame-request.v1\0` |
| Answer digest | `semaprax.source-owned-frame-answer.v1\0` |
| Result digest | `semaprax.source-owned-frame-result.v1\0` |
| Pending-cleanup digest | `semaprax.source-owned-frame-cleanup.v1\0` |
| Confirmation digest | `semaprax.source-owned-frame-confirmation.v1\0` |
| Evidence digest | `semaprax.source-owned-frame-evidence.v1\0` |

Schema selection comes from the checked profile, never a stored discriminant
or filename. Existing v1/v2 journal and source checkpoint v1–v7 bytes, domains,
APIs and refusal fixtures remain unchanged. Source Live v7 does not gain an
owned-frame interpretation. This first route is interpreter/durable only;
ordinary native/Wasm emission still refuses B116/W126. Target preparation must
refuse this profile with H006 until an independently gated implementation is
added; scalar target preparation is unchanged.

New JSON has recursively lexicographic object keys, no insignificant spaces,
UTF-8 strings, declaration-ordered field arrays, numeric scalar encodings from
the current checked carrier codec, and lowercase hex for Bytes. Unknown,
missing or duplicate keys are rejected before structural admission. Re-encode
and compare exact input bytes. Final envelope/record has one LF; MAC input is
the canonical object without its authentication field or final LF, preceded
by the exact domain. Journal MAC also commits the sequence number and prior
record MAC, as distinct fields in that canonical object. Digests render as
`sha256:` plus 64 lowercase hex digits. MAC renders 64 lowercase hex digits.

Checkpoint fields are exactly schema, scope, function, plan digest, signature,
argument digest, frame, site, request, journal generation, journal sequence,
cumulative reserved fuel, and authentication. These render respectively as
`schema`, `scope`, `function`, `plan_digest`, `signature`, `argument_digest`,
`frame`, `site`, `request`, `journal_generation`, `journal_sequence`,
`reserved_total`, `authentication`. Scope binds program root,
invocation and policy epoch. Frame contains nominal declaration, ordered
field identities and values, whole storage identity, leaf live flags,
and canonical suspension/failure/completion cleanup facts. No host handles,
keys, pointers or completed result are encoded in the checkpoint.

The row is named `Created` everywhere; there is no `Started` alias. It binds
profile, exact scope/function/plan/signature, canonical admitted
argument and its digest, maximum steps/fuel, and fixed capacity limits. Generation is the hash of the canonical Created facts (without common
row/MAC fields) under its generation domain. A reused journal identity is
refused by fresh start. Recovery derives generation again from trusted
expected facts plus the authenticated Created carrier. Each row binds the
same generation and true combined sequence. Yielded stores the
checkpoint and its digest; Answered binds site and exact answer. Completed
stores inert result data, not a fresh public owner. CleanupSettled stores the
exact ordered obligation/outcome receipt; ResultClaimed binds its completed
result digest. The following closed field tables define this proposed wire contract. Common row fields are `schema`, `generation`, `sequence`,
`previous_mac`, `kind`, and `authentication`; each row has exactly those plus
its listed fields. Integer counters are unsigned, except ordinary signed
scalar payloads. A fresh previous MAC is 64 zero hex digits.

| Row kind | Additional fields |
| --- | --- |
| Created | `profile`, `scope`, `function`, `plan_digest`, `signature`, `argument`, `argument_digest`, `max_steps`, `max_reserved_fuel`, `limits` |
| ArgumentCommitted | `argument_digest`, `storage`, `leaf_flags` |
| StartReserved | `causal_sequence`, `reservation`, `reserved_total` |
| Yielded | `causal_sequence`, `checkpoint`, `checkpoint_digest`, `consumed_steps` |
| Dispatched | `yielded_sequence`, `checkpoint_digest`, `request_digest` |
| Answered | `dispatched_sequence`, `answer`, `answer_digest` |
| ResumeReserved | `answered_sequence`, `reservation`, `reserved_total` |
| ReplayReserved | `basis`, `reservation`, `reserved_total` |
| ReplayValidated | `reservation_sequence`, `basis`, `consumed_steps` |
| Completed | `causal_sequence`, `result`, `result_digest`, `pending_cleanup`, `consumed_steps` |
| Failed | `causal_sequence`, `failure`, `language_status`, `pending_cleanup`, `consumed_steps` |
| CleanupStarted | `terminal_sequence`, `cleanup_digest` |
| CleanupSettled | `cleanup_started_sequence`, `receipt` |
| ResultClaimed | `completed_sequence`, `cleanup_settled_sequence`, `result_digest` |

`scope` has exactly `program_root`, `invocation`, `policy_epoch`. Frame has
exactly `declaration`, `fields`, `storage`, `leaf_flags`, `suspension_cleanup`,
`failure_cleanup`, `completion_cleanup`; each field has `identity`, `value`.
A Bytes value has exactly `kind: "bytes"`, `hex`; Copy values use the frozen
canonical scalar data encoding. Storage, leaf flags and cleanup operations use the pinned metadata subset
below, with exact ordered vectors and independent compiler agreement. `limits` has exactly `record_fields: 8`, `bytes_leaves: 8`,
`bytes_per_leaf: 1024`, `total_bytes: 8192`, `stable_identity_bytes: 256`,
`invocation_identity_bytes: 128`, `carrier_bytes: 32768`,
`checkpoint_bytes: 65536`, `record_bytes: 163840`, `journal_bytes: 524288`,
`records: 64`.
`receipt` is a closed tagged object: `{kind: "observed", settlement,
operations}` with settlement `completed`/`failed` and the canonical ordered
`{operation, outcome}` array, or `{kind: "host_confirmed", confirmation_digest}`
with no operations/observed settlement. Operation outcome is `completed` or
`failed`. The confirmation variant is allowed only for Failed as above.
Cancellation selects `host_abandoned` in this first profile; it does not add
a new wire failure class. `failure` is one of `language_failure`, `fuel_exhausted`,
`call_depth_exceeded`, `evaluation_rejected`, `handler_failed`,
`answer_type_mismatch`, `host_abandoned`. Language status is retained separately
in the Failed row as `language_status` (null except language_failure), using
the complete normalized status v1 data object described below; no exception message enters
it. All statuses must match the compiler/runtime owner, never a free string.

### 6.1 Pinned value encodings

This profile fixes the existing eight scalar codec semantics at base
`5ae54dd4`, `interpreter::resumable::checkpoint::{scalar_json,scalar_from_json}`:
`{tag: "i64"|"i32"|"u8"|"usize"|"char"|"bool", value}` with checked
integer ranges, Unicode scalar validity, bool type, and `{tag: "f32"|"f64",
bits}` with exactly 8/16 lowercase hex digits. Float payloads bind bit-for-bit,
including negative zero and NaNs. New owned-frame wire only uses the admitted
native64/wasm32-independent usize range 0..=4294967295. This restriction is
profile-specific and does not alter old scalar wire acceptance.

Signature is the v7 four-field source signature data shape pinned at that
base: exactly `request_shape`, `answer_shape`, `plan_identity`, `yield_count`.
Shapes are `semaprax.resolved-type.v1:` plus the checked type identity key;
plan identity uses this new owned-frame plan domain; yield_count is exactly 1.
This preserves the shape encoding, not v7 admission or its authority rules.

The source cleanup plan has one of the explicitly closed
`semaprax.cleanup-plan.v2` through `semaprax.cleanup-plan.v13` identities,
selected and independently replay-validated by the compiler at this base.
No later plan schema is implicitly accepted. `cleanup_plan_digest` hashes the exact
`graph_cleanup::cleanup_plan_json` bytes from this base under
`semaprax.source-owned-frame-cleanup-plan.v1\0`; their existing field order is
preserved, not reserialized through the new lexicographic wire renderer.
The owned wire embeds only this fixed subset, rederived from that plan:

- storage: `{kind: "value", value: <ValueId>}` for the parameter/root, or
  `{kind: "provisional_result"}` only for the unpublished terminal root;
- leaf flag: `{field: <DeclarationId>, flag: <u32>, live: true,
  lifecycle: <DeclarationId>}` in structural field order;
- cleanup operation: `{kind: "finalize", source: {kind: "cleanup_place",
  storage, projections: [<field DeclarationId>]}, lifecycle_id,
  guard_flag: <u32>}` in the plan's runtime order. No active_case, resource,
  nested path or unknown kind is admitted. Empty operations are explicit `[]`.

The checked binding additionally includes the full record liveness shape
from `graph_cleanup::liveness_shape_json` at the same base (record root,
field_liveness entries, leaf/no_drop shapes); no generic model substitutes
for this real metadata. The outer `plan_digest` hashes the complete checked owned-frame binding from
section 2 under `semaprax.source-owned-frame-plan.v1\0`. It includes source
and function/site identity, parameter/result/channel shapes, exact record
liveness, cleanup vectors, profile, and the source cleanup schema and full
cleanup bytes. The separate `cleanup_plan_digest` is a derived fact, not an
extra concatenand in the implemented plan hash. Signature
`plan_identity` binds this same full outer plan digest; no new signature wire
fields are added.

`profile` is exactly `semaprax.source-owned-frame.v1`, `kind` is the exact
case-sensitive row name in the table, and `schema` is the stated new journal
identity. `created_sequence` is 0 and each sequence increments by one.
Row sequences, causal references, counts, reservation and fuel totals are u64;
site is the exact checked suspension expression's revision-scoped ExpressionId,
not an authored persistent @id. It is bound under the exact source/program and
outer plan digest; a body/path change requires a newly derived site/plan.
Generations
and digest fields are sha256-prefixed strings; authentication/previous_mac are
64-digit lowercase hex. Stable declaration/value/site strings have the stated
UTF-8 bounds. `argument`, `result`, `frame`, `answer`, `request`, `signature`,
`pending_cleanup` and `limits` are typed objects/vectors as defined above, not
JSON strings or arbitrary blobs. Pending cleanup is exactly the canonical
operation vector. `checkpoint` is the complete canonical envelope UTF-8 string
including its LF; byte cap counts decoded UTF-8 and record cap counts escaped
rendering. `scope.policy_epoch` is u64. `language_status` is exactly null or
the complete `conformance::NormalizedStatus::to_json` v1 data object:
`{schema: "semaprax.status.v1", domain_id: <string>, code: <nonzero u32>,
class: "contract" | "arithmetic", retryable: <bool | "unknown">}`. The new
outer renderer canonically orders that object's keys without losing any
field. Its values are independently rederived from the selected checked
source failure, including compiler-owned domain/code/class/retryability.
Import, ExplicitClose and Adapter exist in the ordinary normalized status
enum but are not source language failures in this call-free, effect-free
profile; they cannot be relabelled as contract/arithmetic. No invented
"semantic" class is accepted. No defaulted/nullable invented replay fields
or other scalar/schema fallback exists.

### 6.2 Exact digest preimages

Let `C(x)` be the exact recursively lexicographic compact UTF-8 JSON object
or vector, without LF, retaining declaration/runtime vector order. Let
`D(domain, bytes)` be SHA-256 of the domain's UTF-8 bytes, including its trailing
NUL, followed by exactly `bytes`, rendered `sha256:` plus 64 lowercase hex
digits. The domains in the table above have these exact preimages:

| Digest | Bytes after domain |
| --- | --- |
| Arguments | `C({declaration, fields})`, with declaration-ordered `{identity, value}` fields |
| Frame | `C(frame)`, the complete closed frame object above |
| Checkpoint | Complete authenticated checkpoint UTF-8, including its final LF |
| Request | `C(request)`, the frozen scalar object |
| Answer | `C(answer)`, the frozen scalar object |
| Result | `C({declaration, fields})`, the same inert data shape as arguments |
| Pending cleanup | `C(pending_cleanup)`, the exact compiler-compared operation vector |
| Confirmation | `C({cleanup_digest, cleanup_started_sequence, function, generation, scope, terminal_sequence})` |
| Generation | `C({created, store_identity})`, as defined below |
| Journal name | Exact UTF-8 invocation string |

Confirmation scope is exactly `{invocation, policy_epoch, program_root}`.
The explicit non-Clone grant binds every confirmation field to independent
live facts and current policy before append. It grants only the Failed plus
CleanupInDoubt route. The host-confirmed receipt has exactly `kind` and
`confirmation_digest`, with no observed operations or settlement.

Generation's `created` object contains exactly the Created row's additional
fields: `argument`, `argument_digest`, `function`, `limits`,
`max_reserved_fuel`, `max_steps`, `plan_digest`, `profile`, `scope`, `signature`.
It excludes schema, kind, generation, sequence, previous MAC and authentication,
so its definition is nonrecursive. `store_identity` contains exactly the
independently registered u64 fields `directory_device`, `directory_inode`,
`file_device`, `file_inode`. These are generation preimage inputs, not new
Created wire fields. Fresh creation obtains an exclusive file before rendering
generation/Created. Recovery uses caller-pinned identities, expected scope,
fuel and plan alongside authenticated argument facts. This binding detects
identity disagreement; it cannot detect a protected-history violation through
a complete valid same-inode rollback.

The new journal filename is the journal-name hash's 64 lowercase hex digits
plus `.owned-frame.jsonl`, never a raw invocation pathname. Older journal
names remain unchanged.

Preserve the implemented plan identity exactly: SHA-256 of
`semaprax.source-owned-frame-plan.v1\0`, then PROFILE bytes, exact function-ID
bytes, exact `graph::to_hir_json(program, "semaprax.source-owned-frame-plan.v1")`
bytes, then exact `graph_cleanup::cleanup_plan_json(entry.cleanup_plan)` bytes.
No separators, JSON wrapper or separate cleanup hash are inserted. The graph
and checked compiler proof bind actual liveness and cleanup provenance.
Cleanup-plan digest separately hashes those exact cleanup JSON bytes under
its own stated domain.

Checkpoint MAC authenticates `C(envelope without authentication)` under the
checkpoint-authentication domain. Journal MAC authenticates
`C(row without authentication)` under the journal-record domain, including
schema, generation, sequence, previous MAC, kind and the closed row fields.
`previous_mac` is the preceding row's actual MAC, initially 64 zeroes. The final
authenticated document/row has exactly one LF, excluded from its MAC preimage.
No digest grants authority or changes any predecessor key/domain/wire meaning.

## 7. Exact bounds and preflight

Proposed hard limits, inclusive:

- 8 record fields, 8 Bytes leaves, 1,024 bytes per leaf, 8,192 total Bytes;
- 256 UTF-8 bytes per stable identity; 128 bytes per invocation identity;
- 32 KiB canonical argument/frame/result data each;
- 64 KiB complete authenticated checkpoint, including its LF;
- 160 KiB complete journal record, including MAC and LF;
- 512 KiB complete journal; 64 total records per invocation.

No unbounded diagnostic/provider text enters this journal. Stable failure and
per-leaf cleanup outcome enums are closed; external text is not a wire field.
Bounds are validated before ownership commit. Codec checks both semantic
payload bounds and rendered-byte bounds. A scalar-only or empty-Bytes record
still follows the exact owned profile if its checked record has Bytes fields.

Capacity reservation is phase specific and computed from the actual canonical
renderer, bounded maximum future payloads, sequence-number widths, MACs and LF:

- before ArgumentCommitted: reserve its start/park path and the worst terminal
  failure, cleanup and result-claim closure;
- before Dispatched: reserve Answered, ResumeReserved, worst successful result
  or failure, cleanup receipt and claim; include abandonment without answer;
- before any replay reservation: reserve that reservation, ReplayValidated,
  and the still-owed closure from its causal phase;
- before CleanupStarted: reserve maximum CleanupSettled and any success claim;
- before ResultClaimed: reserve that row exactly.

Mutually exclusive branches use their maximum, not a sum requiring both.
A logical capacity refusal occurs before the relevant consuming/dispatch
boundary. Once dispatch is acknowledged, bounded answer, failure, cleanup and
claim append must not fail merely because the journal is full. Physical I/O
failure remains possible and enters the poisoned/recovery path. A near-limit
proof must use serialized maximum-width rows; guessed average sizes are not
an acceptable capacity argument.

## 8. Fuel and evidence

Start, resume and each structural replay reserve `max_steps` fuel before any
interpreter work; `max_steps` is positive. Use checked arithmetic against
caller-chosen cumulative `max_reserved_fuel`. The reservation remains spent
on evaluator failure or crash. Actual consumed evaluator steps are reported
separately and cannot exceed the reservation. No implicit/unmetered replay is
allowed in checkpoint validation. Pure codec/hash/shape validation is bounded
by the byte/field limits and does not evaluate source.

Recovering state therefore has two stages: authenticated structural fold,
then a separately persisted reservation before source replay. Metered replay
validates the parked owner's exact plan without minting another owner. Budget
exhaustion performs no new source evaluation or host dispatch and leaves
explicit abandonment/settlement available under the retained lease. Cleanup
and durable terminal closure do not require additional source-evaluation fuel.
Counters do not reset at restart; the journal commits reservation count,
reserved total and observed consumed steps. Runtime root/evidence joins these
facts to the exact terminal result/failure and cleanup receipt. Evidence alone
never grants a result, owner, dispatch or cleanup capability.

The evidence digest is
`D("semaprax.source-owned-frame-evidence.v1\0", C(evidence))`. Evidence has
exactly `cleanup_receipt`, `consumed_total`, `generation`, `journal_mac`,
`journal_sequence`, `plan_digest`, `reservation_count`, `reserved_total`,
`result_claimed_sequence`, `scope`, `terminal`. Terminal is null or an object
with `kind` and exactly the recorded Completed/Failed additional fields.
Cleanup receipt is null or the exact recorded typed receipt; claim sequence is
null unless recorded. Journal sequence and MAC identify the current acknowledged
tail. Consumed total sums only durably recorded observations, never invented
work counts for a crash. Reserved total retains every fully charged reservation.
This inert join grants no authority.

## 9. Acceptance gates required before implementation acceptance

1. Compile the State helper with its actual record parameter cleanup metadata;
   canonical source round-trip plus graph assertions for nominal/field/storage
   identities, site liveness, transfer unit and canonical cleanup vectors.
2. Real interpreter success through start/park/checkpoint/exclusive restore/
   answer/resume/result claim, preserving all Bytes and Copy fields. Empty,
   maximum and embedded-zero Bytes cases; exactly one dispatched request.
3. Actual record-liveness negative controls: altered leaf flags/storage IDs,
   missing/reordered leaf, forged partial move, live loan, transferred root,
   plan/nominal/field drift. Each fails before evaluator/owner restoration.
4. Source refusals: partial field move/update, extra owned local, nested record,
   owned variant, String, borrow/resource, call/effect, control yield, second
   yield and owned answer. Assert stable diagnostics, not generic H006.
5. Real ownership observations on precommit failure, false precondition,
   resume contract failure, wrong answer, handler failure, cancellation,
   abandonment and cleanup failure. No skipped leaf, repeat drop, primary
   replacement, premature result publication or remaining live owner.
6. Inject crash before/after each append and physical boundary in section 5.
   Repeated recovered dispatch remains in doubt with zero new handler entries;
   repeated cleanup recovery never invokes the sink again. ResultClaimed
   recovery never returns another owner, including lost-delivery crash.
7. Decode without a lease stays inert; two simultaneous restoration attempts
   cannot produce two active owners. Wrong key/scope/generation, forged tail,
   torn-tail observation, noncanonical bytes and all old/new schema pairings
   fail closed before source evaluation or authority-bearing action.
8. Capacity/fuel exact edges, repeated interrupted charged replay, near-full
   post-dispatch settlement, wrong consumed-step counter and overflow controls.
9. Created-only restoration returns no token/owner; uncertain commit never
   returns an argument. Pinned-directory/file substitution, complete valid-tail
   rollback injection violates the protected-store scope and must make the
   store registration unavailable; copying/replacement/second registration
   are refused before owner restoration. Do not claim an HMAC detects an
   in-place complete valid-tail rollback in an unprotected directory. Pre-yield retry has no checkpoint digest; every
   interrupted start/resume/replay retry consumes fresh fuel. Terminal bases
   never enter source replay. HostConfirmed cannot claim any result.
10. Foundation real Weak/strong-count lifetime tests cover precondition,
   postcondition, fuel, cancellation, park Drop/unwind, terminal Drop/unwind,
   explicit settlement, result Drop and consuming result handoff. Unsettled
   backing disposal never masquerades as semantic release.
11. Frozen scalar/control/Copy journal/checkpoint fixtures and backend refusal
   selectors remain unchanged. No target parity claim from interpreter tests.

Place tests in existing owning harness/modules; do not add a new top-level
integration binary. Compiler ownership lives in `hir::resolve_yield` and
`cleanup_plan::owned_liveness`, with workspace relink checks joined. Lowering,
interpreter, signature, checkpoint and durable journal/driver each own their
corresponding derivation/validation. Add submodules to keep source within its
budget and join every textual audit across them. Agent lifecycle integration
and target implementations require their own reviewed follow-up packet.

## 10. Review boundary

Independent review must approve these consuming API, parameter/record
liveness, authoritative-store, wire and uncertainty choices before code begins.
Implementation is split into two review packets: first compiler liveness plus
the sealed consuming evaluator with real Arc/owner lifetime tests; then the
pinned durable codecs/store/fold and crash gates. Approval of the foundation
does not authorize shipping durable recovery before the second packet passes
all of its specified gates. Any proposed change to those
choices amends this contract for review; implementation cannot approve its own
scope reduction. This document grants no completed R20 acceptance, deployment,
hosted evidence, issue mutation, asynchronous scheduler or storage authority.
