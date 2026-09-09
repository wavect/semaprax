//! Exact owned-Bytes iterator feature selection. Layout never grants admission.
use super::*;
pub(crate) fn item_ownership(element: &ResolvedType, borrowed: bool) -> OwnershipMode {
    if *element == ResolvedType::Bytes {
        if borrowed {
            OwnershipMode::Borrow
        } else {
            OwnershipMode::Own
        }
    } else {
        OwnershipMode::Value
    }
}
pub(crate) fn resolved_type_uses_owned_iterator(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            (matches!(declaration.as_str(), ITER_ID | STEP_ID)
                && arguments.as_slice() == [ResolvedType::Bytes])
                || arguments.iter().any(resolved_type_uses_owned_iterator)
        }
        ResolvedType::Function { parameters, result } => {
            parameters.iter().any(resolved_type_uses_owned_iterator)
                || resolved_type_uses_owned_iterator(result)
        }
        _ => false,
    }
}
pub(crate) fn function_uses_owned_iterator(function: &crate::hir::ResolvedFunction) -> bool {
    if resolved_type_uses_owned_iterator(&function.return_type)
        || function
            .params
            .iter()
            .any(|p| resolved_type_uses_owned_iterator(&p.ty))
    {
        return true;
    }
    let mut pending: Vec<_> = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect();
    while let Some(expr) = pending.pop() {
        if resolved_type_uses_owned_iterator(&expr.ty) {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
    false
}
pub(crate) fn template_uses_owned_iterator(
    template: &crate::hir::ResolvedFunctionTemplate,
) -> bool {
    if resolved_type_uses_owned_iterator(&template.return_type)
        || template
            .params
            .iter()
            .any(|p| resolved_type_uses_owned_iterator(&p.ty))
    {
        return true;
    }
    let mut pending: Vec<_> = template
        .requires
        .iter()
        .chain(std::iter::once(&template.body))
        .chain(&template.ensures)
        .collect();
    while let Some(expr) = pending.pop() {
        if resolved_type_uses_owned_iterator(&expr.ty) {
            return true;
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
    false
}
pub(crate) fn resolved_program_uses_owned_iterator(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_uses_owned_iterator)
}
fn ast_type(ty: &Type) -> bool {
    match ty {
        Type::Named { name, arguments } => {
            (matches!(name.as_str(), "Iter" | "IterStep") && arguments.as_slice() == [Type::Bytes])
                || arguments.iter().any(ast_type)
        }
        Type::Function { parameters, result } => {
            parameters.iter().any(ast_type) || ast_type(result)
        }
        _ => false,
    }
}
pub(crate) fn program_uses_owned_iterator(program: &crate::ast::Program) -> bool {
    fn uses_function(function: &crate::ast::Function) -> bool {
        if ast_type(&function.return_type) || function.params.iter().any(|p| ast_type(&p.ty)) {
            return true;
        }
        let mut pending: Vec<_> = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect();
        while let Some(expr) = pending.pop() {
            match &expr.kind {
                crate::ast::ExprKind::Call {
                    name,
                    type_arguments,
                    ..
                } if by_name(name).is_some() && type_arguments.as_slice() == [Type::Bytes] => {
                    return true
                }
                crate::ast::ExprKind::ConstructVariant {
                    type_name,
                    type_arguments,
                    ..
                } if type_name == "IterStep" && type_arguments.as_slice() == [Type::Bytes] => {
                    return true
                }
                _ => {}
            }
            let mut index = 0;
            while let Some(child) = expr.child(index) {
                pending.push(child);
                index += 1;
            }
        }
        false
    }
    program.functions.iter().any(uses_function)
        || program.types.iter().any(|declaration| {
            matches!(&declaration.kind, crate::ast::TypeDeclarationKind::Class { methods, .. }
                if methods.iter().any(uses_function))
        })
}
