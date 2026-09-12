//! `ModelBudget v1`: the declared per-dimension ceilings a source Agent,
//! a deployment policy, and one invocation each may independently narrow,
//! and their intersection into one [`EffectiveModelBudget`] enforced for
//! the whole invocation.
//!
//! #179 step 2: "Compute effective limits as the minimum of source Agent
//! ceilings, deployment policy, and invocation budget. Reject contradictory
//! or zero-impossible policies before handler access." [`intersect`] is
//! exactly that: an elementwise minimum across the three
//! [`ModelBudgetLimits`] values, validated *before* it is ever handed to
//! [`super::ledger::ModelPolicyLedger`] — a caller cannot construct a
//! ledger from a rejected [`PolicyRejection`], so an impossible policy never
//! reaches attempt admission at all.

/// One source's declared ceilings across every #179 budget dimension.
/// Every field is a hard maximum in its own named unit; `0` is a legal,
/// meaningful value everywhere except `max_calls` (see
/// [`ModelBudgetLimits::validate`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModelBudgetLimits {
    /// Maximum number of model-invocation attempts total, counting the
    /// first attempt plus every retry and every failover switch.
    pub max_calls: u32,
    /// Maximum number of those attempts that may be retries of a
    /// proven-safe prior outcome.
    pub max_retries: u32,
    /// Maximum number of distinct failover provider switches.
    pub max_providers: u32,
    /// Maximum context tokens any single attempt may submit.
    pub max_context_tokens: u64,
    /// Maximum output tokens any single attempt may request.
    pub max_output_tokens: u64,
    /// Maximum context-plus-output tokens summed across every attempt in
    /// the invocation.
    pub max_aggregate_tokens: u64,
    /// Maximum cumulative estimated cost, in caller-defined micro-units,
    /// across every attempt. Never a live price lookup — the same
    /// "operator-supplied pricing, conservative reservation" scope #113
    /// already declares for its own monetary ceiling.
    pub max_cost_micros: i64,
    /// Maximum wall-clock duration, in milliseconds, from the invocation's
    /// bind time to its deadline. A duration, not an absolute instant —
    /// [`super::ledger::ModelPolicyLedger`] adds this to a caller-supplied
    /// start instant exactly once, at construction.
    pub max_latency_millis: i64,
}

impl ModelBudgetLimits {
    /// A ceiling that permits nothing extra beyond exactly one plain call:
    /// no retries, no failover, and token/cost fields left at `0` (a caller
    /// intersecting this in is declaring "I contribute no ceiling on this
    /// dimension" only by using [`u32::MAX`]/[`u64::MAX`]/[`i64::MAX`]
    /// instead — this constructor is for tests and for a source that means
    /// exactly what it says).
    #[must_use]
    pub fn single_call_only(max_context_tokens: u64, max_output_tokens: u64) -> Self {
        Self {
            max_calls: 1,
            max_retries: 0,
            max_providers: 0,
            max_context_tokens,
            max_output_tokens,
            max_aggregate_tokens: max_context_tokens.saturating_add(max_output_tokens),
            max_cost_micros: i64::MAX,
            max_latency_millis: i64::MAX,
        }
    }

    /// A ceiling that contributes no restriction on any dimension — the
    /// identity element for [`intersect`]. Useful when a source (say, no
    /// deployment-level policy at all) genuinely has nothing to say.
    #[must_use]
    pub fn unbounded() -> Self {
        Self {
            max_calls: u32::MAX,
            max_retries: u32::MAX,
            max_providers: u32::MAX,
            max_context_tokens: u64::MAX,
            max_output_tokens: u64::MAX,
            max_aggregate_tokens: u64::MAX,
            max_cost_micros: i64::MAX,
            max_latency_millis: i64::MAX,
        }
    }

    fn elementwise_min(self, other: Self) -> Self {
        Self {
            max_calls: self.max_calls.min(other.max_calls),
            max_retries: self.max_retries.min(other.max_retries),
            max_providers: self.max_providers.min(other.max_providers),
            max_context_tokens: self.max_context_tokens.min(other.max_context_tokens),
            max_output_tokens: self.max_output_tokens.min(other.max_output_tokens),
            max_aggregate_tokens: self.max_aggregate_tokens.min(other.max_aggregate_tokens),
            max_cost_micros: self.max_cost_micros.min(other.max_cost_micros),
            max_latency_millis: self.max_latency_millis.min(other.max_latency_millis),
        }
    }

    /// Refuses a policy that is internally contradictory (a negative
    /// ceiling — never a meaningful "at least" bound) or zero-impossible: a
    /// `max_calls` of `0` declares a model policy that can never dispatch a
    /// single call, which is a self-contradiction for a *model* budget
    /// rather than a meaningful restriction, so it is refused up front
    /// rather than silently admitted and left to fail confusingly at the
    /// first attempt.
    fn validate(self) -> Result<Self, PolicyRejection> {
        if self.max_cost_micros < 0 {
            return Err(PolicyRejection::NegativeLimit {
                dimension: "max_cost_micros",
            });
        }
        if self.max_latency_millis < 0 {
            return Err(PolicyRejection::NegativeLimit {
                dimension: "max_latency_millis",
            });
        }
        if self.max_calls == 0 {
            return Err(PolicyRejection::ZeroImpossible {
                dimension: "max_calls",
            });
        }
        Ok(self)
    }
}

