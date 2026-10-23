//! One invocation's explicitly paid insertion receivers and terminal report.
use super::super::physical_storage::btree_node_upper;
use super::*;
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
    owned_empty: usize,
    held_empty: usize,
    owned_one: usize,
    held_one: usize,
    owned_many: usize,
    held_many: usize,

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
            // Eight checked amount producers: operand actual/formal values,
            // checked result/unwrap receiver, default and returned usize.
            // No per-entry Option callbacks or forwarding fee helper exists.
            + size_of::<[(usize,usize,Option<usize>,Option<usize>,usize,usize);8]>()
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
            + 6 * size_of::<Incoming<T>>()
            + 6 * size_of::<Box<T>>()
            + 2 * size_of::<BTreeSet<Box<T>>>()
            + 3 * size_of::<usize>()
            // contains result + named duplicate, each branch predicate and
            // match selector. The renewed bank precedes all warm hits too.
            + 8 * size_of::<bool>()
            + 2 * size_of::<Option<usize>>()
            + 2 * size_of::<&mut Incoming<T>>()
            + 2 * size_of::<&Incoming<T>>()
            + 2 * size_of::<&T>()
    }
    #[allow(
        clippy::manual_saturating_arithmetic,
        reason = "retain the separately prepaid checked Option and unwrap receiver banks"
    )]
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
        let boxed = size_of::<T>()
            .checked_add(2 * size_of::<Box<T>>())
            .unwrap_or(usize::MAX);
        let held_empty = Self::entry_controls();
        let owned_empty = held_empty.checked_add(boxed).unwrap_or(usize::MAX);
        let two_nodes = node.checked_mul(2).unwrap_or(usize::MAX);
        let held_one = two_nodes.checked_add(held_empty).unwrap_or(usize::MAX);
        let owned_one = held_one.checked_add(boxed).unwrap_or(usize::MAX);
        let held_many = node.checked_add(held_empty).unwrap_or(usize::MAX);
        let owned_many = held_many.checked_add(boxed).unwrap_or(usize::MAX);
        let mut result = Self {
            failed: false,
            refusal: None,
            entry: true,
            function: std::ptr::from_ref(function),
            budget: std::ptr::from_ref(budget),
            ledger: ClassifierLedger::empty_receiving(),
            owned_empty,
            held_empty,
            owned_one,
            held_one,
            owned_many,
            held_many,
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
    // Existing owner/ledger authentication consumes exactly one entry bank.
    // The complete next bank is prepaid even for Empty, equal and held hits.
    pub(super) fn insert_owned(
        &mut self,
        function: &ResolvedFunction,
        target: &mut Incoming<T>,
        value: T,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        if self.failed {
            return;
        }
        let duplicate = target.contains(&value);
        let bytes = if duplicate {
            self.held_empty
        } else {
            match target {
                Incoming::Empty => self.owned_empty,
                Incoming::One(_) => self.owned_one,
                Incoming::Many(_) => self.owned_many,
            }
        };
        if let Err(error) = budget.reserve_compact_bytes(
            function,
            bytes,
            "compact incoming owned state and publication renewal",
        ) {
            self.refusal = Some(error.into_diagnostic());
            self.failed = true;
            return;
        }
        if !duplicate {
            target.publish(Box::new(value));
        }
        self.entry = true;
    }
    pub(super) fn insert_held(
        &mut self,
        function: &ResolvedFunction,
        target: &mut Incoming<T>,
        value: Box<T>,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        if self.failed {
            return;
        }
        let duplicate = target.contains(value.as_ref());
        let bytes = if duplicate {
            self.held_empty
        } else {
            match target {
                Incoming::Empty => self.held_empty,
                Incoming::One(_) => self.held_one,
                Incoming::Many(_) => self.held_many,
            }
        };
        if let Err(error) = budget.reserve_compact_bytes(
            function,
            bytes,
            "compact incoming held state and publication renewal",
        ) {
            self.refusal = Some(error.into_diagnostic());
            self.failed = true;
            return;
        }
        if !duplicate {
            target.publish(value);
        }
        self.entry = true;
    }
    pub(super) fn failure(&mut self) -> Diagnostic {
        assert!(self.failed);
        self.refusal
            .take()
            .expect("ownership insertion report moves once")
    }
}

// Invocation-local incoming states: singleton ownership does not allocate a tree node.
use std::collections::btree_set;
#[derive(Debug)]
pub(super) enum Incoming<T> {
    Empty,
    One(Box<T>),
    Many(BTreeSet<Box<T>>),
}
#[allow(
    clippy::derivable_impls,
    reason = "unconditional Default preserves non-Default T and the paid callback receiver contract"
)]
impl<T> Default for Incoming<T> {
    fn default() -> Self {
        Self::Empty
    }
}
impl<T: Ord> Incoming<T> {
    #[allow(
        clippy::borrowed_box,
        reason = "retain the exact existing boxed owner without constructing a tree"
    )]
    pub(super) fn one(&self) -> &Box<T> {
        let Self::One(v) = self else {
            unreachable!("singleton frontier")
        };
        v
    }
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Many(v) => v.len(),
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn contains(&self, value: &T) -> bool {
        match self {
            Self::Empty => false,
            Self::One(old) => old.as_ref().cmp(value) == std::cmp::Ordering::Equal,
            Self::Many(v) => v.contains(value),
        }
    }
    // Called only after all box, node/split, move, empty replacement and Result
    // roles have been acquired. On a refused payment no target is taken.
    fn publish(&mut self, value: Box<T>) {
        match self {
            Self::Empty => *self = Self::One(value),
            Self::One(_) => {
                let Self::One(old) = std::mem::take(self) else {
                    unreachable!()
                };
                let mut tree = BTreeSet::new();
                tree.insert(old);
                tree.insert(value);
                *self = Self::Many(tree);
            }
            Self::Many(tree) => {
                tree.insert(value);
            }
        }
    }
    pub(super) fn iter(&self) -> Iter<'_, T> {
        match self {
            Self::Empty => Iter::One(None.into_iter()),
            Self::One(v) => Iter::One(Some(v).into_iter()),
            Self::Many(v) => Iter::Many(v.iter()),
        }
    }
    pub(super) fn into_one(self) -> std::option::IntoIter<Box<T>> {
        let Self::One(v) = self else {
            unreachable!("singleton frontier")
        };
        Some(v).into_iter()
    }
    pub(super) fn into_many(self) -> BTreeSet<Box<T>> {
        let Self::Many(v) = self else {
            unreachable!("multiple-state frontier")
        };
        v
    }
}
#[allow(
    clippy::borrowed_box,
    reason = "borrow the exact boxed owner for held insertion and same-box transport"
)]
pub(super) enum Iter<'a, T> {
    One(std::option::IntoIter<&'a Box<T>>),
    Many(btree_set::Iter<'a, Box<T>>),
}
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a Box<T>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(v) => v.next(),
            Self::Many(v) => v.next(),
        }
    }
}
