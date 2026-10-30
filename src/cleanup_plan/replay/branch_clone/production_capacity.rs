//! One funded production TAKE/copy shape traversal; no direct-clone authority.
use super::*;
#[cfg(test)]
mod before;
#[cfg(test)]
mod main_c883;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::cleanup_plan::replay) struct Decision {
    pub(in crate::cleanup_plan::replay) take: bool,
    pub(in crate::cleanup_plan::replay) copy_capacity: Option<usize>,
}
#[cfg(test)]
impl Decision {
    const NONE: Self = Self {
        take: false,
        copy_capacity: None,
    };
}

// Added to the earlier same-invocation branch-inline payment, before clone work
// and the unchanged TAKE payment. No new reserve ABI or old-price credit exists.
pub(in crate::cleanup_plan::replay) const ADDITIONAL_CONTROLS: usize = 2 * size_of::<Decision>() // actual returned value and caller-retained receiver
    + 4 * size_of::<Option<usize>>() // terminal Some/receiver and caller field/match
    + 4 * size_of::<usize>() // terminal operands/value and selected-copy binding
    + 3 * size_of::<&Vec<EdgeId>>() // Branch pattern, as_slice actual/formal
    + 2 * size_of::<&[EdgeId]>() // as_slice return and slice-pattern receiver
    + 4 * size_of::<&EdgeId>() // Goto/Branch bindings and common selected-edge receive
    + 10 * size_of::<bool>(); // fork/cache, branch/exit/fit/copy predicates and receives
                              // Same zero-argument ABI and complete literal old pricing expression; no
                              // forwarding call/Result is introduced. The earlier reserve retains this bank
                              // on the same budget/function/invocation until the actual probe is entered.
pub(in crate::cleanup_plan::replay) const fn production_inline_controls<T: Clone>() -> usize {
    size_of::<Result<(), Diagnostic>>() // caller payment result
        + 4 * size_of::<Result<(), Diagnostic>>() // clone/push work, clone census, conversion
        + 2 * size_of::<Result<(), Diagnostic>>() // backing wrapper and nested byte payment
        + size_of::<Result<usize, Diagnostic>>() // checked array size conversion
        + size_of::<Option<usize>>()
        + 2 * size_of::<Vec<T>>() // initializer and the enumerator's named branch
        + size_of::<&mut ReplayBudget>() + size_of::<&ResolvedFunction>()
        + size_of::<&Vec<T>>() // direct clone census argument
        + 2 * size_of::<&str>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&mut Vec<T>>() + size_of::<&[T]>()
        + 2 * size_of::<std::slice::Iter<'_, T>>()
        + 2 * size_of::<std::iter::Cloned<std::slice::Iter<'_, T>>>()
        + ADDITIONAL_CONTROLS
        + direct::PROBE_CONTROLS
}

