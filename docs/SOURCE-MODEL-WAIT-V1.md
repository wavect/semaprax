# Source Model Wait v1

Status: **DESIGN DRAFT FOR INDEPENDENT REVIEW; NOT IMPLEMENTED**.
Audience: source-runtime, checkpoint, and model-operation contributors.

This document proposes the smallest R20 model-wait bridge for the direct
standalone source runtime's FixtureAgent. It uses an interpreter continuation
inside the authoritative Source Live journal. It does not claim complete Agent
state migration, linked-project support, or C11/native/Wasm yield execution.

## 1. Scope and authority

The opt-in route retains the existing checked Agent loop, proposal decoder,
model operation binding, live grants, cancellation, clock, model reservations,
and failure classes. `SourceExecutionSession` remains the sole causal owner.
The host supplies the existing exclusive `CheckpointStore` writer and latest
generation. There is one journal, one generation sequence, and the existing
accounting owners; no companion continuation journal or second dispatch grant.

The first profile composes with ordinary Source Live execution v2 only. Priced,
I/O, migrated, policy-v6, and linked-project profiles fail binding rather than
silently dropping their contracts. This restriction does not permit invoking a
model without its existing checked SourceModelBinding, live ModelGrant, or
explicit adapter invocation capability. A source requiring a policy-v6 route
must be refused by this profile. Profile composition requires a later contract.

The additive API accepts a compiled lifecycle, checked wrapper selection,
caller-owned `SourceCheckpointKey`, and all ordinary durable-run inputs. The key
is never persisted or derived from journal bytes. Existing APIs do not opt in.
The bridge uses aggregate interpreter and source-checkpoint-v7 APIs directly;
it does not construct `AggregateDurableInvocation`, whose separate journal and
dispatch lifecycle would create a second owner.

## 2. Checked wrapper admission

FixtureAgent's retained checked declarations must supply exactly this meaning:

```spx
@id("fixture.agent.fn.await_proposal")
fn await_proposal(observation: Observation) -> Proposal
    yields Observation -> Proposal
{ yield observation }
```

Admission inspects the checked expression graph and lowered suspension facts.
There is one by-value parameter of the Agent's exact nominal Observation type;
return and response have the Agent's exact nominal Proposal type. The request
is that parameter directly. The function body is exactly one direct yield whose
value is the result. There are no blocks with additional statements, calls,
branches, bindings, transformed requests/results, effects, contracts with
runtime work, or additional yields. Stable declaration IDs, field order and
leaf types must match the compiler's Agent schemas. All four boundaries are
Copy-only aggregates admitted by `channel_arguments_v1`; Bytes leaves and
RecordBytes/VariantBytes representations are refused.

The checked lowering must contain exactly one suspension, no owned locals,
empty suspension cleanup and empty completion cleanup. Empty cleanup is a
compiler-derived fact, never a host-supplied callback, permission, or repaired
plan. A nonempty plan fails binding. Model-operation declarations themselves
gain no yields annotation. The wrapper is an ordinary checked function in the
same compiled source module; its checkpoint uses the existing sequential
aggregate interpreter lane and source checkpoint v7.

## 3. Identities and version domains

This profile's journal schema is
`semaprax.live-invocation.source-persisted-journal.v7`. This namespace is
independent of `semaprax.source-resumable-checkpoint.v7`. V1--V6 encoders,
decoders, identity construction, enums exposed by existing APIs, and canonical
bytes remain unchanged. There is no implicit migration or profile detection
from embedded checkpoint bytes.

All hashes below are SHA-256 over the named domain bytes followed by canonical
UTF-8 JSON bytes. Digests are lowercase 64-character hexadecimal. JSON has no
insignificant whitespace, duplicate/unknown keys, floating numbers, or alternate
number/string encodings. Field order below is normative. `\0` denotes one NUL
byte in a domain, not two text characters.

