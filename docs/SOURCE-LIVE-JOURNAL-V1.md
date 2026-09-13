# Source Live Journal v1 — draft contract

Status: **PRIVATE CHECKPOINT PRIMITIVES; SUPERSEDED FOR THE RUNTIME ROUTE BY [SOURCE-LIVE-JOURNAL-V2](SOURCE-LIVE-JOURNAL-V2.md)**.
Audience: compiler contributors and live-invocation integrators.
This v1 document retains the primitive checkpoint schema and the design history
from before the checked source driver existed. The actual runtime route now has
its own v2 execution schema, terminal receipt, stage-fuel replay, and recovery
rules in [Source Live Journal v2](SOURCE-LIVE-JOURNAL-V2.md). It does not change
the generic live-invocation v1 wire. This draft must not be read as a claim that
the v2 runtime route is unimplemented, nor as hosted or provider evidence. The
remaining v1 sections record its primitive boundary and pre-v2 design limits,
not the current runtime status.

## Ownership and compatibility

`src/agent_lifecycle/iterative/driver.rs` owns the checked source route:
initialize, observe, bounded proposal attempts, compiler proposal decode,
authorization, effect dispatch, reduce, and terminal selection. The private
`opencode_host::source::OpenCodeProposalSource` owns only context encoding and
the explicit OpenCode transport callback. Its `OpenCodeSourceAccounting`
reserves through the existing `InvocationBudgetHook` and retains in-memory
receipts; those receipts are not a checkpoint. A durable source driver must
own the causal event order and replay cursor. The host adapter may prepare an
attempt and perform its physical call only when that driver has acknowledged
the durable intent. No host receipt or journal entry can grant authority.

The generic `live_invocation::journal` v1 validator admits exactly one
`RequestIntent`/response pair per turn. The source route admits up to four
malformed proposals per turn, each a separately charged model attempt. Its
current `run_live` has no journal or recovery parameter, and
`iterative/effects/durable::run_durable` accepts frozen proposal strings, not
a live `ProposalSource`. Therefore the generic v1 wire and validator must
remain byte-for-byte unchanged. Source mode needs a **separately tagged,
versioned phase grammar in the same causal-journal family**, with one source
run as its owner. It must not be implemented by pretending each source
attempt is a generic kernel turn, by storing an independent budget counter,
or by keeping a parallel effect log that can disagree with the source run.

The source mode reuses the caller-owned
`agent_lifecycle::durable::CheckpointStore` contract: a generation commit
atomically replaces the whole stored document or leaves the prior generation
intact. It may reuse canonical rendering, chain-link, and
`CheckpointJournalSink`/`recover_journal` mechanics after adapting them to
the new entry type; their existing signatures and the generic v1 envelope
must not be described as already accepting source events. The caller loads
the retained document from a trusted store and supplies exclusive writer
authority. A monotonically advancing generation is required on every
successful append.

## Bind-time identity and envelope

One source invocation identity, retained by opaque `SourceInvocationBinding`,
is derived before any model or effect call from a
domain-separated canonical encoding of:

- the exact compiled lifecycle digest and semantic source revision;
- the exact deployment/model policy binding, including the admitted provider
  and model profile, independent of source identity;
- the task objective **bytes** and task budget, plus the effective iterative
  stage, iteration, and attempt limits;
- the compiler-derived proposal schema digest and the response-byte cap;
- the caller-selected cumulative ceiling and positive fixed reservation
  units, both in the same declared policy unit;
- the initial clock reading and one absolute deadline in a named domain that
  remains comparable across process restarts; and
- the exact ProgramRoot when the bound source execution has one (an explicit
  absent tag otherwise, never a substituted root).

The canonical seed contains no model response, observed usage, session ID,
credential, or journal generation. Raw task bytes are inputs to the identity
hash; the persisted envelope need store only the resulting digest. A change
to any bind-time field is a different invocation and must be refused before
dispatch when presented with the prior checkpoint. The existing
`agent_lifecycle::iterative::live_invocation_digest` does not bind source
revision or deployment, and generic `LiveInvocationSeed` does not bind an
absolute deadline; neither is sufficient as this source identity unchanged.

The primitive envelope uses the following keys (shown formatted here; the
accepted wire is compact, in this exact key order, with one terminal LF):

