//! Factored typed-control skeleton comparison.
//!
//! The enumerating comparison materializes every terminal path of the typed
//! HIR and of the cleanup CFG and compares the sorted multisets. Independent
//! decisions in sequence multiply that count (2^N for N independent `if`s,
//! `&&` operands, or scalar `match`es), although a decision that leaves the
//! cleanup state unchanged contributes nothing a single path would not.
//!
//! This module decides the same multiset equality without enumerating the
//! product:
//!
//! * The typed-HIR skeleton is derived by the unchanged walk with path-state
//!   merging enabled: whenever paths are sequenced, paths whose cleanup state
//!   (owned source, failure, residual, and call commit list) is identical are
//!   merged into one path carrying the union of their observation sequences.
//!   Every later skeleton step reads only that state, so the merged walk
//!   denotes exactly the enumerated paths, and its size follows the number of
//!   distinct cleanup states rather than the number of decision combinations.
//! * Both the merged HIR paths and the cleanup CFG become acyclic automata
//!   over skeleton observations, each accepting sequence ending in one
//!   terminal label.
//! * Bottom-up hash-consing of the weighted subset construction assigns two
//!   states the same id exactly when they accept every sequence the same
//!   number of times, so equal roots prove equal path multisets.
use super::skeleton_work::ObservationDag;
use super::*;
use std::rc::Rc;

/// Path count above which the factored comparison replaces enumeration.
const FACTORED_PATH_THRESHOLD: usize = 4_096;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Label {
    Observation(Rc<SkeletonObservation>),
    End(SkeletonTerminal),
}

#[derive(Default)]
struct Node {
    moves: Vec<(Label, usize)>,
    epsilon: Vec<usize>,
}

/// `true` when this function's typed-control skeleton is compared factored.
/// A cyclic CFG saturates the path census and keeps the census diagnostic.
pub(super) fn selected(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
) -> bool {
    (cfg_paths > FACTORED_PATH_THRESHOLD || semantic_paths > FACTORED_PATH_THRESHOLD)
        && validate_reachable_acyclic_cfg(function).is_ok()
}

/// Run the factored comparison when it is selected. `Ok(true)` means the
/// skeleton is authenticated; `Ok(false)` leaves the enumerating comparison
/// in charge.
pub(super) fn validate(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let cfg_paths = branch_sensitive_cfg_bounds(function)?.terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function)?;
    if !selected(function, cfg_paths, semantic_paths) {
        #[cfg(test)]
        cross_check(program, function);
        return Ok(false);
    }
    let saved = (budget.remaining, budget.skeleton_remaining);
    match compare(program, function, budget) {
        Ok(Some(true)) => Ok(true),
        Ok(Some(false)) => Err(skeleton_mismatch(function)),
        verdict => {
            if cfg_paths <= MAX_REPLAY_PATHS && semantic_paths <= MAX_REPLAY_PATHS {
                // Enumeration still fits: let it decide with the full budget.
                (budget.remaining, budget.skeleton_remaining) = saved;
                return Ok(false);
            }
            match verdict {
                Err(error) if !error.message.contains("budget") => Err(error),
                _ => path_summary::path_budget_errors(function, cfg_paths, semantic_paths)
                    .map(|()| false),
            }
        }
    }
}

fn skeleton_mismatch(function: &ResolvedFunction) -> Diagnostic {
    replay_error(
        function,
        "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
    )
}

