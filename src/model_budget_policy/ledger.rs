//! [`ModelPolicyLedger`]: the pre-dispatch admission gate that reserves one
//! model-invocation attempt (fresh, retry, or failover) against an
//! [`EffectiveModelBudget`], nonrefundably, before that attempt is ever
//! allowed to reach [`crate::live_invocation::model_invoke::ModelHandler`].
//!
//! # Where this sits relative to #113
//!
//! `live_invocation::budget::CumulativeBudgetLedger` (#113) already
//! enforces one monetary ceiling and one absolute deadline behind
//! [`crate::live_invocation::model_invoke::InvocationBudgetHook`], per
//! turn, nonrefundably. This ledger is a *second*, independent admission
//! gate a caller consults *before* even building a
//! [`crate::live_invocation::model_invoke::ModelInvocationRequest`] — it
//! adds the dimensions #113 does not cover (call count, retry count,
//! failover/provider count, per-attempt and aggregate token ceilings) and a
//! retry/failover *safety* classification (#113 has no retry or failover
//! concept at all; every kernel turn is a fresh, unconditional attempt). A
//! deployment wires both: this ledger decides *whether an attempt of this
//! kind is permitted at all*, and `CumulativeBudgetLedger` (or an
//! equivalent [`InvocationBudgetHook`]) still separately reserves the
//! monetary/deadline cost of the one attempt this ledger admits. Neither
//! duplicates the other's committed state, and this module does not
//! reimplement `CumulativeBudgetLedger`'s own monetary/deadline logic.
//!
//! # Nonrefundability
//!
//! [`ModelPolicyLedger::reserve_attempt`] both decides admission and
//! commits every consumed ceiling in the same call, before returning
//! `Ok`. [`ModelPolicyLedger::record_outcome`] only retains
//! [`AttemptUsage`] as evidence — it never reduces any committed counter,
//! regardless of what the recorded outcome claims (see
//! `tests::a_self_reported_zero_cost_outcome_never_reopens_spent_capacity`
//! for the direct proof that model-reported usage cannot widen a budget
//! back open).

use crate::agent_runtime::AgentCancellation;
use crate::live_invocation::budget::InvocationClock;

use super::classification::{retry_is_permitted, AttemptOutcomeClass};
use super::limits::EffectiveModelBudget;
use super::provider_policy::{ProviderPolicy, ProviderRefusal};

/// What kind of attempt one [`AttemptRequest`] describes. Every kind
/// produces its own distinct, uniquely ordinaled [`AttemptReservation`] —
/// a failover is never recorded as though it replayed the original
/// attempt, and a retry is never recorded as though it were the first call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttemptKind {
    /// The first attempt of the invocation (or of a fresh, unrelated
    /// question the caller is not treating as a retry of anything).
    Fresh,
    /// A repeat of the same request to the *same* provider, permitted only
    /// after a proven-safe prior classification.
    Retry,
    /// A repeat of the same request routed to the *next* provider in the
    /// deployment's exact ordered failover policy, permitted only after a
    /// proven-safe prior classification and only when that next provider
    /// is authorized for the task.
    Failover,
}

/// One attempt a caller wants to make, described *before* dispatch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptRequest {
    pub kind: AttemptKind,
    /// The provider this attempt targets. Ignored for admission purposes on
    /// [`AttemptKind::Fresh`]/[`AttemptKind::Retry`] (those stay on the
    /// provider already in use); checked against the exact next ordered,
    /// authorized alternative for [`AttemptKind::Failover`].
    pub provider_id: String,
    pub context_tokens: u64,
    pub requested_output_tokens: u64,
    pub estimated_cost_micros: i64,
    /// The classification of the attempt this one follows. Required
    /// (`Some`) for [`AttemptKind::Retry`] and [`AttemptKind::Failover`];
    /// must be `None` for [`AttemptKind::Fresh`], which by definition has
    /// no prior outcome to have been classified safe.
    pub prior_classification: Option<AttemptOutcomeClass>,
}

/// One admitted, nonrefundably committed attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttemptReservation {
    /// Strictly increasing, unique across the whole ledger's lifetime —
    /// including across kinds, so a failover's ordinal is never mistaken
    /// for a continuation of the attempt it followed.
    pub ordinal: u64,
    pub kind: AttemptKind,
    pub provider_id: String,
    pub reserved_context_tokens: u64,
    pub reserved_output_tokens: u64,
    pub reserved_cost_micros: i64,
}