```json
{"schema":"semaprax.live-invocation.source-persisted-journal.v1",
 "invocation":"sha256:<source invocation digest>",
 "generation":1,
 "clock_domain":"operator-clock.v1",
 "last_checked_millis":123,
 "chain":"sha256:<source-domain chain of envelope fields and entries>",
 "entries":[{"seq":0,"kind":"run_opened"}]}
```

The source domain separator and schema differ from generic
`semaprax.live-invocation.persisted-journal.v1`. Decoding requires exact
keys, canonical entry encoding and sequence numbers, bounded counts and
bytes, a matching invocation identity, a valid whole-journal chain, and the
source phase validator below. A v1 generic document is never silently
upgraded to source mode or vice versa. The chain detects torn, reordered,
truncated, or accidentally altered storage; it is not a signature against
a party able to rewrite the trusted store and recompute the chain.

The deadline is the **same absolute instant** on recovery. A smoke clock
created from `Instant::now()` for each process is not a recoverable clock
domain. The host must provide a restart-stable, explicitly identified time
domain, refuse an unavailable or observably regressed clock, and derive any
OpenCode subprocess timeout from remaining time rather than resetting a
fresh full duration. The store's last checked time may help detect rollback
between committed generations; the clock provider's correctness remains a
host assumption, not something a digest proves.

## Source event grammar

The following source-mode entries are variants of `SourceJournalEntry`,
separate from the generic Rust `JournalEntry` enum. Every event names the invocation and a
strict sequence position through its containing envelope/chain. Integer
fields have bounded, canonical encodings. An event holding bytes uses a
length bound and a digest over those exact bytes; recovery verifies both.

| Entry | Required content | Valid successor |
| --- | --- | --- |
| `RunOpened` | exact source invocation identity | first `TurnObserved` or policy `Stop` |
| `TurnObserved` | source turn, checked State and Observation digests, exact feedback digest | first `AttemptIntent` or policy `Stop` |
| `AttemptIntent` | turn, attempt, source-attempt digest, `ModelInvocationRequest` digest, prompt/context digest, request bytes, reserved units, response cap | `AttemptSettled` or `AttemptFailed`, or uncertain end of journal |
| `AttemptSettled` | same turn/attempt, bounded raw response bytes and digest, measured bytes; optional validated, redacted usage counters | `ProposalAdmitted` or `ProposalRefused` |
| `AttemptFailed` | same turn/attempt, closed transport or deadline reason, bounded attempted bytes; no asserted zero billing | `Stop` |
| `ProposalRefused` | same turn/attempt, closed compiler-decode or pre-decode deadline reason | next attempt only for malformed decode, or `Stop` |
| `ProposalAdmitted` | same turn/attempt, canonical decoded-proposal digest | `AuthorizationConsumed` or `AuthorizationRefused` |
| `AuthorizationConsumed` | checked grant digest | effect intent or pre-effect policy `Stop` |
| `AuthorizationRefused` | closed refusal, including post-admission deadline/cancellation | `Stop` |
| `EffectIntent` | exact operation/request/authorization binding | `EffectObserved` or `EffectFailed`, or uncertain end of journal |
| `EffectObserved` | matching operation, bounded observation bytes and digest | checked reducer `Transition` or late-deadline `Stop` before reduce |
| `EffectFailed` | matching operation and closed failure | `Stop` |
| `Transition` | checked reducer's `continue`, `complete`, `suspend`, or `fail`, with carrier digest | next consecutive source turn, policy `Stop` after any non-`fail` candidate transition, or matching terminal outcome |
| `Stop` | closed host/policy status and reason, with current turn/attempt when one exists; **not** a fabricated reducer result | matching `TerminalOutcome` |
| `TerminalOutcome` | closed terminal status and optional carrier digest matching `Stop` (no carrier) or a terminal reducer `Transition` | no successor |

