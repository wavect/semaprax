//! The closed attempt-outcome classification retry/failover safety is
//! decided from.
//!
//! Issue #179's "explicitly safe failure classes" requirement needs one
//! closed vocabulary distinguishing *model-attempt uncertainty* (did the
//! call even reach the provider, and if so did it produce billable work)
//! from *effect-delivery uncertainty* (did the result reach the caller).
//! [`AttemptOutcomeClass`] is that vocabulary. It is deliberately smaller
//! and orthogonal to [`crate::live_invocation::model_invoke::ModelFailure`]
//! (a *transport*-shaped failure taxonomy): this classification answers one
//! question only — "is retrying or failing over after this outcome
//! provably safe?" — and every variant states which side of that question
//! it falls on in its own doc comment.
//!
//! # Model data carries no authority here
//!
//! A value of this type is only ever constructed by trusted adapter/host
//! code that already knows, out of band, whether a provider was
//! contacted — never parsed or inferred from raw model response bytes. This
//! module exposes no `From<&[u8]>` or similar decoder for it, and
//! [`retry_is_permitted`] is a pure function of the classification alone: a
//! response payload has no path to widen what it returns, matching the
//! "model data carries no authority" invariant this module's ledger
//! ([`super::ledger`]) also enforces for budget commitments.

/// The closed classification of one settled (or never-dispatched) model
/// attempt, as decided by trusted adapter code — never derived from
/// provider response bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AttemptOutcomeClass {
    /// The provider was never contacted at all (refused locally before
    /// transport, or cancelled before dispatch). No billable work could
    /// have happened. Safe to try again: nothing external occurred.
    NotDispatched,
    /// The provider rejected the request before any generation/processing
    /// began (e.g. a request-shape or capacity refusal returned
    /// synchronously, before token production). The adapter contract
    /// proves no partial billable work occurred. Safe to retry.
    RejectedBeforeProcessing,
    /// A response was received and processing completed, successfully or
    /// not. This is model-attempt *certainty*, not effect-delivery
    /// uncertainty: the call is known to have happened and (usually)
    /// billed. It is never blindly retried as if it were a fresh
    /// attempt — a caller who wants to try again issues a new attempt on
    /// its own terms, not an automatic retry of this one.
    CompletedWithResponse,
    /// Whether the provider processed, billed, or will still respond to
    /// this call is unknown (timeout with no delivery confirmation,
    /// connection reset mid-stream, cancellation observed only after
    /// dispatch). This is exactly the "effect-delivery uncertainty" case
    /// #179 requires be distinguished from certain outcomes. Never safe to
    /// retry automatically: a retry here could double-bill and double-act
    /// on a call that already reached the provider.
    Uncertain,
    /// The provider itself reported a specific, explicitly retryable
    /// condition (for example a rate limit with an advertised retry
    /// affordance) as part of its normalized failure, and the adapter
    /// contract proves this class never carries partial billable output.
    /// Safe to retry.
    ProviderReportedRetryable,
}

impl AttemptOutcomeClass {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotDispatched => "not_dispatched",
            Self::RejectedBeforeProcessing => "rejected_before_processing",
            Self::CompletedWithResponse => "completed_with_response",
            Self::Uncertain => "uncertain",
            Self::ProviderReportedRetryable => "provider_reported_retryable",
        }
    }
}

/// Whether an attempt classified `class` may be automatically retried or
/// failed over. Exactly [`AttemptOutcomeClass::NotDispatched`],
/// [`AttemptOutcomeClass::RejectedBeforeProcessing`] and
/// [`AttemptOutcomeClass::ProviderReportedRetryable`] are proven-safe;
/// [`AttemptOutcomeClass::Uncertain`] and
/// [`AttemptOutcomeClass::CompletedWithResponse`] are refused — the former
/// because delivery status is unknown, the latter because the call is
/// already known to have happened and a retry of it would not be "trying
/// the same thing again" but a new, separately accounted attempt.
#[must_use]
pub fn retry_is_permitted(class: AttemptOutcomeClass) -> bool {
    matches!(
        class,
        AttemptOutcomeClass::NotDispatched
            | AttemptOutcomeClass::RejectedBeforeProcessing
            | AttemptOutcomeClass::ProviderReportedRetryable
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_dispatched_is_retry_permitted() {
        assert!(retry_is_permitted(AttemptOutcomeClass::NotDispatched));
    }

    #[test]
    fn rejected_before_processing_is_retry_permitted() {
        assert!(retry_is_permitted(
            AttemptOutcomeClass::RejectedBeforeProcessing
        ));
    }

    #[test]
    fn provider_reported_retryable_is_retry_permitted() {
        assert!(retry_is_permitted(
            AttemptOutcomeClass::ProviderReportedRetryable
        ));
    }

    #[test]
    fn uncertain_is_never_retry_permitted() {
        assert!(!retry_is_permitted(AttemptOutcomeClass::Uncertain));
    }

    #[test]
    fn completed_with_response_is_never_retry_permitted() {
        assert!(!retry_is_permitted(
            AttemptOutcomeClass::CompletedWithResponse
        ));
    }

    #[test]
    fn as_str_is_stable_and_distinct_per_variant() {
        let all = [
            AttemptOutcomeClass::NotDispatched,
            AttemptOutcomeClass::RejectedBeforeProcessing,
            AttemptOutcomeClass::CompletedWithResponse,
            AttemptOutcomeClass::Uncertain,
            AttemptOutcomeClass::ProviderReportedRetryable,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for class in all {
            assert!(
                seen.insert(class.as_str()),
                "two variants share one wire tag"
            );
        }
    }
}
