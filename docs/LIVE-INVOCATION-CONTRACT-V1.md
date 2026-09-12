# Live Invocation Contract v1

Status: **LOCAL** bounded design + reference kernel, fixture-backed. This is
the co-designed contract for issue #108 ("Define live invocation identity
and causal-journal contract") and issue #177 ("Add a provider-independent
`model.invoke` effect to the source-native Agent runtime"). Both issues share
exactly **one** runtime kernel and **one** causal journal — this document and
`src/live_invocation/` are that shared design, not two parallel ones.

Audience: implementers of the fourteen open issues in this lane (#109–#116,
#178–#181) that consume these interfaces, and reviewers of the identity,
authority and recovery contract per #108's review checkpoint.

This is a design checkpoint with a small executable reference, per #108's
bounded scope ("Produce one owned design and a small executable
journal/reference state machine... not permission to rewrite the frozen
runtime"). It does not touch `agent_lifecycle`, `agent_runtime_v2`, HIR, or
the parser. Wiring `model.invoke` into the compiled Agent pipeline (source
syntax, HIR node, deployment provider/model binding, the real compiler-
derived proposal grammar, the real `agent_lifecycle::authorization` mint) is
downstream implementation work against the traits this document fixes.

## Why a new module instead of extending typed effects v3

[Agent Typed Effects v3](AGENT-TYPED-EFFECTS-V3.md) already binds an
injected host boundary to deployed tool contracts, with its own registry,
budget and evidence. It was considered and rejected as `model.invoke`'s
transport: its admitted host scalars are `bool`/`i32`/`i64`/`u8`/`usize`,
at most eight argument and eight result fields, and "nested carriers remain
outside this profile." A `model.invoke` request's observation/context
projection and a response's raw bytes are naturally unbounded byte payloads,
not eight scalar fields — forcing them through that registry would either
violate its own admitted-vocabulary invariant or silently narrow what a
model call can carry. `model.invoke` therefore gets its own typed boundary
([`model_invoke`](#the-modelinvoke-effect)), but shares [operation checkpoint
v2](AGENT-OPERATION-CHECKPOINT-V2.md)'s vocabulary and Intent-before-dispatch
discipline for its journal, described below.

## Invocation identity

A live invocation's identity is derived once, from exactly the bytes known
before any model or effect call is made:

```
LiveInvocationSeed {
    program_root:               String,   // exact ProgramRoot
    deployment_policy:          String,   // provider/model binding digest
    task:                       Vec<u8>,
    budget:                     i64,      // total invocation budget ceiling
    interaction_schema_digest:  String,   // compiler-derived proposal grammar
    approved_providers:         Vec<String>, // deployment's approved providers, in order
}
```

`LiveInvocationId::derive(&seed)` folds a domain separator and the seed's
canonical encoding through SHA-256 into `sha256:<hex>`. Two identical seeds
produce identical identities; any differing byte — including provider list
*order*, since a reordered fallback policy is a different deployment
decision — produces a different one.

**Why this is stable across retry, resume and recovery.** No field of
`LiveInvocationSeed` can only be known after a model or effect call: there is
no response digest, no usage counter, no provider-reported identifier. A
model response is structurally incapable of contributing to the seed — the
type has no field for one. Retrying an uncertain `model.invoke` call,
resuming a suspended invocation, and recovering after a crash all re-derive
the identity from the *same* pre-dispatch bytes the original attempt used, so
a resumed causal journal is recognised as continuing the same chain rather
than starting a new one (every `TurnOpened` entry names this identity, and
[`validate`](../src/live_invocation/journal.rs) rejects any entry naming a
different one — see "Cross-invocation pairing" below).

Identity carries **no authority**. It names a causal journal chain; it does
not grant permission to extend it, dispatch a call against it, or mint an
authorization.

## The causal journal

One journal, shared by #108 and #177, records what a live invocation
actually committed before crossing each external boundary. Every
`model.invoke` call is one `RequestIntent`/`ResponseRecorded` (or
`ResponseFailed`) pair; a further tool effect within the same turn is the
matching `EffectIntent`/`EffectObserved` pair, in the same journal, the same
vocabulary, chain-linked the same way.

### Record format

`JournalEntry` (`src/live_invocation/journal.rs`) is a closed enum. Every
turn's entries appear in exactly this shape:

| Entry | When | Binds |
|---|---|---|
| `TurnOpened` | Observation prepared. No external boundary approached yet. | invocation identity, observation digest |
| `RequestIntent` | The `model.invoke` request is durable *before* dispatch. | request digest, reserved budget |
| `ResponseRecorded` **or** `ResponseFailed` | After the physical call settles. | response digest + bytes, **or** closed failure tag + attempted bytes |
| `ProposalAdmitted` **or** `ProposalRefused` | After compiler-derived decode, only following `ResponseRecorded`. | decoded proposal digest, **or** refusal reason |
| `AuthorizationConsumed` **or** `AuthorizationRefused` | The one opaque grant this turn consumed, only following `ProposalAdmitted` — **or** authorization never consumed one (gate refusal, or cancellation observed after `ProposalAdmitted`; issue #113). | grant digest, **or** closed refusal reason |
| `EffectIntent` / `EffectObserved` **or** `EffectIntent` / `EffectFailed` | Zero or more further tool-shaped effects within the turn, matched by operation identity — the effect either settles or fails/is cancelled before it is ever called (issue #113). | operation id, request/observation digest, **or** operation id + closed failure reason |
| `Transition` | The deterministic reduction's selection: `continue`/`complete`/`suspend`/`fail`. | case, carrier digest |
| `TerminalOutcome` | Only after a non-`continue` `Transition`. Nothing may follow. | case, carrier digest |

`AuthorizationRefused` and `EffectFailed` (issue #113) close a gap the
original #108/#177 vocabulary left: before they existed, an authorization
refusal or an effect failure/cancellation produced a journal that skipped
straight from `ProposalAdmitted`/`EffectIntent` to `Transition` — a shape
`journal::validate` itself rejected, so that outcome could never actually be
replayed through `kernel::run_live_invocation` a second time. Both new
entries are, like `ResponseFailed`, terminal for their own turn's remaining
phases; neither is ever a [`ModelFailure`](#closed-failure-taxonomy) — an
authorization or effect outcome is never mistaken for a model/provider
failure because it has its own entry kind and its own reason text.

This is exactly the state list #108 asked for: *observation prepared,
request intent durable, response recorded, proposal admitted/refused,
authorization consumed, effect intent/observation, transition, terminal
outcome* — plus the distinction the issue also asked for, between
*model-attempt uncertainty* (`ResponseFailed` covers `Cancelled` explicitly)
and *effect-delivery uncertainty* (the separate `EffectIntent`/`EffectObserved`
pair).

The canonical wire is a JSON array with one entry per line-equivalent object,
closed keys, sorted encoding and a `sha256:<64 hex>` digest format for every
digest field (`src/live_invocation/journal.rs::render`/`decode`). Response
and observation *bytes* — not just digests — are recorded in
`ResponseRecorded`, because a replay must consume the trusted recorded
observation, never reconstruct one by hashing caller-provided data (see
"Determinism and replay" below). Terminal carrier *bytes* are deliberately
**not** stored, only their digest — the same "digest, without exposing
payloads" discipline [Agent Iterative Lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md)
already uses for its terminal-carrier evidence.

### Ordering rules

`journal::validate` is a small table-driven state machine, not a monotonic
rank check: one turn's entries must appear in exactly the table order above.
`Transition { case: "continue" }` must be followed by the next turn's
`TurnOpened` (turn number advancing by exactly one, same invocation id); any
other case must be followed by exactly one matching `TerminalOutcome`, after
which the journal must end. It rejects, by construction:

- **Omission** — a required entry (e.g. `RequestIntent`) missing from a
  turn's sequence.
- **Reorder** — any two adjacent entries transposed.
- **Cross-invocation pairing** — a `TurnOpened` naming an `invocation` other
  than the journal's own bound identity.
- **Schema drift** — enforced one layer up, by the kernel comparing a
  `ProposalDecoder`'s bound `schema_digest()` against the invocation's
  `interaction_schema_digest` *before* any dispatch (`LiveKernelError::SchemaDrift`);
  a decode that succeeds against the wrong grammar is exactly what this
  check exists to prevent from ever reaching the handler.
- **Post-terminal continuation** — any entry after `TerminalOutcome`.

A journal that ends immediately after `RequestIntent`, with no recorded
response, is **uncertain** (`ValidatedJournal::uncertain_intent`): the kernel
refuses to proceed — including refusing to redispatch the same request —
before any further stage, store write, or host call. This mirrors [operation
checkpoint v2](AGENT-OPERATION-CHECKPOINT-V2.md)'s identical rule for tool
effects.

A journal that ends cleanly right after a `continue` `Transition` (or is
empty) is **resumable** (`ValidatedJournal::resumable_turn`): the next turn
may open without redispatching anything already recorded. Every other
mid-turn ending (decode/authorize/effect pending) is neither terminal nor
resumable in this bounded kernel; the reference implementation returns
`LiveKernelError::UnresolvedPrefix` rather than guess the missing entries. A
real deployment reconciles such a state out of band (the same declared
nonclaim [Agent Checkpoint v1](../src/agent_lifecycle/durable.rs) documents
for its own uncertain window) before calling back in.

### Trust model

The journal carries **no key material and no signature** — a party who can
rewrite the caller's storage can also recompute its hash chain
(`journal::chain`). This is a declared nonclaim, matching
`src/agent_lifecycle/durable/journal.rs`'s identical position, not a gap this
contract papers over. What makes a forged, truncated or withheld journal
harmless to *authority* is that [`AuthorizationGrant`](#the-modelinvoke-effect)
is minted by re-running `AuthorizationGate::authorize` against a state and
proposal the kernel recomputes itself — never by decoding a journal entry.
A journal can misdescribe or omit a turn; it can never produce a grant for
one, and it can never resurrect a model response that was never actually
recorded: `ResponseRecorded` carries the response *bytes*, so replay
consumes what was actually observed, not a hash a caller could fabricate.

### Receipts are a projection, not a second log

`journal::receipt_projection` folds an already-`validate`d journal into a
compact summary (turn count, model call/failure counts, effect call count,
terminal case). It is a pure function of the journal's entries — there is no
receipt-only state anywhere in this module, and two callers folding the same
journal always derive the same projection. Issue #180 ("receipts") is scoped
to build its richer receipt document as a further projection of *this*
journal, not as a value tracked independently alongside it.

## Determinism and replay

`kernel::run_live_invocation` is the **one** runtime kernel both issues
share. It takes a starting journal (possibly empty) and continues from
exactly where that journal validates to:

- **Fresh start** — empty journal, begins at turn 0.
- **Resume** — a journal ending right after a `continue` transition begins
  at the next turn, without redispatching the recorded prefix.
- **Replay** — an already-terminal journal is recognised immediately
  (`ValidatedJournal::terminal`); the kernel returns its recorded outcome
  and `LiveKernelRun::dispatched == 0` — the turn loop never runs, so
  `ModelHandler::invoke` and every other injected seam is never called. The
  reference test `replaying_a_terminal_journal_makes_zero_dispatches_and_reproduces_the_outcome`
  wires every seam to a fixture that panics if touched, to make this an
  executable, not just an asserted, property.
- **Uncertain intent** — a journal ending right after `RequestIntent` with
  no response is refused (`LiveKernelError::UncertainIntent`) before any
  further work, including redispatch.

Recontacting a model after an uncertain or failed attempt is a **new**
`model.invoke` call under new accounting — this kernel does not attempt to
"replay" a nondeterministic model response. A retry is a fresh call through
`ModelHandler::invoke`; only the *deterministic* stages (observe, decode,
authorize, reduce) are ever replayed from recorded bytes.

## The `model.invoke` effect

`src/live_invocation/model_invoke.rs` defines the provider-independent
effect boundary; `src/live_invocation/kernel.rs` defines the seams a real
deployment binds it through.

### Typed request/response boundary

```
ModelInvocationRequest {
    turn:                      u32,
    task:                      Vec<u8>,
    observation:               Vec<u8>,     // this turn's deterministic context projection
    proposal_grammar_digest:   String,      // compiler-derived grammar identity
    deployment_binding:        String,      // exact deployment/model policy digest
    max_response_bytes:        usize,
    effective_budget:          i64,
}
```

Every field is compiler- or deployment-derived, built *before* any provider
is contacted — never model output. The request structurally cannot carry a
raw filesystem path, environment variable, credential, or unchecked dynamic
map: a real `ModelHandler` implementation is responsible for attaching
provider credentials from outside checked program data.

```
enum ModelInvocationOutcome {
    Settled(Vec<u8>),                                   // untrusted response bytes
    Failed { failure: ModelFailure, attempted_bytes: usize },
}
```

The raw response is recorded (`ResponseRecorded`) *before* decode is
attempted; decode (`ProposalDecoder::decode`) is the only path from response
bytes to a value anything downstream calls a proposal.
`AuthorizationGate::authorize` and, if bound, a further tool effect never see
raw model output — only the decoded, admitted proposal.

### Capability requirement

```
ModelInvokeCapability::grant(reason: impl Into<String>) -> Self
```

There is no `Default` implementation and no ambient constructor. Compiled
program text and generated code hold no path to a `ModelInvokeCapability`
without an explicit host-supplied value passed into
`kernel::run_live_invocation`'s `LiveInvocationHandlers`. This is the
concrete mechanism behind AGENTS.md's "capabilities are explicit... no
ambient... network... authority": declaring a `propose` role in source, or
compiling a module that uses one, creates no path to a live provider call by
itself.

### Closed failure taxonomy

```
enum ModelFailure { Timeout, Cancelled, CapacityExceeded, ProviderError, MalformedResponse, Refused }
```

Provider-specific errors never become language semantics: a real handler
normalizes whatever a transport reports into exactly one of these before
returning, and the journal records only the closed tag plus a bounded
attempted-byte count — never provider-shaped detail. `MalformedResponse`
also covers a response the kernel itself rejects as oversized
(`response.len() > max_response_bytes`), checked before decode is attempted,
so an enormous or truncated payload cannot drive unbounded downstream work.

### Cancellation point

The kernel checks `AgentCancellation::is_cancelled()` at five points per
turn (issue #113 added the fourth and fifth), mirroring [Agent Iterative
Lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md)'s "checked at each
deterministic stage and before dispatch":

1. **Before opening a turn.** If cancelled here, the kernel stops cleanly
   with no entries written for that turn; the journal is left non-terminal
   (`LiveInvocationOutcome::Cancelled`).
2. **After `TurnOpened`, before committing `RequestIntent`.** Same clean
   stop; the turn's observation was recorded but nothing durable was
   committed toward a call.
3. **After `RequestIntent` is committed, immediately before calling
   `ModelHandler::invoke`.** Cancellation here is folded into the closed
   failure domain as `ModelFailure::Cancelled` — the request was already
   durable, so the turn must still resolve to a recorded outcome
   (`ResponseFailed`) rather than leaving an uncertain intent behind.
4. **After `ProposalAdmitted` is committed, immediately before calling
   `AuthorizationGate::authorize`.** Authorization is never a model call, so
   this is recorded as `AuthorizationRefused { reason: "cancelled" }`, never
   `ModelFailure::Cancelled` — the two must not collapse into one tag.
5. **After `EffectIntent` is committed, immediately before calling
   `TurnEffect::call`.** Same reasoning: recorded as
   `EffectFailed { reason: "cancelled" }`. This is also the checkpoint that
   makes "cancellation in flight blocks subsequent effects and result
   publication" (issue #113's required case) true: the effect is never
   called, and the turn resolves to `Fail`, never `Complete`/`Suspend` — a
   result a cancelled turn produced is never published.

Every checkpoint after the first two follows the same rule 3 already
established: once some entry is already durable for this turn, cancellation
cannot leave the journal stuck mid-turn — it must still resolve to a
recorded, replayable outcome. `tests::cancellation_after_proposal_admitted_stops_before_authorize_is_ever_called`
and `tests::cancellation_after_authorization_consumed_stops_before_the_effect_is_ever_called`
are checkpoints 4 and 5's dedicated tests, each proving the downstream seam
(`AuthorizationGate`/`TurnEffect`) is never actually called.

An acknowledged cancellation never proves a real provider stopped billing or
processing — that is a declared nonclaim, matching Direct Runtime v2's
existing cancellation contract.

### The budget hook

```
trait InvocationBudgetHook {
    fn reserve(&mut self, request: &ModelInvocationRequest) -> Result<ReservedBudget, BudgetRefusal>;
    fn record(&mut self, usage: &InvocationUsage);
}
```

The kernel reserves before every dispatch and records usage after every
settlement; the hook itself decides policy. The shipped `FixtureBudgetHook`
remains a trivial per-invocation counter, useful only for tests that don't
care about cumulative enforcement. `budget::CumulativeBudgetLedger` (issue
#113) is the first real policy behind this hook: one monetary ceiling and,
optionally, one absolute deadline (via an injected `InvocationClock`),
enforced identically at every attempt and nonrefundable once committed. See
[Live Invocation Budget and Deadline Accounting v1](#budget-and-deadline-accounting-issue-113)
below for the full design; issue #179 may extend this further (e.g. real
provider pricing), but does not need to invent a second hook to do it.

### Budget and deadline accounting (issue #113)

**Where the nonrefundable decrement happens, relative to dispatch.**
`CumulativeBudgetLedger::reserve` both decides whether an attempt fits the
remaining ceiling and, if it does, commits that amount against the ceiling
in the same call, before returning — strictly before the kernel's own
dispatch to `ModelHandler::invoke` (the kernel journals and persists
`RequestIntent.reserved_budget` immediately after this call and before that
dispatch; see `kernel.rs`). By the time a call could possibly have reached a
provider, its cost is already charged and already durable.

**Why a retry cannot double-spend.** `CumulativeBudgetLedger` is never the
durable source of truth for `committed` — `CumulativeBudgetLedger::resume`
reconstructs it by folding over an already-persisted journal prefix and
summing every `RequestIntent.reserved_budget` seen so far, the exact value
`reserve` already committed and the kernel already made durable before
dispatch. Replaying that fold after a real or simulated crash always yields
the same total, so a reservation is nonrefundable by construction — there is
no separate in-memory counter to lose. The fault-injection test proving this
directly: `budget::tests::resuming_after_a_simulated_crash_never_refunds_the_already_committed_reservation`
builds a journal ending in an uncertain `RequestIntent` (no recorded
response — the exact shape a crash between reservation and settlement
leaves behind) and shows a fresh ledger resuming from it still refuses a
retry that would exceed what actually remains.

**`record` never refunds.** A settlement using fewer bytes than reserved, or
a failed attempt using none at all, never credits the difference back onto
`remaining` — matching issue #113's own scope note that "monetary limits are
conservative reservations...not a promise of exact live billing." This is
also why a timeout with unknown billing never appears as zero usage: the
reservation it already consumed stays consumed regardless of what `record`
is later told.

**Budget-exhausted, deadline-exceeded and cancelled never collapse into one
tag.** `reserve`'s refusal is always one of the closed reasons
`budget::BUDGET_EXHAUSTED`, `budget::DEADLINE_EXCEEDED` or
`budget::NEGATIVE_REQUEST` — never `ModelFailure`. Previously the kernel
hardcoded `ModelFailure::CapacityExceeded` for *any* budget-hook refusal,
misrecording a self-imposed refusal as if the provider itself had reported
no capacity; the kernel now writes the hook's own reason text into
`ResponseFailed.failure` instead (`kernel.rs`,
`tests::a_cumulative_budget_ledger_stops_the_run_once_its_ceiling_is_exhausted_not_the_turn_counter`
asserts the recorded tag is never `ModelFailure::CapacityExceeded`).
Cancellation is a third, independently-checked thing (see "Cancellation
point" above) recorded through its own journal entries, never through this
hook at all.

**The deadline is an absolute instant, not a duration.**
`CumulativeBudgetLedger::with_deadline` takes an absolute `deadline_millis`
in the bound `InvocationClock`'s own units, not "N milliseconds from now." A
caller resuming a suspended or recovered invocation re-supplies the same
absolute value it used originally (typically `invocation_started_at +
max_duration`, computed once at bind time, the same way
`program_root`/`task`/`deployment_binding` are already re-supplied
identically on every call into `kernel::run_live_invocation`) — there is
structurally no "from now" constructor, so a resumed call cannot reset the
deadline merely by supplying a fresh duration.
`budget::tests::resume_preserves_an_absolute_deadline_across_the_same_simulated_crash`
exercises this directly.

**Not a live price lookup.** `effective_budget`/`ceiling` stay opaque
caller-defined units (`ModelInvocationRequest::effective_budget`'s existing
documentation: "the deployment defines what one unit costs"). This module
does no currency conversion and no provider pricing lookup; live price
lookup, if ever added, stays outside the compiler per issue #113's own scope
note.

**Reference:** `src/live_invocation/budget.rs` (`CumulativeBudgetLedger`,
`InvocationClock`) and its `tests` submodule; `fixture::StepClock` is the
one deterministic clock implementation this crate ships.

## What downstream issues implement against

| Interface | Owns |
|---|---|
| `ModelHandler` | A real provider transport (#180/#181 own multiple providers; #112 owns the first live one) |
| `ProposalDecoder` | The real compiler-derived proposal grammar (#109 owns the rich schema) |
| `AuthorizationGate` | `agent_lifecycle::authorization`'s real mint site |
| `InvocationBudgetHook` | Cumulative budget/deadline policy: `budget::CumulativeBudgetLedger` (#113); further extension (e.g. real provider pricing) is #179's scope |
| `TurnObserver` / `TurnPolicy` | The compiled Agent's `observe`/`reduce` stages (source/HIR wiring, #109–#116) |
| `TurnEffect` | A deployed tool call via `agent_lifecycle::iterative::effects::TypedEffectHandler` |
| `journal::receipt_projection` | The richer receipt document (#180) |
| `journal::JournalEntry` (streamed) | The streaming extension (#178) — a stream is a further-refined view of the same `RequestIntent`→`ResponseRecorded` pair, not a second journal |

### Two independences this design makes structurally true

- **#112 (first live provider) does not depend on #181 (two-provider SDK).**
  `ModelHandler` is one trait with one `invoke` method; a single provider
  binds it directly. Nothing in `model_invoke.rs` or `kernel.rs` requires
  more than one registered handler to exist — a two-provider *selection*
  policy (#181) is a caller-side concern (which `ModelHandler` gets
  constructed and passed in), entirely outside this boundary.
- **#109 (rich schema) does not depend on #178 (streaming extension).**
  `ProposalDecoder::decode` takes a complete `&[u8]` response and returns
  one `ProposalOutcome`; there is no partial-decode state threaded through
  the journal. A streaming transport is free to buffer provider events into
  one complete response before calling this same boundary, so a richer
  grammar (#109) needs nothing from a streaming transport (#178) to exist.

## Issue #177 acceptance-criteria audit

Issue #177 ("Add a provider-independent `model.invoke` effect to the
source-native Agent runtime") named five acceptance criteria against a
kernel that, per its own audit baseline, was already landed and tested for
#108. This section maps each one to what `src/live_invocation/` provides
today, so a later reader does not have to reconstruct the mapping from the
issue thread. It distinguishes what this module's trait boundary provides
from what still requires wiring this document already scopes to downstream
issues (see "What downstream issues implement against" above) — closing a
gap that belongs to another issue's owning module is not this audit's job,
and is called out as such rather than claimed.

| # | Criterion | Status | Evidence / gap |
|---|---|---|---|
| 1 | A source-native Agent can perform a live provider-neutral propose step through Direct Runtime v2. | **Not met** | No parser/HIR/source syntax exists for `model.invoke` (declared non-goal, below), and this module does not touch `agent_lifecycle` or `agent_runtime_v2`. The trait boundary a compiled `propose` role would call through exists (`model_invoke::ModelHandler`, `kernel::run_live_invocation`); wiring a source Agent's `propose` role to dispatch through it is #109–#116's scope. |
| 2 | Model invocation is declared, capability-gated, bounded, cancellable, and evidence-bearing. | **Met at the trait-boundary level; not met at the source-declaration level** | Capability-gated: `ModelInvokeCapability` has no `Default` and no ambient constructor (`model_invoke.rs`). Bounded: `InvocationBudgetHook::reserve`/`record` plus `max_response_bytes`/`max_turns` (`kernel::run_live_invocation`). Cancellable: three checkpoints as of this audit's #177 baseline, each with a dedicated test — `tests::cancellation_before_any_turn_opens_stops_cleanly_with_an_empty_journal` (checkpoint 1), `tests::cancellation_after_turn_opened_stops_cleanly_before_any_request_intent` (checkpoint 2), `tests::cancellation_after_request_intent_is_committed_folds_into_a_recorded_cancelled_failure` (checkpoint 3) — issue #113 later added checkpoints 4 and 5 (before authorize, before the effect dispatch); see "Cancellation point" above for the current five. Evidence-bearing: the causal journal and `journal::receipt_projection`. "Declared" only holds as a Rust trait boundary — there is no `.spx` source syntax to declare a `propose` role against this effect (see criterion 1 and the non-goals below). |
| 3 | No provider name or credential becomes part of core language semantics. | **Met** | `ModelInvocationRequest` structurally has no credential, path, environment, or unchecked dynamic-map field (`model_invoke.rs` doc comment and field list). `ModelFailure` is a closed six-variant enum with no provider-shaped variant. The only implementations this crate ships are `fixture::*`, so no provider name appears anywhere in this module's non-test code. |
| 4 | Model output cannot mint or bypass authorization. | **Met** | `kernel::run_live_invocation` passes only the *decoded* proposal (`ProposalOutcome::Admitted` bytes) to `AuthorizationGate::authorize` and to `TurnPolicy::reduce`; raw response bytes never reach either. Negative case: `tests::a_malformed_response_is_refused_before_authorize`, `tests::a_closed_model_failure_ends_the_attempt_without_decoding_or_authorizing`, `tests::an_oversized_response_is_treated_as_malformed_before_decode` (each asserts `gate.granted == 0`). Positive case, closing what was previously an evidence gap because every other test's fixture decoder admits its input unchanged: `tests::authorize_and_completion_see_the_decoded_proposal_never_the_raw_response_bytes` uses a decoder that actually transforms the bytes and proves both the authorization digest and the completion payload reflect the decoded value, never the raw response. `AuthorizationGrant` is itself opaque (carries a digest only, `model_invoke.rs`), so a journal reader cannot forge one even having read every recorded byte. The real mint (`agent_lifecycle::authorization::run_authorize_stage`) is not wired — `fixture::FixtureAuthorizationGate` always grants within a ceiling — but that does not weaken this criterion: the boundary structurally prevents raw model output from ever being the value authorized, independent of which `AuthorizationGate` is bound. |
| 5 | The new route has a versioned contract and hosted evidence before support claims. | **Partially met** | Versioned contract: this document, status **LOCAL**. Hosted evidence: **not met** — every one of the 39 `live_invocation` lib tests runs against `fixture::FixtureModelHandler`'s scripted queue; none has run in a hosted environment against a real provider, and none is claimed to. This is the same declared nonclaim as "No live network call, no real provider credential, no model spend" below, restated against this specific criterion so it cannot be read as satisfied by a passing local `cargo test`. |

Required-evidence checklist, same audit:

- **Deterministic scripted provider success, malformed proposal, refusal,
  timeout, cancellation, capacity, and provider-error cases.** Met: success
  (`tests::a_three_turn_fixture_invocation_completes_with_one_dispatch_per_turn_and_one_effect`),
  malformed proposal / decode refusal
  (`tests::a_malformed_response_is_refused_before_authorize`), refusal
  (`tests::a_refused_failure_ends_the_attempt_without_decoding_or_authorizing`),
  timeout (`tests::a_timeout_failure_ends_the_attempt_without_decoding_or_authorizing`),
  cancellation (the three checkpoint tests named in row 2 above), capacity —
  both the budget-hook path
  (`tests::a_refused_budget_reservation_fails_the_turn_without_dispatching_the_handler`)
  and the distinct handler-reported path
  (`tests::a_handler_reported_capacity_exceeded_failure_ends_the_attempt_without_decoding_or_authorizing`)
  — and provider-error
  (`tests::a_closed_model_failure_ends_the_attempt_without_decoding_or_authorizing`).
- **Provider/model substitution preserves Agent semantic identity but
  changes DeploymentRoot/ExecutionRevision.** Not met here, and not this
  module's to close: "Agent semantic identity", `DeploymentRoot` and
  `ExecutionRevision` are concepts owned by `agent_lifecycle`/
  `execution_revision` (the latter frozen and out of this lease's reach).
  This module can only show that its own `deployment_policy` seed field
  changes `LiveInvocationId` on substitution
  (`tests::identity_binds_program_root_deployment_and_task_so_a_changed_input_changes_the_chain`)
  — necessary but not sufficient evidence for this criterion, which needs
  the cross-module binding #109–#116 own.
- **Source or Proposal type changes stale an existing deployment and
  invocation.** Not met, not this module's to close: there is no compiled
  Proposal type yet (`fixture::FixtureProposalDecoder` is a toy, #109's
  scope), so nothing exists to change or stale.
- **No handler call occurs on missing capability, stale ProgramRoot, invalid
  deployment, exhausted budget, or cancelled invocation.** Missing
  capability is met by construction, not by a runtime test: `ModelHandler::invoke`
  takes `&ModelInvokeCapability` as a required parameter with no `Default`
  and no ambient constructor, so there is no code path that reaches the
  handler without one — the case cannot be exercised because it cannot be
  constructed. Exhausted budget and cancelled invocation are met
  (`tests::a_refused_budget_reservation_fails_the_turn_without_dispatching_the_handler`;
  the three cancellation-checkpoint tests, all of which assert `handler.calls == 0`
  or `dispatched == 0`). Stale ProgramRoot and invalid deployment are **not
  met here**: this kernel takes `program_root`/`deployment_binding` as
  trusted caller-supplied strings and does not itself consult a
  ProgramRoot/deployment registry to judge staleness or validity — that
  registry lookup is downstream integration work, not a gap in this
  boundary's own logic.
- **Decoded proposal is the only value passed to authorize; raw model
  output never reaches effect dispatch.** Met, per criterion 4 above.
- **Existing one-pass/scripted compatibility routes remain unchanged.** Met:
  this round changed no production code path apart from adding one
  `#[cfg(test)]`-only helper (`kernel::proposal_digest_for_test`); every
  existing test still passes unmodified.

## Non-goals and known limitations (this round)

- **No live network call, no real provider credential, no model spend.**
  Every test in `src/live_invocation/tests.rs` uses `fixture::FixtureModelHandler`
  with a scripted response queue.
- **No parser/HIR/source syntax.** `model.invoke` is not yet a declarable
  Agent role or effect in `.spx` source; this contract only fixes the Rust
  trait boundary and journal a future syntax lowers to.
- **No real compiled proposal grammar.** `fixture::FixtureProposalDecoder`
  checks a toy JSON shape, not the compiler-derived
  `CompiledAgentProposalSchema` from `agent_proposal`. Binding the real one
  is #109's scope.
- **No real authorization mint.** `fixture::FixtureAuthorizationGate` always
  grants within a call ceiling; it does not call
  `agent_lifecycle::authorization::run_authorize_stage`. Wiring that is
  downstream integration, not a change to this contract's shape.
- **Resume is bounded.** The kernel resumes only from a journal ending
  cleanly between turns; a journal stuck mid-turn (decode/authorize/effect
  pending) returns `LiveKernelError::UnresolvedPrefix` rather than being
  automatically reconciled. Automatic reconciliation of that state is
  unimplemented and is not claimed here.
- **No receipt document, no streaming transport, no multi-provider
  selection.** These are the explicit hooks named above; implementing the
  policy behind each is the named downstream issue's scope, not this one's.
  (Cumulative budget/deadline policy — the fourth item this list used to
  name — landed as `budget::CumulativeBudgetLedger`, issue #113; see
  "Budget and deadline accounting" above. It operates entirely against the
  fixture provider, opaque caller-defined budget units, and an injected,
  test-controlled `InvocationClock` — no live model call, no real provider
  pricing, no real wall-clock wiring into a compiled Agent's deployment.
  Issue #179 may still extend this, e.g. with real provider pricing.)
- **Persistence across a process boundary is a separate document.** This
  contract's kernel and journal are exercised purely in memory here.
  [Live Invocation Persistence v1](LIVE-INVOCATION-PERSISTENCE-V1.md) (issue
  #114) adds the write-side seam (`LiveInvocationHandlers::sink`), the
  caller-owned store adapter, and the recovery envelope/checks a process
  restart needs, reusing this document's kernel and journal unchanged.

## Executable reference

`src/live_invocation/` (`identity.rs`, `journal.rs`, `model_invoke.rs`,
`kernel.rs`, `persistence.rs`, `budget.rs`, `fixture.rs`, `tests.rs`) is the
complete reference implementation this document describes, exercised end to
end through the fixture provider with no network access. Focused gate:

```sh
cargo test --locked -p semaprax --lib live_invocation
```
