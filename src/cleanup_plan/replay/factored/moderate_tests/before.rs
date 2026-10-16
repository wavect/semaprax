use super::*;
const FACTORED_PATH_THRESHOLD: usize = 4_096;

pub(super) fn selected_paid(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if cfg_paths <= FACTORED_PATH_THRESHOLD && semantic_paths <= FACTORED_PATH_THRESHOLD {
        return Ok(false);
    }
    match structural_control::validate_ordinary_cfg(function, budget) {
        Ok(()) => Ok(true),
        Err(error) if budget.refusal_spent || crate::bounded_output::active_refused() => Err(error),
        Err(_) => Ok(false),
    }
}
