//! Independent admission for authored generic owned variant functions.
use super::{
    DeclarationId, DeclarationIndex, FunctionExecutionId, FunctionInstanceId, OwnershipMode,
    ResolvedFunction, ResolvedFunctionTemplate, ResolvedMatchMode, ResolvedProgram, ResolvedType,
};

pub(crate) fn substitutions() -> Vec<Vec<ResolvedType>> {
    super::monomorphize::resolved_owned_record_substitutions(1)
}
pub(crate) fn arguments(arguments: &[ResolvedType]) -> bool {
    matches!(arguments, [ty] if super::type_reachability::nested_record_copy_scalar_is_admitted(ty))
}
pub(crate) fn concrete(declarations: &DeclarationIndex, ty: &ResolvedType) -> bool {
    !matches!(ty, ResolvedType::Nominal { declaration, .. }
        if matches!(declaration.as_str(), crate::prelude::OPTION_ID | crate::prelude::RESULT_ID))
        && super::type_reachability::is_admitted_concrete_owned_byte_variant(declarations, ty)
}
pub(crate) fn slot(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
    owner: &DeclarationId,
    count: usize,
) -> bool {
    if super::generic_collection::slot(ty, owner, count)
        && matches!(ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == crate::iterator_ops::STEP_ID)
    {
        return true;
    }
    count == 1
        && matches!(ty, ResolvedType::Nominal { arguments, .. }
            if matches!(arguments.as_slice(), [ResolvedType::Bytes, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }] if parameter_owner == owner))
        && substitutions().iter().all(|arguments| {
            super::monomorphize::substitute_type(ty, owner, arguments)
                .is_ok_and(|ty| concrete(declarations, &ty))
        })
}
pub(crate) fn profile(program: &ResolvedProgram, template: &ResolvedFunctionTemplate) -> bool {
    if super::generic_collection::profile(template) {
        let mut pending = vec![&template.body];
        while let Some(expression) = pending.pop() {
            if matches!(&expression.ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == crate::iterator_ops::STEP_ID)
            {
                return true;
            }
            super::push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
    }
    if super::generic_collection::profile(template) && std::iter::once(&template.return_type).chain(template.params.iter().map(|p| &p.ty)).any(|ty| matches!(ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == crate::iterator_ops::STEP_ID)) { return true; }
    let carrier = |ty: &ResolvedType| {
        slot(
            &program.declarations,
            ty,
            &template.id,
            template.type_parameters.len(),
        )
    };
    let scalar = |ty: &ResolvedType| {
        super::type_reachability::nested_record_copy_scalar_is_admitted(ty)
            || matches!(ty, ResolvedType::TypeParameter { owner, index: 0 } if owner == &template.id)
    };
    let owned = template
        .params
        .iter()
        .filter(|p| p.ownership == OwnershipMode::Own)
        .collect::<Vec<_>>();
    template.effects.is_empty()
        && owned.len() == 1
        && carrier(&owned[0].ty)
        && (carrier(&template.return_type) || scalar(&template.return_type))
        && template.params.iter().all(|p| {
            (p.ownership == OwnershipMode::Own && carrier(&p.ty))
                || (p.ownership == OwnershipMode::Value && scalar(&p.ty))
        })
}

/// Authenticate full substituted function meaning, including proof-only
/// substitutions that have not entered the discovered executable cache yet.
pub(crate) fn bounded_template<'a>(
    program: &'a ResolvedProgram,
    function: &ResolvedFunction,
) -> Option<&'a ResolvedFunctionTemplate> {
    let mut matched = None;
    for template in &program.function_templates {
        if !profile(program, template) {
            continue;
        }
        let exact = template_substitutions(template).iter().any(|arguments| {
            super::monomorphize::materialize_function_template(template, arguments).is_ok_and(
                |expected| super::monomorphize::same_function_meaning(&expected, function),
            )
        });
        if exact {
            if matched.is_some() {
                return None;
            }
            matched = Some(template);
        }
    }
    matched
}
pub(crate) fn match_result(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    mode: ResolvedMatchMode,
    ty: &ResolvedType,
    ownership: OwnershipMode,
) -> bool {
    if iterator_match_result(program, mode, ty, ownership) {
        return true;
    }
    bounded_template(program, function).is_some()
        && ((ownership == OwnershipMode::Value
            && super::type_reachability::nested_record_copy_scalar_is_admitted(ty))
            || (mode == ResolvedMatchMode::Own
                && ownership == OwnershipMode::Own
                && concrete(&program.declarations, ty)))
}
pub(crate) fn match_result_execution(
    program: &ResolvedProgram,
    execution: &FunctionExecutionId,
    mode: ResolvedMatchMode,
    ty: &ResolvedType,
    ownership: OwnershipMode,
) -> bool {
    if iterator_match_result(program, mode, ty, ownership) {
        return true;
    }
    let FunctionExecutionId::Generic(id) = execution else {
        return false;
    };
    program
        .function_templates
        .iter()
        .filter(|template| profile(program, template))
        .any(|template| {
            template_substitutions(template).iter().any(|arguments| {
                FunctionInstanceId::derive(&template.id, arguments) == *id
                    && ((ownership == OwnershipMode::Value
                        && super::type_reachability::nested_record_copy_scalar_is_admitted(ty))
                        || (mode == ResolvedMatchMode::Own
                            && ownership == OwnershipMode::Own
                            && concrete(&program.declarations, ty)))
            })
        })
}

