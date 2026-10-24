//! One invocation's explicitly paid insertion receivers and terminal report.
use super::*;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
use std::{collections::BTreeSet, marker::PhantomData, mem::size_of};

pub(super) struct Insertions<T> {
    pub(super) failed: bool,
    refusal: Option<ByteFailure>,
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
    // inert ledger return, compact refusal None/Some/take and terminal Diagnostic moves.
    // Every full Diagnostic/ByteFailure terminal bank remains: positive failures
    // retain their actual ByteFailure until failure() moves its identical Cow into
    // the Diagnostic. Foreign rejection still constructs its original Diagnostic
    // first and compacts only after the actual held ledger rejected storage.
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
            // A foreign/function/budget rejection still constructs the actual
            // original Diagnostic before compacting it. This NEW affine call
            // owns a Diagnostic actual/formal and returned compact carriers;
            // its bank is separate from the retained failure() conversion.
            + 2 * size_of::<Diagnostic>()
            + 2 * size_of::<ByteFailure>()
    }
    // A single entry's actual method/gateway/positive-reservation receivers.
    // Each entry consumes this bank and pays the next bank afresh. The final
    // unused entry remains reserved: no reset, refund or allowance donation.
    const ENTRY_CONTROLS: usize = 2 * size_of::<&mut Self>()
            + 4 * size_of::<&ResolvedFunction>()
            + 4 * size_of::<&mut ReplayBudget>()
            + size_of::<&mut Option<ByteFailure>>()
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
            // The lexical match keeps its actual &Incoming view. The old
            // contains callee's independent formal &Incoming no longer exists.
            + size_of::<&Incoming<T>>()
            + 2 * size_of::<&T>();
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
            #[cfg(test)]
            contains_tests::record_entry_bank();
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
            self.refusal = Some(ByteFailure::from_budget(Diagnostic::io(
                "SPX-H006",
                String::new(),
            )));
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
        let duplicate = match &*target {
            Incoming::Empty => false,
            Incoming::One(old) => old.as_ref().cmp(&value) == std::cmp::Ordering::Equal,
            Incoming::Many(v) => v.contains(&value),
        };
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
            self.refusal = Some(error);
            self.failed = true;
            return;
        }
        #[cfg(test)]
        if budget.pays_allocations() {
            contains_tests::record_entry_bank();
        }
        #[cfg(feature = "unstable-workflow-profiling")]
        if duplicate {
            crate::bounded_output::exclusive_debits::path_census::event(
                crate::bounded_output::exclusive_debits::path_census::Event::Duplicate,
            );
        }
        if !duplicate {
            #[cfg(feature = "unstable-workflow-profiling")]
            crate::bounded_output::exclusive_debits::path_census::event(
                crate::bounded_output::exclusive_debits::path_census::Event::OwnedBox,
            );
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
        let duplicate = match &*target {
            Incoming::Empty => false,
            Incoming::One(old) => old.as_ref().cmp(value.as_ref()) == std::cmp::Ordering::Equal,
            Incoming::Many(v) => v.contains(value.as_ref()),
        };
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
            self.refusal = Some(error);
            self.failed = true;
            return;
        }
        #[cfg(test)]
        if budget.pays_allocations() {
            contains_tests::record_entry_bank();
        }
        #[cfg(feature = "unstable-workflow-profiling")]
        if duplicate {
            crate::bounded_output::exclusive_debits::path_census::event(
                crate::bounded_output::exclusive_debits::path_census::Event::Duplicate,
            );
        }
        if !duplicate {
            #[cfg(feature = "unstable-workflow-profiling")]
            crate::bounded_output::exclusive_debits::path_census::event(
                crate::bounded_output::exclusive_debits::path_census::Event::HeldTransfer,
            );
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
            self.refusal = Some(ByteFailure::from_budget(Diagnostic::io(
                "SPX-H006",
                String::new(),
            )));
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
    #[cfg(feature = "unstable-workflow-profiling")]
    pub(super) fn observe_incoming(&self) {
        use crate::bounded_output::exclusive_debits::path_census::{event, Event};
        event(match self {
            Self::Empty => Event::InputEmpty,
            Self::One(_) => Event::InputOne,
            Self::Many(_) => Event::InputMany,
        });
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    // Called only after all box, node/split, move, empty replacement and Result
    // roles have been acquired. On a refused payment no target is taken.
    fn publish(&mut self, value: Box<T>) {
        match self {
            Self::Empty => {
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::bounded_output::exclusive_debits::path_census::event(
                    crate::bounded_output::exclusive_debits::path_census::Event::Empty,
                );
                *self = Self::One(value);
            }
            Self::One(_) => {
                #[cfg(feature = "unstable-workflow-profiling")]
                {
                    crate::bounded_output::exclusive_debits::path_census::event(
                        crate::bounded_output::exclusive_debits::path_census::Event::One,
                    );
                    crate::bounded_output::exclusive_debits::path_census::event(
                        crate::bounded_output::exclusive_debits::path_census::Event::Promotion,
                    );
                }
                let Self::One(old) = std::mem::take(self) else {
                    unreachable!()
                };
                let mut tree = BTreeSet::new();
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::bounded_output::exclusive_debits::path_census::event(
                    crate::bounded_output::exclusive_debits::path_census::Event::TreeInsert,
                );
                tree.insert(old);
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::bounded_output::exclusive_debits::path_census::event(
                    crate::bounded_output::exclusive_debits::path_census::Event::TreeInsert,
                );
                tree.insert(value);
                *self = Self::Many(tree);
            }
            Self::Many(tree) => {
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::bounded_output::exclusive_debits::path_census::event(
                    crate::bounded_output::exclusive_debits::path_census::Event::Many,
                );
                #[cfg(feature = "unstable-workflow-profiling")]
                crate::bounded_output::exclusive_debits::path_census::event(
                    crate::bounded_output::exclusive_debits::path_census::Event::TreeInsert,
                );
                tree.insert(value);
            }
        }
    }
    #[cfg(test)]
    pub(super) fn iter(&self) -> Iter<'_, T> {
        match self {
            Self::Empty => Iter::One(None.into_iter()),
            Self::One(v) => Iter::One(Some(v).into_iter()),
            Self::Many(v) => Iter::Many(v.iter()),
        }
    }
    #[cfg(test)]
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

// FUSED_COMPLETE_INDEPENDENT_STORAGE: namespace-only compile adaptation.
#[path = "storage"]
#[rustfmt::skip]
pub(super) mod storage {
//! Precharge actual owned clone carriers. Shared identity Arcs allocate nothing
//! when cloned; projection/type/case containers retain their actual backing.
use super::*;
use std::mem::size_of;
pub(in crate::cleanup_plan::replay::path_join) mod atomic_controls;
// FUSED_COMPLETE_EMPTY_RENEWALS
pub(in crate::cleanup_plan::replay::path_join) mod empty_renewals {
//! Ordinary builder branch clones reconstruct an empty renewal map directly.
//! The original clone and census remain complete for every other route.
use super::*;

/// The receiving constructor cannot clone the original renewal map: this
/// affine borrowed carrier contains only the other seven state fields.
pub(in crate::cleanup_plan::replay::path_join) struct Payload<'a> {
    live_order: &'a Vec<LivenessFlagId>,
    conditional_variants: &'a Vec<ReplayConditionalVariant>,
    pending_failure: &'a Option<StatusSourceId>,
    selected_failure: &'a Option<StatusSourceId>,
    staged_copy_result: &'a Option<StagedCopyResultSource>,
    published: bool,
    returned: &'a Option<ReturnedSource>,
}

fn controls_bytes() -> usize {
    // Producer view, outward Result and receiving view are separately priced.
    2 * size_of::<Payload<'_>>() + size_of::<Result<Payload<'_>, Diagnostic>>()
}

pub(super) fn clone_leaf(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Never
            | ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::ArrayU8(_)
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
    )
}

fn needs_type_scratch(source: &StagedCopyResultSource) -> bool {
    match source {
        StagedCopyResultSource::Body { instance, .. } => !clone_leaf(instance),
        StagedCopyResultSource::TryResidual {
            source_instance,
            target_instance,
            ..
        }
        | StagedCopyResultSource::TryOptionNone {
            source_instance,
            target_instance,
            ..
        } => !clone_leaf(source_instance) || !clone_leaf(target_instance),
    }
}

fn staged_bytes(source: &StagedCopyResultSource) -> Option<usize> {
    if needs_type_scratch(source) {
        staged(source)
    } else {
        Some(0)
    }
}

pub(in crate::cleanup_plan::replay::path_join) fn selected(
    state: &PathState,
    budget: &ReplayBudget,
) -> bool {
    matches!(budget.payer, AllocationPayer::ActiveBuilder)
        && budget.admission == AdmissionMode::Ordinary
        && state.renewals.is_empty()
}

fn payload_bytes(state: &PathState) -> Option<usize> {
    if !state.renewals.is_empty() {
        return None;
    }
    let mut bytes = array::<LivenessFlagId>(state.live_order.len())?.checked_add(array::<
        ReplayConditionalVariant,
    >(
        state.conditional_variants.len(),
    )?)?;
    for value in &state.conditional_variants {
        bytes = bytes.checked_add(variant(value)?)?;
    }
    if let Some(value) = &state.staged_copy_result {
        bytes = bytes.checked_add(staged_bytes(value)?)?;
    }
    if let Some(value) = &state.returned {
        bytes = bytes.checked_add(returned(value)?)?;
    }
    Some(bytes)
}

pub(in crate::cleanup_plan::replay::path_join) fn reserve<'a>(
    function: &ResolvedFunction,
    state: &'a PathState,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<Payload<'a>, Diagnostic> {
    if !selected(state, budget) {
        return Err(Diagnostic::io_static(
            "SPX-H006",
            "empty renewal clone requires an ordinary builder and an empty map",
        ));
    }
    budget.reserve_bytes(function, controls_bytes(), phase)?;
    // Atomic type tags have no child containers, so no cursor is constructed.
    // Compound types keep the complete original depth/work census and scratch.
    if state
        .staged_copy_result
        .as_ref()
        .is_some_and(needs_type_scratch)
    {
        budget.reserve_array::<Option<(&ResolvedType, usize)>>(
            function,
            crate::cache_codec::MAX_DEPTH + 1,
            "clone type scratch",
        )?;
    }
    budget.reserve_bytes(function, payload_bytes(state).unwrap_or(usize::MAX), phase)?;
    Ok(Payload {
        live_order: &state.live_order,
        conditional_variants: &state.conditional_variants,
        pending_failure: &state.pending_failure,
        selected_failure: &state.selected_failure,
        staged_copy_result: &state.staged_copy_result,
        published: state.published,
        returned: &state.returned,
    })
}

pub(in crate::cleanup_plan::replay::path_join) fn copy(payload: Payload<'_>) -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: payload.live_order.clone(),
        conditional_variants: payload.conditional_variants.clone(),
        pending_failure: payload.pending_failure.clone(),
        selected_failure: payload.selected_failure.clone(),
        staged_copy_result: payload.staged_copy_result.clone(),
        published: payload.published,
        returned: payload.returned.clone(),
    }
}

