//! Actual selective source/reuse receiver with one invocation-owned base lease.
use super::*;
#[derive(Debug)]
pub(crate) enum BaseSelectiveFailure {
    Project(Vec<Diagnostic>),
    Resolve(crate::hir::identity_resolution::NominalReceiverFailure),
}
impl FrontendPass {
    pub(crate) fn resolve_borrowed_cold_source_paid_with_base<'p>(
        &self,
        path: &str,
        synthetic: &'p dyn crate::ast::program_source::ProgramSource,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
        invocation: &mut crate::hir::prelude_bundle::Invocation<Refused>,
        owner: &mut IdentityReservation,
    ) -> Option<Result<SelectiveResolvedModule, BaseSelectiveFailure>> {
        if !self.permits_borrowed_cold_source(path) {
            return None;
        }
        self.resolve_borrowed_source_paid_with_base(path, synthetic, private, invocation, owner)
    }

    pub(crate) fn resolve_borrowed_source_paid_with_base<'p>(
        &self,
        path: &str,
        synthetic: &'p dyn crate::ast::program_source::ProgramSource,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
        invocation: &mut crate::hir::prelude_bundle::Invocation<Refused>,
        owner: &mut IdentityReservation,
    ) -> Option<Result<SelectiveResolvedModule, BaseSelectiveFailure>> {
        if !self.semantic {
            return None;
        }
        if synthetic.functions().len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(BaseSelectiveFailure::Project(capacity(
                "checked function inventory exceeds its bound",
            ))));
        }
        let previous = self
            .checked
            .get(path)
            .and_then(|entry| entry.synthetic.ready());
        Some(
            crate::hir::identity_resolution::resolve_source_checked_previous_with_base(
                synthetic,
                previous
                    .as_ref()
                    .map(|source| source as &dyn crate::ast::program_source::ProgramSource),
                self.checked.get(path).map(|entry| &entry.resolved),
                self.checked.get(path).map(|entry| &entry.function_costs),
                private,
                invocation,
                |bytes| owner.reserve(bytes),
            )
            .map(module)
            .map_err(BaseSelectiveFailure::Resolve),
        )
    }

    pub(crate) fn resolve_functions_paid_with_base<'p>(
        &self,
        path: &str,
        synthetic: &'p crate::ast::Program,
        private: Option<crate::source_verify::private_names::PrivateNames<'p>>,
        invocation: &mut crate::hir::prelude_bundle::Invocation<Refused>,
        owner: &mut IdentityReservation,
    ) -> Option<Result<SelectiveResolvedModule, BaseSelectiveFailure>> {
        if !self.semantic {
            return None;
        }
        if synthetic.functions.len() > MAX_PROJECT_CHECKED_FUNCTIONS {
            return Some(Err(BaseSelectiveFailure::Project(capacity(
                "checked function inventory exceeds its bound",
            ))));
        }
        let entry = self.checked.get(path);
        let previous = entry.and_then(|entry| entry.synthetic.ready());
        Some(
            crate::hir::identity_resolution::resolve_source_checked_previous_with_base(
                synthetic,
                previous
                    .as_ref()
                    .map(|source| source as &dyn crate::ast::program_source::ProgramSource),
                entry.map(|entry| &entry.resolved),
                entry.map(|entry| &entry.function_costs),
                private,
                invocation,
                |bytes| owner.reserve(bytes),
            )
            .map(module)
            .map_err(BaseSelectiveFailure::Resolve),
        )
    }
}
