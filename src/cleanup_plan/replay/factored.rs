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
//!   Lexical functions restrict those merges to independently identified Copy
//!   expression scopes; every abrupt/failure/owning state remains separate.
//! * Both the merged HIR paths and the cleanup CFG become acyclic automata
//!   over skeleton observations, each accepting sequence ending in one
//!   terminal label.
//! * Bottom-up hash-consing of the weighted subset construction assigns two
//!   states the same id exactly when they accept every sequence the same
//!   number of times, so equal roots prove equal path multisets.
use super::skeleton_work::{ObservationDag, ObservationDagKind};
use super::*;
use std::rc::Rc;
#[cfg(test)]
mod before_closure_invocation;
#[cfg(test)]
mod borrowed_canonical;
#[cfg(test)]
mod canonical_before;
mod canonical_shared;
mod compact_nodes;
pub(super) mod copy_regions;
mod dag_memo;
#[cfg(test)]
mod instrumentation;
mod storage;
#[cfg(test)]
use canonical_before::Receiver as Canonicalizer;
use compact_nodes::Node;
use storage::{memo_insert, push_end, push_frame, reserve_transition_observation};

/// Path count above which the factored comparison replaces enumeration.
const FACTORED_PATH_THRESHOLD: usize = 4_096;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Label {
    Observation(Rc<SkeletonObservation>),
    End(SkeletonTerminal),
}

