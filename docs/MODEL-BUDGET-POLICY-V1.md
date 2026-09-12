# Model Budget Policy v1 (issue #179)

Status: **LOCAL** bounded design + reference implementation, fixture-backed.
No live provider call, budget charge, or billing record was produced to
support any claim here.

Audience: implementers of issue #179 ("Enforce model call, token, context,
latency, retry, failover, and cancellation budgets") and reviewers of the
pre-dispatch admission gate it adds ahead of `live_invocation`.

Implements the pre-dispatch admission gate for the model-call budget
dimensions issue #113 does not cover: maximum call count, maximum retry
count (with a proven-safe retry classification gating it), maximum
failover/provider-switch count (bound to the deployment's exact ordered,
confidentiality-checked provider list), and per-attempt/cumulative context
and output token ceilings. Lives at `src/model_budget_policy/`.

## Relationship to #113 and `live_invocation`

`src/live_invocation/model_invoke.rs` documents its
`InvocationBudgetHook` seam as the one place "issues #113/#179 attach
cumulative budget policy behind." Issue #113
(`src/live_invocation/budget.rs`'s `CumulativeBudgetLedger`) already,
completely, and correctly delivers that seam's two dimensions:

- one opaque monetary ceiling, nonrefundable, reserved-and-committed in one
  atomic call before the kernel's own dispatch;
- one absolute deadline (an *instant*, not a duration), so a resumed
  invocation cannot reset it by supplying a fresh duration;
- crash-safe resume, by folding over an already-persisted journal prefix
  rather than trusting a second durable counter;
- the distinctness of `budget_exhausted`, `deadline_exceeded`, and
  `cancelled` as three separate refusal reasons, never collapsed into one
  tag or into a provider-reported failure.

None of that is reimplemented here. `src/live_invocation/**` stays
read-only for this module: this module does not modify the kernel, the
journal, or `CumulativeBudgetLedger`, and does not invent a second monetary
or deadline seam. `ModelPolicyLedger` (below) is a *second*, independent
admission gate a caller consults before it ever builds a
`ModelInvocationRequest` and calls into `InvocationBudgetHook::reserve`. A
deployment wiring both reserves against each independently for the same
attempt: this ledger decides whether an attempt of a given *kind* (fresh,
retry, failover) and *shape* (token counts, provider) is admissible at all;
`CumulativeBudgetLedger` (or an equivalent `InvocationBudgetHook`) still
separately charges the monetary/deadline cost of the one attempt this
ledger admits.

## What #179 asks for beyond #113

Reading #179's "Required outcome" and "Implementation sequence" against
what #113 shipped:

| #179 dimension | Owned by #113? | Owned by this module |
| --- | --- | --- |
| Monetary ceiling | Yes (`CumulativeBudgetLedger`) | Not duplicated |
| Absolute deadline | Yes | Not duplicated |
| Cancellation checkpoints (model call, authorization, effect) | Yes (5 checkpoints in `kernel.rs`) | Adds a 6th: before this ledger's own `reserve_attempt`, so a cancelled attempt costs nothing on *this* ledger either |
| Maximum calls | No | `ModelBudgetLimits::max_calls`, `ModelPolicyLedger::calls_committed` |
| Maximum retries + retry-safety classification | No | `classification::AttemptOutcomeClass`, `classification::retry_is_permitted`, `ModelBudgetLimits::max_retries` |
| Maximum providers / failover, exact order, confidentiality | No | `provider_policy::ProviderPolicy`, `ModelBudgetLimits::max_providers` |
| Context/output/aggregate token ceilings | No | `ModelBudgetLimits::{max_context_tokens,max_output_tokens,max_aggregate_tokens}` |
| Effective-limit intersection (source Agent x deployment x invocation) | No | `limits::intersect` |
| Reject contradictory/zero-impossible policy before handler access | No | `limits::intersect`'s validation, refused before a `ModelPolicyLedger` can even be constructed |
| Failover is not a free retry, separately accounted, never a replay | No | Every `AttemptReservation` carries a unique, strictly increasing `ordinal` and an explicit `AttemptKind`; a failover's ordinal is never reused for the attempt it followed |

**#179 is not a duplicate of #113.** #113's scope is exactly two
dimensions (money, deadline) plus the cancellation-checkpoint completion
work; #179 additionally requires call/retry/failover/token accounting and
a retry-safety classification that #113 has no concept of at all (every
`live_invocation` kernel turn is an unconditional fresh attempt — there is
no retry or failover primitive anywhere in that module). This module adds
exactly that remaining surface, without touching #113's files.

## Effective limits: `ModelBudgetLimits` and `intersect`

`ModelBudgetLimits` is one source's declared ceiling across all eight
dimensions above. `limits::intersect(source_agent, deployment_policy,
invocation_budget) -> Result<EffectiveModelBudget, PolicyRejection>`
computes the elementwise minimum and validates it before returning:

- a negative ceiling on any dimension is `PolicyRejection::NegativeLimit`;
- an intersected `max_calls` of `0` is `PolicyRejection::ZeroImpossible` — a
  model policy that can never dispatch a single call is a contradiction,
  not a meaningful restriction, and is refused before a `ModelPolicyLedger`
  can be constructed from it at all (`ledger::ModelPolicyLedger` has no
  constructor that accepts anything but an already-validated
  `EffectiveModelBudget`).
- `max_retries == 0` or `max_providers == 0` are legal, meaningful,
  restrictive policies (no retries; no failover) and are *not* rejected.

## Retry/failover safety: `AttemptOutcomeClass`

