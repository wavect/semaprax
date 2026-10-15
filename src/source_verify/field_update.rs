//! Independent source signature; the last literal is metadata, never an owner.
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Type};
pub(super) fn field_type(
    types: &TypeTable<'_>,
    type_arguments: &[Type],
    args: &[Expr],
) -> Option<Type> {
    let [Type::Named { name, arguments }] = type_arguments else {
        return None;
    };
    let [_, _, _, literal] = args else {
        return None;
    };
    let ExprKind::String(name_field) = &literal.kind else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let declaration = types.declaration(name)?;
    crate::vec_ops::field_update::source(declaration, name_field)?;
    let crate::ast::TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    Some(fields.iter().find(|f| f.name == *name_field)?.ty.clone())
}

pub(super) fn prepare(
    types: &TypeTable<'_>,
    current: &crate::ast::Function,
    expression: &Expr,
    program: &dyn crate::ast::program_source::ProgramSource,
) -> Result<Option<Type>, crate::diagnostic::Diagnostic> {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    let span = expression.span;
    if name != crate::vec_ops::field_update::NAME {
        return Ok(None);
    }
    if !current.type_parameters.is_empty() {
        return Err(super::diagnostics::error(
            program,
            "SPX-T320",
            crate::vec_ops::field_update::SHAPE,
            span,
        ));
    }
    field_type(types, type_arguments, args)
        .map(Some)
        .ok_or_else(|| {
            super::diagnostics::error(
                program,
                "SPX-T320",
                crate::vec_ops::field_update::SHAPE,
                span,
            )
        })
}
pub(super) fn params(
    op: crate::vec_ops::VecOp,
    element: &Type,
    owned_element: bool,
    field: Option<&Type>,
) -> Vec<crate::ast::Param> {
    let mut params = crate::vec_ops::ast_params_with_owned_element(op, element, owned_element);
    if let Some(field) = field {
        params[2].ty = field.clone();
    }
    params
}