#[cfg(test)]
pub(super) mod tests {
use super::*;
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

pub(in crate::cleanup_plan::replay::path_join) fn function() -> ResolvedFunction {
    let parsed = crate::check(
        "module clone; @id(\"main\") fn main()->i64 {0}",
        "clone.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed).unwrap().functions.remove(0)
}

pub(in crate::cleanup_plan::replay::path_join) fn state(
    previously_populated: bool,
) -> PathState {
    let owner = crate::hir::FunctionExecutionId::Monomorphic(DeclarationId::new("clone.owner"));
    let expression = ExpressionId::new(&owner, "body.clone");
    let shared = DeclarationId::new("clone.case:é\0");
    let place = CleanupPlace {
        storage: StorageId::ProvisionalResult,
        projections: vec![shared.clone()],
    };
    let mut renewals = BTreeMap::new();
    if previously_populated {
        renewals.insert(
            expression.clone(),
            crate::cleanup_plan::renewal_history::RenewalHistory::new(
                vec![LivenessFlagId(1)],
                None,
            ),
        );
        drop(renewals.pop_first().unwrap());
    }
    PathState {
        renewals,
        live_order: vec![LivenessFlagId(1), LivenessFlagId(2)],
        conditional_variants: vec![ReplayConditionalVariant {
            root: place.clone(),
            variant: shared.clone(),
            cases: vec![(shared.clone(), vec![LivenessFlagId(3), LivenessFlagId(4)])],
        }],
        pending_failure: Some(StatusSourceId {
            expression: expression.clone(),
            lane: StatusLane::ContractFalse,
        }),
        selected_failure: Some(StatusSourceId {
            expression: ExpressionId::new(&owner, "body.selected"),
            lane: StatusLane::ContractFalse,
        }),
        staged_copy_result: Some(StagedCopyResultSource::Body {
            expression,
            instance: ResolvedType::Function {
                parameters: vec![ResolvedType::Nominal {
                    declaration: shared,
                    arguments: vec![ResolvedType::I64],
                }],
                result: Box::new(ResolvedType::I64),
            },
        }),
        published: true,
        returned: Some(ReturnedSource::Owned(place)),
    }
}

struct Receipt {
    ok: bool,
    work: usize,
    metrics: Metrics,
    refused: bool,
    used: usize,
    first: Option<ReservationRefusal>,
}

fn observe(function: &ResolvedFunction, state: &PathState, current: bool, limit: usize) -> Receipt {
    let (((ok, work), metrics), refused, used, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let result = (|| {
                // This is the unchanged real caller's request, before either
                // physical clone receiver is selected.
                budget.charge(
                    function,
                    crate::cleanup_plan::replay::renewal::retained_units(state),
                    "renewal branch history clone",
                )?;
                let copied = if current {
                    assert!(selected(state, &budget));
                    let payload = reserve(function, state, &mut budget, "ownership branch clone")?;
                    copy(payload)
                } else {
                    super::super::reserve_state_clone(
                        function,
                        state,
                        &mut budget,
                        "ownership branch clone",
                    )?;
                    state.clone()
                };
                assert_eq!(&copied, state);
                drop(copied);
                Ok::<(), Diagnostic>(())
            })();
            let ok = result.is_ok();
            if let Err(error) = result {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                let before = current_metrics().allocated;
                let Err(later) = reserve(function, state, &mut budget, "ownership branch clone")
                else {
                    panic!("a failed builder cannot publish another payload");
                };
                assert_eq!(later.code, "SPX-H006");
                drop(later);
                assert_eq!(current_metrics().allocated, before);
            }
            let work = budget.remaining;
            drop(budget);
            (ok, work)
        })
    });
    Receipt {
        ok,
        work,
        metrics,
        refused,
        used,
        first,
    }
}