A closed classification, distinct from
`live_invocation::model_invoke::ModelFailure` (a transport-shaped failure
taxonomy) and answering one narrower question: is retrying or failing over
after this outcome provably safe?

- `NotDispatched`, `RejectedBeforeProcessing`, `ProviderReportedRetryable`:
  safe — `retry_is_permitted` returns `true`.
- `CompletedWithResponse`, `Uncertain`: never safe — `retry_is_permitted`
  returns `false`. `Uncertain` is the *model-attempt uncertainty* case
  (the call may or may not have happened, and may or may not have billed);
  `CompletedWithResponse` is *effect-delivery certainty* the wrong
  direction (the call is known to have happened, so "retrying" it would
  not be trying the same thing again).

A value of this type is only ever constructed by trusted adapter/host code
that already knows the outcome out of band; nothing in this module decodes
one from raw provider response bytes. This is a direct instance of "model
data carries no authority": the classification that gates a retry cannot
be produced by the model whose call is being classified.

Both `AttemptKind::Retry` and `AttemptKind::Failover` require
`prior_classification` to be `Some(class)` with `retry_is_permitted(class)`
true; `AttemptKind::Fresh` requires it to be `None`. Failover is checked
against the *same* safety rule as retry — it is not a free escape hatch for
an uncertain outcome.

## Ordered, confidentiality-checked failover: `ProviderPolicy`

`ProviderPolicy` holds an ordered `Vec<ProviderSlot>`; index `0` is the
primary provider (already in use before any failover), and failover always
advances forward through the list one index at a time.
`ProviderPolicy::admit_failover(next_index, requested_id)` refuses:

- `AlternativesExhausted` — past the end of the list;
- `OutOfOrder` — the requested id is not the exact next-in-order id (a
  caller, or anything acting on model output, cannot skip ahead or invent
  an id not present at that position);
- `NotAuthorized` — the next-in-order provider exists but is not cleared
  for the task's confidentiality classification, even though it is
  correctly positioned.

`ModelPolicyLedger` calls this before admitting any `AttemptKind::Failover`
attempt, in addition to its own `max_providers` ceiling check.

## The ledger: `ModelPolicyLedger`

`reserve_attempt(cancellation, &AttemptRequest) ->
Result<AttemptReservation, AttemptRefusal>` is the single admission point.
Order of checks: cancellation first (a cancelled attempt costs nothing, not
even a call-count unit) → absolute deadline → `max_calls` → kind-specific
checks (retry-safety + `max_retries`, or failover policy + `max_providers`)
→ per-attempt context tokens → per-attempt output tokens → cumulative
aggregate tokens → cumulative cost. Every check that passes commits its
ceiling in the same call, before `Ok` is returned — there is no window
between "decided admissible" and "durably committed."

`record_outcome(AttemptUsage)` is evidence-only: it is appended to a
retained list and never adjusts any committed counter, regardless of what
it reports (including a reported zero-cost, zero-token outcome). This is
the direct analogue of `CumulativeBudgetLedger::record` never crediting a
reservation back, applied to every dimension this ledger tracks.

`ModelPolicyLedger::resume` reconstructs committed counters by folding over
an already-committed `&[AttemptReservation]` prefix, mirroring
`CumulativeBudgetLedger::resume`'s "resume from the journal, not from a
second counter" discipline.

## Tests

38 tests across `classification.rs`, `provider_policy.rs`, `limits.rs`,
and `ledger.rs` (`cargo test --locked -p semaprax --lib model_budget_policy`),
including:

- an exact zero/exact-limit/limit-plus-one boundary case for every
  dimension: `max_calls`, `max_retries`, `max_providers`,
  `max_context_tokens`, `max_output_tokens`, `max_aggregate_tokens`,
  `max_cost_micros`, and the absolute deadline instant;
- `a_failed_attempt_then_its_retry_both_spend_and_neither_is_ever_refunded`:
  the nonrefundability proof — a provider-retryable failure followed by its
  permitted retry shows committed cost and committed tokens increase
  *twice*, never restored in between;
- `a_self_reported_zero_cost_outcome_never_reopens_spent_capacity`: a
  recorded outcome claiming zero usage cannot reopen an already-exhausted
  ceiling;
- `an_uncertain_prior_outcome_never_permits_a_retry` and
  `a_completed_response_never_permits_a_retry_either`: the two unsafe
  classes are refused, each paired with a permitted-class success case
  exercising the identical code path;
- `failover_out_of_the_deployments_exact_order_is_refused` and
  `failover_to_an_unauthorized_provider_is_refused_even_if_it_is_next_in_order`:
  the ordered-policy and confidentiality checks, each with the exact
  expected/requested identifiers asserted;
- `resuming_from_a_committed_prefix_reproduces_the_same_totals_a_fresh_run_would_have_reached`:
  crash-safe resume parity.

## No live network, no real provider, no key

Every type in this module is pure, offline data. `ModelPolicyLedger`
contacts no provider and holds no credential. Tests reuse
`live_invocation::fixture::StepClock` (already public) for deterministic
time instead of a real wall clock.

## Out of scope here

Wiring `ModelPolicyLedger` into the actual `live_invocation` kernel loop,
into `AgentDeployment`, or into a real compiled proposal/HIR integration is
downstream work against the same seams `live_invocation`'s own module
documentation names (#178–#181): this module ships the policy/ledger types
and their tests, not a kernel change. `src/model_call_receipt/**` billing
reconciliation (provider invoice vs. local accounting) is a separate,
already-shipped concern this module does not touch or duplicate.
