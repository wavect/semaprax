//! Allocation guards for independent lexical-certificate authentication.
use super::*;

pub(in crate::cleanup_plan::replay) fn insert<K: Ord, V>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
) -> Result<Option<V>, Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(map.insert(key, value));
    }
    if !map.contains_key(&key) {
        budget.reserve_btree_node::<K, V>(function, "lexical certificate map")?;
    }
    Ok(map.insert(key, value))
}
pub(super) fn insert_set<K: Ord>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    set: &mut BTreeSet<K>,
    key: K,
) -> Result<bool, Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(set.insert(key));
    }
    if set.contains(&key) {
        return Ok(false);
    }
    budget.reserve_btree_node::<K, ()>(function, "lexical certificate set")?;
    Ok(set.insert(key))
}
/// Selected reachability iterates borrowed edges, never materializes an edge
/// vector from the supplied certificate. Its successor order is unchanged.
fn successors(
    function: &ResolvedFunction,
    block: BlockId,
    mut visit: impl FnMut(BlockId) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    match &plan.blocks[block.0 as usize].terminator {
        CleanupTerminator::Goto(edge) => visit(plan.edges[edge.0 as usize].to)?,
        CleanupTerminator::Branch(edges) => {
            for edge in edges {
                visit(plan.edges[edge.0 as usize].to)?;
            }
        }
        CleanupTerminator::Exit(exit) => match &plan.exits[exit.0 as usize].continuation {
            ExitContinuation::Continue(edge)
            | ExitContinuation::LoopBreak { edge, .. }
            | ExitContinuation::FunctionReturn { edge, .. } => {
                visit(plan.edges[edge.0 as usize].to)?
            }
            ExitContinuation::LoopLatch { .. } => {
                if let Some(next) = latch_resume(plan, *exit) {
                    visit(next)?;
                }
            }
            ExitContinuation::CommitResult { .. }
            | ExitContinuation::ReturnFailure { .. }
            | ExitContinuation::ReturnUnit => {}
        },
    }
    Ok(())
}
pub(super) fn reach(
    function: &ResolvedFunction,
    start: BlockId,
    avoid: Option<BlockId>,
    stop: &BTreeSet<BlockId>,
    excluded: Option<&[bool]>,
    budget: &mut ReplayBudget,
) -> Result<Vec<bool>, Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(super::reach(function, start, avoid, stop, excluded));
    }
    budget.reserve_array::<bool>(
        function,
        function.cleanup_plan.blocks.len(),
        "certificate reach bitmap",
    )?;
    let mut seen = vec![false; function.cleanup_plan.blocks.len()];
    if Some(start) == avoid {
        return Ok(seen);
    }
    seen[start.0 as usize] = true;
    let mut stack = Vec::new();
    budget.reserve_vec(function, &mut stack, 1, "certificate reach stack")?;
    stack.push(start);
    while let Some(block) = stack.pop() {
        successors(function, block, |next| {
            let index = next.0 as usize;
            if seen[index]
                || Some(next) == avoid
                || stop.contains(&next)
                || excluded.is_some_and(|excluded| excluded[index])
            {
                return Ok(());
            }
            budget.reserve_vec(function, &mut stack, 1, "certificate reach stack")?;
            seen[index] = true;
            stack.push(next);
            Ok(())
        })?;
    }
    Ok(seen)
}
/// Borrowed selected chain query preserves the ordinary unknown-region error;
/// region acyclicity has already been independently validated by the driver.
pub(super) fn chain_contains(
    function: &ResolvedFunction,
    block: BlockId,
    region: CleanupRegionId,
    selected: bool,
) -> Result<bool, Diagnostic> {
    if !selected {
        return super::chain_contains(function, block, region);
    }
    let mut current = Some(function.cleanup_plan.blocks[block.0 as usize].region);
    let mut found = false;
    let mut remaining = function.cleanup_plan.regions.len();
    while let Some(id) = current {
        remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| replay_error(function, "cleanup region tree contains a cycle"))?;
        let item = function
            .cleanup_plan
            .regions
            .get(id.0 as usize)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup region chain references an unknown region",
                )
            })?;
        found |= id == region;
        current = item.parent;
    }
    Ok(found)
}
pub(in crate::cleanup_plan::replay) fn region_chain(
    function: &ResolvedFunction,
    start: CleanupRegionId,
    budget: &mut ReplayBudget,
) -> Result<Vec<CleanupRegionId>, Diagnostic> {
    if !budget.pays_allocations() {
        return region_chain_to_root(function, start);
    }
    let mut current = Some(start);
    let mut count = 0usize;
    let mut remaining = function.cleanup_plan.regions.len();
    while let Some(id) = current {
        remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| replay_error(function, "cleanup region tree contains a cycle"))?;
        let item = function
            .cleanup_plan
            .regions
            .get(id.0 as usize)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup region chain references an unknown region",
                )
            })?;
        count += 1;
        current = item.parent;
    }
    budget.reserve_array::<CleanupRegionId>(function, count, "certificate region chain")?;
    let mut chain = Vec::with_capacity(count);
    let mut current = Some(start);
    while let Some(id) = current {
        chain.push(id);
        current = function.cleanup_plan.regions[id.0 as usize].parent;
    }
    Ok(chain)
}