#[test]
fn empty_renewal_branch_clone_preserves_every_field_work_and_complete_drop() {
    let function = function();
    for previously_populated in [false, true] {
        let state = state(previously_populated);
        // Both orders retain the same immutable input and observe each complete
        // producer through its result and budget's final Drop.
        for current_first in [false, true] {
            let first = observe(&function, &state, current_first, usize::MAX);
            let second = observe(&function, &state, !current_first, usize::MAX);
            let (current, old) = if current_first {
                (first, second)
            } else {
                (second, first)
            };
            for receipt in [&current, &old] {
                assert!(receipt.ok && !receipt.refused && receipt.first.is_none());
                assert!(!receipt.metrics.invalid);
                assert_eq!(receipt.metrics.live, 0);
                assert_eq!(receipt.metrics.allocated, receipt.metrics.freed);
                assert!(receipt.metrics.allocated <= receipt.used);
            }
            assert_eq!(current.work, old.work);
            let root = super::super::super::super::physical_storage::btree_node_upper::<
                ExpressionId,
                crate::cleanup_plan::renewal_history::RenewalHistory,
            >()
            .unwrap();
            assert_eq!(current.used + root, old.used + controls_bytes());
            assert!(current.used < old.used);
            // Independent requested-layout oracle: one refusal String, five
            // ownership/variant Vecs, three type buffers and returned projections.
            let field_bytes = 4 * size_of::<LivenessFlagId>()
                + size_of::<ReplayConditionalVariant>()
                + 2 * size_of::<DeclarationId>()
                + size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
                + 3 * size_of::<ResolvedType>();
            let expected = field_bytes + function.id.as_str().len() + 512;
            assert_eq!(current.metrics.allocations, 10);
            assert_eq!(current.metrics.allocated, expected);
            assert!(current.metrics.allocations <= old.metrics.allocations);
            assert!(current.metrics.allocated <= old.metrics.allocated);
            assert_eq!(current.metrics.reallocations, old.metrics.reallocations);
        }
    }
}

