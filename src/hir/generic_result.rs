//! Independent HIR admission for one owned Result<Bytes,E> generic carrier.
use super::{DeclarationId, OwnershipMode, ResolvedFunctionTemplate, ResolvedType};

pub(crate) fn slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool {
    count == 1
        && matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::RESULT_ID
            && matches!(arguments.as_slice(), [ResolvedType::Bytes, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }] | [ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }, ResolvedType::Bytes] if parameter_owner == owner))
}

pub(crate) fn profile(template: &ResolvedFunctionTemplate) -> bool {
    slot(
        &template.return_type,
        &template.id,
        template.type_parameters.len(),
    ) && template
        .params
        .iter()
        .filter(|p| p.ownership == OwnershipMode::Own)
        .count()
        == 1
        && template.params.iter().all(|p| {
            (p.ownership == OwnershipMode::Own && p.ty == template.return_type)
                || (p.ownership == OwnershipMode::Value
                    && matches!(p.ty, ResolvedType::I64 | ResolvedType::Bool))
        })
}

pub(crate) fn arguments(result: &ResolvedType, arguments: &[ResolvedType]) -> bool {
    if copy_success(result) && arguments == [ResolvedType::Bytes] {
        return false;
    }
    matches!(arguments, [ty] if *ty == ResolvedType::Bytes || super::type_reachability::nested_record_copy_scalar_is_admitted(ty))
}

pub(crate) fn copy_success(result: &ResolvedType) -> bool {
    matches!(result, ResolvedType::Nominal { arguments, .. } if arguments.first().is_some_and(|ty| *ty != ResolvedType::Bytes))
}

pub(crate) fn substitutions(result: &ResolvedType) -> Vec<Vec<ResolvedType>> {
    [
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::Char,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
        ResolvedType::Bytes,
    ]
    .into_iter()
    .filter(|ty| !copy_success(result) || *ty != ResolvedType::Bytes)
    .map(|ty| vec![ty])
    .collect()
}

/// Private linked carriers; public export validators retain their scalar ABI.
pub(crate) fn concrete_signature(function: &super::ResolvedFunction) -> bool {
    let carrier = |ty: &ResolvedType| {
        matches!(ty,
        ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::RESULT_ID
            && matches!(arguments.as_slice(), [ResolvedType::Bytes, error] | [error, ResolvedType::Bytes]
                if *error == ResolvedType::Bytes || super::type_reachability::nested_record_copy_scalar_is_admitted(error)))
    };
    let contains_carrier = carrier(&function.return_type)
        || function
            .params
            .iter()
            .any(|parameter| carrier(&parameter.ty));
    contains_carrier
        && (super::type_reachability::nested_record_copy_scalar_is_admitted(&function.return_type)
            || carrier(&function.return_type))
        && function.params.iter().all(|parameter| {
            (parameter.ownership == OwnershipMode::Value
                && super::type_reachability::nested_record_copy_scalar_is_admitted(&parameter.ty))
                || (parameter.ownership == OwnershipMode::Own && carrier(&parameter.ty))
        })
}