// Retain the original added-production-price threshold. The current query
// bank is separately paid before entry; net application gains remain pending.
pub(super) const COPY_SLOTS: usize =
    ADDITIONAL_CONTROLS / size_of::<std::rc::Rc<SkeletonObservation>>() + 1;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn probe(input: TakeCandidate<'_>) -> Decision {
    #[cfg(test)]
    if super::tests::original_selected() {
        return Decision::NONE;
    }
    if input.budget.payer != AllocationPayer::ActiveBuilder || input.budget.selected() {
        return Decision::NONE;
    }
    let mut bounds = TakeBounds {
        required: input.values.len().checked_add(1),
        capacity: None,
        block: None,
        exit: None,
        next: input.target,
        spare: 0,
        remaining: input.values.len(),
    };
    let Some(required) = bounds.required else {
        return Decision::NONE;
    };
    bounds.capacity = physical_storage::replay_growth_capacity(input.values.len(), required);
    let Some(capacity) = bounds.capacity else {
        return Decision::NONE;
    };
    // growth_capacity guarantees capacity >= required. The same spare owner
    // now counts down from the clone growth bound, rather than the held Vec.
    bounds.spare = capacity - required;
    let take_eligible = input.remaining_edges == 0
        && input.edge_count >= 2
        && input.values.capacity() > input.values.len()
        && input.values.capacity() <= capacity
        && capacity
            .checked_mul(size_of::<std::rc::Rc<SkeletonObservation>>())
            .is_some_and(|bytes| bytes >= TAKE_BYTES);
    let mut forked = false;
    while bounds.remaining != 0 {
        bounds.remaining -= 1;
        bounds.block = input.plan.blocks.get(bounds.next.0 as usize);
        let Some(block) = bounds.block else {
            return Decision::NONE;
        };
        bounds.required = bounds.spare.checked_sub(block.transitions.len());
        let Some(spare) = bounds.required else {
            return Decision::NONE;
        };
        bounds.spare = spare;
        // Both Goto and the one possible final Branch select one borrowed ID.
        // There is exactly one actual edge Vec/slice/get receiver below.
        let selected_edge = match &block.terminator {
            CleanupTerminator::Goto(edge) => edge,
            CleanupTerminator::Branch(edges) => {
                if forked {
                    return Decision::NONE;
                }
                let [_, .., last] = edges.as_slice() else {
                    return Decision::NONE;
                };
                forked = true;
                last
            }
            CleanupTerminator::Exit(exit) => {
                bounds.exit = input.plan.exits.get(exit.0 as usize);
                let Some(exit) = bounds.exit else {
                    return Decision::NONE;
                };
                if !matches!(
                    exit.continuation,
                    ExitContinuation::CommitResult { .. }
                        | ExitContinuation::ReturnUnit
                        | ExitContinuation::ReturnFailure { .. }
                ) {
                    return Decision::NONE;
                }
                // spare only decreased by checked_sub, so this cannot wrap.
                bounds.required = Some(capacity - bounds.spare);
                let Some(required) = bounds.required else {
                    return Decision::NONE;
                };
                return Decision {
                    take: take_eligible && !forked && required <= input.values.capacity(),
                    copy_capacity: if bounds.spare >= COPY_SLOTS {
                        Some(required)
                    } else {
                        None
                    },
                };
            }
        };
        bounds.required = bounds.spare.checked_sub(1);
        let Some(spare) = bounds.required else {
            return Decision::NONE;
        };
        bounds.spare = spare;
        let Some(edge) = input.plan.edges.get(selected_edge.0 as usize) else {
            return Decision::NONE;
        };
        bounds.next = edge.to;
    }
    Decision::NONE
}

// All arguments are lexical identifiers, not new runtime forwarding owners.
macro_rules! produce {
    ($budget:ident, $function:ident, $observations:ident, $observation:ident,
     $plan:ident, $edge:ident, $remaining_edges:ident, $edges:ident,
     $branch:ident, $queue:ident) => {{
        let decision = $crate::cleanup_plan::replay::branch_clone::direct::probe(
            $crate::cleanup_plan::replay::branch_clone::TakeCandidate {
                values: &$observations,
                remaining_edges: $remaining_edges.len(),
                edge_count: $edges.len(),
                budget: $budget,
                plan: $plan,
                target: $edge.to,
            },
        );
        if decision.take {
            note_skeleton_materialization();
            $budget.charge_skeleton($function, 1, "plan branch observation push")?;
            note_skeleton_materialization();
            $budget.reserve_bytes($function, 0, "plan branch observation push")?;
            $budget.reserve_bytes(
                $function,
                physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(
                    &$observation,
                ),
                "plan branch observation push",
            )?;
            #[cfg(test)]
            branch_clone::note_take();
            #[cfg(test)]
            branch_clone::note_generic_take();
            $branch = std::mem::take(&mut $observations);
            $branch.push($observation.into());
            skeleton_queue_push(
                $budget,
                $function,
                &mut $queue,
                ($edge.to, $branch),
                "plan branch state push",
            )?;
            continue;
        }
        $budget.reserve_clone(
            $function,
            &$observations,
            "cleanup-plan skeleton branch clone",
        )?;
        note_skeleton_materialization();
        // Original push work precedes every extra backing debit.
        $budget.charge_skeleton($function, 1, "plan branch observation push")?;
        note_skeleton_materialization();
        let capacity = if let Some(capacity) = decision.copy_capacity {
            capacity
        } else if $observations.is_empty() {
            1
        } else {
            if $observations.len() > usize::MAX / 2 {
                return Err($budget.budget_refusal(
                    $function,
                    "cleanup replay branch capacity overflowed",
                    "",
                ));
            }
            $observations.len() * 2
        };
        $budget.reserve_array::<std::rc::Rc<SkeletonObservation>>(
            $function,
            capacity - $observations.len(),
            "plan branch observation push",
        )?;
        $budget.reserve_bytes(
            $function,
            physical_clone::conversion_bytes::<std::rc::Rc<SkeletonObservation>, _>(&$observation),
            "plan branch observation push",
        )?;
        $branch = Vec::with_capacity(capacity);
        $branch.extend($observations.iter().cloned());
        $branch.push($observation.into());
        // Count only the successfully filled bounded-copy Vec.
        // TAKE, shape probes and original fallback do not count.
        #[cfg(test)]
        if decision.copy_capacity.is_some() {
            branch_clone::note_plan_copy();
        }
        skeleton_queue_push(
            $budget,
            $function,
            &mut $queue,
            ($edge.to, $branch),
            "plan branch state push",
        )?;
        continue;
    }};
}
pub(in crate::cleanup_plan::replay) use produce;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) mod before_short;

