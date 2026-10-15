//! Full-chain validation precedes prefix membership, matching the old error order.
use super::*;

struct Chain {
    output: Vec<CleanupRegionId>,
    current: Option<CleanupRegionId>,
}
struct ContinueControls<'a> {
    certificate: Option<&'a crate::cleanup_plan::LexicalExitCertificate>,
    lp: Option<&'a crate::cleanup_plan::LoopCertificate>,
    owner: Option<(usize, CleanupRegionId)>,
    contains: bool,
}
struct ExitRegions {
    source: Vec<CleanupRegionId>,
    target: Vec<CleanupRegionId>,
    cut: usize,
}

fn chain(
    function: &ResolvedFunction,
    start: CleanupRegionId,
    budget: &mut ReplayBudget,
) -> Result<Vec<CleanupRegionId>, Diagnostic> {
    const PHASE: &str = "cleanup receiving region chain";
    budget.reserve_bytes(function, 0, PHASE)?;
    budget.charge(function, 0, PHASE)?;
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<&mut ReplayBudget>()
            + std::mem::size_of::<Chain>()
            + std::mem::size_of::<Result<Vec<CleanupRegionId>, Diagnostic>>()
            + std::mem::size_of::<Vec<CleanupRegionId>>()
            + std::mem::size_of::<CleanupRegionId>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupRegion>>()
            + std::mem::size_of::<&crate::cleanup_plan::CleanupRegion>()
            + std::mem::size_of::<usize>()
            + std::mem::size_of::<bool>(),
        PHASE,
    )?;
    budget.charge(function, 5, PHASE)?;
    let mut state = Chain {
        output: Vec::new(),
        current: Some(start),
    };
    while let Some(region) = state.current {
        budget.charge(function, 3, PHASE)?;
        let item = function
            .cleanup_plan
            .regions
            .get(region.0 as usize)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup region chain references an unknown region",
                )
            })?;
        if state.output.len() == state.output.capacity() {
            let units = state.output.len().checked_add(1).ok_or_else(|| {
                budget.budget_refusal(function, "region chain relocation work overflow", "")
            })?;
            budget.charge(function, units, PHASE)?;
        }
        budget.reserve_vec(function, &mut state.output, 1, PHASE)?;
        state.output.push(region);
        state.current = item.parent;
    }
    Ok(state.output)
}

pub(in crate::cleanup_plan::replay::expression_facts::receiving) fn expected_exit_regions(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    budget: &mut ReplayBudget,
) -> Result<Vec<CleanupRegionId>, Diagnostic> {
    if !budget.pays_allocations() {
        return crate::cleanup_plan::replay::expected_exit_regions(function, exit);
    }
    const PHASE: &str = "cleanup expected exit region result";
    budget.reserve_bytes(function, 0, PHASE)?;
    budget.charge(function, 0, PHASE)?;
    let plan = &function.cleanup_plan;
    let source_region = plan
        .blocks
        .get(exit.from.0 as usize)
        .ok_or_else(|| replay_error(function, "cleanup exit has an unknown source block"))?
        .region;
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<&mut ReplayBudget>()
            + std::mem::size_of::<ExitRegions>()
            + std::mem::size_of::<Result<Vec<CleanupRegionId>, Diagnostic>>()
            + std::mem::size_of::<Vec<CleanupRegionId>>()
            + 2 * std::mem::size_of::<CleanupRegionId>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupBlock>>()
            + std::mem::size_of::<&crate::cleanup_plan::CleanupBlock>()
            + std::mem::size_of::<Option<usize>>()
            + std::mem::size_of::<usize>()
            + 2 * std::mem::size_of::<
                std::iter::Enumerate<std::slice::Iter<'static, CleanupRegionId>>,
            >()
            + 2 * std::mem::size_of::<std::slice::Iter<'static, CleanupRegionId>>()
            + std::mem::size_of::<Option<(usize, &CleanupRegionId)>>()
            + 2 * std::mem::size_of::<&CleanupRegionId>()
            + std::mem::size_of::<bool>(),
        PHASE,
    )?;
    budget.charge(function, 6, PHASE)?;
    let mut state = ExitRegions {
        source: chain(function, source_region, budget)?,
        target: Vec::new(),
        cut: 0,
    };
    match &exit.continuation {
        ExitContinuation::LoopLatch { target } => {
            if let Some(cut) = owned_continue_cut(function, exit.id, &state.source, budget)? {
                state.source.truncate(cut);
                return Ok(state.source);
            }
            let region = lexical_cert::loop_region(plan, target).ok_or_else(|| {
                replay_error(function, "loop latch names an unknown lexical loop")
            })?;
            state.cut = state.source.len();
            for (index, candidate) in state.source.iter().enumerate() {
                budget.charge(function, 1, PHASE)?;
                if *candidate == region {
                    state.cut = index;
                    break;
                }
            }
            if state.cut == state.source.len() {
                return Err(replay_error(
                    function,
                    "loop latch exit does not originate inside its loop region",
                ));
            }
            state.source.truncate(state.cut);
            Ok(state.source)
        }
        ExitContinuation::Continue(edge)
        | ExitContinuation::LoopBreak { edge, .. }
        | ExitContinuation::FunctionReturn { edge, .. } => {
            let target = plan
                .edges
                .get(edge.0 as usize)
                .and_then(|edge| plan.blocks.get(edge.to.0 as usize))
                .ok_or_else(|| {
                    replay_error(function, "cleanup continuation has an unknown target")
                })?;
            // Complete the original target traversal before any membership read.
            // Only membership was observed from its old Set; authored order and
            // duplicate region values do not change equality membership.
            state.target = chain(function, target.region, budget)?;
            state.cut = state.source.len();
            'source: for (index, source) in state.source.iter().enumerate() {
                for target in &state.target {
                    budget.charge(function, 1, PHASE)?;
                    if source == target {
                        state.cut = index;
                        break 'source;
                    }
                }
            }
            state.source.truncate(state.cut);
            Ok(state.source)
        }
        ExitContinuation::CommitResult { .. }
        | ExitContinuation::ReturnFailure { .. }
        | ExitContinuation::ReturnUnit => Ok(state.source),
    }
}

