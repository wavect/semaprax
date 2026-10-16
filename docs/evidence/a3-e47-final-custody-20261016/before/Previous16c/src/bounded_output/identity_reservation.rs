//! Caller-owned identity/report storage on one existing ActiveBuilder ledger.
//! No new budget or allowance is created. Semantic diagnostics remain legacy.
use super::*;
use crate::diagnostic::Diagnostic;
use std::mem::size_of;

const MESSAGE: &str = "declaration identity backing reservation refused";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refused {
    NoOwner,
    Budget(ReservationRefusal),
    Allocation,
    Capacity,
    Detached,
    Arithmetic,
}

/// Affine report and exact existing owner; never a source/admission proof.
pub(crate) struct IdentityReservation {
    budget: Rc<Budget>,
    report: Vec<Diagnostic>,
    text: String,
}

impl IdentityReservation {
    /// Exact setup prebound, shared with the ordinary compatibility dispatcher.
    /// A short public setup keeps its preexisting owned diagnostic path; this
    /// predicate grants neither source admission nor reservation credit.
    pub(crate) fn setup_bytes() -> Option<usize> {
        size_of::<Self>()
            .checked_add(size_of::<Diagnostic>())
            .and_then(|bytes| bytes.checked_add(MESSAGE.len()))
    }

    pub(crate) fn acquire() -> Result<Self, Refused> {
        let budget = active().ok_or(Refused::NoOwner)?;
        let bytes = Self::setup_bytes().ok_or_else(|| {
            record_refusal(&budget, usize::MAX);
            budget.overflowed.set(true);
            Refused::Arithmetic
        })?;
        required(&budget, bytes)?;
        let mut report = Vec::new();
        let mut text = String::new();
        if report.try_reserve_exact(1).is_err() || text.try_reserve_exact(MESSAGE.len()).is_err() {
            poison(&budget, 0);
            return Err(Refused::Allocation);
        }
        // Allocator discrepancy is exceptional: debit excess, but never admit
        // an output whose actual storage differs from the prepaid request.
        if report.capacity() != 1 || text.capacity() != MESSAGE.len() {
            let excess = report
                .capacity()
                .saturating_sub(1)
                .checked_mul(size_of::<Diagnostic>())
                .and_then(|bytes| bytes.checked_add(text.capacity().saturating_sub(MESSAGE.len())))
                .ok_or_else(|| {
                    poison(&budget, usize::MAX);
                    Refused::Arithmetic
                })?;
            required(&budget, excess)?;
            poison(&budget, 0);
            return Err(Refused::Capacity);
        }
        Ok(Self {
            budget,
            report,
            text,
        })
    }

    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), Refused> {
        // An owner cannot spend after its invocation or inside another budget
        // whose settlement would account the same activity a second time.
        if !active().is_some_and(|current| Rc::ptr_eq(&current, &self.budget)) {
            poison(&self.budget, bytes);
            return Err(Refused::Detached);
        }
        required(&self.budget, bytes)
    }

    /// Transfer exactly the preallocated slot/text, without formatting detail.
    pub(crate) fn publish(mut self) -> Vec<Diagnostic> {
        self.text.push_str(MESSAGE);
        self.report.push(Diagnostic::io("SPX-H006", self.text));
        self.report
    }

    #[cfg(test)]
    pub(crate) fn pointers(&self) -> (*const Diagnostic, *const u8) {
        (self.report.as_ptr(), self.text.as_ptr())
    }
}

fn poison(budget: &Budget, bytes: usize) {
    record_refusal(budget, bytes);
    budget.overflowed.set(true);
}
fn required(budget: &Budget, bytes: usize) -> Result<(), Refused> {
    if reserve(Some(budget), bytes) {
        return Ok(());
    }
    poison(budget, bytes);
    Err(Refused::Budget(
        budget
            .first_refusal
            .get()
            .expect("required refusal recorded"),
    ))
}

#[cfg(test)]
mod tests;

mod base_prelude;

mod producer_failure;