* Wrapper binding domain: `semaprax.source-model-wait.binding.v1\0`.
  Payload: `{"source_revision":S,"agent":A,"model_operation":M,"wrapper":W,
  "observation":O,"proposal":P,"signature":G,"cleanup":"empty-copy",
  "checkpoint_schema":"semaprax.source-resumable-checkpoint.v7",
  "evaluation_fuel":F,"checkpoint_limit":32768}`. `G` is the canonical
  checked aggregate signature object already defined by the checkpoint
  contract; S is the exact compiled source revision digest. All stable IDs
  come from the checked module, never host declarations.
* Outer invocation domain: `semaprax.live-invocation.source-id.v7\0`.
  Payload: `{"execution":E,"wait_binding":B}`. E is the ordinary v2
  execution binding digest over all unchanged seed/evaluator inputs. B is
  the wrapper binding digest. Optional ProgramRoot must be absent in v1 of
  this bridge; this direct route does not mint a linked ProgramRoot.
* Wait domain: `semaprax.source-model-wait.attempt.v1\0`.
  Payload: `{"invocation":I,"turn":T,"attempt":N,"wrapper_binding":B}`.
  The resulting wait ID identifies exactly one outer attempt, including
  malformed-proposal retries. Existing model attempt/request/prompt digest
  domains and model-source bindings remain authoritative and unchanged.
* Carrier domain: `semaprax.source-model-wait.carrier.v1\0`.
  Payload: `{"wait":Q,"kind":K,"declaration":D,"fields":V}`; K is
  `observation` or `proposal`, and V is the checked declaration-order scalar
  field array using the existing aggregate carrier's canonical scalar encoding.
* Checkpoint-byte domain: `semaprax.source-model-wait.checkpoint.v1\0`.
  Input after the domain is the exact authenticated v7 envelope bytes,
  including its contracted trailing LF, rather than a JSON reserialization.
* Journal chain domain: `semaprax.live-invocation.source-chain.v7\0`.
  The envelope and chain-input field order is the existing Source Live order:
  schema, invocation, generation, clock_domain, last_checked_millis, entries;
  the envelope inserts chain before entries and ends in one LF.

`SourceCheckpointScope` uses program_root=S, invocation_id=Q, policy_epoch=0.
Here S denotes the standalone checked source revision, not a linked ProgramRoot.
The supplied key authenticates the ordinary v7 envelope under its existing
domain. Wrapper identity/signature and original Observation arguments are
checked again by its decoder. Embedded bytes and outer hashes carry no authority.

## 4. Canonical wait events

New events are accepted only by this explicit profile. Existing ordinary model
intent/settlement/usage/refusal/admission events retain their field encodings.
Entry-array position is the zero-based sequence reference. Every object begins
with `"seq":Z`, where Z equals that position, followed by the exact ordered
fields below. The examples omit only that common seq field:

```text
{"kind":"wait_evaluation_reserved","turn":T,"attempt":N,"wait":Q,
 "phase":P,"replay_of":R,"fuel":F}
{"kind":"wait_prepared","turn":T,"attempt":N,"wait":Q,
 "reservation":R,"observation_digest":D,"checkpoint_digest":C,"checkpoint":H}
{"kind":"wait_completed","turn":T,"attempt":N,"wait":Q,
 "reservation":R,"proposal_digest":D}
{"kind":"wait_replay_checked","turn":T,"attempt":N,"wait":Q,
 "reservation":R,"original":O,"result_digest":D}
```

P is the closed tag `start` or `resume`. Reservation `replay_of` is null for
first evaluation, otherwise the sequence of the original same-phase reservation.
F equals the bound positive evaluation_fuel, which is no larger than existing
max_steps_per_stage. H is lowercase hex of at most 32768 decoded bytes; its
decoded form is the exact v7 envelope. All references point backward, have the
same Q/T/N, and cannot be reused or refer to another role/profile. Each
reservation has at most one closing event. `wait_replay_checked` refers to the
original prepared/completed event and repeats its checkpoint/proposal digest.
It cannot substitute a new result. Replays of replays reference the original,
not another replay reservation.

