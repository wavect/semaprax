//! Cumulative physical backing reservations for selected cleanup replay.
//!
//! This ledger is independent of semantic work and path counters. Callers must
//! derive bytes from their actual carrier layouts, nested backing and overlap,
//! and reserve before construction. It neither estimates nor allocates storage.
//! Resetting a work allowance never restores this ledger or its enclosing debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalRefusal {
    Overflow,
    LocalLimit,
    EnclosingBudget,
}

pub(crate) struct PhysicalReservation {
    remaining: usize,
    charged: usize,
    refusal: Option<PhysicalRefusal>,
    pub(in crate::cleanup_plan::replay) direct_anchor: Option<super::SelectedDirectAnchor>,
}

impl PhysicalReservation {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            remaining: limit,
            charged: 0,
            refusal: None,
            direct_anchor: None,
        }
    }

    pub(crate) fn charged(&self) -> usize {
        self.charged
    }

    /// No allocation or refund occurs here. Failed requests cannot subsequently
    /// authorize even a zero-byte construction through this ledger.
    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), PhysicalRefusal> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        let Some(charged) = self.charged.checked_add(bytes) else {
            return self.fail(PhysicalRefusal::Overflow);
        };
        let Some(remaining) = self.remaining.checked_sub(bytes) else {
            return self.fail(PhysicalRefusal::LocalLimit);
        };
        if crate::bounded_output::active_refused()
            || !crate::bounded_output::reserve_active_required(bytes)
        {
            return self.fail(PhysicalRefusal::EnclosingBudget);
        }
        self.charged = charged;
        self.remaining = remaining;
        Ok(())
    }

    /// Exact reserve(0) frontier without constructing its Result receiver.
    /// Adding/subtracting zero cannot overflow valid usize fields. The held
    /// refusal remains first; enclosing floor authentication still runs.
    pub(in crate::cleanup_plan::replay) fn authenticate_zero(&mut self) -> bool {
        if self.refusal.is_some() {
            return false;
        }
        if crate::bounded_output::active_refused()
            || !crate::bounded_output::reserve_active_required(0)
        {
            self.refusal = Some(PhysicalRefusal::EnclosingBudget);
            return false;
        }
        true
    }

    /// `T` describes the actual allocated element, including its padding. Nested
    /// String/Vec/map/identity backing is separate and must be charged by callers.
    pub(crate) fn reserve_array<T>(&mut self, count: usize) -> Result<(), PhysicalRefusal> {
        if let Some(refusal) = self.refusal {
            return Err(refusal);
        }
        match count.checked_mul(std::mem::size_of::<T>()) {
            Some(bytes) => self.reserve(bytes),
            None => self.fail(PhysicalRefusal::Overflow),
        }
    }

    pub(super) fn reject(&mut self) {
        if self.refusal.is_none() {
            self.refusal = Some(PhysicalRefusal::LocalLimit);
        }
    }

    fn fail(&mut self, refusal: PhysicalRefusal) -> Result<(), PhysicalRefusal> {
        self.refusal = Some(refusal);
        Err(refusal)
    }
}

/// Deterministic growth target. Every replacement is reserved in full while
/// the previous backing remains charged; spare capacity never refunds a debit.
pub(crate) fn replay_growth_capacity(capacity: usize, required: usize) -> Option<usize> {
    if required <= capacity {
        return Some(capacity);
    }
    Some(required.max(capacity.checked_mul(2)?))
}

/// Selected geometric vector replacement, reserved while old backing is live.
pub(crate) fn replay_reserve_vec<T>(
    reservation: &mut PhysicalReservation,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), PhysicalRefusal> {
    reservation.reserve(0)?;
    let Some(required) = values.len().checked_add(additional) else {
        return reservation.fail(PhysicalRefusal::Overflow);
    };
    if required <= values.capacity() {
        return reservation.reserve(0);
    }
    let Some(target) = replay_growth_capacity(values.capacity(), required) else {
        return reservation.fail(PhysicalRefusal::Overflow);
    };
    reservation.reserve_array::<T>(target)?;
    if values.try_reserve_exact(target - values.len()).is_err() {
        return reservation.fail(PhysicalRefusal::LocalLimit);
    }
    if values.capacity() != target {
        return reservation.fail(PhysicalRefusal::LocalLimit);
    }
    Ok(())
}

