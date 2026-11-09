//! One invocation's explicitly paid insertion receivers and terminal report.
use super::super::physical_storage::btree_node_upper;
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::{collections::BTreeSet, marker::PhantomData, mem::size_of};

pub(super) struct Insertions<T> {
    pub(super) failed: bool,
    refusal: Option<Diagnostic>,
    entry: bool,
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
    owned: usize,
    held: usize,
    marker: PhantomData<fn() -> T>,
}
impl<T: Ord> Insertions<T> {
    // Aggregate initializer, named mutable result and caller receiving owner;
    // the Result<Self> wrapper remains separate, plus constructor/payment Results,
    // inert ledger return, refusal None/Some/take and terminal Diagnostic moves.
    pub(super) fn controls() -> usize {
        3 * size_of::<Self>()
            + size_of::<Result<Self, Diagnostic>>()
            + size_of::<Result<(), Diagnostic>>()
            + size_of::<ClassifierLedger>()
            + 3 * size_of::<Option<Diagnostic>>()
            + 2 * size_of::<Diagnostic>()
            + size_of::<&mut Self>()
            + size_of::<&mut Option<Diagnostic>>()
            + 5 * size_of::<Option<usize>>()
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::insertion_terminal_controls()
            // At most one positive-payment failure reaches conversion. Pay its
            // actual/formal compact values and Diagnostic construction/return
            // once, before any entry; sticky failure forbids reuse.
            + 2 * size_of::<ByteFailure>()
            + 2 * size_of::<Diagnostic>()
    }
    // A single entry's actual method/gateway/positive-reservation receivers.
    // Each entry consumes this bank and pays the next bank afresh. The final
    // unused entry remains reserved: no reset, refund or allowance donation.
    pub(super) fn entry_controls() -> usize {
        2 * size_of::<&mut Self>()
            + 4 * size_of::<&ResolvedFunction>()
            + 4 * size_of::<&mut ReplayBudget>()
            + size_of::<&mut Option<Diagnostic>>()
            + size_of::<Result<(), ByteFailure>>()
            + ClassifierLedger::current_receiving_controls()
    }
    pub(super) fn acquire(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                Self::controls() + Self::entry_controls(),
                "ownership insertion receiving controls",
            )?;
        }
        let node = btree_node_upper::<Box<T>, ()>().unwrap_or(usize::MAX);
        let owned = node
            .checked_add(size_of::<T>())
            .and_then(|n| n.checked_add(2 * size_of::<Box<T>>()))
            .and_then(|n| n.checked_add(Self::entry_controls()))
            .unwrap_or(usize::MAX);
        let held = node.saturating_add(Self::entry_controls());
        let mut result = Self {
            failed: false,
            refusal: None,
            entry: true,
            function: std::ptr::from_ref(function),
            budget: std::ptr::from_ref(budget),
            ledger: ClassifierLedger::empty_receiving(),
            owned,
            held,
            marker: PhantomData,
        };
        result.ledger.capture_current_into();
        Ok(result)
    }
    fn authenticate(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget) {
        if self.failed {
            return;
        }
        assert!(self.entry);
        self.entry = false;
        // Compare exact receiver identity before querying a possibly foreign
        // payer. The held ledger poisons only its original owner on mismatch.
        if self.function != std::ptr::from_ref(function)
            || self.budget != std::ptr::from_ref(budget)
            || !self.ledger.current()
        {
            self.ledger.reject_storage();
            self.refusal = Some(Diagnostic::io("SPX-H006", String::new()));
            self.failed = true;
            return;
        }
        budget.authenticate_zero_into(
            function,
            "incoming ownership state authority",
            &mut self.refusal,
        );
        self.failed = self.refusal.is_some();
    }
    pub(super) fn insert_owned(
        &mut self,
        function: &ResolvedFunction,
        target: &mut BTreeSet<Box<T>>,
        value: T,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        if self.failed {
            return;
        }
        if target.contains(&value) {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::entry_controls(),
                "ownership duplicate receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
        } else {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                self.owned,
                "incoming ownership state node, box and receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
            target.insert(Box::new(value));
        }
        self.entry = true;
    }
    pub(super) fn insert_held(
        &mut self,
        function: &ResolvedFunction,
        target: &mut BTreeSet<Box<T>>,
        value: Box<T>,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        if self.failed {
            return;
        }
        if target.contains(value.as_ref()) {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::entry_controls(),
                "ownership held duplicate receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
        } else {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                self.held,
                "incoming held ownership state node and receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
            target.insert(value);
        }
        self.entry = true;
    }
    // One terminal publication: callers return immediately. The owner remains
    // failed after the report moves, so neither backing nor a bank can revive.
    pub(super) fn failure(&mut self) -> Diagnostic {
        assert!(self.failed);
        self.refusal
            .take()
            .expect("ownership insertion report moves once")
    }
}

#[cfg(test)]
#[path = "boxed_states/before.rs"]
mod before;
#[cfg(test)]
use before::{insert_held, insert_owned};
#[cfg(test)]
#[path = "boxed_states/receiving_tests.rs"]
mod receiving_tests;
#[cfg(test)]
mod tests;
