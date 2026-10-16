//! Paid immutable weighted backing for the ordinary canonical receiver.
//! Frames, open states and completed memo keys share only owned immutable
//! payloads. Label order, route weights, signature algebra and work debits keep
//! the complete original receiver in canonical_before.rs as an independent control.
use super::super::{
    physical_clone, replay_error, Automaton, BTreeMap, Diagnostic, Label, Rc, ReplayBudget,
    ResolvedFunction, Weighted,
};

use super::{app_census, inline_weighted, successor_targets, tests};
#[path = "before_empty/closure_receiving.rs"]
mod closure_receiving;
#[path = "before_empty/memo.rs"]
mod memo;
#[path = "before_empty/ordered_alias.rs"]
mod ordered_alias;
#[path = "before_empty/ordered_weights.rs"]
mod ordered_weights;
use inline_weighted::SmallWeighted;

type SharedWeighted = Rc<SmallWeighted>;

enum CanonicalFrame {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
}

// The caller has already paid the moved vector's complete allocated backing.
// Rc owns that vector without copying it; its counters and full enum header are
// separately acquired while all preceding owners remain fully charged.
fn own_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: Weighted,
) -> Result<SharedWeighted, Diagnostic> {
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            2 * std::mem::size_of::<SmallWeighted>()
                + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>()
                + std::mem::size_of::<Result<(), Diagnostic>>(),
            "factored moved many weighted enum receiving",
        )?;
    }
    budget.reserve_bytes(
        function,
        physical_clone::rc_backing_bytes::<SmallWeighted>(),
        "factored weighted shared header",
    )?;
    let shared = SmallWeighted::Many(state);
    Ok(Rc::new(shared))
}

// Vec's FromIterator growth policy may round 1..3 items up to four slots.
// Build this producer through its exact precharged capacity instead.
fn weighted_successor(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    targets: BTreeMap<usize, u64>,
) -> Result<SharedWeighted, Diagnostic> {
    budget.reserve_array::<(usize, u64)>(
        function,
        targets.len(),
        "factored weighted successor vector",
    )?;
    let mut state = Vec::with_capacity(targets.len());
    state.extend(targets);
    own_weighted(function, budget, state)
}

fn share_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: &SharedWeighted,
) -> Result<SharedWeighted, Diagnostic> {
    let work = state
        .len()
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "factored weighted state work overflowed"))?;
    budget.charge(function, work, "factored weighted state copy")?;
    // Authentication remains sticky even though this clone creates no backing.
    // The receiving frame/map slot has its own full carrier reservation.
    budget.reserve_bytes(function, 0, "factored weighted state backing")?;
    Ok(Rc::clone(state))
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
pub(super) struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: memo::RetainedMemo<Rc<ordered_weights::OrderedWeights>>,
    states: BTreeMap<SharedWeighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
    cursor: Vec<(usize, bool)>,
    frames: Vec<CanonicalFrame>,
    open: Vec<SharedWeighted>,
    scratch_paid: bool,
}

