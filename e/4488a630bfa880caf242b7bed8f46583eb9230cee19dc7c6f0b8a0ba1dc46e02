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