/// The closed admission-refusal vocabulary. Every variant names the exact
/// ceiling and the exact numbers involved.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttemptRefusal {
    /// Cancellation was observed before this attempt reserved anything.
    /// Checked first, before any other ceiling — a cancelled call must
    /// cost nothing, not even a call-count unit.
    Cancelled,
    /// The absolute deadline has already passed.
    DeadlineExceeded,
    /// Every admitted call (fresh, retry, and failover combined) has
    /// already reached `max_calls`.
    CallsExhausted { max_calls: u32 },
    /// A retry was requested but `max_retries` retries are already
    /// committed.
    RetriesExhausted { max_retries: u32 },
    /// A failover was requested but `max_providers` provider switches are
    /// already committed.
    ProvidersExhausted { max_providers: u32 },
    /// A retry or failover names a `Fresh` request with no prior outcome,
    /// or a `Retry`/`Failover` request whose prior outcome's classification
    /// is not one of the proven-safe classes.
    PriorOutcomeNotRetryable {
        kind: AttemptKind,
        class: Option<AttemptOutcomeClass>,
    },
    /// The deployment's ordered failover policy refused this exact
    /// provider switch (out of order, unauthorized, or exhausted — see
    /// [`ProviderRefusal`]).
    ProviderRefused(ProviderRefusal),
    /// This attempt alone would exceed the per-attempt context-token
    /// ceiling.
    ContextTokensExceeded { requested: u64, max: u64 },
    /// This attempt alone would exceed the per-attempt output-token
    /// ceiling.
    OutputTokensExceeded { requested: u64, max: u64 },
    /// This attempt would push the cumulative context+output token total
    /// past `max_aggregate_tokens`.
    AggregateTokensExhausted { requested: u64, remaining: u64 },
    /// This attempt would push cumulative estimated cost past
    /// `max_cost_micros`.
    CostExhausted { requested: i64, remaining: i64 },
}

/// Evidence retained about one already-admitted attempt's settlement.
/// Purely observational: see [`ModelPolicyLedger::record_outcome`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AttemptUsage {
    pub ordinal: u64,
    pub classification: AttemptOutcomeClass,
    pub actual_context_tokens: u64,
    pub actual_output_tokens: u64,
    pub actual_cost_micros: i64,
}

/// The pre-dispatch admission gate and nonrefundable ledger for one live
/// invocation's model-call policy. See the module documentation for how
/// this composes with #113's `CumulativeBudgetLedger`.
pub struct ModelPolicyLedger<'a> {
    limits: EffectiveModelBudget,
    providers: ProviderPolicy,
    clock: &'a dyn InvocationClock,
    deadline_millis: Option<i64>,
    calls_committed: u32,
    retries_committed: u32,
    providers_committed: u32,
    next_provider_index: usize,
    aggregate_tokens_committed: u64,
    cost_committed_micros: i64,
    next_ordinal: u64,
    usage: Vec<AttemptUsage>,
}

impl<'a> ModelPolicyLedger<'a> {
    /// Starts a fresh ledger. `deadline_millis`, if any, is an *absolute*
    /// instant in `clock`'s own units (computed once by the caller as
    /// `start_millis + limits.limits().max_latency_millis`, the same
    /// "absolute instant, not a duration" discipline
    /// `CumulativeBudgetLedger::with_deadline` documents) — resuming a
    /// suspended invocation re-supplies the same absolute value rather than
    /// a fresh duration, so a resumed ledger cannot reset its own deadline.
    #[must_use]
    pub fn new(
        limits: EffectiveModelBudget,
        providers: ProviderPolicy,
        deadline_millis: Option<i64>,
        clock: &'a dyn InvocationClock,
    ) -> Self {
        Self {
            limits,
            providers,
            clock,
            deadline_millis,
            calls_committed: 0,
            retries_committed: 0,
            providers_committed: 0,
            // Index 0 of `providers` is the primary provider, already in
            // use before any failover — the first failover targets index
            // 1, the first fallback alternative.
            next_provider_index: 1,
            aggregate_tokens_committed: 0,
            cost_committed_micros: 0,
            next_ordinal: 0,
            usage: Vec::new(),
        }
    }