/// Semantic nonselection remains false; a physical scratch refusal is sticky
/// and must reach the caller instead of becoming enumerating-route authority.
pub(super) fn selected_paid(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if cfg_paths <= FACTORED_PATH_THRESHOLD && semantic_paths <= FACTORED_PATH_THRESHOLD {
        return Ok(false);
    }
    match structural_control::validate_ordinary_cfg(function, budget) {
        Ok(()) => Ok(true),
        Err(error) if budget.refusal_spent || crate::bounded_output::active_refused() => Err(error),
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
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
#[cfg(test)]
pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    validate_with_index(program, function, budget, None)
}
pub(super) fn validate_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<bool, Diagnostic> {
    validate_with_index_storage(program, function, budget, helpers, None)
}
pub(super) fn validate_with_index_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<bool, Diagnostic> {
    let cfg_paths = branch_sensitive_cfg_bounds(function)?.terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function)?;
    if !selected_paid(
        function,
        cfg_paths,
        semantic_paths,
        &mut ReplayBudget::new(),
    )? {
        #[cfg(test)]
        oracle_hook::cross_check_with_storage(program, function, budget, continuations)?;
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
        Ok(Some(true)) => Ok(true),
        Ok(Some(false)) => Err(skeleton_mismatch(function)),
        verdict => {
            if cfg_paths <= MAX_REPLAY_PATHS && semantic_paths <= MAX_REPLAY_PATHS {
                // Enumeration still fits; an attempted factored comparison
                // keeps every work and physical debit already spent.
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
#[cfg(test)]
pub(super) fn compare(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    compare_with_index(program, function, budget, None)
}
pub(super) fn compare_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<Option<bool>, Diagnostic> {
    compare_with_index_storage(program, function, budget, helpers, None)
}
fn compare_with_index_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<Option<bool>, Diagnostic> {
    let BuiltComparison {
        automaton,
        sink,
        hir_root,
        plan_root,
    } = build_comparison(program, function, budget, helpers, continuations)?;

    #[cfg(test)]
    if budget.oracle_lease().is_some() {
        // The exact reconstructed automaton is unchanged. An Oracle account
        // pays the full general label order; it grants no borrowed-helper lease.
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "oracle plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        lease.validate(program, function, budget)?;
        let mut canonical =
            borrowed_canonical::BorrowedCanonicalizer::new(function, &automaton, sink, budget)?;
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed HIR weighted root")?;
        let Some(hir) = canonical.id(function, vec![(hir_root, 1)], budget)? else {
            return Ok(None);
        };
        budget.reserve_array::<(usize, u64)>(function, 1, "borrowed plan weighted root")?;
        let Some(plan) = canonical.id(function, vec![(plan_root, 1)], budget)? else {
            return Ok(None);
        };
        return Ok(Some(hir == plan));
    }

    let mut canonical = canonical_shared::Canonicalizer::new(&automaton, sink);
    if budget.pays_allocations() {
        budget.reserve_bytes(
            function,
            canonical_shared::closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
            "factored held closure invocation controls",
        )?;
    }
    let mut receiving =
        canonical_shared::closure_receiving::ClosureReceiving::from_prepaid(function, budget);
    budget.reserve_array::<(usize, u64)>(function, 1, "factored HIR weighted root")?;
    let Some(hir) =
        canonical.id_with_receiving(function, vec![(hir_root, 1)], budget, &mut receiving)?
    else {
        return Ok(None);
    };
    budget.reserve_array::<(usize, u64)>(function, 1, "factored plan weighted root")?;
    let Some(plan) =
        canonical.id_with_receiving(function, vec![(plan_root, 1)], budget, &mut receiving)?
    else {
        return Ok(None);
    };
    Ok(Some(hir == plan))
}

struct BuiltComparison {
    automaton: Automaton,
    sink: usize,
    hir_root: usize,
    plan_root: usize,
}

/// Reconstruct both exact observation languages once, retaining every debit.
/// The exact strategy census borrows this same physical owner; it never
/// retries construction after choosing a representation.
fn build_comparison<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<BuiltComparison, Diagnostic> {
    // Both independent constructors debit skeleton helpers as well as ordinary
    // constructors. Keep one active allowance through the HIR walk AND plan
    // automaton construction; the held program reservation belongs to other
    // enumerated functions and cannot pay for this deferred attempt.
    #[cfg(test)]
    instrumentation::begin(function, budget);
    let skeleton_reserve = budget.skeleton_remaining;
    budget.skeleton_remaining = std::mem::take(&mut budget.remaining);
    budget.lent_skeleton = true;
    let result = (|| {
        budget.merge_paths = true;
        let hir_paths = hir_skeleton_paths_with_index_storage(
            program,
            function,
            budget,
            helpers,
            continuations,
        );
        budget.merge_paths = false;
        #[cfg(test)]
        instrumentation::phase(
            function,
            budget,
            "HIR skeleton",
            hir_paths.as_ref().map(|paths| paths.as_slice()).ok(),
        );
        let hir_paths = hir_paths?;

        let mut automaton = Automaton::default();
        let sink = automaton.node(function, budget)?;
        let success = automaton.end(function, budget, sink, SkeletonTerminal::Success)?;
        let failure = automaton.end(function, budget, sink, SkeletonTerminal::Failure)?;
        let latch = automaton.end(function, budget, sink, SkeletonTerminal::LoopLatch)?;
        let hir_root = automaton.node(function, budget)?;
        let mut memo = dag_memo::new(program, function, budget)?;
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
                SkeletonTerminal::LoopLatch => latch,
            };
            automaton.epsilon(function, budget, end, terminal)?;
        }
        #[cfg(test)]
        instrumentation::phase(function, budget, "HIR automaton", None);
        let plan_root = automaton.add_plan(function, success, failure, latch, budget)?;
        #[cfg(test)]
        instrumentation::phase(function, budget, "plan automaton", None);
        Ok(BuiltComparison {
            automaton,
            sink,
            hir_root,
            plan_root,
        })
    })();
    // Restore only the unused allowance. Successful debits from either
    // constructor remain spent, including an attempt ending in refusal.
    budget.merge_paths = false;
    budget.lent_skeleton = false;
    budget.remaining = std::mem::replace(&mut budget.skeleton_remaining, skeleton_reserve);
    #[cfg(test)]
    instrumentation::phase(
        function,
        budget,
        if result.is_ok() {
            "constructed"
        } else {
            "constructor refusal"
        },
        None,
    );
    result
}

#[path = "factored/strategy.rs"]
pub(super) mod strategy;

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
        if matches!(self.nodes[from], Node::Empty) {
            // Existing incoming tuple remains; only the enclosing inline enum
            // is a new initializer. Payment replaces the old reserve_vec call.
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Node>(),
                "factored automaton moves",
            )?;
            self.nodes[from] = Node::Movement((label, to));
        } else {
            if !matches!(self.nodes[from], Node::Many(_)) {
                budget.reserve_bytes(
                    function,
                    Node::PROMOTION_CONTROLS,
                    "factored compact node promotion controls",
                )?;
                self.nodes[from].promote(function, budget, true)?;
            } else {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Result<(), Diagnostic>>()
                        + 2 * std::mem::size_of::<&mut Box<compact_nodes::Many>>()
                        + 2 * std::mem::size_of::<&mut compact_nodes::Many>(),
                    "factored compact many receiving controls",
                )?;
                let Node::Many(many) = &mut self.nodes[from] else {
                    unreachable!()
                };
                budget.reserve_vec(function, &mut many.moves, 1, "factored automaton moves")?;
            }
            let Node::Many(many) = &mut self.nodes[from] else {
                unreachable!()
            };
            many.moves.push((label, to));
        }
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
        if matches!(self.nodes[from], Node::Empty) {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Node>(),
                "factored automaton epsilon moves",
            )?;
            self.nodes[from] = Node::Epsilon(to);
        } else {
            if !matches!(self.nodes[from], Node::Many(_)) {
                budget.reserve_bytes(
                    function,
                    Node::PROMOTION_CONTROLS,
                    "factored compact node promotion controls",
                )?;
                self.nodes[from].promote(function, budget, false)?;
            } else {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Result<(), Diagnostic>>()
                        + 2 * std::mem::size_of::<&mut Box<compact_nodes::Many>>()
                        + 2 * std::mem::size_of::<&mut compact_nodes::Many>(),
                    "factored compact many receiving controls",
                )?;
                let Node::Many(many) = &mut self.nodes[from] else {
                    unreachable!()
                };
                budget.reserve_vec(
                    function,
                    &mut many.epsilon,
                    1,
                    "factored automaton epsilon moves",
                )?;
            }
            let Node::Many(many) = &mut self.nodes[from] else {
                unreachable!()
            };
            many.epsilon.push(to);
        }
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

    fn chain(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        mut end: usize,
        items: &[Rc<SkeletonObservation>],
    ) -> Result<usize, Diagnostic> {
        for item in items {
            let next = self.node(function, budget)?;
            self.movement(
                function,
                budget,
                end,
                Label::Observation(item.clone()),
                next,
            )?;
            end = next;
        }
        Ok(end)
    }

    /// Build `observations` forward from `start` and return its end node.
    /// A shared DAG node is built once per start node, so a merged prefix
    /// that many later paths extend exists once in the automaton.
    fn add_observations(
        &mut self,
        function: &ResolvedFunction,
        observations: &Observations,
        start: usize,
        memo: &mut dag_memo::Memo,
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
        self.chain(function, budget, end, items)
    }

    fn add_dag(
        &mut self,
        function: &ResolvedFunction,
        root: &Rc<ObservationDag>,
        start: usize,
        memo: &mut dag_memo::Memo,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let key = dag_memo::key;
        budget.reserve_array::<BuildFrame<'_>>(function, 1, "factored DAG initial frame")?;
        let mut frames = vec![BuildFrame::Visit(root, start)];
        let mut ends = Vec::<usize>::new();
        while let Some(frame) = frames.pop() {
            match frame {
                BuildFrame::Visit(dag, start) => {
                    budget.charge(function, 1, "factored HIR automaton")?;
                    let retained = dag_memo::get(function, memo, &key(dag, start), budget)?;
                    #[cfg(test)]
                    instrumentation::visit(dag, retained.is_some());
                    if let Some(end) = retained {
                        push_end(function, budget, &mut ends, end)?;
                        continue;
                    }
                    match &dag.kind {
                        ObservationDagKind::Seq(prefix, _) => {
                            push_frame(function, budget, &mut frames, BuildFrame::Seq(dag, start))?;
                            match prefix {
                                Some(prefix) => push_frame(
                                    function,
                                    budget,
                                    &mut frames,
                                    BuildFrame::Visit(prefix, start),
                                )?,
                                None => push_end(function, budget, &mut ends, start)?,
                            }
                        }
                        ObservationDagKind::Cat(_) => {
                            push_end(function, budget, &mut ends, start)?;
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Cat(dag, start, 0),
                            )?;
                        }
                        ObservationDagKind::Alt(members) => {
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Alt(dag, start, members.len()),
                            )?;
                            for member in members {
                                push_frame(
                                    function,
                                    budget,
                                    &mut frames,
                                    BuildFrame::Visit(member, start),
                                )?;
                            }
                        }
                    }
                }
                BuildFrame::Seq(dag, start) => {
                    let ObservationDagKind::Seq(_, items) = &dag.kind else {
                        unreachable!("sequence frame holds a sequence node");
                    };
                    let prefix_end = ends.pop().expect("sequence prefix end retained");
                    budget.charge(function, items.len(), "factored HIR automaton")?;
                    let end = self.chain(function, budget, prefix_end, items)?;
                    memo_insert(function, budget, memo, key(dag, start), end)?;
                    push_end(function, budget, &mut ends, end)?;
                }
                BuildFrame::Cat(dag, start, index) => {
                    let ObservationDagKind::Cat(parts) = &dag.kind else {
                        unreachable!("concatenation frame holds a concatenation node");
                    };
                    let end = ends.pop().expect("concatenation part end retained");
                    match parts.get(index) {
                        Some(part) => {
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Cat(dag, start, index + 1),
                            )?;
                            push_frame(
                                function,
                                budget,
                                &mut frames,
                                BuildFrame::Visit(part, end),
                            )?;
                        }
                        None => {
                            memo_insert(function, budget, memo, key(dag, start), end)?;
                            push_end(function, budget, &mut ends, end)?;
                        }
                    }
                }
                BuildFrame::Alt(dag, start, count) => {
                    let join = self.node(function, budget)?;
                    for _ in 0..count {
                        let end = ends.pop().expect("union member end retained");
                        self.epsilon(function, budget, end, join)?;
                    }
                    memo_insert(function, budget, memo, key(dag, start), join)?;
                    push_end(function, budget, &mut ends, join)?;
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
        latch: usize,
        budget: &mut ReplayBudget,
    ) -> Result<usize, Diagnostic> {
        let plan = &function.cleanup_plan;
        budget.charge(
            function,
            plan_structure_units(plan),
            "factored cleanup-plan automaton",
        )?;
        budget.reserve_array::<usize>(
            function,
            plan.blocks.len(),
            "factored plan block entries",
        )?;
        let mut entries = Vec::with_capacity(plan.blocks.len());
        for _ in &plan.blocks {
            entries.push(self.node(function, budget)?);
        }
        let entry = |block: BlockId| entries[block.0 as usize];
        for (index, block) in plan.blocks.iter().enumerate() {
            let mut node = entries[index];
            for transition in &block.transitions {
                if let Some(observation) = transition_observation(function, budget, transition)? {
                    let next = self.node(function, budget)?;
                    self.observation(function, budget, node, observation, next)?;
                    node = next;
                }
            }
            match &block.terminator {
                CleanupTerminator::Goto(edge) => {
                    let edge = &plan.edges[edge.0 as usize];
                    let to = entry(edge.to);
                    if let EdgeCondition::VariantCase { .. } = &edge.condition {
                        let observation = edge_observation(function, budget, &edge.condition)?;
                        self.observation(function, budget, node, observation, to)?;
                    } else {
                        self.epsilon(function, budget, node, to)?;
                    }
                }
                CleanupTerminator::Branch(edges) => {
                    for edge in edges {
                        let edge = &plan.edges[edge.0 as usize];
                        let observation = edge_observation(function, budget, &edge.condition)?;
                        self.observation(function, budget, node, observation, entry(edge.to))?;
                    }
                }
                CleanupTerminator::Exit(exit) => {
                    let exit = &plan.exits[exit.0 as usize];
                    let target = match &exit.continuation {
                        ExitContinuation::Continue(edge) => entry(plan.edges[edge.0 as usize].to),
                        ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                            success
                        }
                        ExitContinuation::ReturnFailure { .. } => failure,
                        ExitContinuation::LoopBreak { edge, .. }
                        | ExitContinuation::FunctionReturn { edge, .. } => {
                            let marker =
                                lexical_skeleton::plan_exit_observation(budget, function, exit.id)?
                                    .ok_or_else(|| {
                                        replay_error(
                                    function,
                                    "factored lexical exit lacks an exact certificate marker",
                                )
                                    })?;
                            // The marker itself is the labelled continuation edge.
                            // A lexical exit must never become an epsilon edge.
                            self.observation(
                                function,
                                budget,
                                node,
                                marker,
                                entry(plan.edges[edge.0 as usize].to),
                            )?;
                            continue;
                        }
                        ExitContinuation::LoopLatch { target } => {
                            let marker = match lexical_skeleton::plan_exit_observation(
                                budget, function, exit.id,
                            )? {
                                Some(marker) => marker,
                                None => SkeletonObservation::LoopLatch {
                                    target: skeleton_clone(
                                        budget,
                                        function,
                                        target,
                                        "factored loop-latch target clone",
                                    )?,
                                },
                            };
                            let target =
                                lexical_cert::latch_resume(plan, exit.id).map_or(latch, entry);
                            self.observation(function, budget, node, marker, target)?;
                            continue;
                        }
                    };
                    self.epsilon(function, budget, node, target)?;
                }
            }
        }
        Ok(entry(plan.entry))
    }
}

