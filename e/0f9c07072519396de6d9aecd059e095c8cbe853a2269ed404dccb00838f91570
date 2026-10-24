//! One invocation's explicitly paid insertion receivers and terminal report.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::{collections::BTreeSet, marker::PhantomData, mem::size_of};

pub(super) struct Insertions<T> {
    pub(super) failed: bool,
    refusal: Option<Diagnostic>,
    entry: bool,
    group_entry: bool,
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,

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
            + ClassifierLedger::capture_into_controls()
            + ClassifierLedger::insertion_terminal_controls()
            // One independent tree-group entry bank shares the exact held
            // authority/error owner. The original node/renewal tariff remains.
            + Self::GROUP_ENTRY_CONTROLS
            // At most one positive-payment failure reaches conversion. Pay its
            // actual/formal compact values and Diagnostic construction/return
            // once, before any entry; sticky failure forbids reuse.
            + 2 * size_of::<ByteFailure>()
            + 2 * size_of::<Diagnostic>()
    }
    // A single entry's actual method/gateway/positive-reservation receivers.
    // Each entry consumes this bank and pays the next bank afresh. The final
    // unused entry remains reserved: no reset, refund or allowance donation.
    const ENTRY_CONTROLS: usize = 2 * size_of::<&mut Self>()
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
            + 2 * size_of::<&T>();
    pub(super) fn entry_controls() -> usize {
        Self::ENTRY_CONTROLS
    }
    // Exact original tree entry price; only borrowed Self references depend on
    // the receiver type, and both remain thin references to a sized owner.
    pub(super) const GROUP_ENTRY_CONTROLS: usize = 2 * size_of::<&mut Self>()
        + 4 * size_of::<&ResolvedFunction>()
        + 4 * size_of::<&mut ReplayBudget>()
        + size_of::<&mut Option<Diagnostic>>()
        + size_of::<Result<(), ByteFailure>>()
        + ClassifierLedger::current_receiving_controls();
    const NODE: usize = match const_node_upper::<Box<T>, ()>() {
        Some(bytes) => bytes,
        None => usize::MAX,
    };
    const BOXED: usize = size_of::<T>().saturating_add(2 * size_of::<Box<T>>());
    const OWNED_EMPTY: usize = Self::ENTRY_CONTROLS.saturating_add(Self::BOXED);
    const TWO_NODES: usize = Self::NODE.saturating_mul(2);
    const HELD_ONE: usize = Self::TWO_NODES.saturating_add(Self::ENTRY_CONTROLS);
    const OWNED_ONE: usize = Self::HELD_ONE.saturating_add(Self::BOXED);
    const HELD_MANY: usize = Self::NODE.saturating_add(Self::ENTRY_CONTROLS);
    const OWNED_MANY: usize = Self::HELD_MANY.saturating_add(Self::BOXED);
    const GROUP_HELD: usize = Self::NODE.saturating_add(Self::GROUP_ENTRY_CONTROLS);
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
        let mut result = Self {
            failed: false,
            refusal: None,
            entry: true,
            group_entry: true,
            function: std::ptr::from_ref(function),
            budget: std::ptr::from_ref(budget),
            ledger: ClassifierLedger::empty_receiving(),
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
            Self::ENTRY_CONTROLS
        } else {
            match target {
                Incoming::Empty => Self::OWNED_EMPTY,
                Incoming::One(_) => Self::OWNED_ONE,
                Incoming::Many(_) => Self::OWNED_MANY,
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
            Self::ENTRY_CONTROLS
        } else {
            match target {
                Incoming::Empty => Self::ENTRY_CONTROLS,
                Incoming::One(_) => Self::HELD_ONE,
                Incoming::Many(_) => Self::HELD_MANY,
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
    // Complete original held-tree producer, sharing only retained ownership.
    fn authenticate_group(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget) {
        if self.failed {
            return;
        }
        assert!(self.group_entry);
        self.group_entry = false;
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
    pub(super) fn insert_group_held(
        &mut self,
        function: &ResolvedFunction,
        target: &mut BTreeSet<Box<T>>,
        value: Box<T>,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate_group(function, budget);
        if self.failed {
            return;
        }
        if target.contains(value.as_ref()) {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::GROUP_ENTRY_CONTROLS,
                "ownership held duplicate receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
        } else {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::GROUP_HELD,
                "incoming held ownership state node and receiving renewal",
            ) {
                self.refusal = Some(error.into_diagnostic());
                self.failed = true;
                return;
            }
            target.insert(value);
        }
        self.group_entry = true;
    }
    #[cfg(test)]
    pub(super) fn observed_prices() -> [usize; 7] {
        [
            Self::OWNED_EMPTY,
            Self::ENTRY_CONTROLS,
            Self::OWNED_ONE,
            Self::HELD_ONE,
            Self::OWNED_MANY,
            Self::HELD_MANY,
            Self::GROUP_HELD,
        ]
    }
    pub(super) fn failure(&mut self) -> Diagnostic {
        assert!(self.failed);
        self.refusal
            .take()
            .expect("ownership insertion report moves once")
    }
}

// Exact original B-tree layout algorithm, evaluated only by NODE above.
// Its six independently rounded fields and every checked failure keep the
// complete original usize::MAX fallback; no aggregate is formed per entry.
#[allow(
    clippy::manual_saturating_arithmetic,
    reason = "retain exact original checked layout/sentinel semantics"
)]
pub(super) const fn const_node_upper<K, V>() -> Option<usize> {
    use std::mem::align_of;
    let mut alignment = align_of::<K>();
    if align_of::<V>() > alignment {
        alignment = align_of::<V>();
    }
    if align_of::<usize>() > alignment {
        alignment = align_of::<usize>();
    }
    let keys = match size_of::<K>().checked_mul(11) {
        Some(bytes) => bytes,
        None => return None,
    };
    let values = match size_of::<V>().checked_mul(11) {
        Some(bytes) => bytes,
        None => return None,
    };
    let children = match size_of::<usize>().checked_mul(12) {
        Some(bytes) => bytes,
        None => return None,
    };
    let fields = [
        size_of::<Option<std::ptr::NonNull<()>>>(),
        size_of::<u16>(),
        size_of::<u16>(),
        keys,
        values,
        children,
    ];
    let mut total = 0usize;
    let mut index = 0usize;
    while index < fields.len() {
        let rounded = match fields[index].checked_add(alignment - 1) {
            Some(bytes) => bytes / alignment * alignment,
            None => return None,
        };
        total = match total.checked_add(rounded) {
            Some(bytes) => bytes,
            None => return None,
        };
        index += 1;
    }
    Some(total)
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
