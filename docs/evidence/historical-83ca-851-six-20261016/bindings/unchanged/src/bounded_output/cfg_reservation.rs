//! One actual caller ledger and its dedicated allocation-refusal text.
//! Acquisition creates no allowance; this affine owner never proves admission.
use super::*;
use crate::diagnostic::Diagnostic;
use std::alloc::Layout;
use std::mem::size_of;

const MESSAGE: &str = "canonical loan CFG storage reservation refused";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refused {
    NoOwner,
    Budget(ReservationRefusal),
    Allocation,
    Capacity,
    Detached,
    Arithmetic,
}

pub(crate) struct CfgReservation {
    budget: Rc<Budget>,
    text: String,
    first: Cell<Option<Refused>>,
}

impl CfgReservation {
    pub(crate) fn acquire() -> Result<Self, Refused> {
        let budget = active().ok_or(Refused::NoOwner)?;
        let bytes = size_of::<Self>()
            .checked_add(MESSAGE.len())
            .ok_or_else(|| {
                poison(&budget, usize::MAX);
                Refused::Arithmetic
            })?;
        required(&budget, bytes)?;
        let mut text = String::new();
        if text.try_reserve_exact(MESSAGE.len()).is_err() {
            poison(&budget, 0);
            return Err(Refused::Allocation);
        }
        if text.capacity() != MESSAGE.len() {
            required(&budget, text.capacity().saturating_sub(MESSAGE.len()))?;
            poison(&budget, 0);
            return Err(Refused::Capacity);
        }
        Ok(Self {
            budget,
            text,
            first: Cell::new(None),
        })
    }

    pub(crate) fn reserve(&self, bytes: usize) -> Result<(), Refused> {
        // Identity is checked even for zero bytes and spare-capacity requests.
        if !active().is_some_and(|current| Rc::ptr_eq(&current, &self.budget)) {
            return self.fail(Refused::Detached, bytes);
        }
        if let Some(first) = self.first.get() {
            return Err(first);
        }
        required(&self.budget, bytes).inspect_err(|&reason| {
            self.first.set(Some(reason));
        })
    }

    pub(crate) fn array<T>(&self, count: usize) -> Result<(), Refused> {
        self.reserve(0)?;
        match Layout::array::<T>(count) {
            Ok(layout) => self.reserve(layout.size()),
            Err(_) => self.fail(Refused::Arithmetic, usize::MAX),
        }
    }

    pub(crate) fn capacity(&self, requested: usize, excess: usize) -> Result<(), Refused> {
        self.reserve(excess)?;
        self.fail(Refused::Capacity, requested)
    }

    pub(crate) fn allocation(&self, requested: usize) -> Refused {
        self.fail(Refused::Allocation, requested).unwrap_err()
    }

    pub(crate) fn arithmetic(&self) -> Refused {
        self.fail(Refused::Arithmetic, usize::MAX).unwrap_err()
    }

    pub(crate) fn first(&self) -> Option<Refused> {
        self.first.get()
    }

    fn fail(&self, reason: Refused, requested: usize) -> Result<(), Refused> {
        let first = self.first.get().unwrap_or(reason);
        self.first.set(Some(first));
        poison(&self.budget, requested);
        Err(first)
    }

    /// Consumes this exact owner; no new String or formatting after refusal.
    pub(crate) fn diagnostic(mut self) -> Diagnostic {
        debug_assert!(self.first.get().is_some());
        self.text.push_str(MESSAGE);
        Diagnostic::io("SPX-H006", self.text)
    }

    #[cfg(test)]
    pub(crate) fn text_pointer(&self) -> *const u8 {
        self.text.as_ptr()
    }
}

fn poison(budget: &Budget, requested: usize) {
    record_refusal(budget, requested);
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
