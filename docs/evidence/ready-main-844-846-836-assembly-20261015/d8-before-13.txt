//! Complete pre-sharing receiver; its interior is retained verbatim.
use super::*;

type Signatures = BTreeMap<(u64, Vec<(Label, usize)>), usize>;

enum CanonicalFrame {
    Enter(Weighted),
    Exit(Weighted, u64, Vec<(Label, Weighted)>),
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: BTreeMap<usize, Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<Weighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

#[rustfmt::skip]
impl<'a> Canonicalizer<'a> {
    fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: BTreeMap::new(),
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
            budget.reserve_btree_node::<usize, Rc<BTreeMap<usize, u64>>>(
                function,
                "factored closure memo node",
            )?;
            self.closures.insert(node, Rc::new(closure));
        }
        Ok(self.closures.get(&root).cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, Weighted)>)>, Diagnostic> {
        let mut closed = BTreeMap::<usize, u64>::new();
        for (node, weight) in state {
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
        let mut successors = BTreeMap::<Label, BTreeMap<usize, u64>>::new();
        for (node, weight) in &closed {
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
        budget.reserve_array::<(Label, Weighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            budget.reserve_array::<(usize, u64)>(
                function,
                targets.len(),
                "factored weighted successor vector",
            )?;
            result.push((label, targets.into_iter().collect()));
        }
        Ok(Some((accepted, result)))
    }

    fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root_copy = storage::clone_weighted(function, budget, &root)?;
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
                    budget.reserve_btree_node::<Weighted, ()>(
                        function,
                        "factored open-state identity node",
                    )?;
                    open.insert(storage::clone_weighted(function, budget, &state)?);
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
                        let child =
                            storage::clone_weighted(function, budget, &successors[index].1)?;
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
                        budget.reserve_btree_node::<Weighted, usize>(
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

// The wrapper exposes the unchanged receiver to sibling controls.
pub(super) struct Receiver<'a>(Canonicalizer<'a>);
impl<'a> Receiver<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self(Canonicalizer::new(automaton, sink))
    }
    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        self.0.id(function, root, budget)
    }
    pub(super) fn states(&self) -> &BTreeMap<Weighted, usize> {
        &self.0.states
    }
    pub(super) fn signatures(&self) -> &Signatures {
        &self.0.ids
    }
}