#[test]
fn empty_renewal_clone_exact_short_zero_and_each_request_frontier_remain_sticky() {
    let function = function();
    let state = state(true);
    let complete = observe(&function, &state, true, usize::MAX);
    assert!(complete.ok && !complete.refused && complete.used > 0);
    let exact = observe(&function, &state, true, complete.used);
    assert!(exact.ok && !exact.refused);
    assert_eq!(exact.used, complete.used);
    assert_eq!(exact.metrics, complete.metrics);
    let short = observe(&function, &state, true, complete.used - 1);
    assert!(!short.ok && short.refused && short.first.is_some());
    let mut limit = 0;
    for _ in 0..16 {
        let receipt = observe(&function, &state, true, limit);
        assert!(!receipt.metrics.invalid);
        assert_eq!(
            (receipt.metrics.live, receipt.metrics.allocated),
            (0, receipt.metrics.freed)
        );
        if receipt.ok {
            assert!(!receipt.refused);
            assert_eq!(receipt.used, complete.used);
            return;
        }
        assert!(receipt.refused);
        let first = receipt.first.unwrap();
        let next = receipt.used + first.requested + first.floor;
        assert!(next > limit);
        limit = next;
    }
    panic!("every reservation frontier must lead to the complete unchanged clone");
}

#[test]
fn atomic_type_clone_omits_only_the_absent_census_cursor_and_preserves_all_tags() {
    let function = function();
    let mut state = state(true);
    for ty in [
        ResolvedType::Never,
        ResolvedType::Unit,
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::String,
        ResolvedType::ArrayU8(7),
        ResolvedType::Bytes,
        ResolvedType::Str,
        ResolvedType::SliceU8,
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("clone.type.parameter"),
            index: 0,
        },
        ResolvedType::OnceFunction,
        ResolvedType::OnceFunctionI64,
        ResolvedType::OnceFunctionI64Pair,
        ResolvedType::MutFunctionI64,
    ] {
        assert!(clone_leaf(&ty));
        state.staged_copy_result = Some(StagedCopyResultSource::Body {
            expression: ExpressionId::from_owned("atomic.clone".to_owned()),
            instance: ty,
        });
        let old = observe(&function, &state, false, usize::MAX);
        let current = observe(&function, &state, true, usize::MAX);
        assert!(old.ok && current.ok && !old.refused && !current.refused);
        assert_eq!(current.work, old.work);
        let scratch =
            (crate::cache_codec::MAX_DEPTH + 1) * size_of::<Option<(&ResolvedType, usize)>>();
        let root = super::super::super::super::physical_storage::btree_node_upper::<
            ExpressionId,
            crate::cleanup_plan::renewal_history::RenewalHistory,
        >()
        .unwrap();
        assert_eq!(current.used + root + scratch, old.used + controls_bytes());
        assert_eq!(current.metrics.allocations, 7);
        assert!(!current.metrics.invalid);
        assert_eq!(current.metrics.live, 0);
        assert_eq!(current.metrics.allocated, current.metrics.freed);
        let exact = observe(&function, &state, true, current.used);
        assert!(exact.ok && !exact.refused);
        assert_eq!(exact.metrics, current.metrics);
        let short = observe(&function, &state, true, current.used - 1);
        assert!(!short.ok && short.refused && short.first.is_some());
    }
}

