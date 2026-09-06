//! Validation-only authentication of compiler-owned `Box<T>` calls.
use super::*;
pub(super) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    super::vec_intrinsic::reject_reserved_identities(program)?;
    for declaration in program.declarations.declarations() {
        if crate::box_ops::by_id(declaration.id.as_str()).is_some()
            || crate::box_ops::by_name(&declaration.name).is_some()
        {
            return Err(hir_error(format!(
                "resolved {:?} declaration `{}` aliases a compiler-owned box operation",
                declaration.kind, declaration.id
            )));
        }
    }
    for function in &program.functions {
        reject_function(function)?;
    }
    for template in &program.function_templates {
        if crate::box_ops::by_id(template.id.as_str()).is_some()
            || crate::box_ops::by_name(&template.name).is_some()
        {
            return Err(hir_error(format!(
                "resolved function template `{}` aliases a compiler-owned box operation",
                template.id
            )));
        }
    }
    for instance in &program.function_instances {
        reject_function(&instance.function)?;
    }
    Ok(())
}
pub(super) fn authenticate_owned_wrapper(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Result<bool, Diagnostic> {
    Ok(
        super::generic_template::authenticate_vec_wrapper(program, template)?.is_some()
            || super::generic_template::authenticate_box_wrapper(program, template)?.is_some(),
    )
}
fn reject_function(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if crate::box_ops::by_id(function.id.as_str()).is_some()
        || crate::box_ops::by_name(&function.name).is_some()
    {
        return Err(hir_error(format!(
            "resolved function `{}` aliases a compiler-owned box operation",
            function.id
        )));
    }
    Ok(())
}
pub(super) fn is_call(callee: &DeclarationId, instance: &Option<FunctionInstanceId>) -> bool {
    super::vec_intrinsic::is_call(callee, instance)
        || (instance.is_none() && crate::box_ops::by_id(callee.as_str()).is_some())
}
pub(super) fn is_intrinsic_id(callee: &DeclarationId) -> bool {
    crate::vec_ops::by_id(callee.as_str()).is_some()
        || crate::box_ops::by_id(callee.as_str()).is_some()
}
pub(super) fn is_type(declaration: &DeclarationId, arguments: &[ResolvedType]) -> bool {
    super::vec_intrinsic::is_type(declaration, arguments)
        || (declaration.as_str() == crate::prelude::BOX_ID
            && matches!(arguments, [element] if crate::box_ops::resolved_element_is_admitted(element)))
}
pub(super) fn signature(
    callee: &DeclarationId,
    type_arguments: &[ResolvedType],
    instance: &Option<FunctionInstanceId>,
    args: &[ResolvedExpr],
) -> Result<Option<(Vec<ResolvedParam>, ResolvedType)>, Diagnostic> {
    if let Some(signature) =
        super::vec_intrinsic::signature(callee, type_arguments, instance, args)?
    {
        return Ok(Some(signature));
    }
    let Some(op) = crate::box_ops::by_id(callee.as_str()) else {
        return Ok(None);
    };
    if instance.is_some()
        || !matches!(type_arguments, [element] if crate::box_ops::resolved_element_is_admitted(element))
        || args.len() != 1
    {
        return Err(hir_error("invalid box operation call shape"));
    }
    Ok(Some((
        crate::box_ops::resolved_params(op, &type_arguments[0]),
        op.resolved_return_type(&type_arguments[0]),
    )))
}