    /// Reconstructs a ledger's committed counters from an already-durable
    /// prefix of prior reservations (the same "resume by folding over
    /// journaled reservations" discipline
    /// `CumulativeBudgetLedger::resume` uses) rather than trusting a
    /// second, independently-durable counter that could drift from what was
    /// actually reserved.
    #[must_use]
    pub fn resume(
        limits: EffectiveModelBudget,
        providers: ProviderPolicy,
        deadline_millis: Option<i64>,
        clock: &'a dyn InvocationClock,
        already_committed: &[AttemptReservation],
    ) -> Self {
        let mut ledger = Self::new(limits, providers, deadline_millis, clock);
        for reservation in already_committed {
            ledger.calls_committed = ledger.calls_committed.saturating_add(1);
            match reservation.kind {
                AttemptKind::Fresh => {}
                AttemptKind::Retry => {
                    ledger.retries_committed = ledger.retries_committed.saturating_add(1);
                }
                AttemptKind::Failover => {
                    ledger.providers_committed = ledger.providers_committed.saturating_add(1);
                    ledger.next_provider_index = ledger.next_provider_index.saturating_add(1);
                }
            }
            ledger.aggregate_tokens_committed = ledger
                .aggregate_tokens_committed
                .saturating_add(reservation.reserved_context_tokens)
                .saturating_add(reservation.reserved_output_tokens);
            ledger.cost_committed_micros = ledger
                .cost_committed_micros
                .saturating_add(reservation.reserved_cost_micros);
            ledger.next_ordinal = ledger
                .next_ordinal
                .max(reservation.ordinal.saturating_add(1));
        }
        ledger
    }

    #[must_use]
    pub fn calls_committed(&self) -> u32 {
        self.calls_committed
    }

    #[must_use]
    pub fn retries_committed(&self) -> u32 {
        self.retries_committed
    }

    #[must_use]
    pub fn providers_committed(&self) -> u32 {
        self.providers_committed
    }

    #[must_use]
    pub fn aggregate_tokens_committed(&self) -> u64 {
        self.aggregate_tokens_committed
    }

    #[must_use]
    pub fn cost_committed_micros(&self) -> i64 {
        self.cost_committed_micros
    }

    #[must_use]
    pub fn remaining_aggregate_tokens(&self) -> u64 {
        self.limits
            .limits()
            .max_aggregate_tokens
            .saturating_sub(self.aggregate_tokens_committed)
    }

    #[must_use]
    pub fn remaining_cost_micros(&self) -> i64 {
        (self.limits.limits().max_cost_micros - self.cost_committed_micros).max(0)
    }

    /// Every retained [`AttemptUsage`] record, in the order recorded.
    #[must_use]
    pub fn usage(&self) -> &[AttemptUsage] {
        &self.usage
    }

