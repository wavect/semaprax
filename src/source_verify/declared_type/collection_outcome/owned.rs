//! Source-owned proof for finite outcomes carrying actual owned collections.
use super::*;

pub(super) fn admitted(types: &TypeTable<'_>, declaration: &crate::ast::TypeDeclaration) -> bool {
    if !declaration.explicit_id || !declaration.type_parameters.is_empty() {
        return false;
    }
    let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
        return false;
    };
    if cases.len() != 2
        || cases
            .iter()
            .any(|case| !case.explicit_id || case.fields.iter().any(|field| !field.explicit_id))
    {
        return false;
    }
    let success = |fields: &[crate::ast::FieldDeclaration]| {
        (1..=2).contains(&fields.len())
            && fields.iter().all(|field| {
                matches!(&field.ty, Type::Named { name, arguments } if name == "Vec"
                && matches!(arguments.as_slice(), [element]
                    if super::super::owned_leaf_collection::admitted(types, element)))
            })
    };
    let error = |fields: &[crate::ast::FieldDeclaration]| {
        matches!(fields, [code, offset, field]
        if code.ty == Type::I64 && offset.ty == Type::Usize && field.ty == Type::I64)
    };
    success(&cases[0].fields) && error(&cases[1].fields)
        || success(&cases[1].fields) && error(&cases[0].fields)
}
