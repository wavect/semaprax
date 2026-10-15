//! Exact weighted canonicalization with actual charged dictionary probes.
//! This owner is reachable only from an opaque complete borrowed-helper lease.
use super::super::super::borrowed_canonical::dictionary;
use super::*;
use dictionary::Dictionary;
pub(super) struct BorrowedCanonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: Dictionary<usize, Rc<Dictionary<usize, u64>>>,
    states: Dictionary<Weighted, usize>,
    ids: Dictionary<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> BorrowedCanonicalizer<'a> {
    pub(super) fn new(
        function: &ResolvedFunction,
        automaton: &'a Automaton,
        sink: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        budget.reserve_array::<Self>(function, 1, "borrowed canonical owner header")?;
        Ok(Self {
            automaton,
            sink,
            closures: Dictionary::new(),
            states: Dictionary::new(),
            ids: Dictionary::new(),
        })
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<Dictionary<usize, u64>>>, Diagnostic> {
        budget.reserve_array::<(usize, bool)>(function, 1, "factored closure initial cursor")?;
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains(function, &node, budget)? {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                grow(function, budget, &mut stack, 1, "factored closure cursors")?;
                stack.push((node, true));
                for target in &state.epsilon {
                    budget.charge(function, 1, "borrowed closure actual epsilon child")?;
                    if !self.closures.contains(function, target, budget)? {
                        grow(function, budget, &mut stack, 1, "factored closure cursors")?;
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            budget.reserve_array::<Dictionary<usize, u64>>(
                function,
                1,
                "borrowed closure dictionary header",
            )?;
            let mut closure = Dictionary::new();
            if !state.moves.is_empty() || node == self.sink {
                closure.insert(function, node, 1_u64, budget)?;
            }
            for target in &state.epsilon {
                budget.charge(
                    function,
                    1,
                    "borrowed closure actual completed epsilon child",
                )?;
                let Some(inner) = self.closures.get(function, target, budget)? else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
                    budget.charge(function, 1, "factored closure route addition")?;
                    let slot = closure.entry(function, *inner_node, budget)?;
                    let Some(sum) = slot.checked_add(*routes) else {
                        return Ok(None);
                    };
                    *slot = sum;
                }
            }
            budget.charge(
                function,
                closure
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| replay_error(function, "borrowed closure work overflowed"))?,
                "factored skeleton closure",
            )?;
            budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<Dictionary<usize, u64>>(),
                "factored closure shared header",
            )?;
            self.closures
                .insert(function, node, Rc::new(closure), budget)?;
        }
        Ok(self.closures.get(function, &root, budget)?.cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, Weighted)>)>, Diagnostic> {
        budget.reserve_array::<Dictionary<usize, u64>>(
            function,
            1,
            "borrowed weighted closure header",
        )?;
        let mut closed = Dictionary::<usize, u64>::new();
        for (node, weight) in state {
            budget.charge(function, 1, "borrowed actual weighted state member")?;
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            for (inner, routes) in closure.iter() {
                let Some(product) = routes.checked_mul(*weight) else {
                    return Ok(None);
                };
                let previous = closed.get(function, inner, budget)?.copied().unwrap_or(0);
                let Some(sum) = previous.checked_add(product) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                closed.insert(function, *inner, sum, budget)?;
            }
        }
        let accepted = closed
            .get(function, &self.sink, budget)?
            .copied()
            .unwrap_or(0);
        budget.reserve_array::<Dictionary<Label, Dictionary<usize, u64>>>(
            function,
            1,
            "borrowed successor dictionary header",
        )?;
        let mut successors = Dictionary::<Label, Dictionary<usize, u64>>::new();
        for (node, weight) in closed.iter() {
            let moves = &self.automaton.nodes[*node].moves;
            budget.charge(
                function,
                moves
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| replay_error(function, "borrowed movement work overflowed"))?,
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                let targets = successors.entry(function, label.clone(), budget)?;
                let slot = targets.entry(function, *child, budget)?;
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
        for (label, targets) in successors.into_rows() {
            result.push((label, targets.into_rows()));
        }
        Ok(Some((accepted, result)))
    }

    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root_copy = clone_weighted(function, budget, &root)?;
        let mut frames = vec![CanonicalFrame::Enter(root_copy)];
        budget.reserve_array::<Dictionary<Weighted, ()>>(
            function,
            1,
            "borrowed open dictionary header",
        )?;
        let mut open = Dictionary::<Weighted, ()>::new();
        while let Some(frame) = frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains(function, &state, budget)? {
                        continue;
                    }
                    if open.contains(function, &state, budget)? {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    open.insert(
                        function,
                        clone_weighted(function, budget, &state)?,
                        (),
                        budget,
                    )?;
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    grow(
                        function,
                        budget,
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
                        let child = clone_weighted(function, budget, &successors[index].1)?;
                        grow(
                            function,
                            budget,
                            &mut frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(function, &state, budget)?;
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
                        let Some(id) = self.states.get(function, &child, budget)? else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let key = (accepted, key);
                    let id = match self.ids.get(function, &key, budget)? {
                        Some(id) => *id,
                        None => {
                            let id = self.ids.len();
                            self.ids.insert(function, key, id, budget)?;
                            id
                        }
                    };
                    self.states.insert(function, state, id, budget)?;
                }
            }
        }
        Ok(self.states.get(function, &root, budget)?.copied())
    }
}

fn clone_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    value: &Weighted,
) -> Result<Weighted, Diagnostic> {
    budget.charge(
        function,
        value
            .len()
            .checked_add(1)
            .ok_or_else(|| replay_error(function, "borrowed weighted clone work overflowed"))?,
        "borrowed actual weighted clone pairs",
    )?;
    budget.reserve_array::<(usize, u64)>(
        function,
        value.len(),
        "borrowed exact weighted clone backing",
    )?;
    Ok(value.clone())
}

fn grow<T>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    values: &mut Vec<T>,
    additional: usize,
    phase: &str,
) -> Result<(), Diagnostic> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or_else(|| replay_error(function, "borrowed canonical cursor length overflowed"))?;
    if required > values.capacity() {
        budget.charge(
            function,
            values.len(),
            "borrowed canonical actual cursor relocation",
        )?;
    }
    budget.reserve_vec(function, values, additional, phase)
}