An attempt number starts at zero, increments by exactly one **only when a
malformed-decode `ProposalRefused` takes a retry**, and is strictly less than
`driver::MAX_PROPOSAL_ATTEMPTS` (currently four). A new source turn follows
only a durable `continue` transition and increments by exactly one. No
settlement is paired with a different turn or attempt. An intent may have
only one settlement. A failed transport attempt is terminal for that source
run; the existing OpenCode adapter has no automatic transport retry or
fallback. A malformed *settled* proposal may take another attempt only
through the driver's existing bounded decode-retry rule and a fresh intent.
Expiry between durable settlement and decode uses `ProposalRefused` with
`deadline_exceeded`, followed by `Stop`; it is not a malformed retry.
Cancellation, stage exhaustion, capacity refusal, and deadline expiry before
a model attempt may use `Stop` after `RunOpened` or `TurnObserved`. A late
deadline after `EffectObserved` uses `Stop` without running reduce. A non-`fail` transition is a candidate until terminal publication: expiry or
cancellation after its checkpoint callback may append `Stop` instead of
publishing `complete`/`suspend`. A checkpoint callback must not publish the
result as a side effect; the journal owner retains the final publication gate.
A checked
reducer `Fail` is sticky: it remains the terminal reducer decision, not a
later policy stop. These entries preserve the existing distinction between
a host/policy termination and an actual checked `Step` transition.
The source wire's terminal status is a closed enum: `complete`, `suspend`,
`fail` (checked reducer results), `rejected`, `model_failed`,
`effect_failed`, `cancelled`, `budget_exhausted`, and `deadline_exceeded`
(driver/host stops). A `Stop` carries the applicable latter status and a
separate closed reason. A terminal `Transition` maps only its own checked
case to the corresponding terminal status; no deadline may replace a
checked reducer `fail`. The current `IterativeStatus` lacks
`DeadlineExceeded`, so a future source API must either add that status or
return a distinct host diagnostic while preserving the journal's closed
`deadline_exceeded` reason. An unresolved intent is **uncertain recovery**,
not a fabricated terminal `Stop` or proof of failed delivery.
The phase validator rejects duplicates, gaps, reordered events, changed
binding, post-terminal entries, and any implied redispatch.

The existing `ModelInvocationRequest::digest` binds its actual fields:
turn, task, observation bytes, proposal grammar, deployment binding,
response cap, and effective budget. It has **no separate attempt, source
revision, source deployment, or deadline field**. The current source adapter
puts attempt, revision, State, Observation and feedback into a bounded
prompt used as those observation bytes, but durable validation must not
depend on parsing model-visible text. Source mode therefore adds a
domain-separated `SourceAttemptDigestV1` over the source invocation ID,
explicit turn and attempt, the model request digest, prompt digest, measured
request bytes, reserved units, response cap and bound absolute deadline. Recovery recomputes both digests from
checked live inputs before replay. Explicit turn, attempt and units remain
in `AttemptIntent` for phase validation and charging.

## Charge, dispatch, settlement, and uncertainty

Before an attempt, the driver verifies binding, clock, event and byte
capacity, and cancellation. The host adapter prepares the bounded prompt
once. The **one** `CumulativeBudgetLedger` reserves the configured positive
units through `InvocationBudgetHook::reserve`. The source driver then appends
`AttemptIntent` carrying the returned amount and commits that generation.
Only an acknowledged successful commit permits `OpenCodeModelHandler` to
start `run`; `export` belongs to the same attempt. If the intent write fails,
no physical call occurs. An in-memory reservation on that aborted path is
not a claim that the prior durable generation contains a charge. The run
stops; recovery folds only successfully persisted intents.

After a physical call, the driver commits either bounded raw settlement or a
closed failure **before** the compiler decoder, authorization, any effect,
or result publication. A response arriving at or after the shared absolute
deadline becomes a charged `AttemptFailed { reason:
"deadline_exceeded" }` with measured attempted bytes and never reaches
decode. A prior provider/cancellation failure keeps its original reason.
Provider-reported token counts and cost remain observations, never a refund,
charge source, or authority. A missing count is unknown; a recorded zero
response-byte count is not evidence of zero provider billing. Every intent's
reservation remains charged through malformed decode, provider failure,
timeout, cancellation in flight, and recovery.

If the settlement commit fails or the process dies between intent and
settlement, the last durable document ends at `AttemptIntent`. Recovery must
report **uncertain delivery** and make zero new model or effect calls. It
must not infer non-occurrence from absence of response, retry the same
prompt, switch provider, refund the reservation, or accept a fresh deadline.
An explicit host reconciliation may later supply a verified settlement or
close the attempt without redispatch, following the existing durable
checkpoint's reconciliation pattern. That is a separately gated extension;
this draft grants no automatic reconciliation authority from OpenCode
session/export text or a caller-supplied journal.