/// Pinned alloc B-tree upper: B=6, eleven key/value slots and twelve
/// child pointers per internal node. Charge the maximum internal layout for
/// every new key (and one potentially retained empty root for a held map).
/// Each field is independently rounded to the maximum alignment, so this
/// upper does not depend on Rust's unspecified field-reordering decisions.
pub(crate) fn btree_node_upper<K, V>() -> Option<usize> {
    use std::mem::{align_of, size_of};
    let alignment = align_of::<K>()
        .max(align_of::<V>())
        .max(align_of::<usize>());
    let rounded = |bytes: usize| {
        bytes
            .checked_add(alignment - 1)
            .map(|n| n / alignment * alignment)
    };
    [
        size_of::<Option<std::ptr::NonNull<()>>>(),
        size_of::<u16>(),
        size_of::<u16>(),
        size_of::<K>().checked_mul(11)?,
        size_of::<V>().checked_mul(11)?,
        size_of::<usize>().checked_mul(12)?,
    ]
    .into_iter()
    .try_fold(0usize, |total, field| total.checked_add(rounded(field)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_and_one_short_use_actual_typed_spare_capacity() {
        // Requested capacity includes every unused entry; initialized length is
        // not a backing bound. This ledger deliberately creates no Vec itself.
        let capacity = 17usize;
        let bytes = capacity * std::mem::size_of::<Option<usize>>();
        let (result, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
            let mut physical = PhysicalReservation::new(bytes);
            physical.reserve_array::<Option<usize>>(capacity).unwrap();
            assert_eq!(physical.charged, bytes);
            assert_eq!(physical.remaining, 0);
            physical.reserve(0)
        });
        assert_eq!(result, Ok(()));
        assert!(!refused);
        assert_eq!(used, bytes);
        let (result, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
            let mut physical = PhysicalReservation::new(bytes);
            let first = physical.reserve_array::<Option<usize>>(capacity);
            assert_eq!(physical.reserve(0), first);
            assert_eq!(physical.charged, 0);
            first
        });
        assert_eq!(result, Err(PhysicalRefusal::EnclosingBudget));
        assert!(refused);
        assert_eq!(used, 0);
    }

    #[test]
    fn local_limit_overflow_and_first_refusal_are_sticky() {
        let mut physical = PhysicalReservation::new(8);
        assert_eq!(physical.reserve(8), Ok(()));
        assert_eq!(physical.reserve(1), Err(PhysicalRefusal::LocalLimit));
        assert_eq!(physical.reserve(0), Err(PhysicalRefusal::LocalLimit));
        assert_eq!(physical.charged, 8);
        let mut physical = PhysicalReservation::new(usize::MAX);
        assert_eq!(
            physical.reserve_array::<[u8; 2]>(usize::MAX),
            Err(PhysicalRefusal::Overflow)
        );
        assert_eq!(physical.reserve(0), Err(PhysicalRefusal::Overflow));
        assert_eq!(physical.charged, 0);
    }

    #[test]
    fn work_reset_and_prior_enclosing_refusal_cannot_restore_physical_allowance() {
        let (_, refused, used) = crate::bounded_output::with_limit_usage(8, || {
            let mut physical = PhysicalReservation::new(8);
            physical.reserve(5).unwrap();
            // A semantic work reset is external to, and cannot reset, this ledger.
            let mut work = super::super::ReplayBudget::new();
            work.remaining = 0;
            work.remaining = super::super::MAX_REPLAY_WORK_UNITS;
            assert_eq!(work.remaining, super::super::MAX_REPLAY_WORK_UNITS);
            physical.reserve(3).unwrap();
            assert_eq!(physical.reserve(1), Err(PhysicalRefusal::LocalLimit));
            assert_eq!(physical.charged, 8);
        });
        assert!(!refused); // Local refusal and enclosing refusal are distinct.
        assert_eq!(used, 8);
        let (_, refused, used) = crate::bounded_output::with_limit_usage(0, || {
            assert!(!crate::bounded_output::reserve_active_required(1));
            let mut physical = PhysicalReservation::new(8);
            assert_eq!(physical.reserve(0), Err(PhysicalRefusal::EnclosingBudget));
            assert_eq!(physical.reserve(1), Err(PhysicalRefusal::EnclosingBudget));
            assert_eq!(physical.charged, 0);
        });
        assert!(refused);
        assert_eq!(used, 0);
    }
}

