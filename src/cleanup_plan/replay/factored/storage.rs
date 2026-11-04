//! Precharge the factored automaton's owned build carriers before allocation.
//! Weighted canonicalization remains a separate owner and activation barrier.
use super::*;

pub(super) fn push_frame<'a>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    frames: &mut Vec<BuildFrame<'a>>,
    frame: BuildFrame<'a>,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "factored DAG build frame")?;
    budget.reserve_vec(function, frames, 1, "factored DAG build frames")?;
    #[cfg(test)]
    super::instrumentation::frame(&frame);
    frames.push(frame);
    Ok(())
}

pub(super) fn push_end(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    ends: &mut Vec<usize>,
    end: usize,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "factored DAG retained end")?;
    budget.reserve_vec(function, ends, 1, "factored DAG retained ends")?;
    ends.push(end);
    Ok(())
}

pub(super) fn memo_insert(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    memo: &mut dag_memo::Memo,
    key: dag_memo::Key,
    end: usize,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "factored DAG identity memo insertion")?;
    dag_memo::insert(function, memo, key, end, budget)
}

pub(super) fn reserve_transition_observation(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    transition: &CleanupTransition,
) -> Result<(), Diagnostic> {
    if !budget.pays_allocations() {
        return Ok(());
    }
    match transition {
        CleanupTransition::Initialize { destination, .. }
        | CleanupTransition::InitializeVariant { destination, .. } => {
            budget.reserve_clone(function, destination, "factored initialize place")?;
        }
        CleanupTransition::Transfer {
            source,
            destination,
            ..
        }
        | CleanupTransition::Renew {
            source,
            destination,
            ..
        }
        | CleanupTransition::TransferVariant {
            source,
            destination,
            ..
        } => {
            budget.reserve_clone(function, source, "factored transfer source")?;
            budget.reserve_clone(function, destination, "factored transfer destination")?;
        }
        CleanupTransition::CallCommit { arguments, .. } => {
            budget.reserve_array::<(u32, CleanupPlace)>(
                function,
                arguments.len(),
                "factored call commit arguments",
            )?;
            for argument in arguments {
                budget.reserve_clone(function, &argument.source, "factored call commit source")?;
            }
        }
        CleanupTransition::StageCopyResult { source } => {
            budget.reserve_clone(function, source, "factored staged result identity")?
        }
        CleanupTransition::ReserveRenewal { .. }
        | CleanupTransition::AuthenticateVariantCase { .. }
        | CleanupTransition::SelectFailure { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn clone_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: &Weighted,
) -> Result<Weighted, Diagnostic> {
    let work = state
        .len()
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "factored weighted state work overflowed"))?;
    budget.charge(function, work, "factored weighted state copy")?;
    budget.reserve_clone(function, state, "factored weighted state backing")?;
    Ok(state.clone())
}

#[cfg(test)]
mod tests;

// Actual complete new local/argument/receiving carriers, not an IR/result bank.
use crate::cleanup_plan::replay::physical_budget::ByteFailure;
const PAID_UNION_CALL: usize = 2 * std::mem::size_of::<Result<Option<usize>, Diagnostic>>()
    + 2 * std::mem::size_of::<Vec<&[Rc<SkeletonObservation>]>>()
    + 2 * std::mem::size_of::<&mut Automaton>()
    + 2 * std::mem::size_of::<&ResolvedFunction>()
    + 2 * std::mem::size_of::<&mut ReplayBudget>()
    + 2 * std::mem::size_of::<&[Rc<ObservationDag>]>()
    + 2 * std::mem::size_of::<usize>();
const PAID_UNION_ROOT: usize = 4 * std::mem::size_of::<Result<usize, Diagnostic>>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + 2 * std::mem::size_of::<usize>();
const PAID_UNION_ROW: usize = 2 * std::mem::size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * std::mem::size_of::<
        std::iter::Rev<std::slice::Iter<'static, &'static [Rc<SkeletonObservation>]>>,
    >()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + 2 * std::mem::size_of::<usize>();
const PAID_UNION_ITEM: usize = 2 * std::mem::size_of::<
    std::slice::Iter<'static, Rc<SkeletonObservation>>,
>() + 2 * std::mem::size_of::<&Rc<SkeletonObservation>>()
    + std::mem::size_of::<Option<usize>>()
    + 2 * std::mem::size_of::<usize>();
const PAID_UNION_SEARCH: usize = 2 * std::mem::size_of::<Result<(), ByteFailure>>()
    + 2 * std::mem::size_of::<std::slice::Iter<'static, (Label, usize)>>()
    + 2 * std::mem::size_of::<(&Label, &usize)>()
    + 2 * std::mem::size_of::<Option<usize>>()
    + 2 * std::mem::size_of::<bool>();