The source validator produces a checked sum of `AttemptIntent.reserved_units`
for the existing cumulative budget policy. A source-aware resume
constructor/fold on `CumulativeBudgetLedger` consumes that validated sum and
the original absolute deadline; it must not instantiate a provider-specific
ledger or misuse the current migration constructor. The current
`CumulativeBudgetLedger::resume` folds generic v1 `RequestIntent` entries
only. Source usage receipts should be read-only projections of the durable
events. The adapter's current in-memory receipts may remain diagnostics
during a process, but cannot override the journal after recovery.

## Replay across decode and effects

A durably settled attempt replays its **exact recorded raw response bytes**
through the compiler-derived source proposal decoder, with zero OpenCode
calls and zero new reservations. The re-executed deterministic source stages
must reconstruct the same State, Observation, context and proposal digests;
drift refuses before effect dispatch. A durable decode refusal can advance
to the next fresh, charged attempt only if the source limit, budget,
deadline, and journal capacity permit it. A recorded admission is not a
grant: authorization is rerun against checked state and the canonical
proposal, and its resulting grant identity is compared with the journal.
Expiry after admission is recorded as `AuthorizationRefused`; expiry after
an effect intent is recorded as `EffectFailed` before dispatch. These are
deadline-policy outcomes, not provider errors. A committed `AttemptFailed`,
`ProposalRefused`, `AuthorizationRefused`, `EffectFailed`, or reducer
`Transition` whose terminal append was interrupted may be replayed through
its remaining deterministic phase with zero external calls. A durable
`AttemptIntent` or `EffectIntent` **without** settlement is different: its
external delivery is uncertain and cannot be replayed into another call.

`run_with_driver_live` currently owns the effect call after authorization;
there is no source effect recovery merely because the model attempt was
persisted. Full source resume requires that **same driver/journal owner** to
commit `EffectIntent` before `driver.read`, commit its bounded result or
failure afterward, and refuse an unresolved effect intent without repeating
the physical call. A durable effect observation may be reused only after
fresh checked authorization derives the same operation binding. A completed
effect remains recorded even if the deadline later prevents transition or
result publication. A terminal checkpoint replays its case without model
or effect dispatch. Until these stage/effect paths exist, an implementation
may claim only durable **source-attempt refusal**, not recovered live Agent
conversations under #114.

## Capacity and trust limits

The source route currently admits at most 4,096 iterations and four
attempts per iteration: **16,384 model intents** is an upper bound, not a
promise that all can fit in one persisted document. Generic v1's 16,384
**entry** cap cannot represent that many source attempts plus their
settlements, decode decisions and effects. The primitive caps source journals at **65,536 entries** and **16 MiB encoded
bytes**. Request work and retained response/effect byte vectors are each capped
at **65,536 bytes**, with a smaller bound response cap permitted. An attempt
failure permits the bound response cap plus one as an overflow sentinel.
A binding admits 1–4 attempts per turn, 1–4,096 iterations, 1–12,289 stages,
1–1,000,000 steps per stage and 1–1,000,000,000 total steps. Fuel is bound into
identity; the checkpoint codec is not a stage evaluator or fuel meter. Before dispatch,
reserve enough remaining journal capacity to persist the worst-case bounded
response (including hex/JSON encoding and event overhead); otherwise refuse
without a model call or monetary reservation. The response, attempted-byte,
prompt, event and whole-document limits must all have exact and one-over
tests. A storage failure after dispatch still leaves an uncertain intent
even if capacity was preflighted.

The journal is evidence and recovery data, not authority. The source task,
model response and retained document cannot construct a
`ModelInvokeCapability`, authorization grant, effect handler, credential,
or store writer. **Bounded raw proposal response bytes are deliberately
retained in `AttemptSettled` for exact decoder replay**. Provider error
bodies, headers, credentials and raw host configuration do not enter source
context or journal entries. A chain is
not a MAC; recovery assumes a caller-authorized trusted store with one
writer. Hostile or malformed document bytes are bounded and rejected
before model or effect calls. No filesystem or power-loss durability is
claimed until a concrete store and interruption test prove its commit
contract.

## Executable gates required before implementation claims

The following are required end-to-end source runtime gates. Primitive unit
checks establish encoding, phase, charge and clock properties, not actual
source-driver dispatch or recovery:

1. Exact and one-over source attempts, entry count, encoded bytes, prompt
   and response; malformed first proposal charges one reservation, then a
   second attempt uses a new intent and cannot exceed the ceiling.
2. Intent-store failure makes zero OpenCode calls; crash immediately after
   durable intent conservatively retains charge and refuses redispatch even
   if the call had not started.