/// The caller immediately appends one full observation. The closed unpaid
/// route acquires that slot together with the original full history copy.
/// Its clone census, logical work, materialization and outward Vec/Result are
/// unchanged; the original push and queue publication remain in the caller.
pub(in crate::cleanup_plan::replay) fn unpaid_clone(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    values: &Vec<std::rc::Rc<SkeletonObservation>>,
    phase: &str,
) -> Result<Vec<std::rc::Rc<SkeletonObservation>>, Diagnostic> {
    if budget.payer != AllocationPayer::LegacyUnobserved
        || budget.admission != AdmissionMode::Ordinary
    {
        return skeleton_clone(budget, function, values, phase);
    }
    budget.charge_skeleton(function, 1, phase)?;
    budget.reserve_clone(function, values, phase)?;
    note_skeleton_materialization();
    let capacity = values
        .len()
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "cleanup replay branch capacity overflowed"))?;
    let mut copy = Vec::with_capacity(capacity);
    copy.extend(values.iter().cloned());
    Ok(copy)
}

#[test]
fn unpaid_exact_plan_branch_keeps_full_original_paths_work_failures_and_materializations() {
    fn before(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Vec<SkeletonPath>, Diagnostic> {
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
                        let at = skeleton_clone(
                            budget,
                            function,
                            at,
                            "plan initialize expression clone",
                        )?;
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
                        let at = skeleton_clone(
                            budget,
                            function,
                            at,
                            "plan variant initialize identity",
                        )?;
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
                        let source = skeleton_clone(
                            budget,
                            function,
                            source,
                            "plan variant transfer source",
                        )?;
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
                        let call =
                            skeleton_clone(budget, function, call, "plan call identity clone")?;
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
                        compact_stage::prepare!(function, budget, &source);
                        let source = CopySource::new(function, budget, source)?;
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
                        let case = skeleton_clone(
                            budget,
                            function,
                            case,
                            "plan final variant case clone",
                        )?;
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
                    compact_path::plan_selection!(budget, function);
                    if edges.len() == 2 && observations.len() < observations.capacity() {
                        let edge = &plan.edges[edges[0].0 as usize];
                        let observation = final_output::observation!(edge, budget, function);
                        let mut branch;
                        if !budget.pays_allocations() {
                            physical_budget::compact_try!(budget.reserve_compact_bytes(
                                function,
                                branch_clone::direct::plan_compact_roles::<
                                    std::rc::Rc<SkeletonObservation>,
                                    SkeletonObservation,
                                >(),
                                "branch clone receiving controls",
                            ));
                            branch = skeleton_clone(
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
                        } else if observations.len() < branch_clone::direct::SMALL_COPY_LIMIT {
                            physical_budget::compact_try!(
                                budget.reserve_compact_bytes(
                                    function,
                                    branch_clone::plan_compact_roles::<
                                        std::rc::Rc<SkeletonObservation>,
                                    >(),
                                    "branch clone receiving controls",
                                )
                            );
                            branch_clone::direct::copy_small_plan!(
                                branch,
                                observations,
                                observation,
                                budget,
                                function
                            );
                        } else {
                            physical_budget::compact_try!(budget.reserve_compact_bytes(
                                function,
                                branch_clone::direct::plan_compact_roles::<
                                    std::rc::Rc<SkeletonObservation>,
                                    SkeletonObservation,
                                >(),
                                "branch clone receiving controls",
                            ));
                            branch = branch_clone::direct::copy_prepaid(
                                budget,
                                function,
                                &observations,
                                observation,
                                "cleanup-plan skeleton branch clone",
                                plan,
                                edge.to,
                            )?;
                        }
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
                    // Same slice iterator as the original for-loop, now named for the final-edge query.
                    let mut remaining_edges = edges.iter();
                    while let Some(edge) = remaining_edges.next() {
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
                        if let ReplayBudget {
                            payer: AllocationPayer::ActiveBuilder,
                            ..
                        } = budget
                        {
                            branch_clone::short_plan::produce!(
                                budget,
                                function,
                                observations,
                                observation,
                                edge,
                                queue
                            );
                            budget.reserve_bytes(
                                function,
                                branch_clone::production_capacity::production_inline_controls::<
                                    std::rc::Rc<SkeletonObservation>,
                                >(),
                                "branch clone receiving controls",
                            )?;
                        } else {
                            budget.reserve_bytes(
                                function,
                                branch_clone::compact_roles::<std::rc::Rc<SkeletonObservation>>(),
                                "branch clone receiving controls",
                            )?;
                        }
                        // The branch producer has no Result<Vec<_>, Diagnostic>
                        // receiver or owned observation forwarding argument. Its
                        // nested work/census/payment Results retain their charges.
                        let mut branch;
                        if !budget.pays_allocations() {
                            branch = skeleton_clone(
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
                        } else {
                            budget.charge_skeleton(
                                function,
                                1,
                                "cleanup-plan skeleton branch clone",
                            )?;
                            // Original clone1 is charged above on both true and
                            // false probe paths; this is a bounded implementation
                            // detail of that materialization, not a CPU ledger.
                            if let ReplayBudget {
                                payer: AllocationPayer::ActiveBuilder,
                                ..
                            } = budget
                            {
                                budget.reserve_bytes(
                                    function,
                                    branch_clone::TAKE_CALL_CONTROLS,
                                    "plan final branch probe receiving",
                                )?;
                                branch_clone::production_capacity::produce!(
                                    budget,
                                    function,
                                    observations,
                                    observation,
                                    plan,
                                    edge,
                                    remaining_edges,
                                    edges,
                                    branch,
                                    queue
                                );
                            }
                            budget.reserve_clone(
                                function,
                                &observations,
                                "cleanup-plan skeleton branch clone",
                            )?;
                            note_skeleton_materialization();
                            // Original push work precedes every extra backing debit.
                            budget.charge_skeleton(function, 1, "plan branch observation push")?;
                            note_skeleton_materialization();
                            let capacity = if observations.is_empty() {
                                1
                            } else {
                                if observations.len() > usize::MAX / 2 {
                                    return Err(budget.budget_refusal(
                                        function,
                                        "cleanup replay branch capacity overflowed",
                                        "",
                                    ));
                                }
                                observations.len() * 2
                            };
                            budget.reserve_array::<std::rc::Rc<SkeletonObservation>>(
                                function,
                                capacity - observations.len(),
                                "plan branch observation push",
                            )?;
                            budget.reserve_bytes(
                                function,
                                physical_clone::conversion_bytes::<
                                    std::rc::Rc<SkeletonObservation>,
                                    _,
                                >(&observation),
                                "plan branch observation push",
                            )?;
                            branch = Vec::with_capacity(capacity);
                            branch.extend(observations.iter().cloned());
                            branch.push(observation.into());
                        }
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
    fn balanced(count: usize) -> String {
        match count {
            0 => "0".to_owned(),
            1 => "f()".to_owned(),
            _ => format!("({}+{})", balanced(count / 2), balanced(count - count / 2)),
        }
    }
    for count in [0, 1, 2, 8, 32, 64, 128] {
        let calls = balanced(count);
        let text=format!("module unpaid.branch; @id(\"f\") fn f()->i64 {{1}} @id(\"main\") fn main()->i64 {{{calls}}}");
        let checked = crate::check(&text, "unpaid-branch.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        for function in &program.functions {
            for limit in [0, 1, 2, 8, 32, 128, 4096, MAX_REPLAY_WORK_UNITS] {
                let mut old = ReplayBudget::with_skeleton_limit(limit);
                let mut current = ReplayBudget::with_skeleton_limit(limit);
                assert_eq!(old.payer, AllocationPayer::LegacyUnobserved);
                let start = skeleton_materializations();
                let expected = before(function, &mut old);
                let old_events = skeleton_materializations() - start;
                let start = skeleton_materializations();
                let actual =
                    crate::cleanup_plan::replay::plan_skeleton_paths(function, &mut current);
                let current_events = skeleton_materializations() - start;
                assert_eq!(actual,expected,"all ordered observation fields and terminal values, calls={count}, limit={limit}");
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                assert_eq!(current_events, old_events);
            }
        }
    }
}