// Return the exact old prefix length instead of allocating its to_vec output or
// cloning the remainder StorageId. Full old implementation remains in lexical_cert.
fn owned_continue_cut(
    function: &ResolvedFunction,
    exit: crate::cleanup_plan::ExitTargetId,
    chain: &[CleanupRegionId],
    budget: &mut ReplayBudget,
) -> Result<Option<usize>, Diagnostic> {
    const PHASE: &str = "cleanup owning continue prefix";
    budget.reserve_bytes(
        function,
        2 * std::mem::size_of::<&mut ReplayBudget>()
            + std::mem::size_of::<Result<Option<usize>, Diagnostic>>()
            + std::mem::size_of::<Option<usize>>()
            + std::mem::size_of::<ContinueControls<'static>>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::LexicalControlPlan>>()
            + std::mem::size_of::<&crate::cleanup_plan::LexicalExitCertificate>()
            + std::mem::size_of::<&crate::cleanup_plan::LoopCertificate>()
            + std::mem::size_of::<&crate::hir::ValueId>()
            + 2 * std::mem::size_of::<
                std::slice::Iter<'static, crate::cleanup_plan::LexicalExitCertificate>,
            >()
            + 2 * std::mem::size_of::<
                std::slice::Iter<'static, crate::cleanup_plan::LoopCertificate>,
            >()
            + 2 * std::mem::size_of::<
                std::iter::Enumerate<std::slice::Iter<'static, CleanupRegionId>>,
            >()
            + 2 * std::mem::size_of::<std::slice::Iter<'static, StorageId>>()
            + std::mem::size_of::<Option<(usize, &CleanupRegionId)>>()
            + std::mem::size_of::<Option<&crate::cleanup_plan::CleanupRegion>>()
            + std::mem::size_of::<&crate::cleanup_plan::CleanupRegion>()
            + std::mem::size_of::<&StorageId>()
            + 3 * std::mem::size_of::<CleanupRegionId>()
            + 3 * std::mem::size_of::<usize>(),
        PHASE,
    )?;
    budget.charge(function, 6, PHASE)?;
    let plan = &function.cleanup_plan;
    let Some(control) = plan.lexical_control.as_deref() else {
        return Ok(None);
    };
    let mut state = ContinueControls {
        certificate: None,
        lp: None,
        owner: None,
        contains: false,
    };
    for item in &control.exits {
        budget.charge(function, 1, PHASE)?;
        if item.exit == exit {
            state.certificate = Some(item);
            break;
        }
    }
    let Some(certificate) = state.certificate else {
        return Ok(None);
    };
    if certificate.kind != crate::ast::LexicalExitKind::Continue {
        return Ok(None);
    }
    for item in &control.loops {
        budget.charge(function, 1, PHASE)?;
        if item.target == certificate.target {
            state.lp = Some(item);
            break;
        }
    }
    let Some(lp) = state.lp else {
        return Ok(None);
    };
    let crate::hir::LoopLatchKind::OwnedIteratorStep { remainder, .. } = &lp.latch else {
        return Ok(None);
    };
    for (index, region) in chain.iter().enumerate() {
        budget.charge(function, 3, PHASE)?;
        let arm = *region;
        let Some(region) = plan.regions.get(region.0 as usize) else {
            continue;
        };
        state.contains = false;
        for storage in &region.slots {
            budget.charge(function, 1, PHASE)?;
            if let StorageId::Value(value) = storage {
                let units = value
                    .as_str()
                    .len()
                    .min(remainder.as_str().len())
                    .checked_add(1)
                    .ok_or_else(|| {
                        budget.budget_refusal(function, "owning continue key work overflow", "")
                    })?;
                budget.charge(function, units, PHASE)?;
                if value == remainder {
                    state.contains = true;
                    break;
                }
            }
        }
        if state.contains {
            if state.owner.is_some() {
                return Err(replay_error(
                    function,
                    "owning continue remainder scope is ambiguous",
                ));
            }
            state.owner = Some((index, arm));
        }
    }
    let (index, arm) = state.owner.ok_or_else(|| {
        replay_error(
            function,
            "owning continue remainder has no active Yield scope",
        )
    })?;
    if index == 0 {
        return Err(replay_error(
            function,
            "owning continue remainder scope is ambiguous",
        ));
    }
    let body = plan.regions[arm.0 as usize]
        .parent
        .ok_or_else(|| replay_error(function, "owning continue Yield scope has no body parent"))?;
    if plan.regions[body.0 as usize].parent != Some(lp.region) {
        return Err(replay_error(
            function,
            "owning continue retained scope chain differs",
        ));
    }
    Ok(Some(index - 1))
}