/// Why [`intersect`] refused to produce an [`EffectiveModelBudget`]. Carries
/// the exact offending dimension's name rather than a bare "invalid
/// policy".
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyRejection {
    /// A source declared a negative ceiling for `dimension`.
    NegativeLimit { dimension: &'static str },
    /// The intersected ceiling for `dimension` makes every future attempt
    /// impossible.
    ZeroImpossible { dimension: &'static str },
}

/// The one effective ceiling set — the elementwise minimum of the source
/// Agent's, the deployment policy's, and the invocation's own declared
/// limits — [`super::ledger::ModelPolicyLedger`] enforces for the whole
/// invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EffectiveModelBudget(pub(crate) ModelBudgetLimits);

impl EffectiveModelBudget {
    #[must_use]
    pub fn limits(&self) -> ModelBudgetLimits {
        self.0
    }
}

/// Computes the effective ceiling as the elementwise minimum of all three
/// sources, refusing before any handler ever sees the result. This is the
/// *only* constructor for [`EffectiveModelBudget`] — there is no bare
/// `EffectiveModelBudget::new` a caller could use to skip the
/// contradiction/zero-impossible check.
pub fn intersect(
    source_agent: ModelBudgetLimits,
    deployment_policy: ModelBudgetLimits,
    invocation_budget: ModelBudgetLimits,
) -> Result<EffectiveModelBudget, PolicyRejection> {
    let effective = source_agent
        .elementwise_min(deployment_policy)
        .elementwise_min(invocation_budget)
        .validate()?;
    Ok(EffectiveModelBudget(effective))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_takes_the_strictest_source_per_dimension() {
        let source = ModelBudgetLimits {
            max_calls: 10,
            ..ModelBudgetLimits::unbounded()
        };
        let deployment = ModelBudgetLimits {
            max_calls: 5,
            max_retries: 2,
            ..ModelBudgetLimits::unbounded()
        };
        let invocation = ModelBudgetLimits {
            max_calls: 8,
            ..ModelBudgetLimits::unbounded()
        };
        let effective = intersect(source, deployment, invocation)
            .expect("all three sources are individually valid")
            .limits();
        assert_eq!(effective.max_calls, 5, "the deployment's tighter bound wins");
        assert_eq!(
            effective.max_retries, 2,
            "a dimension only one source restricts still narrows the result"
        );
    }

    #[test]
    fn a_negative_cost_ceiling_from_any_one_source_is_refused() {
        let poisoned = ModelBudgetLimits {
            max_cost_micros: -1,
            ..ModelBudgetLimits::unbounded()
        };
        let rejection = intersect(
            poisoned,
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .unwrap_err();
        assert_eq!(
            rejection,
            PolicyRejection::NegativeLimit {
                dimension: "max_cost_micros"
            }
        );
    }

    #[test]
    fn a_negative_latency_ceiling_is_refused() {
        let poisoned = ModelBudgetLimits {
            max_latency_millis: -1,
            ..ModelBudgetLimits::unbounded()
        };
        let rejection = intersect(
            ModelBudgetLimits::unbounded(),
            poisoned,
            ModelBudgetLimits::unbounded(),
        )
        .unwrap_err();
        assert_eq!(
            rejection,
            PolicyRejection::NegativeLimit {
                dimension: "max_latency_millis"
            }
        );
    }

    #[test]
    fn an_intersection_collapsing_max_calls_to_zero_is_refused_as_zero_impossible() {
        let zeroed = ModelBudgetLimits {
            max_calls: 0,
            ..ModelBudgetLimits::unbounded()
        };
        let rejection = intersect(
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
            zeroed,
        )
        .unwrap_err();
        assert_eq!(
            rejection,
            PolicyRejection::ZeroImpossible {
                dimension: "max_calls"
            }
        );
    }

    #[test]
    fn max_retries_or_max_providers_at_zero_is_a_legal_restrictive_policy_not_a_rejection() {
        let no_retry_no_failover = ModelBudgetLimits {
            max_retries: 0,
            max_providers: 0,
            ..ModelBudgetLimits::unbounded()
        };
        let effective = intersect(
            no_retry_no_failover,
            ModelBudgetLimits::unbounded(),
            ModelBudgetLimits::unbounded(),
        )
        .expect("zero retries/providers is a meaningful restriction, not a contradiction")
        .limits();
        assert_eq!(effective.max_retries, 0);
        assert_eq!(effective.max_providers, 0);
    }
}
