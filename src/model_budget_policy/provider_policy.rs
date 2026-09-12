//! The ordered, deployment-bound failover policy: which providers a
//! failover attempt may switch to, in which exact order, and which of them
//! are authorized for the task's confidentiality classification.
//!
//! This is deliberately a *closed, caller-supplied* ordering — never
//! derived from a model response, and never reordered at runtime. "Failover
//! follows the exact ordered deployment policy" (#179) means a failover
//! attempt must name the next provider in this exact sequence; naming any
//! other provider, in or out of the list, is refused before it can consume
//! any budget. "Confidentiality classification" means a provider present in
//! the order but not marked `authorized` for this task can never be
//! selected, ordered or not — this is the check that stops failover from
//! "unintentionally exposing data to a provider not authorized for the
//! task" (#179's own failure-case list).

/// One ordered failover alternative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSlot {
    pub id: String,
    /// Whether the deployment has cleared this provider for the task's
    /// confidentiality classification. `false` entries can still occupy an
    /// ordered position (so the order's shape is stable and auditable) but
    /// can never be admitted as a failover target.
    pub authorized: bool,
}

impl ProviderSlot {
    #[must_use]
    pub fn authorized(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            authorized: true,
        }
    }

    #[must_use]
    pub fn unauthorized(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            authorized: false,
        }
    }
}

/// The exact ordered deployment failover policy: alternative 0 is the
/// primary provider (never itself a failover target — failover always
/// moves forward through this list), alternative 1 is the first fallback,
/// and so on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderPolicy {
    ordered: Vec<ProviderSlot>,
}

/// A failover admission refusal, carrying the exact identifiers/positions
/// that disagreed rather than a bare boolean.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderRefusal {
    /// `next_index` is past the end of the ordered list: every declared
    /// alternative has already been used.
    AlternativesExhausted { next_index: usize },
    /// The requested provider id is not the one the deployment's exact
    /// order names for `next_index` — a caller (or a compromised
    /// component acting on model output) tried to jump the order.
    OutOfOrder {
        next_index: usize,
        expected: String,
        requested: String,
    },
    /// The next-in-order provider exists but is not cleared for this
    /// task's confidentiality classification.
    NotAuthorized { provider: String },
}

impl ProviderPolicy {
    #[must_use]
    pub fn new(ordered: Vec<ProviderSlot>) -> Self {
        Self { ordered }
    }

    #[must_use]
    pub fn primary(&self) -> Option<&ProviderSlot> {
        self.ordered.first()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.ordered.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ordered.is_empty()
    }

    /// Admits (or refuses) switching to the provider at `next_index`,
    /// requiring the caller to name exactly that provider's id. Pure and
    /// read-only: this never mutates the policy or any ledger state — the
    /// ledger (`super::ledger`) is what actually commits a failover
    /// attempt after this check passes.
    pub fn admit_failover(
        &self,
        next_index: usize,
        requested_id: &str,
    ) -> Result<(), ProviderRefusal> {
        let Some(slot) = self.ordered.get(next_index) else {
            return Err(ProviderRefusal::AlternativesExhausted { next_index });
        };
        if slot.id != requested_id {
            return Err(ProviderRefusal::OutOfOrder {
                next_index,
                expected: slot.id.clone(),
                requested: requested_id.to_owned(),
            });
        }
        if !slot.authorized {
            return Err(ProviderRefusal::NotAuthorized {
                provider: slot.id.clone(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ProviderPolicy {
        ProviderPolicy::new(vec![
            ProviderSlot::authorized("primary"),
            ProviderSlot::authorized("fallback-a"),
            ProviderSlot::unauthorized("fallback-b-unauthorized"),
        ])
    }

    #[test]
    fn the_exact_next_authorized_provider_is_admitted() {
        assert_eq!(policy().admit_failover(1, "fallback-a"), Ok(()));
    }

    #[test]
    fn a_provider_out_of_order_is_refused_with_the_expected_and_requested_ids() {
        let refusal = policy().admit_failover(1, "fallback-b-unauthorized");
        assert_eq!(
            refusal,
            Err(ProviderRefusal::OutOfOrder {
                next_index: 1,
                expected: "fallback-a".to_owned(),
                requested: "fallback-b-unauthorized".to_owned(),
            })
        );
    }

    #[test]
    fn an_unauthorized_next_in_order_provider_is_refused_and_never_silently_skipped() {
        let refusal = policy().admit_failover(2, "fallback-b-unauthorized");
        assert_eq!(
            refusal,
            Err(ProviderRefusal::NotAuthorized {
                provider: "fallback-b-unauthorized".to_owned(),
            })
        );
    }

    #[test]
    fn past_the_end_of_the_ordered_list_is_refused_as_exhausted() {
        let refusal = policy().admit_failover(3, "anything");
        assert_eq!(
            refusal,
            Err(ProviderRefusal::AlternativesExhausted { next_index: 3 })
        );
    }

    #[test]
    fn naming_a_provider_not_present_at_all_is_out_of_order_not_silently_admitted() {
        // Proves a value a compromised component derived from model output
        // cannot simply invent an id and be admitted: the check is against
        // the exact next-in-order id, not membership in the whole list.
        let refusal = policy().admit_failover(0, "not-in-the-list-at-all");
        assert_eq!(
            refusal,
            Err(ProviderRefusal::OutOfOrder {
                next_index: 0,
                expected: "primary".to_owned(),
                requested: "not-in-the-list-at-all".to_owned(),
            })
        );
    }
}