/// `Some(equal)`, or `None` when a path multiplicity overflows.
pub(super) fn compare(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    // Lend the remaining program-wide budget to the merged skeleton walk.
    let skeleton_reserve = budget.skeleton_remaining;
    budget.skeleton_remaining = std::mem::take(&mut budget.remaining);
    budget.merge_paths = true;
    let hir_paths = hir_skeleton_paths(program, function, budget);
    budget.merge_paths = false;
    budget.remaining = std::mem::replace(&mut budget.skeleton_remaining, skeleton_reserve);
    let hir_paths = hir_paths?;

    let mut automaton = Automaton::default();
    let sink = automaton.node();
    let success = automaton.end(sink, SkeletonTerminal::Success);
    let failure = automaton.end(sink, SkeletonTerminal::Failure);
    let hir_root = automaton.node();
    let mut memo = BTreeMap::new();
    for path in &hir_paths {
        let end = automaton.add_observations(
            function,
            &path.observations,
            hir_root,
            &mut memo,
            budget,
        )?;
        let terminal = match path.terminal {
            SkeletonTerminal::Success => success,
            SkeletonTerminal::Failure => failure,
        };
        automaton.nodes[end].epsilon.push(terminal);
    }
    let plan_root = automaton.add_plan(function, success, failure, budget)?;
    let mut canonical = Canonicalizer::new(&automaton, sink);
    let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
        return Ok(None);
    };
    let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}

#[derive(Default)]
struct Automaton {
    nodes: Vec<Node>,
}

enum BuildFrame<'d> {
    Visit(&'d Rc<ObservationDag>, usize),
    Seq(&'d Rc<ObservationDag>, usize),
    Cat(&'d Rc<ObservationDag>, usize, usize),
    Alt(&'d Rc<ObservationDag>, usize, usize),
}

impl Automaton {
    fn node(&mut self) -> usize {
        self.nodes.push(Node::default());
        self.nodes.len() - 1
    }

    fn end(&mut self, sink: usize, terminal: SkeletonTerminal) -> usize {
        let node = self.node();
        self.nodes[node].moves.push((Label::End(terminal), sink));
        node
    }

    fn chain(&mut self, mut end: usize, items: &[Rc<SkeletonObservation>]) -> usize {
        for item in items {
            let next = self.node();
            self.nodes[end]
                .moves
                .push((Label::Observation(item.clone()), next));
            end = next;
        }
        end
    }

    /// Build `observations` forward from `start` and return its end node.
    /// A shared DAG node is built once per start node, so a merged prefix
    /// that many later paths extend exists once in the automaton.
    fn add_observations(
        &mut self,
        function: &ResolvedFunction,
        observations: &Observations,
        start: usize,
        memo: &mut BTreeMap<(usize, usize), usize>,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let (prefix, items) = observations.parts();
        let mut end = start;
        if let Some(prefix) = prefix {
            end = self.add_dag(function, prefix, start, memo, budget)?;
        }
        budget.charge(
            function,
            items.len().saturating_add(1),
            "factored HIR automaton",
        )?;
        Ok(self.chain(end, items))
    }

    fn add_dag(
        &mut self,
        function: &ResolvedFunction,
        root: &Rc<ObservationDag>,
        start: usize,
        memo: &mut BTreeMap<(usize, usize), usize>,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let key = |dag: &Rc<ObservationDag>, start: usize| (Rc::as_ptr(dag) as usize, start);
        let mut frames = vec![BuildFrame::Visit(root, start)];
        let mut ends = Vec::<usize>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                BuildFrame::Visit(dag, start) => {
                    budget.charge(function, 1, "factored HIR automaton")?;
                    if let Some(end) = memo.get(&key(dag, start)) {
                        ends.push(*end);
                        continue;
                    }
                    match dag.as_ref() {
                        ObservationDag::Seq(prefix, _) => {
                            frames.push(BuildFrame::Seq(dag, start));
                            match prefix {
                                Some(prefix) => frames.push(BuildFrame::Visit(prefix, start)),
                                None => ends.push(start),
                            }
                        }
                        ObservationDag::Cat(_) => {
                            ends.push(start);
                            frames.push(BuildFrame::Cat(dag, start, 0));
                        }
                        ObservationDag::Alt(members) => {
                            frames.push(BuildFrame::Alt(dag, start, members.len()));
                            for member in members {
                                frames.push(BuildFrame::Visit(member, start));
                            }
                        }
                    }
                }
                BuildFrame::Seq(dag, start) => {
                    let ObservationDag::Seq(_, items) = dag.as_ref() else {
                        unreachable!("sequence frame holds a sequence node");
                    };
                    let prefix_end = ends.pop().expect("sequence prefix end retained");
                    budget.charge(function, items.len(), "factored HIR automaton")?;
                    let end = self.chain(prefix_end, items);
                    memo.insert(key(dag, start), end);
                    ends.push(end);
                }
                BuildFrame::Cat(dag, start, index) => {
                    let ObservationDag::Cat(parts) = dag.as_ref() else {
                        unreachable!("concatenation frame holds a concatenation node");
                    };
                    let end = ends.pop().expect("concatenation part end retained");
                    match parts.get(index) {
                        Some(part) => {
                            frames.push(BuildFrame::Cat(dag, start, index + 1));
                            frames.push(BuildFrame::Visit(part, end));
                        }
                        None => {
                            memo.insert(key(dag, start), end);
                            ends.push(end);
                        }
                    }
                }
                BuildFrame::Alt(dag, start, count) => {
                    let join = self.node();
                    for _ in 0..count {
                        let end = ends.pop().expect("union member end retained");
                        self.nodes[end].epsilon.push(join);
                    }
                    memo.insert(key(dag, start), join);
                    ends.push(join);
                }
            }
        }
        Ok(ends.pop().expect("factored DAG build produced an end"))
    }

    fn add_plan(
        &mut self,
        function: &ResolvedFunction,
        success: usize,
        failure: usize,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let plan = &function.cleanup_plan;
        budget.charge(
            function,
            plan_structure_units(plan),
            "factored cleanup-plan automaton",
        )?;
        let entries = plan.blocks.iter().map(|_| self.node()).collect::<Vec<_>>();
        let entry = |block: BlockId| entries[block.0 as usize];
        for (index, block) in plan.blocks.iter().enumerate() {
            let mut node = entries[index];
            for transition in &block.transitions {
                if let Some(observation) = transition_observation(transition) {
                    node = self.chain(node, &[Rc::new(observation)]);
                }
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    let edge = &plan.edges[edge.0 as usize];
                    let to = entry(edge.to);
                    if let EdgeCondition::VariantCase { .. } = &edge.condition {
                        let observation = edge_observation(function, &edge.condition)?;
                        self.nodes[node]
                            .moves
                            .push((Label::Observation(Rc::new(observation)), to));
                    } else {
                        self.nodes[node].epsilon.push(to);
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    for edge in edges {
                        let edge = &plan.edges[edge.0 as usize];
                        let observation = edge_observation(function, &edge.condition)?;
                        self.nodes[node]
                            .moves
                            .push((Label::Observation(Rc::new(observation)), entry(edge.to)));
                    }
                }
                CleanupTerminator::Exit(exit) => {
                    let exit = &plan.exits[exit.0 as usize];
                    let target = match exit.continuation {
                        ExitContinuation::Continue(edge) => entry(plan.edges[edge.0 as usize].to),
                        ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                            success
                        }
                        ExitContinuation::ReturnFailure { .. } => failure,
                    };
                    self.nodes[node].epsilon.push(target);
                }
            }
        }
        Ok(entry(plan.entry))
    }
}

