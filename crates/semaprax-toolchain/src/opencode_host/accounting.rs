//! Bounded source-proposal accounting over the shared live-invocation policy.
//!
//! This is a host wrapper, not a second ledger. It binds an operator-selected
//! reservation amount into each request before delegating every commit and
//! observation to InvocationBudgetHook. It does not make a source run durable
//! or resumable; a caller must not treat these in-memory receipts as recovery
//! evidence.

use semaprax::live_invocation::{
    BudgetRefusal, InvocationBudgetHook, InvocationUsage, ModelInvocationOutcome,
    ModelInvocationRequest, ReservedBudget,
};

use super::OpenCodeUsage;

/// The source lifecycle permits at most 4 proposal attempts across each of
/// its 4,096 bounded iterations. Keeping the same maximum here prevents an
/// adapter configuration from accumulating unbounded host-only receipts.
const MAX_SOURCE_ACCOUNTING_ATTEMPTS: usize = 16_384;

/// Closed local accounting refusals. Arbitrary hook text is intentionally not
/// surfaced through a source diagnostic or receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCodeAccountingRefusal {
    BudgetExhausted,
    DeadlineExceeded,
    InvalidBudget,
    PolicyRefused,
    AttemptCapacity,
    PendingAttempt,
    UnreservedAttempt,
}

/// Configuration rejection before a policy hook or transport is touched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCodeAccountingConfigError {
    NonPositiveReservation,
    ZeroAttemptCapacity,
    AttemptCapacityTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingAttempt {
    turn: u32,
    attempt: usize,
    request_bytes: usize,
    reserved_units: i64,
}

/// A redacted host-only observation of one attempt.
///
/// Provider text, prompt/response bytes, session/message IDs, headers, error
/// bodies and reported cost are deliberately absent. Reported token counters
/// are observational only and never affect the reservation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCodeSourceAttemptReceipt {
    pub turn: u32,
    pub attempt: usize,
    pub reserved_units: i64,
    pub usage: InvocationUsage,
    pub reported_usage: Option<OpenCodeUsage>,
    /// A policy refusal found only after a settled transport response. This
    /// closed tag explains why those bytes were charged but never decoded.
    pub accounting_refusal: Option<OpenCodeAccountingRefusal>,
}

/// Reuses one caller-owned live-invocation policy for bounded source proposals.
///
/// reservation_units is an explicit deployment policy in the opaque units
/// CumulativeBudgetLedger already accepts. It is not calculated from OpenCode
/// token counters or reported cost.
pub struct OpenCodeSourceAccounting<'a> {
    budget: &'a mut dyn InvocationBudgetHook,
    reservation_units: i64,
    max_attempts: usize,
    pending: Option<PendingAttempt>,
    receipts: Vec<OpenCodeSourceAttemptReceipt>,
}

impl<'a> OpenCodeSourceAccounting<'a> {
    /// Creates a bounded wrapper around the caller's one accounting policy.
    pub fn new(
        budget: &'a mut dyn InvocationBudgetHook,
        reservation_units: i64,
        max_attempts: usize,
    ) -> Result<Self, OpenCodeAccountingConfigError> {
        if reservation_units <= 0 {
            return Err(OpenCodeAccountingConfigError::NonPositiveReservation);
        }
        if max_attempts == 0 {
            return Err(OpenCodeAccountingConfigError::ZeroAttemptCapacity);
        }
        if max_attempts > MAX_SOURCE_ACCOUNTING_ATTEMPTS {
            return Err(OpenCodeAccountingConfigError::AttemptCapacityTooLarge);
        }
        Ok(Self {
            budget,
            reservation_units,
            max_attempts,
            pending: None,
            receipts: Vec::with_capacity(max_attempts),
        })
    }

