# Source owned frame v1 — proposed bounded contract

Status: **proposal for independent review; no implementation or completion claim**.
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

Proposed API roles, with final Rust spellings subject to review:

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
responsible for that owner. An ambiguous append poisons the session and must
be reopened/replayed under the same exclusive authority before any decision.

Prepare constructs the plan and pure carrier data only. It grants no key,
filesystem, host-call, answer, cleanup or publication authority.

## 4. Replay and restoration

Fresh evaluation binds the consumed record root once. At park the evaluator
extracts the compiler-proven whole record into the owned frame. Resume binds
that frame root directly, without recreating an owned argument or evaluating
an owning prefix. The pure Copy prefix is replayed only to recheck request,
site, argument facts and binding; all that work is metered.

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
result exists. Recovery never accepts an additional caller argument owner to
replace the committed root. Independent expected scope/key facts are not
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

Pure recovery work has a durable `ReplayReserved` record before evaluation.
Every reservation refers to one causal state and monotonically increases
cumulative reserved fuel. ReplayValidated follows successful metered replay,
refers to that reservation and original causal state, and binds the unchanged
checkpoint digest. It changes no ownership/dispatch phase. An interrupted
ReplayReserved can be followed by another reservation for the same causal
state; it cannot skip replay validation before the next phase transition. A interrupted reservation remains charged; a later
replay reserves again. A reservation is not dispatch authority.

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
tail is CleanupInDoubt and never reruns settlement. Any host-confirmation route
must require explicit confirmation evidence under the live lease, preserve
the primary status, and identify itself as HostConfirmed rather than claiming
an observed physical cleanup receipt.

Success retains the root as the unpublished result owner while non-result
cleanup finishes. Failed postconditions settle that root instead. Only
Completed plus successful required non-result cleanup can reach result claim.
`ResultClaimed` is synchronized before handing the opaque result to the
caller. Recovery of that row never hands out a second owner. A crash between
claim append and delivery is ResultDeliveryInDoubt; it returns evidence, not a
new result. This conservative loss window must be explicit in the public API.
A settled failure has no result claim.

## 6. New encodings and frozen preservation

Use new independent identities; do not reinterpret any current scalar,
control-owned, aggregate-channel, whole-function Copy, or Source Live codec:

| Item | Proposed identity/domain |
| --- | --- |
| Checked plan | `semaprax.source-owned-frame-plan.v1` |
| Checkpoint schema | `semaprax.source-owned-frame-checkpoint.v1` |
| Checkpoint MAC | `semaprax.source-owned-frame-checkpoint-authentication.v1\0` |
| Journal schema | `semaprax.source-owned-frame-journal.v1` |
| Journal record MAC | `semaprax.source-owned-frame-journal-record.v1\0` |
| Journal name hash | `semaprax.source-owned-frame-journal-name.v1\0` |
| Argument digest | `semaprax.source-owned-frame-arguments.v1\0` |
| Frame digest | `semaprax.source-owned-frame-payload.v1\0` |
| Generation digest | `semaprax.source-owned-frame-generation.v1\0` |

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

Started binds profile, exact scope/function/plan/signature, canonical admitted
argument and its digest, maximum steps/fuel, and fixed capacity limits. Generation is the hash of the canonical Created facts (without common
row/MAC fields) under its generation domain. A reused journal identity is
refused by fresh start. Recovery derives generation again from trusted
expected facts plus the authenticated Created carrier. Each row binds the
same generation and true combined sequence. Yielded stores the
checkpoint and its digest; Answered binds site and exact answer. Completed
stores inert result data, not a fresh public owner. CleanupSettled stores the
exact ordered obligation/outcome receipt; ResultClaimed binds its completed
result digest. The proposed closed field tables below must be independently reviewed before
wire implementation. Common row fields are `schema`, `generation`, `sequence`,
`previous_mac`, `kind`, and `authentication`; each row has exactly those plus
its listed fields. Integer counters are unsigned, except ordinary signed
scalar payloads. A fresh previous MAC is 64 zero hex digits.