#[test]
fn nonempty_and_selected_or_unobserved_renewal_routes_keep_the_original_receiver() {
    let function = function();
    let mut state = state(false);
    let mut unobserved = ReplayBudget::unobserved();
    let mut paid = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    for budget in [&mut unobserved, &mut paid] {
        assert!(!selected(&state, budget));
        let Err(error) = reserve(&function, &state, budget, "closed route") else {
            panic!("a different replay route cannot publish this payload");
        };
        assert_eq!(
            error,
            Diagnostic::io_static(
                "SPX-H006",
                "empty renewal clone requires an ordinary builder and an empty map"
            )
        );
        super::super::reserve_state_clone(&function, &state, budget, "original clone").unwrap();
        assert_eq!(state.clone(), state);
    }
    state.renewals.insert(
        ExpressionId::from_owned("nonempty".to_owned()),
        crate::cleanup_plan::renewal_history::RenewalHistory::new(Vec::new(), None),
    );
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        assert!(!selected(&state, &budget));
        assert!(payload_bytes(&state).is_none());
        super::super::reserve_state_clone(
            &function,
            &state,
            &mut budget,
            "original nonempty clone",
        )
        .unwrap();
        assert_eq!(state.clone(), state);
    });
    assert!(!refused);
}
}
// END_FUSED_EMPTY_RENEWALS_TESTS
}
// END_FUSED_EMPTY_RENEWALS
fn array<T>(count: usize) -> Option<usize> {
    count.checked_mul(size_of::<T>())
}
fn place(place: &CleanupPlace) -> Option<usize> {
    array::<DeclarationId>(place.projections.len())
}
fn ty(ty: &ResolvedType) -> Option<usize> {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some((ty, 0usize));
    let mut len = 1;
    let mut bytes = 0usize;
    let mut work = 65_536usize;
    while len != 0 {
        let (value, next) = stack[len - 1]?;
        if next == 0 {
            work = work.checked_sub(1)?;
            let owned = match value {
                ResolvedType::Function { parameters, .. } => {
                    array::<ResolvedType>(parameters.len())?
                        .checked_add(size_of::<ResolvedType>())?
                }
                ResolvedType::Nominal { arguments, .. } => array::<ResolvedType>(arguments.len())?,
                ResolvedType::TypeParameter { .. } => 0,
                ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::ArrayU8(_)
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64 => 0,
            };
            bytes = bytes.checked_add(owned)?;
        }
        let child = match value {
            ResolvedType::Function { parameters, result } => parameters
                .get(next)
                .or_else(|| (next == parameters.len()).then_some(result.as_ref())),
            ResolvedType::Nominal { arguments, .. } => arguments.get(next),
            _ => None,
        };
        if let Some(child) = child {
            if len == stack.len() {
                return None;
            }
            stack[len - 1] = Some((value, next + 1));
            stack[len] = Some((child, 0));
            len += 1;
        } else {
            len -= 1;
        }
    }
    Some(bytes)
}
fn staged(source: &StagedCopyResultSource) -> Option<usize> {
    match source {
        StagedCopyResultSource::Body { instance, .. } => ty(instance),
        StagedCopyResultSource::TryResidual {
            source_instance,
            target_instance,
            ..
        }
        | StagedCopyResultSource::TryOptionNone {
            source_instance,
            target_instance,
            ..
        } => ty(source_instance)?.checked_add(ty(target_instance)?),
    }
}
fn returned(source: &ReturnedSource) -> Option<usize> {
    match source {
        ReturnedSource::Scalar(_) => Some(0),
        ReturnedSource::Owned(value) => place(value),
    }
}
fn variant(value: &ReplayConditionalVariant) -> Option<usize> {
    value.cases.iter().try_fold(
        place(&value.root)?.checked_add(array::<(DeclarationId, Vec<LivenessFlagId>)>(
            value.cases.len(),
        )?)?,
        |n, (_, flags)| n.checked_add(array::<LivenessFlagId>(flags.len())?),
    )
}
fn state_bytes(state: &PathState) -> Option<usize> {
    let node = super::super::physical_storage::btree_node_upper::<
        ExpressionId,
        crate::cleanup_plan::renewal_history::RenewalHistory,
    >()?;
    let mut bytes = node
        .checked_add(array::<LivenessFlagId>(state.live_order.len())?)?
        .checked_add(array::<ReplayConditionalVariant>(
            state.conditional_variants.len(),
        )?)?;
    for history in state.renewals.values() {
        bytes = bytes
            .checked_add(node)?
            .checked_add(history.clone_payload_bytes()?)?;
    }
    for value in &state.conditional_variants {
        bytes = bytes.checked_add(variant(value)?)?;
    }
    if let Some(value) = &state.staged_copy_result {
        bytes = bytes.checked_add(staged(value)?)?;
    }
    if let Some(value) = &state.returned {
        bytes = bytes.checked_add(returned(value)?)?;
    }
    Some(bytes)
}
#[track_caller]
pub(in crate::cleanup_plan::replay) fn reserve_btree_node<K, V>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    budget.reserve_bytes(
        function,
        super::super::physical_storage::btree_node_upper::<K, V>().unwrap_or(usize::MAX),
        phase,
    )
}
pub(in crate::cleanup_plan::replay) fn reserve_state_clone(
    function: &ResolvedFunction,
    state: &PathState,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    if state.staged_copy_result.is_some() {
        budget.reserve_array::<Option<(&ResolvedType, usize)>>(
            function,
            crate::cache_codec::MAX_DEPTH + 1,
            "clone type scratch",
        )?;
    }
    budget.reserve_bytes(function, state_bytes(state).unwrap_or(usize::MAX), phase)
}
pub(in crate::cleanup_plan::replay::path_join) fn reserve_ownership_clone(
    function: &ResolvedFunction,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    let bytes = (|| {
        state.conditional_variants.iter().try_fold(
            array::<LivenessFlagId>(state.live_order.len())?.checked_add(array::<
                ReplayConditionalVariant,
            >(
                state.conditional_variants.len(),
            )?)?,
            |n, value| n.checked_add(variant(value)?),
        )
    })();
    budget.reserve_bytes(
        function,
        bytes.unwrap_or(usize::MAX),
        "joined ownership clone",
    )
}
pub(in crate::cleanup_plan::replay::path_join) fn reserve_control_clone(
    function: &ResolvedFunction,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    if state.staged_copy_result.is_some() {
        budget.reserve_array::<Option<(&ResolvedType, usize)>>(
            function,
            crate::cache_codec::MAX_DEPTH + 1,
            "control clone type scratch",
        )?;
    }
    let bytes = (|| {
        let mut bytes = 0usize;
        if let Some(value) = &state.staged_copy_result {
            bytes = bytes.checked_add(staged(value)?)?;
        }
        if let Some(value) = &state.returned {
            bytes = bytes.checked_add(returned(value)?)?;
        }
        Some(bytes)
    })();
    budget.reserve_bytes(
        function,
        bytes.unwrap_or(usize::MAX),
        "ownership group control clone",
    )
}

