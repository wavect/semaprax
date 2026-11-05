//! Acquires canonical owners after the complete original index/type checks.
use super::*;
fn shared<T: nominal_payload::NominalElement, E>(
    values: &mut NominalPayload<T>,
    reserve: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), E> {
    // This private funded producer only receives its freshly constructed
    // Owned payloads or its explicitly funded compiler bundle. Never invoke
    // Legacy COW before a reservation by treating it as a paid source.
    assert!(
        !values.is_legacy_shared(),
        "legacy nominal promotion requires explicit paid materialization"
    );
    if values.is_shared() {
        return reserve(0);
    }
    if values.is_empty() {
        // Empty metadata owns no payload backing to share. Moving its legacy
        // Vec carrier avoids a control block per monomorphic function.
        return reserve(std::mem::size_of::<NominalPayload<T>>());
    }
    let owner = NominalPayload::from_funded_vec(&mut **values, reserve)?;
    *values = owner;
    Ok(())
}
impl DeclarationIndex {
    pub(super) fn drain_legacy_type_roots(&mut self, mut dispose: impl FnMut(ResolvedType)) {
        for (_, fields) in std::mem::take(&mut self.case_fields) {
            fields.drain(&mut dispose);
        }
        for (_, fields) in std::mem::take(&mut self.record_fields) {
            if !fields.has_shared_owner() {
                fields.drain_recursive_types(&mut dispose);
            }
        }
        for (_, cases) in std::mem::take(&mut self.variant_cases) {
            if !cases.has_shared_owner() {
                cases.drain_recursive_types(&mut dispose);
            }
        }
    }

    pub(in crate::hir) fn record_payload(
        &self,
        owner: &DeclarationId,
    ) -> Option<&NominalPayload<ResolvedFieldDeclaration>> {
        self.record_fields.get(owner)
    }
    pub(in crate::hir) fn case_payload(
        &self,
        owner: &DeclarationId,
    ) -> Option<&NominalPayload<ResolvedVariantCaseDeclaration>> {
        self.variant_cases.get(owner)
    }
    pub(in crate::hir) fn parameter_payload(
        &self,
        owner: &DeclarationId,
    ) -> Option<&NominalPayload<ResolvedTypeParameterDeclaration>> {
        self.type_parameters.get(owner)
    }

    pub(super) fn share_nominal_payloads<E>(
        &mut self,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        for values in self.type_parameters.values_mut() {
            shared(values, reserve)?;
        }
        for values in self.record_fields.values_mut() {
            shared(values, reserve)?;
        }
        for values in self.variant_cases.values_mut() {
            shared(values, reserve)?;
        }
        for cases in self.variant_cases.values() {
            for (ordinal, case) in cases.iter().enumerate() {
                reserve(std::mem::size_of::<CaseFields>())?;
                self.case_fields.insert(
                    case.id.clone(),
                    CaseFields::canonical(cases.clone(), ordinal),
                );
            }
        }
        Ok(())
    }
}

mod legacy;