| Row kind | Additional fields |
| --- | --- |
| Created | `scope`, `function`, `plan_digest`, `signature`, `argument`, `argument_digest`, `max_steps`, `max_reserved_fuel`, `limits` |
| ArgumentCommitted | `argument_digest`, `storage`, `leaf_flags` |
| StartReserved | `causal_sequence`, `reservation`, `reserved_total` |
| Yielded | `causal_sequence`, `checkpoint`, `checkpoint_digest`, `consumed_steps` |
| Dispatched | `yielded_sequence`, `checkpoint_digest`, `request_digest` |
| Answered | `dispatched_sequence`, `answer`, `answer_digest` |
| ResumeReserved | `answered_sequence`, `reservation`, `reserved_total` |
| ReplayReserved | `causal_sequence`, `reservation`, `reserved_total` |
| ReplayValidated | `reservation_sequence`, `causal_sequence`, `checkpoint_digest`, `consumed_steps` |
| Completed | `causal_sequence`, `result`, `result_digest`, `pending_cleanup`, `consumed_steps` |
| Failed | `causal_sequence`, `failure`, `pending_cleanup`, `consumed_steps` |
| CleanupStarted | `terminal_sequence`, `cleanup_digest` |
| CleanupSettled | `cleanup_started_sequence`, `settlement`, `operations` |
| ResultClaimed | `completed_sequence`, `cleanup_settled_sequence`, `result_digest` |

`scope` has exactly `program_root`, `invocation`, `policy_epoch`. Frame has
exactly `declaration`, `fields`, `storage`, `leaf_flags`, `suspension_cleanup`,
`failure_cleanup`, `completion_cleanup`; each field has `identity`, `value`.
A Bytes value has exactly `kind: "bytes"`, `hex`; Copy values use the frozen
canonical scalar data encoding. Storage, leaf flags and cleanup operations
use the existing graph cleanup metadata encoding and exact ordered vectors;
codec acceptance additionally validates the compiler-derived profile-specific
subset. `limits` has exactly `record_fields: 8`, `bytes_leaves: 8`,
`bytes_per_leaf: 1024`, `total_bytes: 8192`, `stable_identity_bytes: 256`,
`invocation_identity_bytes: 128`, `carrier_bytes: 32768`,
`checkpoint_bytes: 65536`, `record_bytes: 163840`, `journal_bytes: 524288`,
`records: 64`.
`operations` is the ordered array of `{operation, outcome}`; outcomes are
`completed` or `failed`. Settlement is `completed`, `failed` or
`host_confirmed`. HostConfirmed has an empty physical operations array and
must never be represented as an observed successful cleanup receipt.
`failure` uses the existing stable durable failure vocabulary. No exception
message enters it. Review must pin the reused signature/cleanup/scalar codecs
by version before implementation; no fallback or open key vocabulary exists.

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
9. Frozen scalar/control/Copy journal/checkpoint fixtures and backend refusal
   selectors remain unchanged. No target parity claim from interpreter tests.

Place tests in existing owning harness/modules; do not add a new top-level
integration binary. Compiler ownership lives in `hir::resolve_yield` and
`cleanup_plan::owned_liveness`, with workspace relink checks joined. Lowering,
interpreter, signature, checkpoint and durable journal/driver each own their
corresponding derivation/validation. Add submodules to keep source within its
budget and join every textual audit across them. Agent lifecycle integration
and target implementations require their own reviewed follow-up packet.

## 10. Review boundary

Independent review must settle the consuming API, actual parameter/record
liveness proof, wire field tables, capacity renderer and conservative
result-delivery uncertainty before code begins. Any proposed change to those
choices amends this contract for review; implementation cannot approve its own
scope reduction. This document grants no completed R20 acceptance, deployment,
hosted evidence, issue mutation, asynchronous scheduler or storage authority.