    /// Admits or refuses one attempt against every #179 budget dimension
    /// this ledger tracks, committing nonrefundably on admission.
    /// Cancellation is checked strictly first: a cancelled call costs
    /// nothing, not even a call-count unit.
    pub fn reserve_attempt(
        &mut self,
        cancellation: &AgentCancellation,
        request: &AttemptRequest,
    ) -> Result<AttemptReservation, AttemptRefusal> {
        if cancellation.is_cancelled() {
            return Err(AttemptRefusal::Cancelled);
        }
        if let Some(deadline) = self.deadline_millis {
            if self.clock.now_millis() >= deadline {
                return Err(AttemptRefusal::DeadlineExceeded);
            }
        }

        let limits = self.limits.limits();

        if self.calls_committed >= limits.max_calls {
            return Err(AttemptRefusal::CallsExhausted {
                max_calls: limits.max_calls,
            });
        }

        match request.kind {
            AttemptKind::Fresh => {
                if request.prior_classification.is_some() {
                    return Err(AttemptRefusal::PriorOutcomeNotRetryable {
                        kind: request.kind,
                        class: request.prior_classification,
                    });
                }
            }
            AttemptKind::Retry => {
                self.require_retry_permitted(request)?;
                if self.retries_committed >= limits.max_retries {
                    return Err(AttemptRefusal::RetriesExhausted {
                        max_retries: limits.max_retries,
                    });
                }
            }
            AttemptKind::Failover => {
                self.require_retry_permitted(request)?;
                if self.providers_committed >= limits.max_providers {
                    return Err(AttemptRefusal::ProvidersExhausted {
                        max_providers: limits.max_providers,
                    });
                }
                self.providers
                    .admit_failover(self.next_provider_index, &request.provider_id)
                    .map_err(AttemptRefusal::ProviderRefused)?;
            }
        }

        if request.context_tokens > limits.max_context_tokens {
            return Err(AttemptRefusal::ContextTokensExceeded {
                requested: request.context_tokens,
                max: limits.max_context_tokens,
            });
        }
        if request.requested_output_tokens > limits.max_output_tokens {
            return Err(AttemptRefusal::OutputTokensExceeded {
                requested: request.requested_output_tokens,
                max: limits.max_output_tokens,
            });
        }
        let requested_tokens = request
            .context_tokens
            .saturating_add(request.requested_output_tokens);
        let remaining_tokens = self.remaining_aggregate_tokens();
        if requested_tokens > remaining_tokens {
            return Err(AttemptRefusal::AggregateTokensExhausted {
                requested: requested_tokens,
                remaining: remaining_tokens,
            });
        }
        if request.estimated_cost_micros < 0 {
            return Err(AttemptRefusal::CostExhausted {
                requested: request.estimated_cost_micros,
                remaining: self.remaining_cost_micros(),
            });
        }
        let remaining_cost = self.remaining_cost_micros();
        if request.estimated_cost_micros > remaining_cost {
            return Err(AttemptRefusal::CostExhausted {
                requested: request.estimated_cost_micros,
                remaining: remaining_cost,
            });
        }

        // Every check passed: commit nonrefundably, in the same call,
        // before returning `Ok` — matching `CumulativeBudgetLedger::
        // reserve`'s "decide and commit atomically" discipline so there is
        // no window in which a caller could observe an admission decision
        // without it already being durable ledger state.
        let ordinal = self.next_ordinal;
        self.next_ordinal = self.next_ordinal.saturating_add(1);
        self.calls_committed = self.calls_committed.saturating_add(1);
        match request.kind {
            AttemptKind::Fresh => {}
            AttemptKind::Retry => {
                self.retries_committed = self.retries_committed.saturating_add(1);
            }
            AttemptKind::Failover => {
                self.providers_committed = self.providers_committed.saturating_add(1);
                self.next_provider_index = self.next_provider_index.saturating_add(1);
            }
        }
        self.aggregate_tokens_committed = self
            .aggregate_tokens_committed
            .saturating_add(requested_tokens);
        self.cost_committed_micros = self
            .cost_committed_micros
            .saturating_add(request.estimated_cost_micros);

        Ok(AttemptReservation {
            ordinal,
            kind: request.kind,
            provider_id: request.provider_id.clone(),
            reserved_context_tokens: request.context_tokens,
            reserved_output_tokens: request.requested_output_tokens,
            reserved_cost_micros: request.estimated_cost_micros,
        })
    }

    fn require_retry_permitted(&self, request: &AttemptRequest) -> Result<(), AttemptRefusal> {
        match request.prior_classification {
            Some(class) if retry_is_permitted(class) => Ok(()),
            other => Err(AttemptRefusal::PriorOutcomeNotRetryable {
                kind: request.kind,
                class: other,
            }),
        }
    }