const PAID_UNION_NEW_EDGE: usize = 2 * std::mem::size_of::<Result<usize, Diagnostic>>()
    + 2 * std::mem::size_of::<Result<(), Diagnostic>>()
    + std::mem::size_of::<Label>()
    + std::mem::size_of::<Rc<SkeletonObservation>>()
    + std::mem::size_of::<usize>();
const PAID_UNION_PROBE: usize = 2 * std::mem::size_of::<Result<bool, ByteFailure>>()
    + 2 * std::mem::size_of::<&Rc<ObservationDag>>()
    + 2 * std::mem::size_of::<&mut Vec<&[Rc<SkeletonObservation>]>>()
    + 2 * std::mem::size_of::<&mut ReplayBudget>()
    + 2 * std::mem::size_of::<&ResolvedFunction>()
    + 2 * std::mem::size_of::<usize>()
    + 2 * std::mem::size_of::<std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 2 * std::mem::size_of::<&[Rc<SkeletonObservation>]>()
    + 2 * std::mem::size_of::<bool>();
impl Automaton {
    pub(super) fn paid_flat_union(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        start: usize,
        members: &[Rc<ObservationDag>],
    ) -> Result<Option<usize>, Diagnostic> {
        budget.reserve_bytes(function, PAID_UNION_CALL, "pure union full call controls")?;
        let mut rows = Vec::new();
        for member in members {
            if !Self::paid_union_rows(function, budget, member, 0, &mut rows)
                .map_err(ByteFailure::into_diagnostic)?
            {
                return Ok(None);
            }
        }
        // This fresh region owns every child being mutated; it never extends
        // a child reachable through an earlier DAG memo. Sharing is restricted
        // to exact same live immutable Rc observations, with no cached address.
        budget.reserve_bytes(function, PAID_UNION_ROOT, "pure union root receivers")?;
        let root = self.node(function, budget)?;
        let join = self.node(function, budget)?;
        self.epsilon(function, budget, start, root)?;
        for items in rows.iter().rev() {
            budget.reserve_bytes(function, PAID_UNION_ROW, "pure union row receivers")?;
            let mut at = root;
            for item in *items {
                budget.reserve_bytes(function, PAID_UNION_ITEM, "pure union item receivers")?;
                let mut found = None;
                for (label, target) in self.nodes[at].moves() {
                    budget
                        .reserve_compact_bytes(
                            function,
                            PAID_UNION_SEARCH,
                            "pure union identity search receivers",
                        )
                        .map_err(ByteFailure::into_diagnostic)?;
                    budget
                        .charge_compact(function, 1, "flat observation identity comparison")
                        .map_err(ByteFailure::into_diagnostic)?;
                    if let Label::Observation(existing) = label {
                        if Rc::ptr_eq(existing, item) {
                            found = Some(*target);
                            break;
                        }
                    }
                }
                if let Some(target) = found {
                    at = target;
                } else {
                    budget.reserve_bytes(
                        function,
                        PAID_UNION_NEW_EDGE,
                        "pure union node and movement receivers",
                    )?;
                    let target = self.node(function, budget)?;
                    self.movement(
                        function,
                        budget,
                        at,
                        Label::Observation(item.clone()),
                        target,
                    )?;
                    at = target;
                }
            }
            // Every original member retains its endpoint edge, duplicates too.
            self.epsilon(function, budget, at, join)?;
        }
        Ok(Some(join))
    }
    // Complete pure union only. Empty Seq wrappers are transparent. Nested
    // concatenation/nonempty suffixes keep the original builder. Both limits
    // select storage representation only, never admit or waive replay work.
    fn paid_union_rows<'a>(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        node: &'a Rc<ObservationDag>,
        depth: usize,
        rows: &mut Vec<&'a [Rc<SkeletonObservation>]>,
    ) -> Result<bool, ByteFailure> {
        budget.reserve_compact_bytes(
            function,
            PAID_UNION_PROBE,
            "pure union shape call receivers",
        )?;
        budget.charge_compact(function, 1, "pure union shape traversal")?;
        if depth >= continuation_storage::FRAME_CAPACITY {
            return Ok(false);
        }
        match &node.kind {
            ObservationDagKind::Seq(None, items) => {
                if rows.len() >= FACTORED_PATH_THRESHOLD {
                    return Ok(false);
                }
                budget.reserve_compact_vec(
                    function,
                    rows,
                    1,
                    "pure union borrowed leaf directory",
                )?;
                rows.push(items);
                Ok(true)
            }
            ObservationDagKind::Seq(Some(prefix), items) if items.is_empty() => {
                Self::paid_union_rows(function, budget, prefix, depth + 1, rows)
            }
            ObservationDagKind::Alt(members) => {
                for member in members {
                    if !Self::paid_union_rows(function, budget, member, depth + 1, rows)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
