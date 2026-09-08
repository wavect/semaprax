//! Private generic functions over exact compiler-owned Copy-element carriers.
use super::{DeclarationId, OwnershipMode, ResolvedFunctionTemplate, ResolvedType};
pub(crate) fn scalar(ty: &ResolvedType) -> bool {
    super::type_reachability::nested_record_copy_scalar_is_admitted(ty)
}
pub(crate) fn slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool {
    count == 1
        && matches!(ty, ResolvedType::Nominal { declaration, arguments }
 if matches!(declaration.as_str(), crate::prelude::BOX_ID | crate::prelude::VEC_ID)
 && matches!(arguments.as_slice(), [ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }] if parameter_owner == owner))
}
pub(crate) fn parameter(ty: &ResolvedType, owner: &DeclarationId) -> bool {
    matches!(ty, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 } if parameter_owner == owner)
}
pub(crate) fn profile(template: &ResolvedFunctionTemplate) -> bool {
    let slot = |ty: &ResolvedType| slot(ty, &template.id, template.type_parameters.len());
    let copy = |ty: &ResolvedType| scalar(ty) || parameter(ty, &template.id);
    let mut uses = slot(&template.return_type) || template.params.iter().any(|p| slot(&p.ty));
    super::visit_resolved_calls(&template.body, &mut |id, _, _| {
        uses |= crate::box_ops::by_id(id.as_str()).is_some()
            || crate::vec_ops::by_id(id.as_str()).is_some();
    });
    template.type_parameters.len() == 1
        && uses
        && (slot(&template.return_type) || copy(&template.return_type))
        && template.params.iter().all(|p| {
            (slot(&p.ty) && p.ownership == OwnershipMode::Own)
                || (copy(&p.ty) && p.ownership == OwnershipMode::Value)
        })
}
pub(crate) fn arguments(arguments: &[ResolvedType]) -> bool {
    matches!(arguments, [ty] if scalar(ty))
}
pub(crate) fn concrete_signature(function: &super::ResolvedFunction) -> bool {
    let carrier = |ty: &ResolvedType| matches!(ty, ResolvedType::Nominal { declaration, arguments } if matches!(declaration.as_str(), crate::prelude::BOX_ID | crate::prelude::VEC_ID) && self::arguments(arguments));
    (carrier(&function.return_type) || function.params.iter().any(|p| carrier(&p.ty)))
        && (carrier(&function.return_type) || scalar(&function.return_type))
        && function.params.iter().all(|p| {
            (carrier(&p.ty) && p.ownership == OwnershipMode::Own)
                || (scalar(&p.ty) && p.ownership == OwnershipMode::Value)
        })
}
