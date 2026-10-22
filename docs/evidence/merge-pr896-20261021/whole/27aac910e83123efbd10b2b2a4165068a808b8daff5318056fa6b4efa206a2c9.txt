//! An affine receipt permit acquired on the exact subject's existing budget.
//! All current success and failure receiving controls are paid before child entry.
use super::{expression_facts, semantic_refusal, Context, ReplayBudget, ResolvedFunction};
use crate::cleanup_plan::renewal_profile::{child_refusal::Refusal, RenewalBindings};
type Error = expression_facts::errors::EngineError;
pub(in super::super) struct Permit<'a> {
    subject: &'a ResolvedFunction,
}
impl<'a> Permit<'a> {
    pub(in super::super) fn acquire<T>(
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
    pub(in super::super) fn refuse(self, reason: Refusal) -> Error {
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

// Actual wrapper argument, inner-result and error-transfer slots. The caller
// acquires each full tuple on its SAME budget before forming wrapper arguments.
pub(in super::super) type ArrayReceiving<'a> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    usize,
    &'a str,
    Result<(), crate::diagnostic::Diagnostic>,
    crate::diagnostic::Diagnostic,
    BudgetDiagnostic,
    Deferred,
    Result<(), Deferred>,
);
pub(in super::super) type NodeReceiving<'a> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    &'a str,
    Result<(), crate::diagnostic::Diagnostic>,
    crate::diagnostic::Diagnostic,
    BudgetDiagnostic,
    Deferred,
    Result<(), Deferred>,
);
impl ReplayBudget {
    #[track_caller]
    pub(in super::super) fn child_reserve_array<T>(
        &mut self,
        function: &ResolvedFunction,
        count: usize,
        phase: &str,
    ) -> Result<(), Deferred> {
        self.reserve_array::<T>(function, count, phase)
            .map_err(|error| Deferred::Budget(BudgetDiagnostic(error)))
    }
    #[track_caller]
    pub(in super::super) fn child_reserve_btree_node<K, V>(
        &mut self,
        function: &ResolvedFunction,
        phase: &str,
    ) -> Result<(), Deferred> {
        self.reserve_btree_node::<K, V>(function, phase)
            .map_err(|error| Deferred::Budget(BudgetDiagnostic(error)))
    }
}

// Additional actual same-ledger primitive calls for the complete structural
// family. No caller may manufacture BudgetDiagnostic from a child Diagnostic.
pub(in super::super) type CloneReceiving<'a, T> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    &'a T,
    &'a str,
    Result<(), crate::diagnostic::Diagnostic>,
    crate::diagnostic::Diagnostic,
    BudgetDiagnostic,
    Deferred,
    Result<(), Deferred>,
);
pub(in super::super) type VecReceiving<'a, T> = (
    &'a mut ReplayBudget,
    &'a ResolvedFunction,
    &'a mut Vec<T>,
    usize,
    &'a str,
    Result<(), crate::diagnostic::Diagnostic>,
    crate::diagnostic::Diagnostic,
    BudgetDiagnostic,
    Deferred,
    Result<(), Deferred>,
);
impl ReplayBudget {
    #[track_caller]
    pub(in super::super) fn child_reserve_clone<T: super::super::ClonePayload>(
        &mut self,
        function: &ResolvedFunction,
        value: &T,
        phase: &str,
    ) -> Result<(), Deferred> {
        self.reserve_clone(function, value, phase)
            .map_err(|error| Deferred::Budget(BudgetDiagnostic(error)))
    }
    #[track_caller]
    pub(in super::super) fn child_reserve_vec<T>(
        &mut self,
        function: &ResolvedFunction,
        values: &mut Vec<T>,
        additional: usize,
        phase: &str,
    ) -> Result<(), Deferred> {
        self.reserve_vec(function, values, additional, phase)
            .map_err(|error| Deferred::Budget(BudgetDiagnostic(error)))
    }
}
pub(in super::super) mod receiving;