    /// Records evidence about an already-admitted attempt's settlement.
    /// Observational only: nothing here ever adjusts `calls_committed`,
    /// `retries_committed`, `providers_committed`,
    /// `aggregate_tokens_committed`, or `cost_committed_micros` — a
    /// reservation, once admitted, stays spent regardless of what the
    /// recorded outcome later claims, including a claim of zero actual
    /// usage. This is the direct analogue of `CumulativeBudgetLedger::
    /// record` never crediting a reservation back.
    pub fn record_outcome(&mut self, usage: AttemptUsage) {
        self.usage.push(usage);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_invocation::fixture::StepClock;
    use crate::model_budget_policy::limits::{intersect, ModelBudgetLimits};

    fn unrestricted_limits() -> EffectiveModelBudget {
        intersect(
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .expect("unbounded sources always intersect cleanly")
    }

    fn limits_with(mutate: impl FnOnce(&mut ModelBudgetLimits)) -> EffectiveModelBudget {
        let mut limits = ModelBudgetLimits::unbounded();
        mutate(&mut limits);
        intersect(
            limits,
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .expect("test-constructed limits are always individually valid")
    }

    fn no_failover_policy() -> ProviderPolicy {
        ProviderPolicy::new(vec![])
    }

    fn fresh(context_tokens: u64, output_tokens: u64, cost_micros: i64) -> AttemptRequest {
        AttemptRequest {
            kind: AttemptKind::Fresh,
            provider_id: "primary".to_owned(),
            context_tokens,
            requested_output_tokens: output_tokens,
            estimated_cost_micros: cost_micros,
            prior_classification: None,
        }
    }

    fn retry_of(prior: AttemptOutcomeClass) -> AttemptRequest {
        AttemptRequest {
            kind: AttemptKind::Retry,
            provider_id: "primary".to_owned(),
            context_tokens: 1,
            requested_output_tokens: 1,
            estimated_cost_micros: 1,
            prior_classification: Some(prior),
        }
    }

    fn failover_to(provider_id: &str, prior: AttemptOutcomeClass) -> AttemptRequest {
        AttemptRequest {
            kind: AttemptKind::Failover,
            provider_id: provider_id.to_owned(),
            context_tokens: 1,
            requested_output_tokens: 1,
            estimated_cost_micros: 1,
            prior_classification: Some(prior),
        }
    }

    // --- Cancellation: checked before dispatch, costs nothing. ---

    #[test]
    fn a_pre_cancelled_attempt_is_refused_and_commits_nothing() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        cancellation.cancel();
        let mut ledger =
            ModelPolicyLedger::new(unrestricted_limits(), no_failover_policy(), None, &clock);
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(10, 10, 10))
            .unwrap_err();
        assert_eq!(refusal, AttemptRefusal::Cancelled);
        assert_eq!(ledger.calls_committed(), 0);
        assert_eq!(ledger.cost_committed_micros(), 0);
    }

    // --- Deadline boundary: one instant before is admitted, the instant
    // itself is refused. ---

    #[test]
    fn one_millisecond_before_the_deadline_is_admitted() {
        let clock = StepClock::new(99);
        let cancellation = AgentCancellation::new();
        let mut ledger = ModelPolicyLedger::new(
            unrestricted_limits(),
            no_failover_policy(),
            Some(100),
            &clock,
        );
        assert!(ledger
            .reserve_attempt(&cancellation, &fresh(1, 1, 1))
            .is_ok());
    }

    #[test]
    fn exactly_the_deadline_instant_is_refused() {
        let clock = StepClock::new(100);
        let cancellation = AgentCancellation::new();
        let mut ledger = ModelPolicyLedger::new(
            unrestricted_limits(),
            no_failover_policy(),
            Some(100),
            &clock,
        );
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(1, 1, 1))
            .unwrap_err();
        assert_eq!(refusal, AttemptRefusal::DeadlineExceeded);
        assert_eq!(ledger.calls_committed(), 0);
    }

    // --- max_calls: zero/exact/over boundary. ---

