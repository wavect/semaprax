//! One immutable selected replay invocation. The lease owns freshly rebuilt
//! renewal sites and borrows the current provider for its whole lifetime.
use super::*;
use crate::cleanup::ReplayContext;
pub(super) struct Context<'a> {
    source: &'a dyn ReplayContext,
    function: &'a ResolvedFunction,
    renewals: crate::cleanup_plan::ReplayRenewalBindings<'a>,
}
impl<'a> Context<'a> {
    #[cfg(test)]
    pub(super) fn new(
        source: &'a dyn ReplayContext,
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        selected_profile::validate(source, function, budget)?;
        selected_owner::owner(source, function, budget)?;
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
        if (function.cleanup_plan.schema == super::super::CLEANUP_PLAN_SCHEMA_V19)
            != renewals.has_helpers()
        {
            return Err(replay_error(
                function,
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
impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        self.source.declarations()
    }
    fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
        self.source.types()
    }
    fn type_count(&self) -> usize {
        self.source.type_count()
    }
    fn type_at(&self, index: usize) -> Option<&crate::hir::ResolvedTypeDeclaration> {
        self.source.type_at(index)
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        self.source.interfaces()
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        self.source.functions()
    }
    fn function_count(&self) -> usize {
        self.source.function_count()
    }
    fn function_at(&self, index: usize) -> Option<&ResolvedFunction> {
        self.source.function_at(index)
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        self.source.function_templates()
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        self.source.function_instances()
    }
    fn resolve_call_target(
        &self,
        callee: &crate::hir::DeclarationId,
        instance: Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        self.source.resolve_call_target(callee, instance)
    }
    fn selected_flat_records(&self) -> bool {
        true
    }
    fn primitive_renewals<'a>(
        &'a self,
        function: &ResolvedFunction,
    ) -> Option<&'a crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        std::ptr::eq(self.function, function).then_some(&self.renewals)
    }
}

#[cfg(test)]
pub(super) mod borrowed;

pub(super) mod child;
pub(super) mod paid;
#[cfg(test)]
mod paid_before_children;

pub(super) mod family;