#[track_caller]
pub(in crate::cleanup_plan::replay) fn insert_set<K: Ord>(
    function: &ResolvedFunction,
    target: &mut BTreeSet<K>,
    value: K,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<bool, Diagnostic> {
    if target.contains(&value) {
        return Ok(false);
    }
    reserve_btree_node::<K, ()>(function, budget, phase)?;
    Ok(target.insert(value))
}
#[track_caller]
pub(in crate::cleanup_plan::replay) fn insert_map<K: Ord, V>(
    function: &ResolvedFunction,
    target: &mut BTreeMap<K, V>,
    key: K,
    value: V,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    if !target.contains_key(&key) {
        reserve_btree_node::<K, V>(function, budget, phase)?;
    }
    target.insert(key, value);
    Ok(())
}

pub(in crate::cleanup_plan::replay::path_join) fn reserve_variant_identity(
    function: &ResolvedFunction,
    root: &CleanupPlace,
    _id: &DeclarationId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    budget.reserve_bytes(
        function,
        place(root).unwrap_or(usize::MAX),
        "joined variant identity",
    )
}

// Count through the existing ordered leaf index before its exact Vec copy.
// Preserve the unknown-storage refusal before attempting any reservation.
pub(in crate::cleanup_plan::replay) fn validate_place(
    function: &ResolvedFunction,
    place: &CleanupPlace,
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<Vec<LivenessFlagId>, Diagnostic> {
    if !known.contains(&place.storage) {
        return super::super::validate_place(function, place, known, leaves);
    }
    if !budget.pays_allocations() {
        return super::super::validate_place(function, place, known, leaves);
    }
    let matches = |leaf: &super::super::Leaf| {
        leaf.place.storage == place.storage
            && leaf.place.projections.starts_with(&place.projections)
    };
    let count = leaves.values().filter(|leaf| matches(leaf)).count();
    if count == 0 {
        return Err(replay_error(
            function,
            "cleanup place has no droppable leaf",
        ));
    }
    budget.reserve_array::<LivenessFlagId>(function, count, "entry place leaves")?;
    let mut result = Vec::with_capacity(count);
    result.extend(
        leaves
            .iter()
            .filter_map(|(flag, leaf)| matches(leaf).then_some(*flag)),
    );
    Ok(result)
}
pub(in crate::cleanup_plan::replay::path_join) fn storage_regions(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<StorageId, CleanupRegionId>, Diagnostic> {
    let mut regions = BTreeMap::new();
    for region in &function.cleanup_plan.regions {
        for storage in &region.slots {
            if regions.contains_key(storage) {
                return Err(replay_error(
                    function,
                    "cleanup storage has multiple owning regions during replay",
                ));
            }
            reserve_btree_node::<StorageId, CleanupRegionId>(
                function,
                budget,
                "storage region index",
            )?;
            regions.insert(storage.clone(), region.id);
        }
    }
    Ok(regions)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cloned_state_payload_counts_owned_buffers_but_not_shared_identity_handles() {
        let mut state = PathState {
            renewals: BTreeMap::new(),
            live_order: vec![LivenessFlagId(1), LivenessFlagId(2)],
            conditional_variants: Vec::new(),
            pending_failure: None,
            selected_failure: None,
            staged_copy_result: None,
            published: false,
            returned: None,
        };
        let root = super::super::super::physical_storage::btree_node_upper::<
            ExpressionId,
            crate::cleanup_plan::renewal_history::RenewalHistory,
        >()
        .unwrap();
        assert_eq!(
            state_bytes(&state),
            array::<LivenessFlagId>(2).unwrap().checked_add(root)
        );
        let history = crate::cleanup_plan::renewal_history::RenewalHistory::new(
            state.live_order.clone(),
            Some(LivenessFlagId(1)),
        );
        let payload = history.clone_payload_bytes().unwrap();
        state
            .renewals
            .insert(ExpressionId::from_owned("renewal".to_owned()), history);
        assert_eq!(
            state_bytes(&state),
            array::<LivenessFlagId>(2)
                .unwrap()
                .checked_add(root)
                .and_then(|n| n.checked_add(root))
                .and_then(|n| n.checked_add(payload))
        );
    }
    #[test]
    fn conditional_clone_literal_backing_keeps_projection_cases_and_flags() {
        let shared = DeclarationId::new("variant:é\0.long.shared");
        let value = ReplayConditionalVariant {
            root: CleanupPlace {
                storage: StorageId::ProvisionalResult,
                projections: vec![shared.clone(), shared.clone()],
            },
            variant: shared.clone(),
            cases: vec![
                (shared.clone(), vec![LivenessFlagId(1)]),
                (
                    DeclarationId::compiler(crate::hir::CompilerDeclaration::Vec),
                    vec![LivenessFlagId(2), LivenessFlagId(3)],
                ),
            ],
        };
        let expected = 2 * size_of::<DeclarationId>()
            + 2 * size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
            + 3 * size_of::<LivenessFlagId>();
        let former_string_payload = 4 * shared.as_str().len() + "core.vec".len();
        assert!(former_string_payload > 0);
        assert_eq!(variant(&value), Some(expected));
        let (_, observed) = crate::test_allocation_observer::measure(|| {
            let copy = value.clone();
            assert_eq!(copy, value);
            drop(copy);
        });
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 4);
        assert_eq!(observed.allocated, expected);
        assert_eq!(observed.freed, expected);
        assert_eq!(observed.live, 0);
    }
    #[test]
    fn exact_clone_charge_and_one_short_refusal_precede_clone_and_remain_sticky() {
        let parsed =
            crate::check("module joins; @id(\"main\") fn main()->i64 {0}", "join.spx").unwrap();
        let function = crate::hir::resolve(&parsed).unwrap().functions.remove(0);
        let state = PathState {
            renewals: BTreeMap::new(),
            live_order: vec![LivenessFlagId(1), LivenessFlagId(2)],
            conditional_variants: Vec::new(),
            pending_failure: None,
            selected_failure: None,
            staged_copy_result: None,
            published: false,
            returned: None,
        };
        let bytes = state_bytes(&state).unwrap();
        let mut exact = ReplayBudget::new();
        exact.admission = AdmissionMode::SelectedRetained;
        exact.payer = AllocationPayer::SelectedReservation;
        exact.physical = Some(Box::new(
            super::super::super::physical_storage::PhysicalReservation::new(bytes),
        ));
        reserve_state_clone(&function, &state, &mut exact, "test state clone").unwrap();
        assert_eq!(state.clone(), state);
        let mut short = ReplayBudget::new();
        short.admission = AdmissionMode::SelectedRetained;
        short.payer = AllocationPayer::SelectedReservation;
        short.physical = Some(Box::new(
            super::super::super::physical_storage::PhysicalReservation::new(bytes - 1),
        ));
        let mut destination = Vec::new();
        let first =
            reserve_state_clone(&function, &state, &mut short, "test state clone").unwrap_err();
        if reserve_state_clone(&function, &state, &mut short, "test state clone").is_ok() {
            destination.push(state.clone());
        }
        assert!(destination.is_empty());
        let later =
            reserve_state_clone(&function, &state, &mut short, "test state clone").unwrap_err();
        assert_eq!(first.code, later.code);
        assert_eq!(state.live_order, vec![LivenessFlagId(1), LivenessFlagId(2)]);
        reserve_state_clone(
            &function,
            &state,
            &mut ReplayBudget::new(),
            "legacy state clone",
        )
        .unwrap();
    }
}
}