/// The skeleton observation one cleanup transition contributes, exactly as
/// the enumerating `plan_skeleton_paths` records it.
fn transition_observation(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    transition: &CleanupTransition,
) -> Result<Option<SkeletonObservation>, Diagnostic> {
    reserve_transition_observation(function, budget, transition)?;
    Ok(Some(match transition {
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
            SkeletonObservation::StageCopyResult(CopySource::new(function, budget, source.clone())?)
        }
        CleanupTransition::AuthenticateVariantCase { .. }
        | CleanupTransition::SelectFailure { .. } => return Ok(None),
    }))
}

/// The skeleton observation one edge condition contributes, exactly as the
/// enumerating `plan_skeleton_paths` records it.
fn edge_observation(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    condition: &EdgeCondition,
) -> Result<SkeletonObservation, Diagnostic> {
    if let EdgeCondition::VariantCase { case, .. } = condition {
        budget.reserve_clone(function, case, "factored edge case identity")?;
    }
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

#[cfg(test)]
enum CanonicalFrame {
    Enter(Weighted),
    Exit(Weighted, u64, Vec<(Label, Weighted)>),
}

/// Test builds compare the factored verdict with the enumerating one on every
/// function small enough to enumerate.
#[cfg(test)]
fn cross_check(program: &dyn crate::cleanup::ReplayContext, function: &ResolvedFunction) {
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

// Pending oracle receiver is exercised only by its explicit private tests.
// The live hook above remains the complete original ordinary receiver.
#[cfg(test)]
mod oracle_hook;

#[cfg(test)]
#[path = "factored/lexical_plan_tests.rs"]
mod lexical_plan_tests;

#[cfg(test)]
mod continuation_tests {
    use super::super::continuation_storage::{Frame, Storage};
    use super::*;
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    #[test]
    fn ordinary_factored_functions_and_contracts_reuse_exact_program_storage_with_old_automata_work_and_drop(
    ) {
        let decisions = (0..13)
            .map(|index| format!("let branch_{index}=if 0=={index} {{1}}else{{2}};"))
            .collect::<String>();
        let source = format!("module scratch.factored; @id(\"first\") fn first()->i64 requires 0==0 ensures result==42 {{{decisions}42}} @id(\"second\") fn second()->i64 requires 0==0 ensures result==42 {{{decisions}42}} @id(\"main\") fn main()->i64 requires 0==0 ensures result==42 {{{decisions}42}}");
        let program =
            crate::hir::resolve(&crate::check(&source, "factored-storage.spx").unwrap()).unwrap();
        let mut storage = Storage::new(&program);
        let mut old_budget = ReplayBudget::unobserved();
        let mut current_budget = ReplayBudget::unobserved();
        for function in &program.functions {
            assert!(selected(
                function,
                branch_sensitive_cfg_bounds(function)
                    .unwrap()
                    .terminal_paths,
                hir_terminal_path_bound(function).unwrap()
            ));
            let old = build_comparison(&program, function, &mut old_budget, None, None).unwrap();
            let current = build_comparison(
                &program,
                function,
                &mut current_budget,
                None,
                Some(&mut storage),
            )
            .unwrap();
            assert_eq!(
                (old.sink, old.hir_root, old.plan_root),
                (current.sink, current.hir_root, current.plan_root)
            );
            assert_eq!(old.automaton.nodes.len(), current.automaton.nodes.len());
            for (old, current) in old.automaton.nodes.iter().zip(&current.automaton.nodes) {
                assert_eq!(old.moves(), current.moves());
                assert_eq!(old.epsilon(), current.epsilon());
            }
            assert!(
                storage.frames.is_some(),
                "top-level continuation lease returned"
            );
            assert!(storage.frames.as_ref().unwrap().is_empty());
        }
        assert_eq!(old_budget.remaining, current_budget.remaining);
        assert_eq!(old_budget.skeleton_spent, current_budget.skeleton_spent);
        // Actual ordinary factored admission, rather than only a direct producer.
        for function in &program.functions {
            assert!(validate_with_index_storage(
                &program,
                function,
                &mut current_budget,
                None,
                Some(&mut storage)
            )
            .unwrap());
        }
        drop(storage);
        let observe = |limit, pooled| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let mut storage = Storage::new(&program);
                    let mut success = true;
                    for function in &program.functions {
                        let result = build_comparison(
                            &program,
                            function,
                            &mut budget,
                            None,
                            if pooled { Some(&mut storage) } else { None },
                        );
                        if result.is_err() {
                            success = false;
                            assert!(budget.reserve_bytes(function, 0, "retry").is_err());
                        }
                        drop(result);
                        if !success {
                            break;
                        }
                    }
                    assert!(
                        storage.frames.is_some(),
                        "top-level continuation lease returned"
                    );
                    drop(storage);
                    drop(budget);
                    success
                })
            })
        };
        let ((old_success, old), old_refused, old_charge, _) = observe(usize::MAX, false);
        let ((success, current), refused, required, _) = observe(usize::MAX, true);
        assert!(old_success && success && !old_refused && !refused);
        // Three functions each evaluate requires/body/ensures: nine original
        // independent 516-frame buffers, versus one retained invocation buffer.
        let removed = 8 * 516 * std::mem::size_of::<Frame<'_>>();
        assert_eq!(old.allocated - current.allocated, removed);
        assert_eq!(old.allocations - current.allocations, 8);
        assert_eq!(old_charge - required, removed);
        assert!(!old.invalid && !current.invalid);
        assert_eq!((old.live, current.live), (0, 0));
        assert_eq!(old.allocated, old.freed);
        assert_eq!(current.allocated, current.freed);
        let ((success, exact), refused, charged, _) = observe(required, true);
        assert!(success && !refused && charged == required && !exact.invalid && exact.live == 0);
        let ((success, short), refused, _, receipt) = observe(required - 1, true);
        assert!(!success && refused && receipt.is_some() && !short.invalid && short.live == 0);
        assert_eq!(short.allocated, short.freed);
    }
}

#[cfg(test)]
mod oracle_order;

#[cfg(test)]
mod continuation_receiver;

#[cfg(test)]
pub(crate) use canonical_shared::app_census::Scope as EpsilonAppCensus;
