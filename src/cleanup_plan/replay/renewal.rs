//! Replay the narrowly authenticated conditional Vec renewal protocol.
use super::*;

pub(super) fn validate_binding(
    function: &ResolvedFunction,
    at: &ExpressionId,
    place: &CleanupPlace,
) -> Result<(), Diagnostic> {
    if function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V12
        || !crate::hir::iterator_loop::renewal_binding(function, at).is_some_and(|binding| {
            *place == CleanupPlace::whole(StorageId::Value(binding.id.clone()))
        })
    {
        return Err(replay_error(
            function,
            "renewal is outside an exact conditional iterator assignment",
        ));
    }
    Ok(())
}
pub(super) fn reject_unmarked_finish(
    function: &ResolvedFunction,
    at: &ExpressionId,
    destination: &CleanupPlace,
) -> Result<(), Diagnostic> {
    if crate::hir::iterator_loop::renewal_binding(function, at).is_some_and(|binding| {
        *destination == CleanupPlace::whole(StorageId::Value(binding.id.clone()))
    }) {
        return Err(replay_error(
            function,
            "conditional renewal uses an ordinary transfer",
        ));
    }
    Ok(())
}
pub(super) fn reserve(
    function: &ResolvedFunction,
    at: &ExpressionId,
    binding: &CleanupPlace,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    validate_binding(function, at, binding)?;
    let flags = validate_place(function, binding, storage, leaves)?;
    if flags.len() != 1 || !state.live_order.contains(&flags[0]) || !state.renewals.is_empty() {
        return Err(replay_error(
            function,
            "renewal reservation requires one live unreserved Vec owner",
        ));
    }
    state.renewals.insert(at.clone(), state.live_order.clone());
    Ok(())
}
pub(super) fn renew(
    function: &ResolvedFunction,
    at: &ExpressionId,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &BTreeMap<LivenessFlagId, Leaf>,
) -> Result<(), Diagnostic> {
    validate_binding(function, at, destination)?;
    let history = state
        .renewals
        .remove(at)
        .ok_or_else(|| replay_error(function, "renewal has no reservation"))?;
    let flags = validate_place(function, destination, storage, leaves)?;
    if flags.len() != 1 {
        return Err(replay_error(
            function,
            "renewal destination is not one Vec leaf",
        ));
    }
    replay_transfer(function, state, source, destination, storage, leaves)?;
    let flag = flags[0];
    if !history.contains(&flag)
        || history
            .iter()
            .filter(|candidate| **candidate != flag)
            .ne(state
                .live_order
                .iter()
                .filter(|candidate| **candidate != flag))
    {
        return Err(replay_error(
            function,
            "renewal changed unrelated cleanup initialization history",
        ));
    }
    state.live_order = history;
    Ok(())
}
pub(super) fn prepend_reservation(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    paths: &mut [ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let binding = crate::hir::iterator_loop::renewal_binding(function, &expression.id)
        .ok_or_else(|| replay_error(function, "renewal HIR binding disappeared"))?;
    for path in paths {
        let at = work.clone_owned(&expression.id, "renewal expression clone")?;
        let binding = CleanupPlace::whole(StorageId::Value(
            work.clone_owned(&binding.id, "renewal binding clone")?,
        ));
        work.charge(
            path.observations.len().saturating_add(1),
            "renewal prefix insertion",
        )?;
        note_skeleton_materialization();
        path.observations
            .insert(0, SkeletonObservation::ReserveRenewal { at, binding });
    }
    Ok(())
}

pub(super) fn validate_join_compatibility(
    function: &ResolvedFunction,
    existing: &BTreeSet<PathState>,
    incoming: &PathState,
    block: BlockId,
) -> Result<(), Diagnostic> {
    if existing.iter().any(|state| {
        state.renewals != incoming.renewals
            || state.pending_failure != incoming.pending_failure
            || state.selected_failure != incoming.selected_failure
            || state.published != incoming.published
    }) {
        return Err(replay_error(
            function,
            format!(
                "cleanup join at block {} has incompatible control states",
                block.0
            ),
        ));
    }

    let histories = existing
        .iter()
        .map(|state| &state.live_order)
        .chain(std::iter::once(&incoming.live_order))
        .collect::<Vec<_>>();
    let flags = histories
        .iter()
        .flat_map(|history| history.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut successors = flags
        .iter()
        .map(|flag| (*flag, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = flags
        .iter()
        .map(|flag| (*flag, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for history in histories {
        for pair in history.windows(2) {
            if successors
                .get_mut(&pair[0])
                .expect("replayed live flag is indexed")
                .insert(pair[1])
            {
                indegree.entry(pair[1]).and_modify(|degree| *degree += 1);
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(flag, degree)| (*degree == 0).then_some(*flag))
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(flag) = ready.pop_first() {
        visited += 1;
        for successor in &successors[&flag] {
            let degree = indegree
                .get_mut(successor)
                .expect("replayed successor flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*successor);
            }
        }
    }
    if visited != flags.len() {
        return Err(replay_error(
            function,
            format!(
                "cleanup join at block {} has conflicting initialization histories",
                block.0
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"
module test.iterator_renewal;
@id("app.main") fn main()->i64 {
    let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), -1), 2);
    let mut output = vec_with_capacity<i64>(2usize);
    for own item in vec_into_iter<i64>(input) {
        if item > 0 { output = vec_push<i64>(output, item); 0 } else { 0 }
    }
    if vec_len<i64>(output) == 1usize { 1 } else { 0 }
}
"#;
    #[test]
    fn iterator_conditional_renewal_requires_exact_reservation_and_schema() {
        let source = crate::check(SOURCE, std::path::Path::new("renewal.spx"))
            .expect("renewal source checks");
        let program = crate::hir::resolve(&source).expect("conditional renewal resolves");
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .expect("main");
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V12);
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .any(|transition| matches!(transition, CleanupTransition::ReserveRenewal { .. })));
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .any(|transition| matches!(transition, CleanupTransition::Renew { .. })));
        validate_structure(&program, function).expect("independent renewal replay");
        let mut missing = function.clone();
        for block in &mut missing.cleanup_plan.blocks {
            block.transitions.retain(|transition| {
                !matches!(transition, CleanupTransition::ReserveRenewal { .. })
            });
        }
        assert!(validate_structure(&program, &missing).is_err());
        let mut unmarked = function.clone();
        for transition in unmarked
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
        {
            if let CleanupTransition::Renew {
                at,
                source,
                destination,
            } = transition
            {
                *transition = CleanupTransition::Transfer {
                    at: at.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                };
            }
        }
        assert!(validate_structure(&program, &unmarked).is_err());
        let mut downgraded = function.clone();
        downgraded.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V11;
        assert!(validate_structure(&program, &downgraded).is_err());
    }
}

pub(super) fn retained_units(state: &PathState) -> usize {
    state.renewals.iter().fold(0usize, |total, (at, flags)| {
        total
            .saturating_add(at.as_str().len())
            .saturating_add(flags.len())
            .saturating_add(1)
    })
}
pub(super) fn charge_transition(
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let units = match transition {
        CleanupTransition::ReserveRenewal { at, .. } => state
            .live_order
            .len()
            .saturating_add(at.as_str().len())
            .saturating_add(1),
        CleanupTransition::Renew { .. } => {
            retained_units(state).saturating_add(state.live_order.len())
        }
        _ => 0,
    };
    budget.charge(function, units, "renewal history materialization")
}