#[cfg(test)]
mod growth_tests;

#[cfg(test)]
mod representative_measurements;

#[cfg(test)]
mod zero_tests;

#[cfg(test)]
mod before_direct_anchor {
    use super::*;
    pub(crate) struct BeforeDirectAnchorReservation {
        remaining: usize,
        charged: usize,
        refusal: Option<PhysicalRefusal>,
    }

    impl BeforeDirectAnchorReservation {
        pub(crate) fn new(limit: usize) -> Self {
            Self {
                remaining: limit,
                charged: 0,
                refusal: None,
            }
        }

        pub(crate) fn charged(&self) -> usize {
            self.charged
        }

        /// No allocation or refund occurs here. Failed requests cannot subsequently
        /// authorize even a zero-byte construction through this ledger.
        pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), PhysicalRefusal> {
            if let Some(refusal) = self.refusal {
                return Err(refusal);
            }
            let Some(charged) = self.charged.checked_add(bytes) else {
                return self.fail(PhysicalRefusal::Overflow);
            };
            let Some(remaining) = self.remaining.checked_sub(bytes) else {
                return self.fail(PhysicalRefusal::LocalLimit);
            };
            if crate::bounded_output::active_refused()
                || !crate::bounded_output::reserve_active_required(bytes)
            {
                return self.fail(PhysicalRefusal::EnclosingBudget);
            }
            self.charged = charged;
            self.remaining = remaining;
            Ok(())
        }

        /// Exact reserve(0) frontier without constructing its Result receiver.
        /// Adding/subtracting zero cannot overflow valid usize fields. The held
        /// refusal remains first; enclosing floor authentication still runs.
        pub(in crate::cleanup_plan::replay) fn authenticate_zero(&mut self) -> bool {
            if self.refusal.is_some() {
                return false;
            }
            if crate::bounded_output::active_refused()
                || !crate::bounded_output::reserve_active_required(0)
            {
                self.refusal = Some(PhysicalRefusal::EnclosingBudget);
                return false;
            }
            true
        }

        /// `T` describes the actual allocated element, including its padding. Nested
        /// String/Vec/map/identity backing is separate and must be charged by callers.
        pub(crate) fn reserve_array<T>(&mut self, count: usize) -> Result<(), PhysicalRefusal> {
            if let Some(refusal) = self.refusal {
                return Err(refusal);
            }
            match count.checked_mul(std::mem::size_of::<T>()) {
                Some(bytes) => self.reserve(bytes),
                None => self.fail(PhysicalRefusal::Overflow),
            }
        }

        pub(super) fn reject(&mut self) {
            if self.refusal.is_none() {
                self.refusal = Some(PhysicalRefusal::LocalLimit);
            }
        }

        fn fail(&mut self, refusal: PhysicalRefusal) -> Result<(), PhysicalRefusal> {
            self.refusal = Some(refusal);
            Err(refusal)
        }
    }
}

#[cfg(test)]
mod direct_anchor_layout_tests {
    use super::*;
    #[test]
    fn direct_anchor_preserves_whole_old_selected_ledger_and_reports_actual_layout_cost() {
        use before_direct_anchor::BeforeDirectAnchorReservation;
        assert_eq!(
            std::mem::size_of::<BeforeDirectAnchorReservation>(),
            std::mem::size_of::<(usize, usize, Option<PhysicalRefusal>)>()
        );
        assert!(
            std::mem::size_of::<PhysicalReservation>()
                >= std::mem::size_of::<BeforeDirectAnchorReservation>()
        );
        let mut old = BeforeDirectAnchorReservation::new(3);
        let mut current = PhysicalReservation::new(3);
        for bytes in [0, 1, 2, 1, 0] {
            assert_eq!(old.reserve(bytes), current.reserve(bytes));
            assert_eq!(old.charged(), current.charged());
        }
        assert_eq!(old.reserve_array::<u8>(0), current.reserve_array::<u8>(0));
        assert_eq!(old.authenticate_zero(), current.authenticate_zero());
        old.reject();
        current.reject();
        assert_eq!(old.reserve(0), current.reserve(0));
        eprintln!(
            "selected ledger old/current bytes={}/{}; ordinary budget carrier layout unchanged",
            std::mem::size_of::<BeforeDirectAnchorReservation>(),
            std::mem::size_of::<PhysicalReservation>()
        );
    }
}