pub(crate) fn symbolic_slot(ty: &ResolvedType, owner: &DeclarationId, count: usize) -> bool {
    if super::generic_collection::slot(ty, owner, count)
        && matches!(ty, ResolvedType::Nominal { declaration, .. } if declaration.as_str() == crate::iterator_ops::STEP_ID)
    {
        return true;
    }
    count == 1
        && matches!(ty,ResolvedType::Nominal{arguments,..} if matches!(arguments.as_slice(),[ResolvedType::Bytes,ResolvedType::TypeParameter{owner:parameter_owner,index:0}] if parameter_owner==owner))
}

/// Re-derive the private two-argument carrier from retained declarations. The
/// workspace separately authenticates each declaration's explicit provenance.
pub(crate) fn concrete_signature(
    types: &[super::ResolvedTypeDeclaration],
    function: &ResolvedFunction,
) -> bool {
    let scalar = super::type_reachability::nested_record_copy_scalar_is_admitted;
    let carrier = |ty: &ResolvedType| {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return false;
        };
        if matches!(
            declaration.as_str(),
            crate::prelude::OPTION_ID | crate::prelude::RESULT_ID
        ) || !matches!(arguments.as_slice(),[ResolvedType::Bytes,marker] if scalar(marker))
        {
            return false;
        }
        let Some(item) = types.iter().find(|item| item.id == *declaration) else {
            return false;
        };
        let super::ResolvedTypeDeclarationKind::Variant { cases } = &item.kind else {
            return false;
        };
        if item.type_parameters.len() != 2 || cases.is_empty() {
            return false;
        }
        let mut owned_cases = 0;
        for case in cases {
            let mut owned = false;
            for field in &case.fields {
                let Ok(ty) =
                    super::monomorphize::substitute_type(&field.ty, declaration, arguments)
                else {
                    return false;
                };
                if ty == ResolvedType::Bytes {
                    owned = true;
                } else if !scalar(&ty) {
                    return false;
                }
            }
            owned_cases += usize::from(owned);
        }
        owned_cases == 1
    };
    (carrier(&function.return_type)
        || function
            .params
            .iter()
            .any(|parameter| carrier(&parameter.ty)))
        && (carrier(&function.return_type) || scalar(&function.return_type))
        && function.params.iter().all(|parameter| {
            (parameter.ownership == OwnershipMode::Own && carrier(&parameter.ty))
                || (parameter.ownership == OwnershipMode::Value && scalar(&parameter.ty))
        })
}

// Iterator v1 retains the same checked owning-variant reconstruction path.
fn iterator_match_result(
    program: &ResolvedProgram,
    mode: ResolvedMatchMode,
    ty: &ResolvedType,
    ownership: OwnershipMode,
) -> bool {
    mode == ResolvedMatchMode::Own
        && ownership == OwnershipMode::Own
        && crate::iterator_ops::step_shape(&program.declarations, ty)
}

fn template_substitutions(template: &ResolvedFunctionTemplate) -> Vec<Vec<ResolvedType>> {
    if super::generic_collection::profile(template) {
        super::generic_collection::substitutions(template.type_parameters.len())
    } else {
        substitutions()
    }
}
