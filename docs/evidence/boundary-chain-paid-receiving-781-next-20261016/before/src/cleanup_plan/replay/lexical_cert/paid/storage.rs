//! Same-order real budget calls for the selected certificate dictionaries/DFS.
use super::{
    BTreeMap, BTreeSet, BlockId, CleanupTerminator, ExitContinuation, ReplayBudget,
    ResolvedFunction,
};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{
    ArrayReceiving, NodeReceiving, Permit, VecReceiving,
};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;
// This original borrowed chain query still owns its original Diagnostic on
// malformed region trees; it is an explicit remaining child, never relabelled.
pub(super) use super::super::storage::chain_contains;

pub(super) fn insert<K: Ord, V>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
) -> Result<Option<V>, Error> {
    let permit = Permit::profile::<Option<V>>(function, budget)?;
    if !budget.pays_allocations() {
        return Ok(map.insert(key, value));
    }
    if !map.contains_key(&key) {
        receive!(
            function,
            budget,
            permit,
            NodeReceiving<'_>,
            8,
            budget.child_reserve_btree_node::<K, V>(function, "lexical certificate map")
        );
    }
    Ok(map.insert(key, value))
}
pub(super) fn insert_set<K: Ord>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    set: &mut BTreeSet<K>,
    key: K,
) -> Result<bool, Error> {
    let permit = Permit::profile::<bool>(function, budget)?;
    if !budget.pays_allocations() {
        return Ok(set.insert(key));
    }
    if set.contains(&key) {
        return Ok(false);
    }
    receive!(
        function,
        budget,
        permit,
        NodeReceiving<'_>,
        8,
        budget.child_reserve_btree_node::<K, ()>(function, "lexical certificate set")
    );
    Ok(set.insert(key))
}
pub(super) fn reach(
    function: &ResolvedFunction,
    start: BlockId,
    avoid: Option<BlockId>,
    stop: &BTreeSet<BlockId>,
    excluded: Option<&[bool]>,
    budget: &mut ReplayBudget,
) -> Result<Vec<bool>, Error> {
    let permit = Permit::profile::<Vec<bool>>(function, budget)?;
    if !budget.pays_allocations() {
        return Ok(super::super::reach(function, start, avoid, stop, excluded));
    }
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<bool>(
            function,
            function.cleanup_plan.blocks.len(),
            "certificate reach bitmap"
        )
    );
    let mut seen = vec![false; function.cleanup_plan.blocks.len()];
    if Some(start) == avoid {
        return Ok(seen);
    }
    seen[start.0 as usize] = true;
    let mut stack = Vec::new();
    receive!(
        function,
        budget,
        permit,
        VecReceiving<'_, BlockId>,
        10,
        budget.child_reserve_vec(function, &mut stack, 1, "certificate reach stack")
    );
    stack.push(start);
    // Exact original visitor body, expanded lexically so there is no additional
    // callback Result or newly forwarded refusal owner. skip does not stop the
    // remaining successors; a real reserve failure returns immediately as before.
    macro_rules! visit {
        ($next:expr) => {{
            let next = $next;
            let index = next.0 as usize;
            if !(seen[index]
                || Some(next) == avoid
                || stop.contains(&next)
                || excluded.is_some_and(|excluded| excluded[index]))
            {
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, BlockId>,
                    10,
                    budget.child_reserve_vec(function, &mut stack, 1, "certificate reach stack")
                );
                seen[index] = true;
                stack.push(next);
            }
        }};
    }
    while let Some(block) = stack.pop() {
        let plan = &function.cleanup_plan;
        match &plan.blocks[block.0 as usize].terminator {
            CleanupTerminator::Goto(edge) => visit!(plan.edges[edge.0 as usize].to),
            CleanupTerminator::Branch(edges) => {
                for edge in edges {
                    visit!(plan.edges[edge.0 as usize].to);
                }
            }
            CleanupTerminator::Exit(exit) => match &plan.exits[exit.0 as usize].continuation {
                ExitContinuation::Continue(edge)
                | ExitContinuation::LoopBreak { edge, .. }
                | ExitContinuation::FunctionReturn { edge, .. } => {
                    visit!(plan.edges[edge.0 as usize].to)
                }
                ExitContinuation::LoopLatch { .. } => {
                    if let Some(next) = super::super::latch_resume(plan, *exit) {
                        visit!(next);
                    }
                }
                ExitContinuation::CommitResult { .. }
                | ExitContinuation::ReturnFailure { .. }
                | ExitContinuation::ReturnUnit => {}
            },
        }
    }
    Ok(seen)
}