    /// Binds and commits the fixed reservation before a transport dispatch.
    ///
    /// The maximum receipt count is checked first, so a full local audit
    /// capacity cannot commit a further attempt that it cannot represent.
    pub fn reserve(
        &mut self,
        mut request: ModelInvocationRequest,
        attempt: usize,
        request_bytes: usize,
    ) -> Result<ReservedBudget, OpenCodeAccountingRefusal> {
        if self.pending.is_some() {
            return Err(OpenCodeAccountingRefusal::PendingAttempt);
        }
        if self.receipts.len() >= self.max_attempts {
            return Err(OpenCodeAccountingRefusal::AttemptCapacity);
        }
        request.effective_budget = self.reservation_units;
        let reserved = self.budget.reserve(&request).map_err(classify_refusal)?;
        self.pending = Some(PendingAttempt {
            turn: request.turn,
            attempt,
            request_bytes,
            reserved_units: reserved.amount,
        });
        Ok(reserved)
    }

    /// Settles and records exactly the one reserved attempt.
    ///
    /// A settled response is checked against the shared deadline before its
    /// bytes can reach proposal decoding. A deadline refusal still consumes
    /// the pending reservation and records the actual bounded byte count as a
    /// failed attempt. Transport failures retain their own closed failure
    /// outcome and are never overwritten by a later deadline observation.
    pub fn finish(
        &mut self,
        outcome: &ModelInvocationOutcome,
        reported_usage: Option<OpenCodeUsage>,
    ) -> Result<(), OpenCodeAccountingRefusal> {
        let pending = self
            .pending
            .take()
            .ok_or(OpenCodeAccountingRefusal::UnreservedAttempt)?;
        let late_refusal = match outcome {
            ModelInvocationOutcome::Settled(_) => {
                self.budget.check_deadline().err().map(classify_refusal)
            }
            ModelInvocationOutcome::Failed { .. } => None,
        };
        let (response_bytes, transport_failed) = match outcome {
            ModelInvocationOutcome::Settled(bytes) => (bytes.len(), false),
            ModelInvocationOutcome::Failed {
                attempted_bytes, ..
            } => (*attempted_bytes, true),
        };
        let usage = InvocationUsage {
            turn: pending.turn,
            request_bytes: pending.request_bytes,
            response_bytes,
            failed: transport_failed || late_refusal.is_some(),
        };
        self.budget.record(&usage);
        self.receipts.push(OpenCodeSourceAttemptReceipt {
            turn: pending.turn,
            attempt: pending.attempt,
            reserved_units: pending.reserved_units,
            usage,
            reported_usage: (!transport_failed).then_some(reported_usage).flatten(),
            accounting_refusal: late_refusal,
        });
        late_refusal.map_or(Ok(()), Err)
    }

    #[must_use]
    pub fn receipts(&self) -> &[OpenCodeSourceAttemptReceipt] {
        &self.receipts
    }
}