pub(in crate::cleanup_plan::replay) fn finalizer_actions(
    function: &ResolvedFunction,
    actions: &[crate::cleanup_plan::FinalizeAction],
    known: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return validate_finalizer_actions(function, actions, known, leaves);
    }
    let mut finalized = BTreeSet::new();
    for action in actions {
        if !known.contains(&action.source.storage) {
            return Err(replay_error(
                function,
                "cleanup place references unknown storage",
            ));
        }
        let mut under = leaves.iter().filter(|(_, leaf)| {
            leaf.place.storage == action.source.storage
                && leaf
                    .place
                    .projections
                    .starts_with(&action.source.projections)
        });
        let first = under.next();
        if first.is_none() {
            return Err(replay_error(
                function,
                "cleanup place has no droppable leaf",
            ));
        }
        let (flag, leaf) = first.expect("nonempty borrowed leaf query");
        if under.next().is_some() || *flag != action.guard_flag || leaf.place != action.source {
            return Err(replay_error(
                function,
                "finalizer guard does not name its exact cleanup leaf",
            ));
        }
        if leaf.lifecycle != action.lifecycle_id
            || !insert_set(function, budget, &mut finalized, action.guard_flag)?
        {
            return Err(replay_error(
                function,
                "finalizer lifecycle or guard is invalid or repeated",
            ));
        }
        if let Some(condition) = &action.active_case {
            // Paying ordinary backing does not select the primitive profile.
            // Keep the original variant/path authentication without a cloned
            // leaf vector; insert_set above funds the only growing scratch.
            if budget.selected() {
                return Err(replay_error(
                    function,
                    "selected lexical boundary conditional ownership is outside the primitive profile",
                ));
            }
            if condition.storage != action.source.storage
                || action.source.projections.first() != Some(&condition.case)
                || function
                    .cleanup_plan
                    .slots
                    .iter()
                    .find(|slot| slot.storage == condition.storage)
                    .is_none_or(|slot| {
                        !matches!(
                            &slot.ty,
                            ResolvedType::Nominal { declaration, .. }
                                if declaration == &condition.variant
                        )
                    })
            {
                return Err(replay_error(
                    function,
                    "conditional finalizer does not bind its exact variant case path",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn function() -> ResolvedFunction {
        let parsed = crate::check(
            "module cert; @id(\"main\") fn main()->i64 { let mut i=0; while i<3 { i=i+1; 0 } i }",
            "cert.spx",
        )
        .unwrap();
        crate::hir::resolve(&parsed).unwrap().functions.remove(0)
    }
    #[test]
    fn borrowed_reach_matches_legacy_order_and_refuses_before_bitmap_growth() {
        let function = function();
        let stop = BTreeSet::new();
        let expected =
            super::super::reach(&function, function.cleanup_plan.entry, None, &stop, None);
        let mut budget = ReplayBudget::new();
        budget.admission = AdmissionMode::SelectedRetained;
        budget.payer = AllocationPayer::SelectedReservation;
        budget.physical = Some(Box::new(physical_storage::PhysicalReservation::new(
            1 << 20,
        )));
        assert_eq!(
            reach(
                &function,
                function.cleanup_plan.entry,
                None,
                &stop,
                None,
                &mut budget
            )
            .unwrap(),
            expected
        );
        let mut short = ReplayBudget::new();
        short.admission = AdmissionMode::SelectedRetained;
        short.payer = AllocationPayer::SelectedReservation;
        short.physical = Some(Box::new(physical_storage::PhysicalReservation::new(0)));
        assert!(reach(
            &function,
            function.cleanup_plan.entry,
            None,
            &stop,
            None,
            &mut short
        )
        .is_err());
        short.remaining = MAX_REPLAY_WORK_UNITS;
        assert!(reach(
            &function,
            function.cleanup_plan.entry,
            None,
            &stop,
            None,
            &mut short
        )
        .is_err());
        assert_eq!(
            super::super::reach(&function, function.cleanup_plan.entry, None, &stop, None),
            expected
        );
    }
    #[test]
    fn exact_borrowed_region_contour_preserves_unknown_and_cycle_refusals() {
        let function = function();
        let start = function.cleanup_plan.blocks[function.cleanup_plan.entry.0 as usize].region;
        let expected = region_chain_to_root(&function, start).unwrap();
        let mut budget = ReplayBudget::new();
        budget.admission = AdmissionMode::SelectedRetained;
        budget.payer = AllocationPayer::SelectedReservation;
        budget.physical = Some(Box::new(physical_storage::PhysicalReservation::new(
            1 << 20,
        )));
        assert_eq!(
            region_chain(&function, start, &mut budget).unwrap(),
            expected
        );
        for region in &expected {
            assert!(chain_contains(&function, function.cleanup_plan.entry, *region, true).unwrap());
        }
        let mut bad = function.clone();
        bad.cleanup_plan.regions[start.0 as usize].parent = Some(CleanupRegionId(u32::MAX));
        assert_eq!(
            region_chain(&bad, start, &mut budget).unwrap_err().message,
            region_chain_to_root(&bad, start).unwrap_err().message
        );
        bad.cleanup_plan.regions[start.0 as usize].parent = Some(start);
        assert!(chain_contains(&bad, bad.cleanup_plan.entry, start, true)
            .unwrap_err()
            .message
            .contains("cycle"));
    }
}

#[cfg(test)]
mod ordinary_finalizers;
