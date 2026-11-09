//! Legacy PLAN prefix ownership; paid callers retain the complete flat constructor.
use super::*;
use skeleton_work::plan_prefix::{self as storage, History};
#[cfg(test)]
#[allow(dead_code, reason = "complete immediate-old PLAN constructor")]
mod before;
#[cfg(test)]
mod tests;

pub(super) const QUERY_CONTROLS: usize = 2 * std::mem::size_of::<bool>()
    + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>();

// Direct .iter() already formed the slice iterator. The restored &Vec
// IntoIterator adapter additionally receives its actual/formal Vec loan and
// returns/forwards the Iter; no per-next role or work is introduced.
const EDGE_ITERATOR_CONTROLS: usize = 2 * std::mem::size_of::<&Vec<EdgeId>>()
    + 2 * std::mem::size_of::<std::slice::Iter<'static, EdgeId>>()
    + 2 * std::mem::size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>();

#[rustfmt::skip]
pub(super) fn build(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    #[cfg(test)]
    let _allocation_phase =
        crate::test_allocation_observer::function_phase_guard("cleanup_plan::replay::plan_paths");
    #[cfg(feature = "unstable-workflow-profiling")]
    let _paid_phase = budget.paid_phase(PaidPhase::PlanPaths);

    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayPlanSkeleton,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayPlanSkeleton,
    );
    budget.reserve_bytes(function, storage::INVOCATION_CONTROLS, "PLAN prefix invocation receiving")?;
    let plan = &function.cleanup_plan;
    let mut queue = VecDeque::new();
    skeleton_queue_push(
        budget,
        function,
        &mut queue,
        (plan.entry, History::new()),
        "cleanup-plan root state push",
    )?;
    let mut paths = Vec::new();
    while let Some((block, mut observations)) = queue.pop_front() {
        let block = &plan.blocks[block.0 as usize];
        // Charge only work performed at this block. The previous charge used the entire accumulated observation length on every linear block, turning a
        // depth-D skewed conditional into artificial O(D^3) work. Observation history is copied only at a real branch, charged below immediately before
        // each clone.
        budget.charge_skeleton(
            function,
            block.transitions.len().saturating_add(1),
            "cleanup-plan skeleton expansion",
        )?;
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan initialize expression clone")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan initialize destination clone",
                    )?;
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Initialize { at, destination },
                        "plan initialize observation push",
                    )?;
                }
                CleanupTransition::ReserveRenewal { at, binding } => {
                    let observation = SkeletonObservation::ReserveRenewal {
                        at: skeleton_clone(budget, function, at, "renewal expression")?,
                        binding: skeleton_clone(budget, function, binding, "renewal binding")?,
                    };
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        observation,
                        "renewal reservation",
                    )?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                }
                | CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan transfer expression clone")?;
                    let source =
                        skeleton_clone(budget, function, source, "plan transfer source clone")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan transfer destination clone",
                    )?;
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Transfer {
                            at,
                            source,
                            destination,
                        },
                        "plan transfer observation push",
                    )?;
                }
                CleanupTransition::AuthenticateVariantCase { .. } => {}
                CleanupTransition::InitializeVariant {
                    at, destination, ..
                } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan variant initialize identity")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan variant initialize destination",
                    )?;
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Initialize { at, destination },
                        "plan variant initialize observation push",
                    )?;
                }
                CleanupTransition::TransferVariant {
                    at,
                    source,
                    destination,
                    ..
                } => {
                    let at =
                        skeleton_clone(budget, function, at, "plan variant transfer identity")?;
                    let source =
                        skeleton_clone(budget, function, source, "plan variant transfer source")?;
                    let destination = skeleton_clone(
                        budget,
                        function,
                        destination,
                        "plan variant transfer destination",
                    )?;
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::Transfer {
                            at,
                            source,
                            destination,
                        },
                        "plan variant transfer observation push",
                    )?;
                }
                CleanupTransition::CallCommit { call, arguments } => {
                    let call = skeleton_clone(budget, function, call, "plan call identity clone")?;
                    let mut cloned_arguments = Vec::new();
                    for argument in arguments {
                        let source = skeleton_clone(
                            budget,
                            function,
                            &argument.source,
                            "plan call argument source clone",
                        )?;
                        skeleton_push(
                            budget,
                            function,
                            &mut cloned_arguments,
                            (argument.parameter_index, source),
                            "plan call argument push",
                        )?;
                    }
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::CallCommit {
                            call,
                            arguments: cloned_arguments,
                        },
                        "plan call observation push",
                    )?;
                }
                CleanupTransition::SelectFailure { .. } => {}
                CleanupTransition::StageCopyResult { source } => {
                    let source = skeleton_clone(
                        budget,
                        function,
                        source,
                        "plan staged-result source clone",
                    )?;
                    compact_stage::prepare!(function, budget, &source);
                    let source = CopySource::new(function, budget, source)?;
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::StageCopyResult(source),
                        "plan staged-result observation push",
                    )?;
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                let edge = &plan.edges[edge.0 as usize];
                if let EdgeCondition::VariantCase {
                    scrutinee,
                    case,
                    matches,
                } = &edge.condition
                {
                    let scrutinee = skeleton_clone(
                        budget,
                        function,
                        scrutinee,
                        "plan final variant scrutinee clone",
                    )?;
                    let case =
                        skeleton_clone(budget, function, case, "plan final variant case clone")?;
                    storage::push!(
                        budget,
                        function,
                        &mut observations,
                        SkeletonObservation::VariantCase {
                            scrutinee,
                            case,
                            matches: *matches,
                        },
                        "plan final variant observation push",
                    )?;
                }
                skeleton_queue_push(
                    budget,
                    function,
                    &mut queue,
                    (edge.to, observations),
                    "plan goto state push",
                )?;
            }
            CleanupTerminator::Branch(edges) => {
                compact_path::plan_selection!(budget, function);
                if edges.len() == 2 && observations.len() < observations.capacity() {
                    let edge = &plan.edges[edges[0].0 as usize];
                    let observation = final_output::observation!(edge, budget, function);
                    let mut branch;

                        physical_budget::compact_try!(budget.reserve_compact_bytes(
                            function,
                            branch_clone::direct::plan_compact_roles::<
                                std::rc::Rc<SkeletonObservation>,
                                SkeletonObservation,
                            >(),
                            "branch clone receiving controls",
                        ));
                        branch = storage::clone_history!(budget, function, observations);
                        storage::push!(
                            budget,
                            function,
                            &mut branch,
                            observation,
                            "plan branch observation push",
                        )?;

                    skeleton_queue_push(
                        budget,
                        function,
                        &mut queue,
                        (edge.to, branch),
                        "plan branch state push",
                    )?;
                    let edge = &plan.edges[edges[1].0 as usize];
                    let observation = final_output::observation!(edge, budget, function);
                    compact_path::plan_move!(budget, function);
                    compact_path::plan_work!(
                        budget,
                        function,
                        "cleanup-plan skeleton branch clone"
                    );
                    note_skeleton_materialization();
                    #[cfg(test)]
                    final_output::record_move(observations.len(), observations.capacity());
                    #[cfg(test)]
                    branch_clone::note_take();
                    let mut branch = observations;
                    compact_path::plan_work!(budget, function, "plan branch observation push");
                    note_skeleton_materialization();
                    compact_path::plan_auth!(budget, function);
                    compact_path::plan_conversion!(budget, function, observation);
                    branch.push(observation.into());
                    skeleton_queue_push(
                        budget,
                        function,
                        &mut queue,
                        (edge.to, branch),
                        "plan branch state push",
                    )?;
                    continue;
                }
                // No remaining-edge query survives on this Legacy-only path.
                // Restore the original for-loop; fund its extra IntoIterator
                // actual/formal Vec loans and returned/forwarded Iter owners
                // before formation. Existing next/body/work order is unchanged.
                budget.reserve_bytes(function, EDGE_ITERATOR_CONTROLS, "PLAN branch iterator receiving")?;
                for edge in edges {
                    let edge = &plan.edges[edge.0 as usize];
                    let observation = match &edge.condition {
                        EdgeCondition::BooleanResult(expression, value) => {
                            SkeletonObservation::Boolean {
                                expression: skeleton_clone(
                                    budget,
                                    function,
                                    expression,
                                    "plan Boolean expression clone",
                                )?,
                                value: *value,
                            }
                        }
                        EdgeCondition::VariantCase {
                            scrutinee,
                            case,
                            matches,
                        } => SkeletonObservation::VariantCase {
                            scrutinee: skeleton_clone(
                                budget,
                                function,
                                scrutinee,
                                "plan variant scrutinee clone",
                            )?,
                            case: skeleton_clone(
                                budget,
                                function,
                                case,
                                "plan variant case clone",
                            )?,
                            matches: *matches,
                        },
                        EdgeCondition::ArmSelected {
                            scrutinee,
                            arm,
                            selected,
                        } => SkeletonObservation::ArmSelected {
                            scrutinee: skeleton_clone(
                                budget,
                                function,
                                scrutinee,
                                "plan scalar-match scrutinee clone",
                            )?,
                            arm: *arm,
                            selected: *selected,
                        },
                        EdgeCondition::StatusZero(source) => SkeletonObservation::Status {
                            source: skeleton_clone(
                                budget,
                                function,
                                source,
                                "plan zero status-source clone",
                            )?,
                            success: true,
                        },
                        EdgeCondition::StatusNonzero(source) => SkeletonObservation::Status {
                            source: skeleton_clone(
                                budget,
                                function,
                                source,
                                "plan nonzero status-source clone",
                            )?,
                            success: false,
                        },
                        EdgeCondition::Always => {
                            return Err(replay_error(
                                function,
                                "branch skeleton contains an unconditional edge",
                            ));
                        }
                    };

                        budget.reserve_bytes(
                            function,
                            branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>(),
                            "branch clone receiving controls",
                        )?;

                    // The branch producer has no Result<Vec<_>, Diagnostic>
                    // receiver or owned observation forwarding argument. Its
                    // nested work/census/payment Results retain their charges.
                    let mut branch;

                        branch = storage::clone_history!(budget, function, observations);
                        storage::push!(
                            budget,
                            function,
                            &mut branch,
                            observation,
                            "plan branch observation push",
                        )?;

                    skeleton_queue_push(
                        budget,
                        function,
                        &mut queue,
                        (edge.to, branch),
                        "plan branch state push",
                    )?;
                }
            }
            CleanupTerminator::Exit(exit) => {
                let exit = &plan.exits[exit.0 as usize];
                match &exit.continuation {
                    ExitContinuation::LoopBreak { edge, .. }
                    | ExitContinuation::FunctionReturn { edge, .. } => {
                        if let Some(marker) =
                            lexical_skeleton::plan_exit_observation(budget, function, exit.id)?
                        {
                            storage::push!(
                                budget,
                                function,
                                &mut observations,
                                marker,
                                "plan lexical exit observation push",
                            )?;
                        }
                        skeleton_queue_push(
                            budget,
                            function,
                            &mut queue,
                            (plan.edges[edge.0 as usize].to, observations),
                            "plan lexical continuation state push",
                        )?;
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
                                    "plan loop-latch target clone",
                                )?,
                            },
                        };
                        storage::push!(
                            budget,
                            function,
                            &mut observations,
                            marker,
                            "plan loop-latch observation push",
                        )?;
                        match lexical_cert::latch_resume(plan, exit.id) {
                            Some(entry) => skeleton_queue_push(
                                budget,
                                function,
                                &mut queue,
                                (entry, observations),
                                "plan generated-latch state push",
                            )?,
                            None => skeleton_push(
                                budget,
                                function,
                                &mut paths,
                                SkeletonPath {
                                    observations: observations.into(),
                                    terminal: SkeletonTerminal::LoopLatch,
                                },
                                "plan loop-latch path push",
                            )?,
                        }
                    }
                    ExitContinuation::Continue(edge) => {
                        skeleton_queue_push(
                            budget,
                            function,
                            &mut queue,
                            (plan.edges[edge.0 as usize].to, observations),
                            "plan continuation state push",
                        )?;
                    }
                    ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                        skeleton_push(
                            budget,
                            function,
                            &mut paths,
                            SkeletonPath {
                                observations: observations.into(),
                                terminal: SkeletonTerminal::Success,
                            },
                            "plan success path push",
                        )?;
                    }
                    ExitContinuation::ReturnFailure { .. } => skeleton_push(
                        budget,
                        function,
                        &mut paths,
                        SkeletonPath {
                            observations: observations.into(),
                            terminal: SkeletonTerminal::Failure,
                        },
                        "plan failure path push",
                    )?,
                }
            }
        }
    }
    Ok(paths)
}

// Flat fallback owners keep their original ordering paths. Shared Legacy paths
// use the existing complete-content comparator, never pointer/representation Ord.
pub(super) fn equal_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    left: &mut [SkeletonPath],
    right: &mut [SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.reserve_bytes(
        function,
        3 * skeleton_work::enumerated_prefix::ENTRY_CALL,
        "PLAN prefix complete-content comparison receiving",
    )?;
    skeleton_work::enumerated_prefix::sort(left, function, budget)?;
    skeleton_work::enumerated_prefix::sort(right, function, budget)?;
    skeleton_work::enumerated_prefix::equal(left, right, function, budget)
}
