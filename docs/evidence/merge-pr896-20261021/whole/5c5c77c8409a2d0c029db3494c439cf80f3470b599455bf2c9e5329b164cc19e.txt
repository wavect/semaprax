//! Paid immutable weighted backing for the ordinary canonical receiver.
//! Frames, open states and completed memo keys share only owned immutable
//! vectors. Label order, route weights, signature algebra and work debits keep
//! the complete original receiver in canonical_before.rs as an independent control.
use super::super::{
    physical_clone, replay_error, BTreeMap, BTreeSet, Diagnostic, Label, Rc, ReplayBudget,
    ResolvedFunction, Weighted,
};

use super::super::{instrumentation, SkeletonTerminal};
use super::{app_census, epsilon_alias};
// Genuine c598 Vec-field automaton. Both historical receivers borrow this same
// representation; current compact-node getters are absent from their cut.
#[derive(Default)]
struct Node {
    moves: Vec<(Label, usize)>,
    epsilon: Vec<usize>,
}

#[derive(Default)]
pub(super) struct Automaton {
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

pub(super) fn graph(
    function: &ResolvedFunction,
    width: usize,
    repeats: usize,
) -> (Automaton, usize, usize) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    let root = graph.node(function, &mut budget).unwrap();
    let mut children = Vec::new();
    for _ in 0..width {
        children.push(
            graph
                .end(function, &mut budget, sink, SkeletonTerminal::Failure)
                .unwrap(),
        );
    }
    for child in children.into_iter().rev() {
        for _ in 0..repeats {
            graph
                .movement(
                    function,
                    &mut budget,
                    root,
                    Label::End(SkeletonTerminal::Success),
                    child,
                )
                .unwrap();
        }
    }
    (graph, sink, root)
}

// Fixture-only equality runs before any measured receiver/payer is created.
// The getter calls belong to neither historical canonicalization arm.
pub(super) fn assert_same_fixture(old: &Automaton, current: &super::super::Automaton) {
    assert_eq!(old.nodes.len(), current.nodes.len());
    for (old, current) in old.nodes.iter().zip(&current.nodes) {
        assert_eq!(old.moves.as_slice(), current.moves());
        assert_eq!(old.epsilon.as_slice(), current.epsilon());
    }
}

#[rustfmt::skip]
mod tests {
    use super::*;
    pub(super) use super::super::tests::note_actual_entry;
pub(super) fn automaton(function: &ResolvedFunction) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::new();
    let mut automaton = Automaton::default();
    let sink = automaton.node(function, &mut budget).unwrap();
    let mut roots = Vec::new();
    for terminal in [
        SkeletonTerminal::Success,
        SkeletonTerminal::Failure,
        SkeletonTerminal::LoopLatch,
    ] {
        roots.push(
            automaton
                .end(function, &mut budget, sink, terminal)
                .unwrap(),
        );
    }
    let duplicate = automaton.node(function, &mut budget).unwrap();
    for _ in 0..2 {
        automaton
            .epsilon(function, &mut budget, duplicate, roots[0])
            .unwrap();
    }
    roots.push(duplicate);
    for targets in [[roots[0], roots[1]], [roots[1], roots[0]]] {
        let root = automaton.node(function, &mut budget).unwrap();
        for target in targets {
            automaton
                .epsilon(function, &mut budget, root, target)
                .unwrap();
        }
        roots.push(root);
    }
    let cycle = automaton.node(function, &mut budget).unwrap();
    automaton
        .movement(
            function,
            &mut budget,
            cycle,
            Label::End(SkeletonTerminal::Success),
            cycle,
        )
        .unwrap();
    roots.push(cycle);
    (automaton, sink, roots)
}
}

#[path = "matched_receiving_layers/memo.rs"]
mod memo;
#[path = "matched_receiving_layers/successor_targets.rs"]
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
                self.closures.insert(function, node, budget, || shared)?;
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

#[rustfmt::skip]
mod before_receiving_layers {
//! Paid immutable weighted backing for the ordinary canonical receiver.
//! Frames, open states and completed memo keys share only owned immutable
//! vectors. Label order, route weights, signature algebra and work debits keep
//! the complete original receiver in canonical_before.rs as an independent control.
use super::{
    Automaton, BTreeMap, BTreeSet, Diagnostic, Label, Rc, ReplayBudget, ResolvedFunction, Weighted,
    physical_clone, replay_error,
};

use super::{app_census, epsilon_alias, memo, successor_targets, tests};

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
pub(in crate::cleanup_plan::replay::factored::canonical_shared) struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: memo::Memo<Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<SharedWeighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> Canonicalizer<'a> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn new(automaton: &'a Automaton, sink: usize) -> Self {
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
                self.closures.insert(function, node, budget, || shared)?;
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
                    let targets =
                        successor_targets::Targets::one(function, budget, *child, *weight)?;
                    successors.insert(label.clone(), targets);
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
                        successor_targets::Targets::One { .. } => {
                            targets.promote(function, budget, *child, *weight)?;
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
            result.push((label, targets.finish(function, budget)?));
        }
        Ok(Some((accepted, result)))
    }

    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn id(
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


impl Canonicalizer<'_> {
    #[allow(clippy::type_complexity, reason = "retained complete canonical memo signature tuple for independent receiver controls")]
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn signatures(&self) -> &BTreeMap<(u64, Vec<(Label, usize)>), usize> { &self.ids }
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn states(&self) -> &BTreeMap<SharedWeighted, usize> { &self.states }
}

}
pub(super) use before_receiving_layers::Canonicalizer as BeforeCanonicalizer;
