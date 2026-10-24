//! A known next push can acquire its existing growth capacity at clone time.
//! This removes the intermediate clone backing; future capacity stays the same.
use super::super::*;
use super::before as branch_clone;

#[track_caller]
pub(in crate::cleanup_plan::replay::branch_clone) fn for_push<T: Clone + ClonePayload>(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<T>,
    phase: &str,
) -> Result<Vec<T>, Diagnostic> {
    #[cfg(test)]
    if super::tests::original_selected() {
        return skeleton_clone(budget, function, values, phase);
    }
    if !budget.pays_allocations() {
        return skeleton_clone(budget, function, values, phase);
    }
    budget.charge_skeleton(function, 1, phase)?;
    // Keep the complete old clone census, including all nested owned backing
    // and its independently funded oracle work. Nothing is repriced or waived.
    budget.reserve_clone(function, values, phase)?;
    let capacity = values
        .len()
        .checked_add(1)
        .and_then(|required| physical_storage::replay_growth_capacity(values.len(), required))
        .ok_or_else(|| {
            budget.budget_refusal(function, "cleanup replay branch capacity overflowed", "")
        })?;
    budget.reserve_array::<T>(function, capacity - values.len(), phase)?;
    note_skeleton_materialization();
    let mut result = Vec::with_capacity(capacity);
    result.extend(values.iter().cloned());
    Ok(result)
}


pub(super) fn plan_skeleton_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let mut queue = VecDeque::new();
    skeleton_queue_push(
        budget,
        function,
        &mut queue,
        (plan.entry, Vec::<std::rc::Rc<SkeletonObservation>>::new()),
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
                    skeleton_push(
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
                    skeleton_push(
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
                    skeleton_push(
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
                    skeleton_push(
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
                    skeleton_push(
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
                    skeleton_push(
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
                    skeleton_push(
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
                    skeleton_push(
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
                    let mut branch = branch_clone::for_push(
                        budget,
                        function,
                        &observations,
                        "cleanup-plan skeleton branch clone",
                    )?;
                    skeleton_push(
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
                            skeleton_push(
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
                        skeleton_push(
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
pub(super) fn main_replay_input(current: &str) -> String {
    let compile_fixed = crate::source_reflows_merge::compile_fixture_input("src/cleanup_plan/replay.rs", current);
    let current = compile_fixed.as_str();
    use sha2::{Digest, Sha256};
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))), CURRENT_REPLAY_SHA);
    let original = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/branch-clone-funded-next-20261016/main-replay.before.rs.txt")).expect("immutable complete Main replay input");
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))), MAIN_REPLAY_SHA);
    original
}
const CURRENT_REPLAY_SHA: &str = "5996c732aceb53c6f4ae7a8924ea020584328b2d86638446bbd3f365e925d503";
const MAIN_REPLAY_SHA: &str = "342cb55dae785e8353f76810c6eccb1a2f74c9c840d36c83ed5c449db971d0b8";
