//! One invocation's explicitly paid insertion receivers and terminal report.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::{collections::BTreeSet, marker::PhantomData, mem::size_of};

// Lexical renewal has the original payment and one-shot failure frontier.
// No helper call, result packet, or new argument transport is introduced.
macro_rules! admit_entry {
    ($label:lifetime, $owner:ident, $function:ident, $budget:ident, $bytes:expr, $phase:literal) => {
        if let Err(error) = $budget.reserve_compact_bytes($function, $bytes, $phase) {
            $owner.refusal = Some(error);
            $owner.failed = true;
            break $label;
        }
        #[cfg(test)]
        if $budget.pays_allocations() {
            {
                contains_tests::record_entry_bank();
                tests::entry_bank();
            }
        }
    };
}
macro_rules! insertion_event {
    ($event:ident) => {
        #[cfg(feature = "unstable-workflow-profiling")]
        crate::bounded_output::exclusive_debits::path_census::event(
            crate::bounded_output::exclusive_debits::path_census::Event::$event,
        );
    };
}

macro_rules! insert_owned_body {
    ($receiver:ty, $owner:ident, $function:ident, $target:ident, $value:ident, $budget:ident, $label:lifetime) => {
        if $owner.failed {
            break $label;
        }
        if let Some(old) = &mut $target.one {
            if old.as_ref().cmp(&$value) == std::cmp::Ordering::Equal {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::ENTRY_CONTROLS,
                    "compact incoming owned state and publication renewal"
                );
                frontier::insertion_event!(Duplicate);
            } else {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::OWNED_ONE,
                    "compact incoming owned state and publication renewal"
                );
                frontier::insertion_event!(OwnedBox);
                // Original owned argument construction preceded entering
                // publish(), hence also preceded taking the old One owner.
                let $value = Box::new($value);
                frontier::insertion_event!(One);
                frontier::insertion_event!(Promotion);
                #[cfg(test)]
                {
                    fused_tests::record_promotion();
                    tests::promotion();
                }
                let Some(old) = $target.one.take() else {
                    unreachable!("one owner before promotion");
                };
                let mut tree = BTreeSet::new();
                frontier::insertion_event!(TreeInsert);
                tree.insert(old);
                frontier::insertion_event!(TreeInsert);
                tree.insert($value);
                $target.many = Some(Box::new(tree));
            }
        } else if let Some(tree) = &mut $target.many {
            let tree = tree.as_mut();
            if tree.contains(&$value) {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::ENTRY_CONTROLS,
                    "compact incoming owned state and publication renewal"
                );
                frontier::insertion_event!(Duplicate);
            } else {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::OWNED_MANY,
                    "compact incoming owned state and publication renewal"
                );
                frontier::insertion_event!(OwnedBox);
                let $value = Box::new($value);
                frontier::insertion_event!(Many);
                frontier::insertion_event!(TreeInsert);
                tree.insert($value);
            }
        } else {
            frontier::admit_entry!(
                $label,
                $owner,
                $function,
                $budget,
                <$receiver>::OWNED_EMPTY,
                "compact incoming owned state and publication renewal"
            );
            frontier::insertion_event!(OwnedBox);
            let $value = Box::new($value);
            frontier::insertion_event!(Empty);
            $target.one = Some($value);
        }
        $owner.entry = true;
    };
}
macro_rules! insert_held_body {
    ($receiver:ty, $owner:ident, $function:ident, $target:ident, $value:ident, $budget:ident, $label:lifetime) => {
        if $owner.failed {
            break $label;
        }
        if let Some(old) = &mut $target.one {
            if old.as_ref().cmp($value.as_ref()) == std::cmp::Ordering::Equal {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::ENTRY_CONTROLS,
                    "compact incoming held state and publication renewal"
                );
                frontier::insertion_event!(Duplicate);
            } else {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::HELD_ONE,
                    "compact incoming held state and publication renewal"
                );
                frontier::insertion_event!(HeldTransfer);
                frontier::insertion_event!(One);
                frontier::insertion_event!(Promotion);
                #[cfg(test)]
                {
                    fused_tests::record_promotion();
                    tests::promotion();
                }
                let Some(old) = $target.one.take() else {
                    unreachable!("one owner before promotion");
                };
                let mut tree = BTreeSet::new();
                frontier::insertion_event!(TreeInsert);
                tree.insert(old);
                frontier::insertion_event!(TreeInsert);
                tree.insert($value);
                $target.many = Some(Box::new(tree));
            }
        } else if let Some(tree) = &mut $target.many {
            let tree = tree.as_mut();
            if tree.contains($value.as_ref()) {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::ENTRY_CONTROLS,
                    "compact incoming held state and publication renewal"
                );
                frontier::insertion_event!(Duplicate);
            } else {
                frontier::admit_entry!(
                    $label,
                    $owner,
                    $function,
                    $budget,
                    <$receiver>::HELD_MANY,
                    "compact incoming held state and publication renewal"
                );
                frontier::insertion_event!(HeldTransfer);
                frontier::insertion_event!(Many);
                frontier::insertion_event!(TreeInsert);
                tree.insert($value);
            }
        } else {
            frontier::admit_entry!(
                $label,
                $owner,
                $function,
                $budget,
                <$receiver>::ENTRY_CONTROLS,
                "compact incoming held state and publication renewal"
            );
            frontier::insertion_event!(HeldTransfer);
            frontier::insertion_event!(Empty);
            $target.one = Some($value);
        }
        $owner.entry = true;
    };
}
macro_rules! authenticate_lexical {
    ($owner:ident, $function:ident, $budget:ident, $label:lifetime) => {
        if $owner.failed {
            break $label;
        }
        assert!($owner.entry);
        $owner.entry = false;
        // Compare exact receiver identity before querying a possibly foreign
        // payer. The held ledger poisons only its original owner on mismatch.
        if $owner.function != std::ptr::from_ref($function)
            || $owner.budget != std::ptr::from_ref($budget)
            || !$owner.ledger.current()
        {
            $owner.ledger.reject_storage();
            $owner.refusal = Some(crate::cleanup_plan::replay::physical_budget::ByteFailure {
                message: std::borrow::Cow::Owned(String::new()),
            });
            $owner.failed = true;
            break $label;
        }
        $budget.authenticate_zero_compact_into(
            $function,
            "incoming ownership state authority",
            &mut $owner.refusal,
        );
        $owner.failed = $owner.refusal.is_some();
    };
}
macro_rules! insert_owned {
    ($ty:ty, $owner:ident, $function:ident, $target:expr, $value:expr, $budget:ident $(,)?) => {{
        // These are the original target/value receiving locals. No owning
        // Incoming aggregate or insertion/authentication method is forwarded.
        let target = $target;
        let value = $value;
        'attempt: {
            frontier::authenticate_lexical!($owner, $function, $budget, 'attempt);
            frontier::insert_owned_body!(frontier::ShippingInsertions<$ty>, $owner, $function, target, value, $budget, 'attempt);
        }
    }};
}
macro_rules! insert_held {
    ($ty:ty, $owner:ident, $function:ident, $target:expr, $value:expr, $budget:ident $(,)?) => {{
        // These are the original target/value receiving locals. No owning
        // Incoming aggregate or insertion/authentication method is forwarded.
        let target = $target;
        let value = $value;
        'attempt: {
            frontier::authenticate_lexical!($owner, $function, $budget, 'attempt);
            frontier::insert_held_body!(frontier::ShippingInsertions<$ty>, $owner, $function, target, value, $budget, 'attempt);
        }
    }};
}
macro_rules! incoming_len {
    ($target:ident) => {
        if let Some(_) = $target.one {
            1
        } else if let Some(tree) = &$target.many {
            tree.len()
        } else {
            0
        }
    };
}
macro_rules! incoming_empty {
    ($target:ident) => {
        if let Some(_) = $target.one {
            false
        } else if let Some(_) = $target.many {
            false
        } else {
            true
        }
    };
}
pub(super) use {
    admit_entry, authenticate_lexical, insert_held, insert_held_body, insert_owned,
    insert_owned_body, insertion_event,
};
pub(super) use {incoming_empty, incoming_len};

#[cfg(test)]
pub(super) type Insertions<T> = Receiving<T, false>;
pub(super) type ShippingInsertions<T> = Receiving<T, true>;
pub(super) struct Receiving<T, const LEXICAL: bool> {
    pub(super) failed: bool,
    pub(super) refusal: Option<ByteFailure>,
    pub(super) entry: bool,
    pub(super) group_entry: bool,
    pub(super) function: *const ResolvedFunction,
    pub(super) budget: *const ReplayBudget,
    pub(super) ledger: ClassifierLedger,

    marker: PhantomData<fn() -> T>,
}
impl<T: Ord, const LEXICAL: bool> Receiving<T, LEXICAL> {
    // Aggregate initializer, named mutable result and caller receiving owner;
    // the Result<Self> wrapper remains separate, plus constructor/payment Results,
    // inert ledger return, compact refusal None/Some/take and terminal Diagnostic moves.
    // Every full Diagnostic/ByteFailure terminal bank remains: positive failures
    // retain their actual ByteFailure until failure() moves its identical Cow into
    // the Diagnostic. Foreign rejection publishes the same owned-empty Cow
    // only after the actual held ledger rejected storage; no Diagnostic is
    // constructed and consumed at that private intermediate boundary.
    pub(super) fn controls() -> usize {
        3 * size_of::<Self>()
            + size_of::<Result<Self, Diagnostic>>()
            + size_of::<Result<(), Diagnostic>>()
            + size_of::<ClassifierLedger>()
            + 3 * size_of::<Option<ByteFailure>>()
            + 2 * size_of::<Diagnostic>()
            + size_of::<&mut Self>()
            + size_of::<&mut Option<ByteFailure>>()
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
            // Foreign authority retains the exact owned-empty H006 payload.
            // Its direct String/Cow/compact aggregates replace the removed
            // Diagnostic::io and from_budget actual/formal gateways. The
            // independent terminal conversion above remains fully reserved.
            + 2 * size_of::<String>()
            + 2 * size_of::<std::borrow::Cow<'static, str>>()
            + 2 * size_of::<ByteFailure>()
    }
    // A single entry's actual method/gateway/positive-reservation receivers.
    // Each entry consumes this bank and pays the next bank afresh. The final
    // unused entry remains reserved: no reset, refund or allowance donation.
    pub(super) const ENTRY_CONTROLS: usize = if LEXICAL { 0 } else { 2 * size_of::<&mut Self>() }
            + 4 * size_of::<&ResolvedFunction>()
            + 4 * size_of::<&mut ReplayBudget>()
            + size_of::<&mut Option<ByteFailure>>()
            + size_of::<Result<(), ByteFailure>>()
            + ClassifierLedger::current_receiving_controls()
            // Field publication forms only Option<Box>, never an owning
            // Incoming aggregate. The lexical input receiving local preserves
            // early value Drop on every refusal, so its Box carrier stays paid.
            // Every Box constructor/node payment remains independently priced.
            + 2 * size_of::<Option<Box<T>>>()
            + 5 * size_of::<Box<T>>()
            // The fused mutable variant binds One/Many receivers. Their shared
            // cmp/contains reborrows are distinct from the retained old loans.
            + 2 * size_of::<&mut Option<Box<T>>>()
            + size_of::<&mut Box<T>>()
            + size_of::<&mut BTreeSet<Box<T>>>()
            + 2 * size_of::<&Box<T>>()
            + 2 * size_of::<&BTreeSet<Box<T>>>()
            // Many field selection and Box::as_mut actual/formal/returned loans.
            + size_of::<&mut Option<Box<BTreeSet<Box<T>>>>>()
            + 2 * size_of::<&mut Box<BTreeSet<Box<T>>>>()
            + 2 * size_of::<&mut BTreeSet<Box<T>>>()
            + 3 * size_of::<usize>()
            // contains result + named duplicate, each branch predicate and
            // match selector. The renewed bank precedes all warm hits too.
            + 12 * size_of::<bool>()
            + 2 * size_of::<Option<usize>>()
            + 2 * size_of::<&mut Incoming<T>>()
            // The lexical match keeps its actual &Incoming view. The old
            // contains callee's independent formal &Incoming no longer exists.
            + size_of::<&Incoming<T>>()
            + 2 * size_of::<&T>();
    // Paid only after a distinct One value has been compared and before
    // taking the One field, its Option transports and new/moved tree exist.
    // No old NODE/BOXED payment or unused common entry is credited/refunded.
    pub(super) const PROMOTION_CONTROLS: usize = 4 * size_of::<Option<Box<T>>>()
            + 2 * size_of::<&mut Option<Box<T>>>()
            // Tree new return/local plus Box::new formal and actual heap payload.
            + 4 * size_of::<BTreeSet<Box<T>>>()
            // Box constructor result, receiving owner, Some field and publication.
            + 4 * size_of::<Box<BTreeSet<Box<T>>>>()
            + 2 * size_of::<Option<Box<BTreeSet<Box<T>>>>>();
    #[cfg(test)]
    pub(super) fn promotion_controls() -> usize {
        Self::PROMOTION_CONTROLS
    }
    pub(super) fn entry_controls() -> usize {
        Self::ENTRY_CONTROLS
    }
    // Exact original tree entry price; only borrowed Self references depend on
    // the receiver type, and both remain thin references to a sized owner.
    pub(super) const GROUP_ENTRY_CONTROLS: usize = 2 * size_of::<&mut Self>()
        + 4 * size_of::<&ResolvedFunction>()
        + 4 * size_of::<&mut ReplayBudget>()
        + size_of::<&mut Option<ByteFailure>>()
        + size_of::<Result<(), ByteFailure>>()
        + ClassifierLedger::current_receiving_controls();
    const NODE: usize = match const_node_upper::<Box<T>, ()>() {
        Some(bytes) => bytes,
        None => usize::MAX,
    };
    const BOXED: usize = size_of::<T>().saturating_add(2 * size_of::<Box<T>>());
    pub(super) const OWNED_EMPTY: usize = Self::ENTRY_CONTROLS.saturating_add(Self::BOXED);
    const TWO_NODES: usize = Self::NODE.saturating_mul(2);
    pub(super) const HELD_ONE: usize = Self::TWO_NODES
        .saturating_add(Self::ENTRY_CONTROLS)
        .saturating_add(Self::PROMOTION_CONTROLS);
    pub(super) const OWNED_ONE: usize = Self::HELD_ONE.saturating_add(Self::BOXED);
    pub(super) const HELD_MANY: usize = Self::NODE.saturating_add(Self::ENTRY_CONTROLS);
    pub(super) const OWNED_MANY: usize = Self::HELD_MANY.saturating_add(Self::BOXED);
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
            #[cfg(test)]
            {
                contains_tests::record_entry_bank();
                tests::entry_bank();
            }
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
    #[cfg(test)]
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
            self.refusal = Some(ByteFailure {
                message: std::borrow::Cow::Owned(String::new()),
            });
            self.failed = true;
            return;
        }
        budget.authenticate_zero_compact_into(
            function,
            "incoming ownership state authority",
            &mut self.refusal,
        );
        self.failed = self.refusal.is_some();
    }
    // Existing owner/ledger authentication consumes exactly one entry bank.
    // The complete next bank is prepaid even for Empty, equal and held hits.
    #[cfg(test)]
    pub(super) fn insert_owned(
        &mut self,
        function: &ResolvedFunction,
        target: &mut Incoming<T>,
        value: T,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        'attempt: {
            frontier::insert_owned_body!(Self, self, function, target, value, budget, 'attempt);
        }
    }
    #[cfg(test)]
    pub(super) fn insert_held(
        &mut self,
        function: &ResolvedFunction,
        target: &mut Incoming<T>,
        value: Box<T>,
        budget: &mut ReplayBudget,
    ) {
        self.authenticate(function, budget);
        'attempt: {
            frontier::insert_held_body!(Self, self, function, target, value, budget, 'attempt);
        }
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
            self.refusal = Some(ByteFailure {
                message: std::borrow::Cow::Owned(String::new()),
            });
            self.failed = true;
            return;
        }
        budget.authenticate_zero_compact_into(
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
                self.refusal = Some(error);
                self.failed = true;
                return;
            }
        } else {
            if let Err(error) = budget.reserve_compact_bytes(
                function,
                Self::GROUP_HELD,
                "incoming held ownership state node and receiving renewal",
            ) {
                self.refusal = Some(error);
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
            .into_diagnostic()
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
#[cfg(test)]
use std::collections::btree_set;
#[derive(Debug)]
pub(super) struct Incoming<T> {
    // Exactly one field can own states. During promotion both fields remain
    // empty until the same original sorted tree is completely constructed.
    pub(super) one: Option<Box<T>>,
    pub(super) many: Option<Box<BTreeSet<Box<T>>>>,
}
impl<T> Default for Incoming<T> {
    fn default() -> Self {
        Self {
            one: None,
            many: None,
        }
    }
}
impl<T: Ord> Incoming<T> {
    #[allow(clippy::borrowed_box, reason = "borrow the existing singleton Box")]
    #[cfg(test)]
    pub(super) fn one(&self) -> &Box<T> {
        self.one.as_ref().expect("singleton frontier")
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        if self.one.is_some() {
            1
        } else if let Some(tree) = &self.many {
            tree.len()
        } else {
            0
        }
    }
    #[cfg(feature = "unstable-workflow-profiling")]
    pub(super) fn observe_incoming(&self) {
        use crate::bounded_output::exclusive_debits::path_census::{event, Event};
        event(if self.one.is_some() {
            Event::InputOne
        } else if self.many.is_none() {
            Event::InputEmpty
        } else {
            Event::InputMany
        });
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[cfg(test)]
    pub(super) fn is_many(&self) -> bool {
        self.many.is_some()
    }
    #[cfg(test)]
    pub(super) fn iter(&self) -> Iter<'_, T> {
        if let Some(v) = self.one.as_ref() {
            Iter::One(Some(v).into_iter())
        } else if let Some(tree) = &self.many {
            Iter::Many(tree.iter())
        } else {
            Iter::One(None.into_iter())
        }
    }
    #[cfg(test)]
    pub(super) fn into_one(self) -> std::option::IntoIter<Box<T>> {
        assert!(self.many.is_none(), "singleton frontier");
        self.one.into_iter()
    }
    #[cfg(test)]
    pub(super) fn into_many(self) -> BTreeSet<Box<T>> {
        assert!(self.one.is_none(), "multiple-state frontier");
        match self.many {
            Some(tree) => *tree,
            None => BTreeSet::new(),
        }
    }
}
#[allow(
    clippy::borrowed_box,
    reason = "borrow the exact boxed owner for held insertion and same-box transport"
)]
#[cfg(test)]
pub(super) enum Iter<'a, T> {
    One(std::option::IntoIter<&'a Box<T>>),
    Many(btree_set::Iter<'a, Box<T>>),
}
#[cfg(test)]
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a Box<T>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::One(v) => v.next(),
            Self::Many(v) => v.next(),
        }
    }
}
