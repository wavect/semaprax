//! Closure construction is a scalar cleanup leaf. Its body is retained for
//! structural capacity elsewhere, then lowered as an independent callable.

use super::*;

pub(super) fn key_for_type(program: &Program, ty: &crate::ast::Type) -> CleanupTypeKey {
    match ty {
        crate::ast::Type::I64
        | crate::ast::Type::I32
        | crate::ast::Type::Char
        | crate::ast::Type::U8
        | crate::ast::Type::Usize
        | crate::ast::Type::F32
        | crate::ast::Type::F64
        | crate::ast::Type::Bool
        | crate::ast::Type::String
        | crate::ast::Type::Str
        | crate::ast::Type::ArrayU8(_)
        | crate::ast::Type::SliceU8
        | crate::ast::Type::Function { .. } => CleanupTypeKey::Scalar,
        crate::ast::Type::Bytes => CleanupTypeKey::Unknown,
        crate::ast::Type::Named { name, .. } => {
            if let Some(index) = program
                .types
                .iter()
                .position(|declaration| declaration.name == *name)
            {
                CleanupTypeKey::Declaration(index)
            } else if matches!(name.as_str(), "Option" | "Result")
                || program.types.iter().any(|declaration| {
                    declaration
                        .type_parameters
                        .iter()
                        .any(|parameter| parameter.name == *name)
                })
                || program.functions.iter().any(|function| {
                    function
                        .type_parameters
                        .iter()
                        .any(|parameter| parameter.name == *name)
                })
            {
                // Prelude Option/Result and admitted direct generic arguments
                // are Copy-only at this boundary.
                CleanupTypeKey::Scalar
            } else {
                CleanupTypeKey::Unknown
            }
        }
    }
}

pub(super) fn construction_child<'a>(
    expression: &'a crate::ast::Expr,
    cursor: &mut usize,
) -> Option<(usize, &'a crate::ast::Expr)> {
    (!matches!(expression.kind, crate::ast::ExprKind::Closure { .. }))
        .then(|| ast_child(expression, cursor))
        .flatten()
}
