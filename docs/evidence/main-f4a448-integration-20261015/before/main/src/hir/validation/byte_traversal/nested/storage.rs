//! Exact current-invocation storage; None ACTIVE retains the explicit ordinary
//! unobserved API policy and is never described as funded. No quota is created.
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::bounded_output::{active_remaining, refuse_active_required, reserve_active_required};
use std::alloc::Layout;
use std::cell::Cell;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::hir::validation::byte_traversal) enum Refused {
    Budget,
    Detached,
    Layout,
    Allocation,
    Capacity,
}
pub(super) struct Storage {
    ledger: ClassifierLedger,
    observed: bool,
    first: Cell<Option<Refused>>,
}
impl Storage {
    pub(super) fn acquire() -> Result<Self, Refused> {
        let observed = active_remaining().is_some();
        let bytes = size_of::<Self>() + size_of::<Result<Self, Refused>>();
        #[cfg(test)]
        if observed {
            super::tests::record_request(bytes);
        }
        if observed && !reserve_active_required(bytes) {
            #[cfg(test)]
            super::tests::record_failure();
            return Err(Refused::Budget);
        }
        Ok(Self {
            ledger: ClassifierLedger::capture(),
            observed,
            first: Cell::new(None),
        })
    }
    pub(super) fn reserve(&self, bytes: usize) -> Result<(), Refused> {
        if let Some(first) = self.first.get() {
            return Err(first);
        }
        if !self.ledger.current() {
            self.first.set(Some(Refused::Detached));
            return Err(Refused::Detached);
        }
        #[cfg(test)]
        if self.observed {
            super::tests::record_request(bytes);
        }
        if self.observed && !reserve_active_required(bytes) {
            #[cfg(test)]
            super::tests::record_failure();
            self.first.set(Some(Refused::Budget));
            return Err(Refused::Budget);
        }
        Ok(())
    }
    fn refuse(&self, requested: usize, kind: Refused) -> Refused {
        if let Err(first) = self.reserve(0) {
            return first;
        }
        self.first.set(Some(kind));
        // Same captured invocation has just been checked. This records a real
        // required allocator frontier; it is not a synthetic reserve request.
        if self.observed {
            refuse_active_required(requested);
        }
        kind
    }
    pub(super) fn vector<T>(&self, count: usize) -> Result<Vec<T>, Refused> {
        self.reserve(0)?;
        let layout =
            Layout::array::<T>(count).map_err(|_| self.refuse(usize::MAX, Refused::Layout))?;
        let controls = size_of::<Vec<T>>() + size_of::<Result<Vec<T>, Refused>>();
        self.reserve(controls)?;
        self.reserve(layout.size())?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| self.refuse(layout.size(), Refused::Allocation))?;
        if size_of::<T>() != 0 && values.capacity() != count {
            return Err(self.refuse(layout.size(), Refused::Capacity));
        }
        Ok(values)
    }
    pub(super) fn push<T: Copy>(&self, values: &mut Vec<T>, value: T) -> Result<(), Refused> {
        self.reserve(0)?;
        // All retained rows have an exact earlier census. A count mismatch is
        // refused; this producer never grows/replaces an admitted row vector.
        if values.len() == values.capacity() {
            let requested = values
                .len()
                .checked_add(1)
                .and_then(|count| Layout::array::<T>(count).ok())
                .map_or(usize::MAX, |layout| layout.size());
            return Err(self.refuse(requested, Refused::Capacity));
        }
        // Distinct actual initializer, while the retained array backing lives.
        self.reserve(size_of::<T>())?;
        values.push(value);
        Ok(())
    }
}
