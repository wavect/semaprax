//! The complete ordinary optional reservation decision, without a forwarding ABI.
#[cfg(feature = "unstable-workflow-profiling")]
use super::exclusive_debits;
#[cfg(test)]
use super::stage_debits;
use super::{record_refusal, Budget};
#[track_caller]
pub(super) fn reserve_optional(budget: Option<&Budget>, length: usize) -> bool {
    #[cfg(feature = "unstable-workflow-profiling")]
    let mut path_request = exclusive_debits::path_census::Request::new(budget, length);
    let Some(budget) = budget else {
        return true;
    };
    // A zero-sized retry cannot authenticate an already refused account.
    // Positive optional writes can still consume a reserved error trailer;
    // they never clear the selected refusal or restore required admission.
    if length == 0 && budget.overflowed.get() {
        return false;
    }
    let remaining = budget.remaining.get();
    if length > remaining {
        record_refusal(budget, length);
        budget.overflowed.set(true);
        return false;
    }
    if length
        .checked_add(budget.floor.get())
        .is_none_or(|required| required > remaining)
    {
        return false;
    }
    budget.remaining.set(remaining - length);
    #[cfg(test)]
    stage_debits::debit(budget, length);
    #[cfg(feature = "unstable-workflow-profiling")]
    exclusive_debits::debit(budget, length);
    #[cfg(feature = "unstable-workflow-profiling")]
    path_request.admitted();
    true
}