/// The skeleton observation one cleanup transition contributes, exactly as
/// the enumerating `plan_skeleton_paths` records it.
fn transition_observation(transition: &CleanupTransition) -> Option<SkeletonObservation> {
    Some(match transition {
        CleanupTransition::Initialize { at, destination }
        | CleanupTransition::InitializeVariant {
            at, destination, ..
        } => SkeletonObservation::Initialize {
            at: at.clone(),
            destination: destination.clone(),
        },
        CleanupTransition::ReserveRenewal { at, binding } => SkeletonObservation::ReserveRenewal {
            at: at.clone(),
            binding: binding.clone(),
        },
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        }
        | CleanupTransition::Renew {
            at,
            source,
            destination,
        }
        | CleanupTransition::TransferVariant {
            at,
            source,
            destination,
            ..
        } => SkeletonObservation::Transfer {
            at: at.clone(),
            source: source.clone(),
            destination: destination.clone(),
        },
        CleanupTransition::CallCommit { call, arguments } => SkeletonObservation::CallCommit {
            call: call.clone(),
            arguments: arguments
                .iter()
                .map(|argument| (argument.parameter_index, argument.source.clone()))
                .collect(),
        },
        CleanupTransition::StageCopyResult { source } => {
            SkeletonObservation::StageCopyResult(source.clone())
        }
        CleanupTransition::AuthenticateVariantCase { .. }
        | CleanupTransition::SelectFailure { .. } => return None,
    })
}