3. One real stub call followed by failed settlement commit recovers an
   uncertain intent with exactly one observed call, no refund, no retry and
   no effect. A settled response committed before decode replays with zero
   model calls and reaches the same compiler decode result.
4. Provider failure, timeout, cancellation in flight and late successful
   settlement retain the charged units and distinct closed reasons; unknown
   billing is not represented as a zero charge.
5. Every changed bind-time field, turn/attempt/request digest, response
   bytes, generation, chain, phase order, duplicate entry, truncated entry
   and wrong schema fails closed before host access, including a document
   whose individual entries still parse.
6. Crash before/after effect intent and observation proves zero duplicate
   physical effects; replay reruns authorization and rejects a substituted
   grant/operation. Completion, suspension and failure terminal replays make
   zero model and effect calls.
7. A recovered run uses the same absolute deadline in a restart-stable
   clock domain. Expiry, missing clock, detectable clock regression and a
   fresh-process timeout reset all refuse before another call.

Until these gates and a concrete driver/store binding pass, end-to-end
source-mode durability remains unimplemented. Existing #114 generic-kernel results remain
valid at their recorded scope; #113 source durability remains open.

## Implemented primitive boundary

`SourceInvocationBinding::bind` validates bounded policy inputs and derives the
source identity. `SourceCheckpointSink::append_at` validates the causal prefix,
retains a nondecreasing clock floor, reserves settlement capacity before an
intent, and commits one whole generation. Any store error poisons that cursor:
the store may have committed while losing its acknowledgement. Reloading the
latest authoritative document may therefore reveal a charged uncertain intent.
Recovery refuses resuming a sink whose last event is an unresolved model or
effect intent. A generation equals its entry count in this append-only profile.

`recover_source_checkpoint` rejects wrong bindings, phase drift, mismatched
effect/terminal records, raw-byte digest mismatch, noncanonical encodings and
extra keys. It returns an opaque validated record. Ledger restoration consumes
its fixed reservation, ceiling and checked intent sum; a different clock domain,
clock rollback, expiry or changed reservation amount refuses. The host must
supply a clock whose epoch survives restart and load the **latest authoritative**
checkpoint under exclusive writer control. The byte decoder cannot detect a
replayed older valid checkpoint or authenticate rewritten trusted-store contents.

Raw response/effect bytes and measured request/failed-response work are retained.
Provider token/cost counters and host receipts are not serialized by this
primitive version. The complete source driver, adapter replay, decoder replay,
effect replay, terminal failure evidence and migration still need the shared
journal wiring.

A future terminal replay reader must classify an already committed terminal
before constructing a continuation ledger. `resume_source` rejects an expired
clock for further work; it does not erase a previously committed terminal or
itself implement the read-only terminal replay path.


## Explicit OpenCode attempt boundary

The private `OpenCodeProposalSource::propose_checkpointed` entry point accepts
one caller-owned source checkpoint sink and restart-stable clock. Its caller
must supply the checked `RunOpened`/`TurnObserved` prefix. The adapter verifies
the source revision, deployment, task bytes/budget and proposal schema available
in its context, plus the bound response cap, fixed reservation and clock domain.
The checked driver still owns lifecycle identity, ProgramRoot, stage limits and
correlation of actual retained state with the observed prefix.

`preflight_at` checks the candidate phase and bounded settlement capacity without
writing or charging. The adapter then reserves through its existing accounting
hook and requires the actual intent write to be acknowledged before invoking
the provider. It rechecks cancellation and clock policy after that callback.
Raw settled bytes or a closed failure are checkpointed before response text can
reach the compiler decoder. A response already late on arrival records a
charged deadline failure with its measured byte count. Expiry during settlement
acknowledgement retains the already committed raw response but refuses exposing
it; the future driver still owns the subsequent proposal refusal and stop. A failed store acknowledgement returns a refusal and poisons the
sink; it never triggers a transport retry.

This method records a single attempt, not an entire checked source execution.
The caller must retain the one matching accounting ledger and authoritative
store; the adapter does not authenticate an arbitrary policy hook or create a
new ledger. It does not replay stored responses, resume uncertain intents,
record compiler proposal decisions, or publish a terminal result. The existing
`ProposalSource::propose` and `run_live` remain nondurable. Neither unit fixtures
nor this optional method establish an end-to-end live-provider durability gate.
