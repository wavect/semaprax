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
[`validate`](src/live_invocation/journal.rs) rejects any entry naming a
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
| `AuthorizationConsumed` | The one opaque grant this turn consumed, only following `ProposalAdmitted`. | grant digest |
| `EffectIntent` / `EffectObserved` | Zero or more further tool-shaped effects within the turn, matched by operation identity. | operation id, request/observation digest |
| `Transition` | The deterministic reduction's selection: `continue`/`complete`/`suspend`/`fail`. | case, carrier digest |
| `TerminalOutcome` | Only after a non-`continue` `Transition`. Nothing may follow. | case, carrier digest |

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

The kernel checks `AgentCancellation::is_cancelled()` at three points per
turn, mirroring [Agent Iterative Lifecycle v2](AGENT-ITERATIVE-LIFECYCLE-V2.md)'s
"checked at each deterministic stage and before dispatch":

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
settlement, but implements **no cumulative policy** of its own — the
shipped `FixtureBudgetHook` is a trivial per-invocation counter. Issues
#113/#179 attach real cumulative budget accounting behind this one hook.

## What downstream issues implement against

| Interface | Owns |
|---|---|
| `ModelHandler` | A real provider transport (#180/#181 own multiple providers; #112 owns the first live one) |
| `ProposalDecoder` | The real compiler-derived proposal grammar (#109 owns the rich schema) |
| `AuthorizationGate` | `agent_lifecycle::authorization`'s real mint site |
| `InvocationBudgetHook` | Cumulative budget policy (#113/#179) |
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
- **No cumulative budget policy, no receipt document, no streaming
  transport, no multi-provider selection.** These are the explicit hooks
  named above; implementing the policy behind each is the named downstream
  issue's scope, not this one's.

## Executable reference

`src/live_invocation/` (`identity.rs`, `journal.rs`, `model_invoke.rs`,
`kernel.rs`, `fixture.rs`, `tests.rs`) is the complete reference
implementation this document describes, exercised end to end through the
fixture provider with no network access. Focused gate:

```sh
cargo test --locked -p semaprax --lib live_invocation
```
