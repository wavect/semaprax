// src/bounded_output/prepaid.rs
//! One-shot prepaid acquisitions against the original active ledger.
//!
//! `acquire` reserves exact bytes from the active budget at once. The only way
//! to spend them is the consuming `authenticate`, which proves the same ledger
//! is still active and returns a non-clonable `PrepaidSpend`. Nothing here
//! credits, refunds or raises a limit. A refused reservation leaves the
//! ledger's original first refusal untouched.
use std::rc::Rc;

use super::{active, reserve, Budget};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PrepaidError {
    /// No bounded_output ledger is active.
    NoLedger,
    /// The ledger refused the reservation. Its first refusal is unchanged.
    Refused,
    /// An unrelated live ledger cannot receive this acquisition.
    WrongLedger,
    /// The captured ledger already holds a selected refusal.
    LedgerRefused,
    /// A spend exceeded the prepaid bytes.
    Overdraw,
    /// Prepaid bytes were left over at `finish`.
    Unspent,
}

#[must_use]
pub(crate) struct PrepaidAcquisition {
    ledger: Rc<Budget>,
    bytes: usize,
}

impl PrepaidAcquisition {
    /// Reserves exactly `bytes` from the active ledger before the caller
    /// allocates anything.
    pub(crate) fn acquire(bytes: usize) -> Result<Self, PrepaidError> {
        let ledger = active().ok_or(PrepaidError::NoLedger)?;
        let carrier = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<PrepaidSpend>())
            .ok_or(PrepaidError::Overdraw)?;
        if !reserve(Some(&ledger), carrier) || !reserve(Some(&ledger), bytes) {
            return Err(PrepaidError::Refused);
        }
        Ok(Self { ledger, bytes })
    }

    /// Consumes the acquisition. Succeeds only on the same ledger that was
    /// charged, and only while that ledger holds no selected refusal.
    pub(crate) fn authenticate(self) -> Result<PrepaidSpend, PrepaidError> {
        if active().is_some_and(|current| !Rc::ptr_eq(&current, &self.ledger)) {
            return Err(PrepaidError::WrongLedger);
        }
        if !reserve(Some(&self.ledger), 0) {
            return Err(PrepaidError::LedgerRefused);
        }
        Ok(PrepaidSpend {
            remaining: self.bytes,
            _ledger: self.ledger,
        })
    }

    // A consuming deferred receipt keeps its original ledger even after the
    // caller's outer context is restored. No ambient payer or extra allowance
    // is supplied or substituted at this boundary.
    pub(crate) fn authenticate_deferred(self) -> Result<PrepaidSpend, PrepaidError> {
        if !reserve(Some(&self.ledger), 0) {
            return Err(PrepaidError::LedgerRefused);
        }
        Ok(PrepaidSpend {
            remaining: self.bytes,
            _ledger: self.ledger,
        })
    }
}

/// Exact prepaid bytes, usable once and only through `take`.
#[must_use]
pub(crate) struct PrepaidSpend {
    remaining: usize,
    _ledger: Rc<Budget>,
}

impl PrepaidSpend {
    /// Declares `bytes` of the prepaid amount as about to be allocated.
    pub(crate) fn take(&mut self, bytes: usize) -> Result<(), PrepaidError> {
        if !reserve(Some(&self._ledger), 0) {
            return Err(PrepaidError::LedgerRefused);
        }
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(PrepaidError::Overdraw)?;
        Ok(())
    }

    /// Requires that every prepaid byte was matched by an actual owner.
    pub(crate) fn finish(self) -> Result<(), PrepaidError> {
        if self.remaining == 0 {
            Ok(())
        } else {
            Err(PrepaidError::Unspent)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounded_output::{reserve_active_required, with_limit};
    #[test]
    fn strict_foreign_payer_and_deferred_original_sticky_refusals_are_preserved() {
        let (proof, refused) = with_limit(1024, || PrepaidAcquisition::acquire(16).unwrap());
        assert!(!refused);
        let (result, _) = with_limit(1024, || proof.authenticate());
        assert!(matches!(result, Err(PrepaidError::WrongLedger)));
        let (proof, refused) = with_limit(1024, || {
            let proof = PrepaidAcquisition::acquire(16).unwrap();
            assert!(!reserve_active_required(1024));
            proof
        });
        assert!(refused);
        assert!(matches!(
            proof.authenticate_deferred(),
            Err(PrepaidError::LedgerRefused)
        ));
    }
}
