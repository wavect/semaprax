//! Source twin: literal names select explicit fields, never runtime strings.
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Param, ParamMode, Span, Type, TypeDeclarationKind};

pub(super) fn signature(
    types: &TypeTable<'_>,
    type_arguments: &[Type],
    args: &[Expr],
    span: Span,
) -> Option<(Vec<Param>, Type)> {
    let [element] = type_arguments else {
        return None;
    };
    let [_, _, selector] = args else {
        return None;
    };
    let ExprKind::String(selected) = &selector.kind else {
        return None;
    };
    let Type::Named { name, arguments } = element else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let declaration = types.declaration(name)?;
    if !super::declared_type::owned_leaf_collection::declaration_admitted(declaration) {
        return None;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    let field = fields.iter().find(|field| &field.name == selected)?;
    let result = match field.ty {
        Type::String => Type::Str,
        Type::Bytes => Type::SliceU8,
        ref ty if crate::vec_ops::ast_element_is_admitted(ty) => ty.clone(),
        _ => return None,
    };
    Some((
        vec![
            Param {
                name: "values".into(),
                mode: ParamMode::Borrow,
                ty: Type::Named {
                    name: "Vec".into(),
                    arguments: vec![element.clone()],
                },
                span,
            },
            Param {
                name: "index".into(),
                mode: ParamMode::Value,
                ty: Type::Usize,
                span,
            },
        ],
        result,
    ))
}

pub(super) fn reserved_function(function: &crate::ast::Function) -> bool {
    crate::vec_ops::by_name(&function.name).is_some()
        || function.name == crate::vec_field::NAME
        || function.stable_id == crate::vec_field::ID
}