fn classify_refusal(refusal: BudgetRefusal) -> OpenCodeAccountingRefusal {
    match refusal.0.as_str() {
        "budget_exhausted" => OpenCodeAccountingRefusal::BudgetExhausted,
        "deadline_exceeded" => OpenCodeAccountingRefusal::DeadlineExceeded,
        "negative_request" => OpenCodeAccountingRefusal::InvalidBudget,
        _ => OpenCodeAccountingRefusal::PolicyRefused,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semaprax::live_invocation::{CumulativeBudgetLedger, InvocationClock, ModelFailure};
    use std::cell::Cell;

    struct Clock(Cell<i64>);

    impl InvocationClock for Clock {
        fn now_millis(&self) -> i64 {
            self.0.get()
        }
    }

    fn request() -> ModelInvocationRequest {
        ModelInvocationRequest {
            turn: 7,
            task: b"task".to_vec(),
            observation: b"source-context".to_vec(),
            proposal_grammar_digest: "sha256:grammar".into(),
            deployment_binding: "sha256:deployment".into(),
            max_response_bytes: 32,
            effective_budget: -99,
        }
    }

    #[test]
    fn zero_exact_and_plus_one_ceilings_bind_the_fixed_reservation_before_dispatch() {
        let mut zero_clock = Clock(Cell::new(0));
        let mut zero_ledger = CumulativeBudgetLedger::new(0, &mut zero_clock);
        let mut zero = OpenCodeSourceAccounting::new(&mut zero_ledger, 1, 1).unwrap();
        assert_eq!(
            zero.reserve(request(), 0, 10),
            Err(OpenCodeAccountingRefusal::BudgetExhausted)
        );
        assert!(zero.receipts().is_empty());
        drop(zero);
        assert_eq!(zero_ledger.committed(), 0);

        let mut exact_clock = Clock(Cell::new(0));
        let mut exact_ledger = CumulativeBudgetLedger::new(3, &mut exact_clock);
        let mut exact = OpenCodeSourceAccounting::new(&mut exact_ledger, 3, 1).unwrap();
        assert_eq!(exact.reserve(request(), 0, 10).unwrap().amount, 3);
        exact
            .finish(&ModelInvocationOutcome::Settled(b"ok".to_vec()), None)
            .unwrap();
        assert_eq!(exact.receipts()[0].reserved_units, 3);
        drop(exact);
        assert_eq!(exact_ledger.committed(), 3);
        assert_eq!(exact_ledger.usage()[0].request_bytes, 10);

        let mut plus_clock = Clock(Cell::new(0));
        let mut plus_ledger = CumulativeBudgetLedger::new(2, &mut plus_clock);
        let mut plus = OpenCodeSourceAccounting::new(&mut plus_ledger, 3, 1).unwrap();
        assert_eq!(
            plus.reserve(request(), 0, 10),
            Err(OpenCodeAccountingRefusal::BudgetExhausted)
        );
        drop(plus);
        assert_eq!(plus_ledger.committed(), 0);
    }

    #[test]
    fn deadline_and_receipt_capacity_refuse_before_a_new_reservation() {
        let mut clock = Clock(Cell::new(10));
        let mut ledger = CumulativeBudgetLedger::with_deadline(10, 10, &mut clock);
        let mut accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 1).unwrap();
        assert_eq!(
            accounting.reserve(request(), 0, 10),
            Err(OpenCodeAccountingRefusal::DeadlineExceeded)
        );
        drop(accounting);
        assert_eq!(ledger.committed(), 0);

        let mut capacity_clock = Clock(Cell::new(0));
        let mut capacity_ledger = CumulativeBudgetLedger::new(10, &mut capacity_clock);
        let mut capacity = OpenCodeSourceAccounting::new(&mut capacity_ledger, 1, 1).unwrap();
        capacity.reserve(request(), 0, 10).unwrap();
        capacity
            .finish(&ModelInvocationOutcome::Settled(b"ok".to_vec()), None)
            .unwrap();
        assert_eq!(
            capacity.reserve(request(), 1, 10),
            Err(OpenCodeAccountingRefusal::AttemptCapacity)
        );
        drop(capacity);
        assert_eq!(capacity_ledger.committed(), 1);
    }

    #[test]
    fn failed_partial_output_is_nonrefundable_and_has_no_provider_usage() {
        let mut clock = Clock(Cell::new(0));
        let mut ledger = CumulativeBudgetLedger::new(5, &mut clock);
        let mut accounting = OpenCodeSourceAccounting::new(&mut ledger, 5, 1).unwrap();
        accounting.reserve(request(), 3, 19).unwrap();
        accounting
            .finish(
                &ModelInvocationOutcome::Failed {
                    failure: ModelFailure::ProviderError,
                    attempted_bytes: 7,
                },
                Some(OpenCodeUsage {
                    total: Some(7),
                    input: Some(3),
                    output: Some(4),
                    reasoning: None,
                    cache_read: None,
                    cache_write: None,
                }),
            )
            .unwrap();

        let receipt = &accounting.receipts()[0];
        assert_eq!(receipt.turn, 7);
        assert_eq!(receipt.attempt, 3);
        assert_eq!(receipt.usage.request_bytes, 19);
        assert_eq!(receipt.usage.response_bytes, 7);
        assert!(receipt.usage.failed);
        assert!(receipt.reported_usage.is_none());
        assert!(receipt.accounting_refusal.is_none());
        drop(accounting);
        assert_eq!(ledger.committed(), 5);
        assert_eq!(ledger.usage()[0].response_bytes, 7);
    }

    #[test]
    fn settled_at_deadline_is_charged_and_refused_before_decode() {
        struct SharedClock(std::rc::Rc<Cell<i64>>);
        impl InvocationClock for SharedClock {
            fn now_millis(&self) -> i64 {
                self.0.get()
            }
        }
        let time = std::rc::Rc::new(Cell::new(0));
        let mut clock = SharedClock(std::rc::Rc::clone(&time));
        let mut ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut clock);
        let mut accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 1).unwrap();
        accounting.reserve(request(), 0, 10).unwrap();
        time.set(10);
        assert_eq!(
            accounting.finish(
                &ModelInvocationOutcome::Settled(b"actual".to_vec()),
                Some(OpenCodeUsage {
                    total: Some(6),
                    input: Some(3),
                    output: Some(3),
                    reasoning: None,
                    cache_read: None,
                    cache_write: None,
                }),
            ),
            Err(OpenCodeAccountingRefusal::DeadlineExceeded)
        );
        let receipt = &accounting.receipts()[0];
        assert!(receipt.usage.failed);
        assert_eq!(receipt.usage.response_bytes, 6);
        assert!(receipt.reported_usage.is_some());
        assert_eq!(
            receipt.accounting_refusal,
            Some(OpenCodeAccountingRefusal::DeadlineExceeded)
        );
        drop(accounting);
        assert_eq!(ledger.committed(), 1);

        let mut before_clock = Clock(Cell::new(9));
        let mut before_ledger = CumulativeBudgetLedger::with_deadline(1, 10, &mut before_clock);
        let mut before = OpenCodeSourceAccounting::new(&mut before_ledger, 1, 1).unwrap();
        before.reserve(request(), 0, 10).unwrap();
        before
            .finish(&ModelInvocationOutcome::Settled(b"ok".to_vec()), None)
            .unwrap();
        assert!(!before.receipts()[0].usage.failed);
        assert!(before.receipts()[0].accounting_refusal.is_none());
    }

    #[test]
    fn unreserved_and_duplicate_attempts_are_not_recorded_twice() {
        let mut clock = Clock(Cell::new(0));
        let mut ledger = CumulativeBudgetLedger::new(2, &mut clock);
        let mut accounting = OpenCodeSourceAccounting::new(&mut ledger, 1, 2).unwrap();
        assert_eq!(
            accounting.finish(&ModelInvocationOutcome::Settled(b"x".to_vec()), None),
            Err(OpenCodeAccountingRefusal::UnreservedAttempt)
        );
        accounting.reserve(request(), 0, 1).unwrap();
        assert_eq!(
            accounting.reserve(request(), 1, 1),
            Err(OpenCodeAccountingRefusal::PendingAttempt)
        );
        accounting
            .finish(&ModelInvocationOutcome::Settled(b"x".to_vec()), None)
            .unwrap();
        assert_eq!(
            accounting.finish(&ModelInvocationOutcome::Settled(b"x".to_vec()), None),
            Err(OpenCodeAccountingRefusal::UnreservedAttempt)
        );
        drop(accounting);
        assert_eq!(ledger.committed(), 1);
        assert_eq!(ledger.usage().len(), 1);
    }

    #[test]
    fn configuration_refuses_unbounded_receipt_growth() {
        let mut clock = Clock(Cell::new(0));
        let mut ledger = CumulativeBudgetLedger::new(1, &mut clock);
        assert!(matches!(
            OpenCodeSourceAccounting::new(&mut ledger, 1, 16_385),
            Err(OpenCodeAccountingConfigError::AttemptCapacityTooLarge)
        ));
    }
}
