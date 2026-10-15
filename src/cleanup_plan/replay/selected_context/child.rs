//! An affine receipt permit acquired on the exact subject's existing budget.
//! All current success and failure receiving controls are paid before child entry.
use super::{Context, ReplayBudget, ResolvedFunction, expression_facts, semantic_refusal};
use crate::cleanup_plan::renewal_profile::{RenewalBindings, child_refusal::Refusal};
type Error = expression_facts::errors::EngineError;
pub(super) struct Permit<'a> {
    subject: &'a ResolvedFunction,
}
impl<'a> Permit<'a> {
    pub(super) fn acquire<T>(
        subject: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        type Controls<'a, T> = (
            Permit<'a>,
            Refusal,
            Option<Refusal>,
            semantic_refusal::Receipt,
            Error,
            Option<Error>,
            Result<T, Refusal>,
            Result<T, Error>,
            Result<Permit<'a>, Error>,
            Result<Context<'a>, Error>,
            Result<(), Error>,
        );
        budget.reserve_array::<Controls<'a, T>>(subject, 1, "selected child refusal controls")?;
        budget.charge(subject, 11, "selected child refusal controls")?;
        Ok(Self { subject })
    }
    /// No new debit after work/physical failure: these full controls were paid
    /// before entry. Moving the receipt does not replace its ledger's first error.
    pub(super) fn refuse(self, reason: Refusal) -> Error {
        Error::Structural(semantic_refusal::Receipt::selected_child(
            self.subject,
            reason,
        ))
    }
}
pub(super) fn renewals<'a>(
    source: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<RenewalBindings<'a>, Error> {
    let permit = Permit::acquire::<RenewalBindings<'a>>(function, budget)?;
    // No ? can escape this closure into an unpriced Diagnostic conversion.
    let outcome = (|| {
        let ReplayBudget {
            remaining,
            physical,
            ..
        } = budget;
        let physical = physical.as_deref_mut().ok_or(Refusal::LostPhysical)?;
        let mut debit = |units| {
            if let Some(next) = remaining.checked_sub(units) {
                *remaining = next;
                Ok(())
            } else {
                *remaining = 0;
                Err(Refusal::RenewalWork)
            }
        };
        crate::cleanup_plan::renewal_profile::bindings_with_reservation_and_work_inline(
            source, function, physical, &mut debit,
        )
    })();
    outcome.map_err(|reason| permit.refuse(reason))
}

#[cfg(test)]
mod tests;