Fresh causal grammar between TurnObserved and the existing proposal decision:

```text
start reservation -> wait_prepared -> ordinary AttemptIntent
-> (AttemptSettled [AttemptUsage] | AttemptFailed [AttemptUsage])
```

The attempt intent is allowed only for that attempt's unique prepared wait.
Association is enforced by the profile fold's exact T/N/Q and checked
request/prompt identity; there is no additional intent or provider dispatch.
After successful raw settlement, the existing decoder chooses:

```text
malformed: existing ProposalRefused -> next bounded attempt or existing Stop
valid: resume reservation -> wait_completed -> existing ProposalAdmitted
failed model: existing model failure/Stop
```

The compiler decoder runs before resume reservation. The valid Proposal carrier
is reconstructed from that exact decoded document and full checked nominal
shape. Completion must equal this carrier. A refused attempt's wait is closed
by its existing ProposalRefused; a failed/stopped attempt by the existing failure
or Stop. No completion, cleanup event, refund, or new model failure is fabricated.
No next attempt starts until the previous outer attempt has closed normally.

## 5. Fuel, capacity, and acknowledgments

Every interpreter start/resume, including historical reconstruction, requires
an acknowledged wait reservation first. Checkpoint decoding/replay work that
executes the interpreter is included in that evaluation's F allowance; APIs
must not run a hidden extra evaluator outside that allowance. Pure structural
validation is bounded by the document/carrier/checkpoint limits.

Each reservation commits F into the existing committed_stage_fuel total and
max_total_steps ceiling. It does not increment Agent stages, max_stages,
completed_stages, stage_rows, effects, attempts, or model reservation totals.
Replay reservations are nonrefundable even if evaluation, cancellation, or
acknowledgment fails. Existing Agent ReplayStageReservation sequencing remains
unchanged; wait replay events form a separately checked subinventory inside
the same total-fuel fold. Recovery restores both inventories before admission.

Before start, cancellation/deadline and fuel are checked and the sink preflights
room for the start reservation, maximum hex checkpoint prepared event, ordinary
intent, maximum bounded raw settlement/usage, resume reservation/completion,
proposal decision and contracted terminal room. Resume/replay similarly
preflight their maximum closure and terminal room before reserving. The existing
16 MiB document and 65536-entry caps still apply; no model dispatch occurs when
its bounded settlement and wait completion cannot fit. Overflow fails closed.

The v7 preflight allowance, in addition to the serialized candidate, is
`2 * 32768 + 2 * response_limit + 65536 + TERMINAL_ROOM_BYTES` bytes and
12 future entries. `TERMINAL_ROOM_BYTES` is the existing execution profile's
contracted bound. This allowance covers an interrupted start/resume closure,
one replay reservation/check pair, model intent/settlement/usage, proposal
decision and terminal records. Every subsequent append rechecks the same
allowance until the wait closes; it is capacity proof, not reserved storage or
permission to exceed limits. Exhausted capacity refuses additional replay;
it does not erase prior charged reservations. Exact terminal appends use the
existing terminal capacity rule.

Evaluation only starts after reservation ACK; provider dispatch only after
ordinary intent ACK; authorization only after checked wait completion ACK and
existing admission. Store ACK loss poisons the session and forbids further
in-memory actions. Reload latest generation before recovery. Guards at existing
boundaries remain authoritative; a selected failure remains sticky.

## 6. Recovery and negative matrix

