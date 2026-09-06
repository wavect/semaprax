//! Validation-only rules for bounded generic templates.

use super::*;

pub(super) fn authenticate_vec_wrapper(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Result<Option<crate::vec_ops::VecOp>, Diagnostic> {
    let candidate = crate::vec_ops::wrapper_by_id(template.id.as_str()).is_some()
        || (program.module == crate::vec_ops::MODULE
            && crate::vec_ops::ALL
                .into_iter()
                .any(|op| crate::vec_ops::wrapper_name(op) == template.name));
    match (
        candidate,
        crate::vec_ops::hir_wrapper_in_program(program, template),
    ) {
        (false, _) => Ok(None),
        (true, Some(op)) => Ok(Some(op)),
        (true, None) => Err(hir_error(format!(
            "generic template `{}` is not an authenticated std.collections vector wrapper",
            template.id
        ))),
    }
}

pub(super) fn vec_wrapper_substitutions() -> Vec<Vec<ResolvedType>> {
    [
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::Char,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
    ]
    .into_iter()
    .map(|ty| vec![ty])
    .collect()
}

pub(super) fn validate_type(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> Result<(), Diagnostic> {
    let admitted = matches!(
        ty,
        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::String
    ) || matches!(ty, ResolvedType::TypeParameter { owner, index }
            if owner == &template.id && usize::try_from(*index).ok()
                .is_some_and(|index| index < template.type_parameters.len()))
        || super::super::type_reachability::is_nested_owned_byte_record_template(
            &program.declarations,
            ty,
            &template.id,
            template.type_parameters.len(),
        )
        || crate::vec_ops::template_type_is_admitted(template, ty);
    admitted.then_some(()).ok_or_else(|| {
        hir_error(format!(
            "generic template `{}` has an invalid direct-scalar signature slot",
            template.id
        ))
    })
}

pub(super) fn is_vec_wrapper_call(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    callee: &DeclarationId,
    type_arguments: &[ResolvedType],
    instance: &Option<FunctionInstanceId>,
) -> bool {
    instance.is_none()
        && crate::vec_ops::hir_wrapper_in_program(program, template)
            == crate::vec_ops::by_id(callee.as_str())
        && matches!(type_arguments,
            [ResolvedType::TypeParameter { owner, index: 0 }] if owner == &template.id)
}
