//! Paid immutable weighted backing for the ordinary canonical receiver.
//! Frames, open states and completed memo keys share only owned immutable
//! vectors. Label order, route weights, signature algebra and work debits keep
//! the complete original receiver in canonical_before.rs as an independent control.
use super::super::*;

#[cfg(test)]
#[path = "main_unit_weight/before_alias.rs"]
mod before_alias;
#[cfg(test)]
#[path = "main_unit_weight/before_index.rs"]
mod before_index;
#[cfg(test)]
#[path = "main_unit_weight/before_unit.rs"]
mod before_unit;
#[path = "main_unit_weight/epsilon_alias.rs"]
mod epsilon_alias;
#[path = "main_unit_weight/memo.rs"]
mod memo;
#[path = "main_unit_weight/unit_weight.rs"]
mod unit_weight;
#[cfg(test)]
#[path = "main_unit_weight/unit_weight_tests.rs"]
mod unit_weight_tests;

#[path = "main_unit_weight/source.rs"]
mod source;
use source::historical_main_source;

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
    closures: memo::Memo<Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<SharedWeighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> Canonicalizer<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: memo::Memo::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
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
        self.closures.authenticate(function, budget)?;
        budget.reserve_array::<(usize, bool)>(function, 1, "factored closure initial cursor")?;
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                stack.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            // Exactly one immutable epsilon child contributes the complete map.
            // Sink/own-move/duplicate-edge and all other shapes keep the full
            // general producer below, including checked route accumulation.
            if node != self.sink && state.moves.is_empty() && state.epsilon.len() == 1 {
                let Some(inner) = self.closures.get(&state.epsilon[0]) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                let shared = match epsilon_alias::share(function, inner, budget) {
                    Ok(shared) => shared,
                    Err(error) => {
                        self.closures.reject_after_error();
                        return Err(error);
                    }
                };
                self.closures.insert(function, node, budget, || shared)?;
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
                .insert(function, node, budget, || Rc::new(closure))?;
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
        #[cfg(test)]
        if unit_weight_tests::original_selected() {
            return self.successors_before_unit(function, state, budget);
        }
        // The id() entry authenticated the exact memo/function/payer first.
        // Pay both new Option constructions and the borrowed receiving view.
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                unit_weight::RECEIVER_BYTES,
                "factored weighted closure receiving view",
            )?;
        }
        budget.reserve_bytes(
            function,
            0,
            "factored weighted closure receiver authentication",
        )?;
        let alias = if let [(node, 1)] = state.as_slice() {
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            Some(unit_weight::receive(function, closure, budget)?)
        } else {
            None
        };
        let mut closed = BTreeMap::<usize, u64>::new();
        if alias.is_none() {
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
        }
        let closed = alias.as_deref().unwrap_or(&closed);
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        let mut successors = BTreeMap::<Label, BTreeMap<usize, u64>>::new();
        for (node, weight) in closed {
            let moves = &self.automaton.nodes[*node].moves;
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, BTreeMap<usize, u64>>(
                        function,
                        "factored successor label node",
                    )?;
                }
                let targets = successors.entry(label.clone()).or_default();
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
        budget.reserve_array::<(Label, SharedWeighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            result.push((label, weighted_successor(function, budget, targets)?));
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
        self.closures.authenticate(function, budget)?;
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root = own_weighted(function, budget, root)?;
        let root_copy = share_weighted(function, budget, &root)?;
        let mut frames = vec![CanonicalFrame::Enter(root_copy)];
        let mut open = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_btree_node::<SharedWeighted, ()>(
                        function,
                        "factored open-state identity node",
                    )?;
                    open.insert(share_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = frames.len();
                    frames.push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &frames[exit_index] else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child = share_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(&state);
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
#[path = "main_unit_weight/tests.rs"]
mod tests;

#[derive(Default)]
struct Node {
    moves: Vec<(Label, usize)>,
    epsilon: Vec<usize>,
}

#[derive(Default)]
struct Automaton {
    nodes: Vec<Node>,
}

impl Automaton {
    fn node(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        budget.charge(function, 1, "factored automaton node construction")?;
        budget.reserve_vec(function, &mut self.nodes, 1, "factored automaton nodes")?;
        self.nodes.push(Node::default());
        #[cfg(test)]
        instrumentation::node();
        Ok(self.nodes.len() - 1)
    }

    fn movement(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        from: usize,
        label: Label,
        to: usize,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "factored automaton labelled movement")?;
        budget.reserve_vec(
            function,
            &mut self.nodes[from].moves,
            1,
            "factored automaton moves",
        )?;
        self.nodes[from].moves.push((label, to));
        #[cfg(test)]
        instrumentation::edge();
        Ok(())
    }

    fn epsilon(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        from: usize,
        to: usize,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "factored automaton epsilon movement")?;
        budget.reserve_vec(
            function,
            &mut self.nodes[from].epsilon,
            1,
            "factored automaton epsilon moves",
        )?;
        self.nodes[from].epsilon.push(to);
        #[cfg(test)]
        instrumentation::edge();
        Ok(())
    }

    fn observation(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        from: usize,
        observation: SkeletonObservation,
        to: usize,
    ) -> Result<(), Diagnostic> {
        budget.reserve_bytes(
            function,
            physical_clone::observation_rc_bytes(),
            "factored automaton observation header",
        )?;
        self.movement(
            function,
            budget,
            from,
            Label::Observation(Rc::new(observation)),
            to,
        )
    }

    fn end(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        sink: usize,
        terminal: SkeletonTerminal,
    ) -> Result<usize, Diagnostic> {
        let node = self.node(function, budget)?;
        self.movement(function, budget, node, Label::End(terminal), sink)?;
        Ok(node)
    }
}
