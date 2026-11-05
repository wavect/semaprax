//! Post-validation legacy storage reuse. Original checking/error order precedes this.
use super::*;
fn share<T: nominal_payload::NominalElement>(values: &mut NominalPayload<T>) {
    if !values.is_empty() && !values.has_shared_owner() {
        *values = NominalPayload::from_legacy_vec_unfunded(&mut **values);
    }
}
impl DeclarationIndex {
    pub(in super::super) fn share_legacy_nominal_payloads_unfunded(&mut self) {
        for values in self.type_parameters.values_mut() {
            share(values);
        }
        for values in self.record_fields.values_mut() {
            share(values);
        }
        for values in self.variant_cases.values_mut() {
            share(values);
        }
        for cases in self.variant_cases.values() {
            if !cases.is_legacy_shared() {
                continue;
            }
            for (ordinal, case) in cases.iter().enumerate() {
                // Never synthesize or overwrite a missing/different fact from
                // an ID match. Preserve the exact earlier constructor winner.
                if self.case_fields.get(&case.id).is_none_or(|fields| {
                    !nominal_payload::legacy_fields_equal(fields.as_slice(), case.fields.as_slice())
                }) {
                    continue;
                }
                self.case_fields.insert(
                    case.id.clone(),
                    CaseFields::legacy_unfunded(cases.clone(), ordinal),
                );
            }
        }
    }
}