| Latest durable state | Required recovery behavior |
| --- | --- |
| Start reservation without prepared | Fresh charged start replay; acknowledge matching prepared; zero provider calls before normal intent ACK. |
| Prepared without intent | Validate scope/arguments/checkpoint; normal outer attempt admission may proceed after guards. |
| Intent without settlement | Existing uncertain failure; zero redispatch, no synthetic answer. |
| Raw settlement without proposal decision | Existing decode; malformed keeps existing bounded retry; valid requires charged resume. |
| Resume reservation without completed | Fresh charged resume replay referencing original reservation; require identical Proposal, then completion. |
| Completed without ProposalAdmitted | Validate charged historical reconstruction and recorded digest; append ordinary admission once. |
| Historical prepared/completed | Before evaluator replay append reservation with replay_of; append wait_replay_checked after exact match. |
| Failed/refused/stopped attempt | Reproduce existing outer outcome; no answer, refund, or extra grant. |
| ACK loss at any new boundary | Poison; reload authoritative latest generation; apply the corresponding row above. |

For an unfinished original reservation, append a fresh replay reservation,
execute once, append the first prepared/completed event referencing the original
reservation, then append wait_replay_checked referencing that closing event and
the fresh replay reservation. All references still point backward. The fold
permits this one intervening causal closure and prohibits intent/admission until
the replay check is acknowledged. Capacity preflight includes both closing rows.
If ACK loss leaves the causal closure but no replay check, recovery validates
that closure, reserves another replay, and checks it; no unfinished replay grants
free evaluation. Historical replay emits only reservation/check pairs.

Required negative gates reject changed wrapper/source/key/scope, wrong nominal
declaration/field count/order/leaf, transformed identity wrapper, additional
yield/call/effect, Bytes carriers, nonempty cleanup, other Agent, other attempt,
duplicate closure, forged replay reference, digest/checkpoint mismatch, oversized
checkpoint, arithmetic overflow, obsolete schema, and unsupported composition.
Rejected host input leaves journal bytes unchanged and performs no provider or
effect calls. Journal corruption is recovery failure, not model refusal.

Success gates use the real FixtureAgent model operation and explicit host
adapter, inspect existing grants/reservations, recover every new ACK window,
and prove zero redispatch after intent. Malformed then valid response must retain
the existing attempt count, charged model reservations, and failure/retry tags.
Fuel gates distinguish wait fuel from Agent stage counts and exhaust replay fuel
before any external boundary. Canonical source format/parse/recheck and graph
assertions prove nominal signatures, one yield and compiler empty cleanup.

## 7. Evidence and implementation ownership

The evidence root is the exact checked standalone source revision S plus the
ordinary lifecycle/evaluator/model-source identities and B/I/Q associations.
Evidence records existing terminal commitments and wait reservation/checkpoint/
completion digests and fuel totals; it contains no key or grant and does not
mint a ProgramRoot. Evidence is local interpreter/host-boundary evidence unless
a separate provider execution gate establishes more. The draft has no executable
completion claim.

Expected owners: source_live session and its existing harness own opt-in loop,
decode/replay/guard association; source_journal profile/wire/fold and its harness
own canonical records, capacity and accounting; checked model-operation and
proposal-schema owners expose a narrow compiler projection to Copy carriers;
aggregate interpreter/checkpoint owners provide admission and evaluation without
hidden uncharged replay. No broad Agent owned-state API is required. Roots,
linked wrapper retention, Bytes suspension and other journal profiles remain
separate work.

## 8. Review decisions still open

This draft fixes the wire/identity and authority contract; it does not authorize
implementation before design review. Review must confirm the ordinary-v2-only
first slice provides the intended real FixtureAgent adapter seam. Supporting a
policy-v6-required adapter would need an explicit composed binding/profile;
omitting its policy is forbidden.

The exact additive Rust API/type names and compiler projection helper placement
remain open. The helper must take the compiler-decoded Proposal and retained
checked declarations, not independently parse model text. The checkpoint owner
must confirm that admission/decode/resume expose a single bounded evaluation
allowance; any currently hidden interpreter replay needs a narrow accounting
API before this contract can be implemented. No fuel-free fallback is allowed.
Independent review must also confirm the fixed capacity allowance covers the
largest permitted field encodings and interrupted replay grammar. These are
specific pre-code decisions, not implemented or tested guarantees.
