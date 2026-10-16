//! Complete original copy-only ownership receiver bodies.
use super::*;
impl HirValidator<'_> {
    #[rustfmt::skip]
    pub(in crate::hir::validation) fn expected_ownership_before(
        &self,
        ty: &ResolvedType,
        non_copy: OwnershipMode,
    ) -> Result<OwnershipMode, Diagnostic> {
        if *ty == ResolvedType::Never {
            return Ok(OwnershipMode::Value);
        }
        self.borrowed_type_facts(ty)?
            .map(|facts| {
                if facts.copy {
                    OwnershipMode::Value
                } else {
                    non_copy
                }
            })
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }
    #[rustfmt::skip]
    pub(in crate::hir::validation) fn is_owned_resource_before(
        &self,
        ty: &ResolvedType,
        ownership: OwnershipMode,
    ) -> Result<bool, Diagnostic> {
        if *ty == ResolvedType::Never {
            return Ok(false);
        }
        self.borrowed_type_facts(ty)?
            .map(|facts| !facts.copy && ownership == OwnershipMode::Own)
            .ok_or_else(|| {
                hir_error(format!(
                    "type `{}` has no semantic facts",
                    ty.identity_key()
                ))
            })
    }
}