/// The skeleton observation one edge condition contributes, exactly as the
/// enumerating `plan_skeleton_paths` records it.
fn edge_observation(
    function: &ResolvedFunction,
    condition: &EdgeCondition,
) -> Result<SkeletonObservation, Diagnostic> {
    Ok(match condition {
        EdgeCondition::BooleanResult(expression, value) => SkeletonObservation::Boolean {
            expression: expression.clone(),
            value: *value,
        },
        EdgeCondition::VariantCase {
            scrutinee,
            case,
            matches,
        } => SkeletonObservation::VariantCase {
            scrutinee: scrutinee.clone(),
            case: case.clone(),
            matches: *matches,
        },
        EdgeCondition::ArmSelected {
            scrutinee,
            arm,
            selected,
        } => SkeletonObservation::ArmSelected {
            scrutinee: scrutinee.clone(),
            arm: *arm,
            selected: *selected,
        },
        EdgeCondition::StatusZero(source) => SkeletonObservation::Status {
            source: source.clone(),
            success: true,
        },
        EdgeCondition::StatusNonzero(source) => SkeletonObservation::Status {
            source: source.clone(),
            success: false,
        },
        EdgeCondition::Always => {
            return Err(replay_error(
                function,
                "branch skeleton contains an unconditional edge",
            ));
        }
    })
}

/// A weighted set of automaton nodes: each node with the number of distinct
/// automaton paths that reach it on one observation sequence.
type Weighted = Vec<(usize, u64)>;

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
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                stack.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            let mut closure = BTreeMap::new();
            if !state.moves.is_empty() || node == self.sink {
                closure.insert(node, 1_u64);
            }
            for target in &state.epsilon {
                let Some(inner) = self.closures.get(target) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
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
                let slot = successors
                    .entry(label.clone())
                    .or_default()
                    .entry(*child)
                    .or_insert(0);
                let Some(sum) = slot.checked_add(*weight) else {
                    return Ok(None);
                };
                *slot = sum;
            }
        }
        Ok(Some((
            accepted,
            successors
                .into_iter()
                .map(|(label, targets)| (label, targets.into_iter().collect()))
                .collect(),
        )))
    }

    fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        let mut frames = vec![CanonicalFrame::Enter(root.clone())];
        let mut open = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if !open.insert(state.clone()) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let children = successors
                        .iter()
                        .map(|(_, child)| CanonicalFrame::Enter(child.clone()))
                        .collect::<Vec<_>>();
                    frames.push(CanonicalFrame::Exit(state, accepted, successors));
                    frames.extend(children);
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(&state);
                    let mut key = Vec::with_capacity(successors.len());
                    for (label, child) in successors {
                        let Some(id) = self.states.get(&child) else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let next = self.ids.len();
                    let id = *self.ids.entry((accepted, key)).or_insert(next);
                    self.states.insert(state, id);
                }
            }
        }
        Ok(self.states.get(&root).copied())
    }
}

/// Test builds compare the factored verdict with the enumerating one on every
/// function small enough to enumerate.
#[cfg(test)]
fn cross_check(program: &ResolvedProgram, function: &ResolvedFunction) {
    let saved = SKELETON_MATERIALIZATIONS.with(Cell::get);
    let mut enumerating = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let enumerated = hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {
        let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
        hir.sort();
        plan.sort();
        Ok(hir == plan)
    });
    if let Ok(true) = enumerated {
        let mut factored = ReplayBudget::with_skeleton_limit(0);
        let verdict = compare(program, function, &mut factored);
        assert!(
            matches!(verdict, Ok(Some(true))),
            "factored skeleton comparison disagrees with enumeration for `{}`: {:?}",
            function.id,
            verdict.map_err(|error| error.message)
        );
    }
    SKELETON_MATERIALIZATIONS.with(|count| count.set(saved));
}
