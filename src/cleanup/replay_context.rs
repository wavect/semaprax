//! Borrowed typed-HIR authority used by independent cleanup replay.
//! A context lends current declarations and bodies; it never lends a verdict.
use crate::hir::*;

pub(crate) trait ReplayContext {
    fn declarations(&self) -> &DeclarationIndex;
    fn types(&self) -> crate::hir::items::View<'_, ResolvedTypeDeclaration>;
    /// Types may be borrowed from several authenticated defining modules.
    /// The view accessor remains the local ordered projection, including sparse selections.
    fn type_count(&self) -> usize {
        self.types().len()
    }
    fn type_at(&self, index: usize) -> Option<&ResolvedTypeDeclaration> {
        self.types().get(index)
    }
    fn interfaces(&self) -> &[ResolvedInterface];
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction>;
    /// Indexed borrowed members may come from several held defining modules.
    fn function_count(&self) -> usize {
        self.functions().len()
    }
    fn function_at(&self, index: usize) -> Option<&ResolvedFunction> {
        self.functions().get(index)
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate];
    fn function_instances(&self) -> &[ResolvedFunctionInstance];
    #[cfg(test)]
    fn borrowed_helper_lease<'a>(
        &'a self,
        _function: &ResolvedFunction,
    ) -> Option<crate::cleanup_plan::ReplayBorrowedLease<'a>> {
        None
    }
    #[cfg(test)]
    fn borrowed_helper_identity(&self) -> Option<&crate::cleanup_plan::ReplayBorrowedIdentity> {
        None
    }
    /// Only the selected immutable replay lease implements this method. It
    /// lends freshly rederived, budgeted sites for one exact held function;
    /// ordinary and detached contexts never lend attached-plan authority.
    fn primitive_renewals<'a>(
        &'a self,
        _function: &ResolvedFunction,
    ) -> Option<&'a crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        None
    }
    /// Routing marker only: every nominal use independently checks its current
    /// direct fields; no attached plan or declaration digest grants a verdict.
    fn selected_flat_records(&self) -> bool {
        false
    }
    /// Only the source-authenticated static receiver requests the closed Bytes
    /// iterator preflight. Complete cleanup and lexical replay still run.
    fn retained_static_byte_iterators(&self) -> bool {
        false
    }
    /// Only an exact-source checked allocation projection can lend this opaque
    /// current member. Ordinary/decoded contexts do not select allocation replay.
    fn allocation_function<'a>(
        &'a self,
        _function: &'a ResolvedFunction,
    ) -> Option<crate::cleanup_plan::checked_allocation::Function<'a>> {
        None
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        None
    }
    fn resolve_call_target(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        match instance {
            None => self
                .functions()
                .iter()
                .find(|function| function.id == *callee),
            Some(id) => self
                .function_instances()
                .iter()
                .find(|candidate| candidate.id == *id && candidate.template == *callee)
                .map(|candidate| &candidate.function),
        }
    }
}
impl ReplayContext for ResolvedProgram {
    fn declarations(&self) -> &DeclarationIndex {
        &self.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, ResolvedTypeDeclaration> {
        crate::hir::items::View::slice(&self.types)
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &self.interfaces
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::View::slice(&self.functions)
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &self.function_templates
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &self.function_instances
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        Some(self)
    }
}