impl<'a> Permit<'a> {
    /// Actual new finite reason and child result slots, plus the existing full
    /// EngineError/receipt receiver. Uses this invocation's existing budget.
    pub(in super::super) fn family_receiver<T>(
        subject: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        budget.reserve_array::<FamilyReceiving<'_>>(
            subject,
            1,
            "selected family finite receiving",
        )?;
        budget.charge(subject, 9, "selected family finite receiving")?;
        // Preserve the complete original nine-role invoice without layout credit.
        budget.reserve_array::<FamilySourceCloneReceiving<'_>>(
            subject,
            1,
            "selected family source clone receiver",
        )?;
        budget.charge(subject, 1, "selected family source clone receiver")?;
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            subject,
            FamilyCallReceiving<'_>,
            3,
            "selected family acquire call receiving",
            Self::acquire::<T>(subject, budget)
        )
    }
    // Complete pre-formal generation, independently compiled at its old fees.
    #[cfg(test)]
    pub(in super::super) fn family_receiver_before_formals<T>(
        subject: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        budget.reserve_array::<BeforeFamilyReceiving<'_>>(
            subject,
            1,
            "selected family finite receiving",
        )?;
        budget.charge(subject, 9, "selected family finite receiving")?;
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            subject,
            FamilyCallReceiving<'_>,
            3,
            "selected family acquire call receiving",
            Self::acquire::<T>(subject, budget)
        )
    }
    pub(in super::super) fn family(&self, reason: super::family::Reason) -> Error {
        Error::Structural(semantic_refusal::Receipt::family(self.subject, reason))
    }
}

// Actual extra family()/Receipt::family() arguments, construction values,
// source-clone result, nested receipt return, error return, and
// this wrapper's acquire-result slot.
// The original Permit receiver below this layer is still paid independently.
pub(in super::super) type FamilyReceiving<'a> = (
    &'a Permit<'a>,
    super::family::Reason,
    &'a ResolvedFunction,
    super::family::Reason,
    crate::hir::ExpressionId,
    semantic_refusal::Atom,
    semantic_refusal::Receipt,
    Error,
    Result<Permit<'a>, Error>,
);

// This additional actual terminal borrower is paid separately: original nine
// fields, their old full physical invoice and their old work remain intact.
pub(in super::super) type FamilySourceCloneReceiving<'a> = &'a crate::hir::ExpressionId;

#[cfg(test)]
pub(in super::super) type BeforeFamilyReceiving<'a> = (
    &'a Permit<'a>,
    super::family::Reason,
    &'a ResolvedFunction,
    super::family::Reason,
    crate::hir::ExpressionId,
    semantic_refusal::Atom,
    semantic_refusal::Receipt,
    Error,
    Result<Permit<'a>, Error>,
);

// Distinct wrapper callee formals and final return; neither the outer actual
// arguments nor the inner acquire actual arguments cover these source roles.
pub(in super::super) type WrapperFormalReceiving<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, Error>,
);

// Caller-side arguments and result, admitted lexically BEFORE a new family or
// acquire invocation forms them; distinct from each callee's retained roles.
pub(in super::super) type FamilyCallReceiving<'a> = (
    &'a ResolvedFunction,
    &'a mut ReplayBudget,
    Result<Permit<'a>, Error>,
);

impl<'a> Permit<'a> {
    pub(in super::super) fn control_receiver<T>(
        subject: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        budget.reserve_array::<ControlReceiving<'_>>(
            subject,
            1,
            "selected control finite receiving",
        )?;
        budget.charge(subject, 12, "selected control finite receiving")?;
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            subject,
            FamilyCallReceiving<'_>,
            3,
            "selected control acquire call receiving",
            Self::acquire::<T>(subject, budget)
        )
    }
    // Complete pre-formal generation, independently compiled at its old fees.
    #[cfg(test)]
    pub(in super::super) fn control_receiver_before_formals<T>(
        subject: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        budget.reserve_array::<ControlReceiving<'_>>(
            subject,
            1,
            "selected control finite receiving",
        )?;
        budget.charge(subject, 12, "selected control finite receiving")?;
        crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
            budget,
            subject,
            FamilyCallReceiving<'_>,
            3,
            "selected control acquire call receiving",
            Self::acquire::<T>(subject, budget)
        )
    }
    pub(in super::super) fn control(
        &self,
        reason: super::super::structural_control::selected::reason::Reason,
    ) -> Error {
        Error::Structural(semantic_refusal::Receipt::control(self.subject, reason))
    }
}
pub(in super::super) type ControlReceiving<'a> = (
    &'a Permit<'a>,
    super::super::structural_control::selected::reason::Reason,
    &'a ResolvedFunction,
    super::super::structural_control::selected::reason::Reason,
    // Dynamic missing-ID clone receiver/result coexist with the terminal
    // body's source clone; these are distinct from both moved Reason roles.
    &'a crate::hir::ExpressionId,
    crate::hir::ExpressionId,
    &'a crate::hir::ExpressionId,
    crate::hir::ExpressionId,
    semantic_refusal::Atom,
    semantic_refusal::Receipt,
    Error,
    Result<Permit<'a>, Error>,
);