impl<'a> Canonicalizer<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: memo::RetainedMemo::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
            cursor: Vec::new(),
            frames: Vec::new(),
            open: Vec::new(),
            scratch_paid: false,
        }
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    // Single-request test entries pay their real owner here; equivalent()
    // instead lends one invocation owner across both roots. No wrapper gain claim.
    #[cfg(test)]
    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        // Preserve the old entry's owner rejection before a fresh test-only slot
        // can query or charge a foreign payer. A valid call still pays acquisition.
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.id_with_receiving(function, root, budget, &mut receiving)
    }
    #[cfg(test)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.successors_with_receiving(function, state, budget, &mut receiving)
    }
    #[cfg(test)]
    pub(super) fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<ordered_weights::OrderedWeights>>, Diagnostic> {
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
        }
        if self.closures.contains_key(&root) {
            budget.charge(function, 1, "factored closure cursor")?;
            return Ok(self.closures.get(&root).cloned());
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.closure_with_receiving(function, root, budget, &mut receiving)
    }
    fn closure_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<Rc<ordered_weights::OrderedWeights>>, Diagnostic> {
        // Validate the held destination before any ordinary payer query.
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
        }
        // The cached route still executes its original one cursor work debit.
        // It constructs neither a cursor Vec nor a closure_inner Result
        // receiver. Cold memo authentication still pays its real Result; warm
        // authentication writes directly into its already paid refusal slot.
        if self.closures.contains_key(&root) {
            budget.charge(function, 1, "factored closure cursor")?;
            return Ok(self.closures.get(&root).cloned());
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::PER_MISS_CONTROLS
                    + std::mem::size_of::<Result<(), Diagnostic>>()
                    + std::mem::size_of::<usize>(),
                "factored closure scratch receiving",
            )?;
        }
        self.prepare_scratch(function, budget)?;
        receiving.begin();
        #[cfg(test)]
        closure_receiving::note_producer();
        self.closure_inner_into(function, root, budget, receiving);
        self.cursor.clear();
        assert!(receiving.ready, "unit producer settled exactly once");
        receiving.ready = false;
        // Do not construct an Option<Diagnostic> receiver on success.
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        Ok(receiving.value.take())
    }

    fn closure_inner_into(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) {
        macro_rules! attempt {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    Err(error) => {
                        receiving.refuse(error);
                        return;
                    }
                }
            };
        }
        attempt!(budget.reserve_vec(
            function,
            &mut self.cursor,
            1,
            "factored closure initial cursor",
        ));
        self.cursor.push((root, false));
        while let Some((node, expanded)) = self.cursor.pop() {
            attempt!(budget.charge(function, 1, "factored closure cursor"));
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                attempt!(budget.reserve_vec(
                    function,
                    &mut self.cursor,
                    1,
                    "factored closure cursors"
                ));
                self.cursor.push((node, true));
                for target in state.epsilon() {
                    if !self.closures.contains_key(target) {
                        attempt!(budget.reserve_vec(
                            function,
                            &mut self.cursor,
                            1,
                            "factored closure cursors",
                        ));
                        self.cursor.push((*target, false));
                    }
                }
                continue;
            }
            // Exactly one immutable epsilon child contributes the complete map.
            // Sink/own-move/duplicate-edge and all other shapes keep the full
            // general producer below, including checked route accumulation.
            if node != self.sink && state.moves().is_empty() && state.epsilon().len() == 1 {
                #[cfg(test)]
                app_census::eligible();
                let Some(inner) = self.closures.get(&state.epsilon()[0]) else {
                    receiving.refuse(replay_error(function, "cleanup CFG contains a cycle"));
                    return;
                };
                #[cfg(test)]
                app_census::child_ready(inner.len());
                let shared = match ordered_alias::share(function, inner, budget) {
                    Ok(shared) => shared,
                    Err(error) => {
                        self.closures.reject_after_error();
                        receiving.refuse(error);
                        return;
                    }
                };
                #[cfg(test)]
                app_census::acquired();
                attempt!(self
                    .closures
                    .insert_retained(function, node, budget, || shared));
                #[cfg(test)]
                app_census::published();
                continue;
            }
            if budget.pays_allocations() {
                attempt!(budget.reserve_bytes(
                    function,
                    ordered_weights::OrderedWeights::CONTROLS,
                    "factored ordered closure controls",
                ));
            }
            let mut closure = ordered_weights::OrderedWeights::new();
            if !state.moves().is_empty() || node == self.sink {
                if closure.needs_capacity() {
                    attempt!(closure.grow(function, budget));
                }
                closure.insert_prepaid(node, 1_u64);
            }
            for target in state.epsilon() {
                let Some(inner) = self.closures.get(target) else {
                    receiving.refuse(replay_error(function, "cleanup CFG contains a cycle"));
                    return;
                };
                for (inner_node, routes) in inner.iter() {
                    attempt!(budget.charge(function, 1, "factored closure route addition"));
                    if closure.get(inner_node).is_none() {
                        if closure.needs_capacity() {
                            attempt!(closure.grow(function, budget));
                        }
                        closure.insert_prepaid(*inner_node, 0_u64);
                    }
                    let slot = closure
                        .get_mut(inner_node)
                        .expect("inserted ordered weight");
                    let Some(sum) = slot.checked_add(*routes) else {
                        receiving.complete(None);
                        return;
                    };
                    *slot = sum;
                }
            }
            attempt!(budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            ));
            attempt!(budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<ordered_weights::OrderedWeights>(),
                "factored closure shared header",
            ));
            attempt!(self
                .closures
                .insert_retained(function, node, budget, || Rc::new(closure)));
        }
        receiving.complete(self.closures.get(&root).cloned());
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        // Only the selected branch initializes its owner. The common view is
        // a borrow, not a large enum containing both possible owner layouts.
        let borrowed;
        let mut owned;
        let closed = if let [(node, 1)] = state.as_slice() {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    2 * std::mem::size_of::<Rc<ordered_weights::OrderedWeights>>()
                        + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                    "factored borrowed closed receiving controls",
                )?;
            }
            let Some(closure) = self.closure_with_receiving(function, *node, budget, receiving)?
            else {
                return Ok(None);
            };
            // The old empty accumulator saw every unique closure entry once.
            // Execute its checked arithmetic and EACH original work debit,
            // before moving the authenticated immutable owner into the view.
            for (_, routes) in closure.iter() {
                let Some(_sum) = routes
                    .checked_mul(1)
                    .and_then(|product| 0_u64.checked_add(product))
                else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
            }
            borrowed = closure;
            &*borrowed
        } else {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    ordered_weights::OrderedWeights::CONTROLS
                        + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                    "factored ordered accumulator controls",
                )?;
            }
            owned = ordered_weights::OrderedWeights::new();
            for (node, weight) in state.iter() {
                let Some(closure) =
                    self.closure_with_receiving(function, *node, budget, receiving)?
                else {
                    return Ok(None);
                };
                for (inner, routes) in closure.iter() {
                    let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                        owned.get(inner).copied().unwrap_or(0).checked_add(product)
                    }) else {
                        return Ok(None);
                    };
                    budget.charge(function, 1, "factored weighted route addition")?;
                    if owned.get(inner).is_none() {
                        if owned.needs_capacity() {
                            owned.grow(function, budget)?;
                        }
                        owned.insert_prepaid(*inner, 0_u64);
                    }
                    *owned.get_mut(inner).expect("inserted ordered weight") = sum;
                }
            }
            &owned
        };
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<BTreeMap<Label, successor_targets::Targets>>()
                    + std::mem::size_of::<Option<&mut successor_targets::Targets>>(),
                "factored successor label receiving controls",
            )?;
        }
        let mut successors = BTreeMap::<Label, successor_targets::Targets>::new();
        for (node, weight) in closed.iter() {
            let moves = self.automaton.nodes[*node].moves();
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, successor_targets::Targets>(
                        function,
                        "factored successor label node",
                    )?;
                    if budget.pays_allocations() {
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<successor_targets::Targets>()
                                + std::mem::size_of::<Option<u64>>()
                                + std::mem::size_of::<Option<successor_targets::Targets>>(),
                            "factored inline singleton target initializer and receiving",
                        )?;
                    }
                    budget.reserve_bytes(
                        function,
                        0,
                        "factored singleton target authentication",
                    )?;
                    let weight = 0_u64
                        .checked_add(*weight)
                        .expect("zero plus u64 cannot overflow");
                    successors.insert(
                        label.clone(),
                        successor_targets::Targets::One {
                            child: *child,
                            weight,
                        },
                    );
                } else {
                    let targets = successors.get_mut(label).expect("existing label");
                    match targets {
                        successor_targets::Targets::One {
                            child: first,
                            weight: total,
                        } if *first == *child => {
                            // Same repeated-child checked_add before any later
                            // target acquisition or error, as in the old map.
                            let Some(sum) = total.checked_add(*weight) else {
                                return Ok(None);
                            };
                            *total = sum;
                        }
                        successor_targets::Targets::One {
                            child: first,
                            weight: total,
                        } => {
                            assert_ne!(*first, *child);
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    2 * std::mem::size_of::<BTreeMap<usize, u64>>()
                                        + std::mem::size_of::<successor_targets::Targets>()
                                        + 2 * std::mem::size_of::<Option<u64>>()
                                        + 2 * std::mem::size_of::<(usize, u64)>(),
                                    "factored inline successor promotion carriers",
                                )?;
                            }
                            budget.reserve_bytes(
                                function,
                                0,
                                "factored successor target promotion authentication",
                            )?;
                            budget.reserve_btree_node::<usize, u64>(
                                function,
                                "factored successor target node",
                            )?;
                            budget.reserve_btree_node::<usize, u64>(
                                function,
                                "factored successor target node",
                            )?;
                            let mut promoted = BTreeMap::new();
                            promoted.insert(*first, *total);
                            let sum = 0_u64
                                .checked_add(*weight)
                                .expect("zero plus u64 cannot overflow");
                            promoted.insert(*child, sum);
                            *targets = successor_targets::Targets::Many(promoted);
                        }
                        successor_targets::Targets::Many(targets) => {
                            if !targets.contains_key(child) {
                                budget.reserve_btree_node::<usize, u64>(
                                    function,
                                    "factored successor target node",
                                )?;
                            }
                            let slot = targets.entry(*child).or_insert(0);
                            let Some(sum) = slot.checked_add(*weight) else {
                                return Ok(None);
                            };
                            *slot = sum;
                        }
                    }
                }
            }
        }
        budget.reserve_array::<(Label, SharedWeighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        // Do not initialize an owning map iterator or its receiving tuple on
        // the empty route. Zero-byte vector authentication above still runs.
        if !successors.is_empty() {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<
                        std::collections::btree_map::IntoIter<Label, successor_targets::Targets>,
                    >() + std::mem::size_of::<Option<(Label, successor_targets::Targets)>>()
                        + std::mem::size_of::<(Label, successor_targets::Targets)>(),
                    "factored successor target iterator receiving",
                )?;
            }
            for (label, targets) in successors {
                if budget.pays_allocations() {
                    budget.reserve_bytes(
                        function,
                        2 * std::mem::size_of::<SharedWeighted>()
                            + std::mem::size_of::<(usize, u64)>(),
                        "factored inline successor finish receiving",
                    )?;
                }
                budget.reserve_bytes(
                    function,
                    0,
                    "factored successor target finish authentication",
                )?;
                let state = match targets {
                    successor_targets::Targets::One { child, weight } => {
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                2 * std::mem::size_of::<SmallWeighted>()
                                    + std::mem::size_of::<[(usize, u64); 1]>()
                                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                                "factored inline weighted enum and array receiving",
                            )?;
                        }
                        budget.reserve_bytes(
                            function,
                            physical_clone::rc_backing_bytes::<SmallWeighted>(),
                            "factored weighted shared header",
                        )?;
                        let singleton = SmallWeighted::One([(child, weight)]);
                        Rc::new(singleton)
                    }
                    successor_targets::Targets::Many(targets) => {
                        // The remaining fallible weighted producer still has
                        // its real map argument and Result receiving carrier.
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                std::mem::size_of::<BTreeMap<usize, u64>>()
                                    + std::mem::size_of::<Vec<(usize, u64)>>()
                                    + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>(),
                                "factored many weighted producer receiving",
                            )?;
                        }
                        weighted_successor(function, budget, targets)?
                    }
                };
                result.push((label, state));
            }
        }
        Ok(Some((accepted, result)))
    }

    pub(super) fn id_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        tests::note_actual_entry(budget);
        // Authenticate even when states already contains the requested root.
        // One equivalent() call holds this function and budget borrowed across
        // both roots; no closure/cache view leaves that invocation.
        // Validate the held destination before any ordinary payer query.
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Result<Option<usize>, Diagnostic>>()
                    + std::mem::size_of::<Weighted>()
                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                "factored canonical scratch receiving and moved root",
            )?;
        }
        self.prepare_scratch(function, budget)?;
        let result = self.id_inner(function, root, budget, receiving);
        // Retain capacity, never owned references, across either root and all
        // success, arithmetic, work, physical and semantic failure exits.
        self.cursor.clear();
        self.frames.clear();
        self.open.clear();
        result
    }

    fn prepare_scratch(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if !self.scratch_paid {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Vec<(usize, bool)>>()
                        + std::mem::size_of::<Vec<CanonicalFrame>>()
                        + std::mem::size_of::<Vec<SharedWeighted>>()
                        + std::mem::size_of::<bool>(),
                    "factored retained canonical scratch controls",
                )?;
            }
            self.scratch_paid = true;
        }
        Ok(())
    }

    fn id_inner(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_vec(
            function,
            &mut self.frames,
            1,
            "factored canonical initial frame",
        )?;
        // This direct root construction has no Diagnostic-bearing helper Result.
        // Both enum values, actual Rc initializer/receiving values and payment
        // Result remain real and prepaid; the general successor helper is unchanged.
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                2 * std::mem::size_of::<SmallWeighted>()
                    + 2 * std::mem::size_of::<SharedWeighted>()
                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                "factored direct root enum receiving",
            )?;
        }
        budget.reserve_bytes(
            function,
            physical_clone::rc_backing_bytes::<SmallWeighted>(),
            "factored weighted shared header",
        )?;
        let shared = SmallWeighted::Many(root);
        let root = Rc::new(shared);
        let root_copy = share_weighted(function, budget, &root)?;
        self.frames.push(CanonicalFrame::Enter(root_copy));
        while let Some(frame) = self.frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if self.open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_vec(
                        function,
                        &mut self.open,
                        1,
                        "factored retained open-state identity slots",
                    )?;
                    self.open.push(share_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) =
                        self.successors_with_receiving(function, &state, budget, receiving)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut self.frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = self.frames.len();
                    self.frames
                        .push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &self.frames[exit_index]
                        else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child = share_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut self.frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        self.frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    self.open.retain(|entry| entry != &state);
                    budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    )?;
                    budget.reserve_array::<(Label, usize)>(
                        function,
                        successors.len(),
                        "factored canonical signature vector",
                    )?;
                    let mut key = Vec::with_capacity(successors.len());
                    for (label, child) in successors {
                        let Some(id) = self.states.get(&child) else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let key = (accepted, key);
                    let id = match self.ids.get(&key) {
                        Some(id) => *id,
                        None => {
                            budget.reserve_btree_node::<(u64, Vec<(Label, usize)>), usize>(
                                function,
                                "factored canonical signature node",
                            )?;
                            let id = self.ids.len();
                            self.ids.insert(key, id);
                            id
                        }
                    };
                    if !self.states.contains_key(&state) {
                        budget.reserve_btree_node::<SharedWeighted, usize>(
                            function,
                            "factored canonical state memo node",
                        )?;
                    }
                    self.states.insert(state, id);
                }
            }
        }
        Ok(self.states.get(&root).copied())
    }
}


#[cfg(test)]
#[path = "before_empty/cached_tests.rs"]
mod cached_tests;
