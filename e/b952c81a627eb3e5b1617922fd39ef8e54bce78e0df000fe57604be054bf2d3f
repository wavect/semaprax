//! Selected-session error data, paid before the first local refusal can exist.
//! This adds no source/replay authority. Unmigrated child Diagnostics stay owned.
use super::{
    Context, ReplayBudget, ReplayContext, ResolvedFunction, expression_facts, replay_error,
    selected_owner, selected_profile, semantic_refusal,
};
type Error = expression_facts::errors::EngineError;
/// Every added full carrier is charged independently; no old-header credit.
/// Six initialized/receiving controls, including the retained optional receipt.
pub(super) fn acquire<T>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    type Controls<T> = (
        semantic_refusal::Receipt,
        Error,
        Result<T, Error>,
        Result<crate::hir::FunctionExecutionId, Error>,
        Result<(), Error>,
        Option<Error>,
    );
    budget.reserve_array::<Controls<T>>(function, 1, "selected session refusal controls")?;
    budget.charge(function, 6, "selected session refusal controls")?;
    Ok(())
}
// Identity handles are immutable shared operands; acquiring the full controls
// precedes even the source-handle clone. No successful-session surcharge.
pub(in super::super) fn refusal(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    detail: &'static str,
) -> Error {
    if let Err(error) = acquire::<Context<'_>>(function, budget) {
        return error;
    }
    Error::Structural(semantic_refusal::Receipt::selected(function, detail))
}
impl<'a> Context<'a> {
    pub(in super::super) fn new_paid_before_children(
        source: &'a dyn ReplayContext,
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
        lexical: bool,
    ) -> Result<Self, Error> {
        if lexical
            && (!crate::hir::lexical_exit::requires(function)?
                || !matches!(
                    function.cleanup_plan.schema,
                    crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
                        | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
                ))
        {
            return Err(refusal(
                function,
                budget,
                "selected retained lexical replay requires an actual lexical function",
            ));
        }
        selected_profile::paid_before_children::validate(source, function, budget)?;
        selected_owner::paid_before_children::owner(source, function, budget)?;
        budget.reserve_array::<Self>(function, 1, "selected immutable replay lease")?;
        let renewals = {
            let ReplayBudget {
                remaining,
                physical,
                ..
            } = budget;
            let physical = physical.as_deref_mut().ok_or_else(|| {
                replay_error(function, "selected replay lost physical reservation")
            })?;
            let mut debit = |units| {
                if let Some(next) = remaining.checked_sub(units) {
                    *remaining = next;
                    Ok(())
                } else {
                    *remaining = 0;
                    Err(replay_error(function, "selected renewal work exhausted"))
                }
            };
            crate::cleanup_plan::renewal_profile::bindings_with_reservation_and_work(
                source, function, physical, &mut debit,
            )?
        };
        if (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19)
            != renewals.has_helpers()
        {
            return Err(refusal(
                function,
                budget,
                "selected lexical schema disagrees with independently proved helper renewal",
            ));
        }
        Ok(Self {
            source,
            function,
            renewals,
        })
    }
}
