//! Paid immutable weighted backing for the ordinary canonical receiver.
//! Frames, open states and completed memo keys share only owned immutable
//! vectors. Label order, route weights, signature algebra and work debits keep
//! the complete original receiver in canonical_before.rs as an independent control.
use super::{
    physical_clone, replay_error, Automaton, BTreeMap, Diagnostic, Label, Rc, ReplayBudget,
    ResolvedFunction, Weighted,
};

#[cfg(test)]
mod before_alias;
#[cfg(test)]
mod before_index;
#[cfg(test)]
mod before_scratch;
#[cfg(test)]
mod before_targets;
mod epsilon_alias;
mod memo;
#[cfg(test)]
mod scratch_tests;
#[cfg(test)]
mod singleton_target_tests;
mod successor_targets;

type SharedWeighted = Rc<Weighted>;

enum CanonicalFrame {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
}

// The caller has already paid the moved vector's complete allocated backing.
// Rc owns that vector without copying it; its two counters and Vec header are
// separately acquired while all preceding owners remain fully charged.
fn own_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: Weighted,
) -> Result<SharedWeighted, Diagnostic> {
    budget.reserve_bytes(
        function,
        physical_clone::rc_backing_bytes::<Weighted>(),
        "factored weighted shared header",
    )?;
    Ok(Rc::new(state))
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
    closures: memo::RetainedMemo<Rc<BTreeMap<usize, u64>>>,
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
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
        self.closures.authenticate_retained(function, budget);
        if self.closures.failed() {
            return Err(self.closures.failure());
        }
        // The cached route still executes its original one cursor work debit.
        // It constructs neither a cursor Vec nor a closure_inner Result
        // receiver. The ordinary memo authentication Result remains paid.
        if self.closures.contains_key(&root) {
            budget.charge(function, 1, "factored closure cursor")?;
            return Ok(self.closures.get(&root).cloned());
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic>>()
                    + std::mem::size_of::<Result<(), Diagnostic>>()
                    + std::mem::size_of::<usize>(),
                "factored closure scratch receiving",
            )?;
        }
        self.prepare_scratch(function, budget)?;
        let result = self.closure_inner(function, root, budget);
        self.cursor.clear();
        result
    }

    fn closure_inner(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
        budget.reserve_vec(
            function,
            &mut self.cursor,
            1,
            "factored closure initial cursor",
        )?;
        self.cursor.push((root, false));
        while let Some((node, expanded)) = self.cursor.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                budget.reserve_vec(function, &mut self.cursor, 1, "factored closure cursors")?;
                self.cursor.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        budget.reserve_vec(
                            function,
                            &mut self.cursor,
                            1,
                            "factored closure cursors",
                        )?;
                        self.cursor.push((*target, false));
                    }
                }
                continue;
            }
            // Exactly one immutable epsilon child contributes the complete map.
            // Sink/own-move/duplicate-edge and all other shapes keep the full
            // general producer below, including checked route accumulation.
            if node != self.sink && state.moves.is_empty() && state.epsilon.len() == 1 {
                #[cfg(test)]
                app_census::eligible();
                let Some(inner) = self.closures.get(&state.epsilon[0]) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                #[cfg(test)]
                app_census::child_ready(inner.len());
                let shared = match epsilon_alias::share(function, inner, budget) {
                    Ok(shared) => shared,
                    Err(error) => {
                        self.closures.reject_after_error();
                        return Err(error);
                    }
                };
                #[cfg(test)]
                app_census::acquired();
                self.closures
                    .insert_retained(function, node, budget, || shared)?;
                #[cfg(test)]
                app_census::published();
                continue;
            }
            let mut closure = BTreeMap::new();
            if !state.moves.is_empty() || node == self.sink {
                budget.reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                closure.insert(node, 1_u64);
            }
            for target in &state.epsilon {
                let Some(inner) = self.closures.get(target) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
                    budget.charge(function, 1, "factored closure route addition")?;
                    if !closure.contains_key(inner_node) {
                        budget
                            .reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                    }
                    let slot = closure.entry(*inner_node).or_insert(0_u64);
                    let Some(sum) = slot.checked_add(*routes) else {
                        return Ok(None);
                    };
                    *slot = sum;
                }
            }
            budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            )?;
            budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<BTreeMap<usize, u64>>(),
                "factored closure shared header",
            )?;
            self.closures
                .insert_retained(function, node, budget, || Rc::new(closure))?;
        }
        Ok(self.closures.get(&root).cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        let mut closed = BTreeMap::<usize, u64>::new();
        for (node, weight) in state.iter() {
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    closed.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if !closed.contains_key(inner) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored weighted closure node",
                    )?;
                }
                closed.insert(*inner, sum);
            }
        }
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
        for (node, weight) in &closed {
            let moves = &self.automaton.nodes[*node].moves;
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
                            + std::mem::size_of::<Vec<(usize, u64)>>()
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
                        budget.reserve_array::<(usize, u64)>(
                            function,
                            1,
                            "factored weighted successor vector",
                        )?;
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                std::mem::size_of::<[(usize, u64); 1]>(),
                                "factored one-entry array initializer",
                            )?;
                        }
                        let state = Vec::from([(child, weight)]);
                        budget.reserve_bytes(
                            function,
                            physical_clone::rc_backing_bytes::<Weighted>(),
                            "factored weighted shared header",
                        )?;
                        Rc::new(state)
                    }
                    successor_targets::Targets::Many(targets) => {
                        // The remaining fallible weighted producer still has
                        // its real map argument and Result receiving carrier.
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                std::mem::size_of::<BTreeMap<usize, u64>>()
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

    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        tests::note_actual_entry(budget);
        // Authenticate even when states already contains the requested root.
        // One equivalent() call holds this function and budget borrowed across
        // both roots; no closure/cache view leaves that invocation.
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
        let result = self.id_inner(function, root, budget);
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
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_vec(
            function,
            &mut self.frames,
            1,
            "factored canonical initial frame",
        )?;
        let root = own_weighted(function, budget, root)?;
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
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
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
mod tests;

#[cfg(test)]
pub(super) mod app_census;

#[cfg(test)]
mod before_receiving_layers;
#[cfg(test)]
mod receiving_layer_tests;