    #[test]
    fn a_zero_max_calls_policy_cannot_be_constructed() {
        // Enforced at `intersect` time (see limits::tests), not here — this
        // test documents that this ledger never receives such a policy in
        // the first place.
        let rejection = crate::model_budget_policy::limits::intersect(
            ModelBudgetLimits {
                max_calls: 0,
                ..ModelBudgetLimits::unbounded()
            },
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .unwrap_err();
        assert_eq!(
            rejection,
            crate::model_budget_policy::limits::PolicyRejection::ZeroImpossible {
                dimension: "max_calls"
            }
        );
    }

    #[test]
    fn the_call_exactly_at_max_calls_is_admitted_and_the_next_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| l.max_calls = 1);
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        assert!(ledger
            .reserve_attempt(&cancellation, &fresh(1, 1, 1))
            .is_ok());
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(1, 1, 1))
            .unwrap_err();
        assert_eq!(refusal, AttemptRefusal::CallsExhausted { max_calls: 1 });
        assert_eq!(
            ledger.calls_committed(),
            1,
            "the refused attempt commits nothing"
        );
    }

    // --- max_retries: zero/exact/over, and retry-permission gating. ---

    #[test]
    fn an_uncertain_prior_outcome_never_permits_a_retry() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let mut ledger =
            ModelPolicyLedger::new(unrestricted_limits(), no_failover_policy(), None, &clock);
        let refusal = ledger
            .reserve_attempt(&cancellation, &retry_of(AttemptOutcomeClass::Uncertain))
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::PriorOutcomeNotRetryable {
                kind: AttemptKind::Retry,
                class: Some(AttemptOutcomeClass::Uncertain),
            }
        );
        assert_eq!(ledger.retries_committed(), 0);
        assert_eq!(ledger.calls_committed(), 0);
    }

    #[test]
    fn a_completed_response_never_permits_a_retry_either() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let mut ledger =
            ModelPolicyLedger::new(unrestricted_limits(), no_failover_policy(), None, &clock);
        let refusal = ledger
            .reserve_attempt(
                &cancellation,
                &retry_of(AttemptOutcomeClass::CompletedWithResponse),
            )
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::PriorOutcomeNotRetryable {
                kind: AttemptKind::Retry,
                class: Some(AttemptOutcomeClass::CompletedWithResponse),
            }
        );
    }

    #[test]
    fn a_fresh_request_carrying_a_prior_classification_is_refused_as_malformed() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let mut ledger =
            ModelPolicyLedger::new(unrestricted_limits(), no_failover_policy(), None, &clock);
        let malformed_fresh = AttemptRequest {
            prior_classification: Some(AttemptOutcomeClass::ProviderReportedRetryable),
            ..fresh(1, 1, 1)
        };
        let refusal = ledger
            .reserve_attempt(&cancellation, &malformed_fresh)
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::PriorOutcomeNotRetryable {
                kind: AttemptKind::Fresh,
                class: Some(AttemptOutcomeClass::ProviderReportedRetryable),
            }
        );
    }

    #[test]
    fn the_retry_exactly_at_max_retries_is_admitted_and_the_next_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| l.max_retries = 1);
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        ledger
            .reserve_attempt(&cancellation, &fresh(1, 1, 1))
            .expect("the initial fresh attempt is unrestricted on retries");
        ledger
            .reserve_attempt(
                &cancellation,
                &retry_of(AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .expect("the first retry reaches exactly max_retries");
        let refusal = ledger
            .reserve_attempt(
                &cancellation,
                &retry_of(AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .unwrap_err();
        assert_eq!(refusal, AttemptRefusal::RetriesExhausted { max_retries: 1 });
        assert_eq!(ledger.retries_committed(), 1);
    }

    // --- max_providers / failover: zero/exact/over, exact-order and
    // confidentiality-authorization enforcement. ---

    #[test]
    fn a_failover_to_the_next_authorized_provider_is_admitted_and_charged_as_its_own_attempt() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let policy = ProviderPolicy::new(vec![
            super::super::provider_policy::ProviderSlot::authorized("primary"),
            super::super::provider_policy::ProviderSlot::authorized("fallback"),
        ]);
        let mut ledger = ModelPolicyLedger::new(unrestricted_limits(), policy, None, &clock);
        let first = ledger
            .reserve_attempt(&cancellation, &fresh(5, 5, 5))
            .expect("fresh attempt admitted");
        let second = ledger
            .reserve_attempt(
                &cancellation,
                &failover_to("fallback", AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .expect("failover to the exact next authorized provider is admitted");
        assert_ne!(
            first.ordinal, second.ordinal,
            "a failover is a distinct, separately ordinaled attempt"
        );
        assert_eq!(second.kind, AttemptKind::Failover);
        assert_eq!(ledger.providers_committed(), 1);
        assert_eq!(
            ledger.calls_committed(),
            2,
            "failover still counts as a call"
        );
    }

    #[test]
    fn failover_is_refused_when_the_prior_outcome_was_uncertain() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let policy = ProviderPolicy::new(vec![
            super::super::provider_policy::ProviderSlot::authorized("primary"),
            super::super::provider_policy::ProviderSlot::authorized("fallback"),
        ]);
        let mut ledger = ModelPolicyLedger::new(unrestricted_limits(), policy, None, &clock);
        let refusal = ledger
            .reserve_attempt(
                &cancellation,
                &failover_to("fallback", AttemptOutcomeClass::Uncertain),
            )
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::PriorOutcomeNotRetryable {
                kind: AttemptKind::Failover,
                class: Some(AttemptOutcomeClass::Uncertain),
            },
            "failover is not a free escape hatch for uncertain delivery either"
        );
        assert_eq!(ledger.providers_committed(), 0);
    }

    #[test]
    fn failover_out_of_the_deployments_exact_order_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let policy = ProviderPolicy::new(vec![
            super::super::provider_policy::ProviderSlot::authorized("primary"),
            super::super::provider_policy::ProviderSlot::authorized("fallback-a"),
            super::super::provider_policy::ProviderSlot::authorized("fallback-b"),
        ]);
        let mut ledger = ModelPolicyLedger::new(unrestricted_limits(), policy, None, &clock);
        let refusal = ledger
            .reserve_attempt(
                &cancellation,
                &failover_to("fallback-b", AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::ProviderRefused(ProviderRefusal::OutOfOrder {
                next_index: 1,
                expected: "fallback-a".to_owned(),
                requested: "fallback-b".to_owned(),
            })
        );
    }

    #[test]
    fn failover_to_an_unauthorized_provider_is_refused_even_if_it_is_next_in_order() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let policy = ProviderPolicy::new(vec![
            super::super::provider_policy::ProviderSlot::authorized("primary"),
            super::super::provider_policy::ProviderSlot::unauthorized("fallback"),
        ]);
        let mut ledger = ModelPolicyLedger::new(unrestricted_limits(), policy, None, &clock);
        let refusal = ledger
            .reserve_attempt(
                &cancellation,
                &failover_to("fallback", AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::ProviderRefused(ProviderRefusal::NotAuthorized {
                provider: "fallback".to_owned(),
            })
        );
    }

    #[test]
    fn the_failover_exactly_at_max_providers_is_admitted_and_the_next_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let policy = ProviderPolicy::new(vec![
            super::super::provider_policy::ProviderSlot::authorized("primary"),
            super::super::provider_policy::ProviderSlot::authorized("fallback-a"),
            super::super::provider_policy::ProviderSlot::authorized("fallback-b"),
        ]);
        let limits = limits_with(|l| l.max_providers = 1);
        let mut ledger = ModelPolicyLedger::new(limits, policy, None, &clock);
        ledger
            .reserve_attempt(
                &cancellation,
                &failover_to("fallback-a", AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .expect("first failover reaches exactly max_providers");
        let refusal = ledger
            .reserve_attempt(
                &cancellation,
                &failover_to("fallback-b", AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::ProvidersExhausted { max_providers: 1 }
        );
        assert_eq!(ledger.providers_committed(), 1);
    }

    // --- Per-attempt context/output token boundaries. ---

    #[test]
    fn context_tokens_exactly_at_the_ceiling_are_admitted_one_over_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| l.max_context_tokens = 100);
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        assert!(ledger
            .reserve_attempt(&cancellation, &fresh(100, 0, 0))
            .is_ok());
        let mut ledger_over = ModelPolicyLedger::new(
            limits_with(|l| l.max_context_tokens = 100),
            no_failover_policy(),
            None,
            &clock,
        );
        let refusal = ledger_over
            .reserve_attempt(&cancellation, &fresh(101, 0, 0))
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::ContextTokensExceeded {
                requested: 101,
                max: 100
            }
        );
    }

    #[test]
    fn output_tokens_exactly_at_the_ceiling_are_admitted_one_over_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| l.max_output_tokens = 50);
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        assert!(ledger
            .reserve_attempt(&cancellation, &fresh(0, 50, 0))
            .is_ok());
        let mut ledger_over = ModelPolicyLedger::new(
            limits_with(|l| l.max_output_tokens = 50),
            no_failover_policy(),
            None,
            &clock,
        );
        let refusal = ledger_over
            .reserve_attempt(&cancellation, &fresh(0, 51, 0))
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::OutputTokensExceeded {
                requested: 51,
                max: 50
            }
        );
    }

    // --- Cumulative aggregate-token boundary across multiple attempts. ---

    #[test]
    fn aggregate_tokens_exactly_exhaust_at_the_ceiling_across_two_attempts_the_third_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| {
            l.max_aggregate_tokens = 150;
            l.max_context_tokens = 100;
            l.max_output_tokens = 100;
        });
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        ledger
            .reserve_attempt(&cancellation, &fresh(50, 50, 0))
            .expect("first attempt uses 100 of 150");
        assert_eq!(ledger.remaining_aggregate_tokens(), 50);
        ledger
            .reserve_attempt(&cancellation, &fresh(30, 20, 0))
            .expect("second attempt uses exactly the remaining 50");
        assert_eq!(ledger.remaining_aggregate_tokens(), 0);
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(1, 0, 0))
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::AggregateTokensExhausted {
                requested: 1,
                remaining: 0
            }
        );
    }

    // --- Cumulative cost boundary. ---

    #[test]
    fn cost_exactly_exhausts_at_the_ceiling_the_next_unit_is_refused() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| l.max_cost_micros = 1000);
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        ledger
            .reserve_attempt(&cancellation, &fresh(0, 0, 1000))
            .expect("exact fit is admitted");
        assert_eq!(ledger.remaining_cost_micros(), 0);
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(0, 0, 1))
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::CostExhausted {
                requested: 1,
                remaining: 0
            }
        );
    }

    #[test]
    fn a_negative_estimated_cost_is_refused_rather_than_treated_as_a_credit() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let mut ledger =
            ModelPolicyLedger::new(unrestricted_limits(), no_failover_policy(), None, &clock);
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(0, 0, -1))
            .unwrap_err();
        assert!(matches!(
            refusal,
            AttemptRefusal::CostExhausted { requested: -1, .. }
        ));
        assert_eq!(ledger.cost_committed_micros(), 0);
    }

    // --- Nonrefundability: a failed attempt followed by a permitted retry
    // must show cumulative spend decreasing (remaining capacity) twice, and
    // never restored in between. ---

    #[test]
    fn a_failed_attempt_then_its_retry_both_spend_and_neither_is_ever_refunded() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| {
            l.max_cost_micros = 100;
            l.max_aggregate_tokens = 100;
        });
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);

        let first = ledger
            .reserve_attempt(&cancellation, &fresh(10, 10, 30))
            .expect("first attempt admitted");
        assert_eq!(ledger.cost_committed_micros(), 30);
        assert_eq!(ledger.aggregate_tokens_committed(), 20);

        // The first attempt is reported as a provider-side retryable
        // failure -- evidence only, must not refund anything.
        ledger.record_outcome(AttemptUsage {
            ordinal: first.ordinal,
            classification: AttemptOutcomeClass::ProviderReportedRetryable,
            actual_context_tokens: 0,
            actual_output_tokens: 0,
            actual_cost_micros: 0,
        });
        assert_eq!(
            ledger.cost_committed_micros(),
            30,
            "recording a zero-usage outcome must not refund the reservation"
        );

        let second = ledger
            .reserve_attempt(
                &cancellation,
                &retry_of(AttemptOutcomeClass::ProviderReportedRetryable),
            )
            .expect("the classified-safe retry is admitted");
        assert_ne!(first.ordinal, second.ordinal);
        assert_eq!(
            ledger.cost_committed_micros(),
            31,
            "the retry adds its own cost on top of the first attempt's, decreasing remaining \
             capacity a second time rather than restoring it"
        );
        assert_eq!(ledger.aggregate_tokens_committed(), 22);
        assert_eq!(ledger.calls_committed(), 2);
        assert_eq!(ledger.retries_committed(), 1);
    }

    // --- Model data carries no authority: a self-reported "free" outcome
    // never reopens spent capacity. ---

    #[test]
    fn a_self_reported_zero_cost_outcome_never_reopens_spent_capacity() {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let limits = limits_with(|l| l.max_cost_micros = 10);
        let mut ledger = ModelPolicyLedger::new(limits, no_failover_policy(), None, &clock);
        let reservation = ledger
            .reserve_attempt(&cancellation, &fresh(0, 0, 10))
            .expect("exact fit exhausts the ceiling");
        // The (fabricated, model-adjacent) outcome claims the call was
        // free. Recording it must not widen remaining capacity.
        ledger.record_outcome(AttemptUsage {
            ordinal: reservation.ordinal,
            classification: AttemptOutcomeClass::CompletedWithResponse,
            actual_context_tokens: 0,
            actual_output_tokens: 0,
            actual_cost_micros: 0,
        });
        assert_eq!(ledger.remaining_cost_micros(), 0);
        let refusal = ledger
            .reserve_attempt(&cancellation, &fresh(0, 0, 1))
            .unwrap_err();
        assert_eq!(
            refusal,
            AttemptRefusal::CostExhausted {
                requested: 1,
                remaining: 0
            },
            "a self-reported zero-cost outcome must never reopen a fully committed ceiling"
        );
    }

    // --- Resume/crash-safety: reconstructing from already-committed
    // reservations reproduces the same committed totals, never fewer. ---

    #[test]
    fn resuming_from_a_committed_prefix_reproduces_the_same_totals_a_fresh_run_would_have_reached()
    {
        let clock = StepClock::new(0);
        let cancellation = AgentCancellation::new();
        let mut original =
            ModelPolicyLedger::new(unrestricted_limits(), no_failover_policy(), None, &clock);
        let first = original
            .reserve_attempt(&cancellation, &fresh(5, 5, 5))
            .unwrap();
        let second = original
            .reserve_attempt(
                &cancellation,
                &retry_of(AttemptOutcomeClass::RejectedBeforeProcessing),
            )
            .unwrap();

        let resumed = ModelPolicyLedger::resume(
            unrestricted_limits(),
            no_failover_policy(),
            None,
            &clock,
            &[first, second],
        );
        assert_eq!(resumed.calls_committed(), original.calls_committed());
        assert_eq!(resumed.retries_committed(), original.retries_committed());
        assert_eq!(
            resumed.cost_committed_micros(),
            original.cost_committed_micros()
        );
        assert_eq!(
            resumed.aggregate_tokens_committed(),
            original.aggregate_tokens_committed()
        );
    }
}
