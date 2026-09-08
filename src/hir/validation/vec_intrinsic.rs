//! Validation-only authentication of compiler-owned `Vec<T>` calls.

use super::*;

pub(super) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for declaration in program.declarations.declarations() {
        reject_reserved_declaration(declaration)?;
    }
    for function in &program.functions {
        reject_reserved_function(function)?;
    }
    for template in &program.function_templates {
        reject_reserved_template(template)?;
    }
    for instance in &program.function_instances {
        reject_reserved_function(&instance.function)?;
    }
    Ok(())
}

pub(super) fn reject_reserved_declaration(declaration: &Declaration) -> Result<(), Diagnostic> {
    if crate::vec_ops::by_id(declaration.id.as_str()).is_some()
        || crate::vec_ops::by_name(&declaration.name).is_some()
    {
        return Err(hir_error(format!(
            "resolved {:?} declaration `{}` aliases a compiler-owned vector operation",
            declaration.kind, declaration.id
        )));
    }
    Ok(())
}

pub(super) fn reject_reserved_function(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if crate::vec_ops::by_id(function.id.as_str()).is_some()
        || crate::vec_ops::by_name(&function.name).is_some()
    {
        return Err(hir_error(format!(
            "resolved function `{}` aliases a compiler-owned vector operation",
            function.id
        )));
    }
    Ok(())
}

pub(super) fn reject_reserved_template(
    template: &ResolvedFunctionTemplate,
) -> Result<(), Diagnostic> {
    if crate::vec_ops::by_id(template.id.as_str()).is_some()
        || crate::vec_ops::by_name(&template.name).is_some()
    {
        return Err(hir_error(format!(
            "resolved function template `{}` aliases a compiler-owned vector operation",
            template.id
        )));
    }
    Ok(())
}

pub(super) fn is_type(declaration: &DeclarationId, arguments: &[ResolvedType]) -> bool {
    declaration.as_str() == crate::prelude::VEC_ID
        && arguments.len() == 1
        && crate::vec_ops::resolved_vec_element_is_admitted(&arguments[0])
}

pub(super) fn is_call(callee: &DeclarationId, instance: &Option<FunctionInstanceId>) -> bool {
    instance.is_none() && crate::vec_ops::by_id(callee.as_str()).is_some()
}

pub(super) fn signature(
    callee: &DeclarationId,
    type_arguments: &[ResolvedType],
    instance: &Option<FunctionInstanceId>,
    args: &[ResolvedExpr],
) -> Result<Option<(Vec<ResolvedParam>, ResolvedType)>, Diagnostic> {
    let Some(op) = crate::vec_ops::by_id(callee.as_str()) else {
        return Ok(None);
    };
    if instance.is_some()
        || type_arguments.len() != 1
        || !crate::vec_ops::resolved_operation_element_is_admitted(op, &type_arguments[0])
        || args.len() != op.arity()
    {
        return Err(hir_error("invalid vector operation call shape"));
    }
    Ok(Some((
        crate::vec_ops::resolved_params(op, &type_arguments[0]),
        op.resolved_return_type(&type_arguments[0]),
    )))
}
