//! An affine receipt permit acquired on the exact subject's existing budget.
//! All current success and failure receiving controls are paid before child entry.
use super::{Context, ReplayBudget, ResolvedFunction, expression_facts, semantic_refusal};
use crate::cleanup_plan::renewal_profile::{RenewalBindings, child_refusal::Refusal};
type Error = expression_facts::errors::EngineError;
pub(in super::super) struct Permit<'a> {
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

/// Sealed producer provenance, not new paid-report or replay authority.
/// The private field can only be made by the real ReplayBudget methods below.
pub(in super::super) struct BudgetDiagnostic(crate::diagnostic::Diagnostic);
/// Only calls on ReplayBudget may construct this budget arm.
/// Finite semantic producers carry Copy data, never a Diagnostic conversion.
pub(in super::super) enum Deferred {
    Budget(BudgetDiagnostic),
    Finite(Refusal),
}
impl From<Refusal> for Deferred {
    fn from(reason: Refusal) -> Self {
        Self::Finite(reason)
    }
}
impl<'a> Permit<'a> {
    pub(in super::super) fn profile<T>(
        subject: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        type Controls<T> = (
            Deferred,
            Option<Deferred>,
            Result<T, Deferred>,
            Result<T, Deferred>,
        );
        budget.reserve_array::<Controls<T>>(subject, 1, "selected profile child controls")?;
        // One initializer unit for each of the four real Controls fields.
        const PROFILE_CONTROL_INITIALIZERS: usize = 4;
        budget.charge(
            subject,
            PROFILE_CONTROL_INITIALIZERS,
            "selected profile child controls",
        )?;
        Self::acquire::<T>(subject, budget)
    }
    pub(in super::super) fn resolve<T>(self, result: Result<T, Deferred>) -> Result<T, Error> {
        match result {
            Ok(value) => Ok(value),
            Err(Deferred::Finite(reason)) => Err(self.refuse(reason)),
            Err(Deferred::Budget(error)) => Err(Error::Owned(error.0)),
        }
    }
}

impl ReplayBudget {
    pub(in super::super) fn child_charge(
        &mut self,
        function: &ResolvedFunction,
        units: usize,
        phase: &str,
    ) -> Result<(), Deferred> {
        self.charge(function, units, phase)
            .map_err(|error| Deferred::Budget(BudgetDiagnostic(error)))
    }
    pub(in super::super) fn child_budget_refusal(
        &mut self,
        function: &ResolvedFunction,
        reason: &str,
        phase: &str,
    ) -> Deferred {
        Deferred::Budget(BudgetDiagnostic(
            self.budget_refusal(function, reason, phase),
        ))
    }
}
