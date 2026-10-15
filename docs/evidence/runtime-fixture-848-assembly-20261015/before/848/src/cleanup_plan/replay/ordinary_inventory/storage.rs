//! Same-invocation physical construction helpers; no forecast credit.
use super::super::{Diagnostic, ReplayBudget, ResolvedFunction};

pub(in crate::cleanup_plan::replay) fn array<T>(
    function: &ResolvedFunction,
    count: usize,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<Vec<T>, Diagnostic> {
    budget.reserve_array::<T>(function, count, phase)?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| {
        budget.budget_refusal(
            function,
            "cleanup replay physical vector allocation failed",
            "",
        )
    })?;
    if values.capacity() != count {
        return Err(budget.budget_refusal(
            function,
            "cleanup replay physical vector capacity disagrees with its reservation",
            "",
        ));
    }
    Ok(values)
}

/// The value's payload has already completed its own paid constructor. Only
/// this vector's actual growth is acquired here, before publishing the element.
pub(in crate::cleanup_plan::replay) fn push<T>(
    values: &mut Vec<T>,
    value: T,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    phase: &str,
) -> Result<(), Diagnostic> {
    budget.reserve_vec(function, values, 1, phase)?;
    values.push(value);
    Ok(())
}
