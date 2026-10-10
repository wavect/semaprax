//! Independent source shape check for direct bounded collection outcomes.
use super::*;

mod nested;
mod owned;

pub(in crate::source_verify) fn runtime_admitted(types: &TypeTable<'_>, ty: &Type) -> bool {
    admitted(types, ty)
        || matches!(ty, Type::Named { name, arguments }
        if arguments.is_empty() && types.declaration(name)
            .is_some_and(|declaration| owned::admitted(types, declaration) || nested::admitted(types, declaration)))
}

pub(in crate::source_verify) fn runtime_declaration_admitted(
    types: &TypeTable<'_>,
    declaration: &crate::ast::TypeDeclaration,
) -> bool {
    declaration_admitted(types, declaration)
        || owned::admitted(types, declaration)
        || nested::admitted(types, declaration)
}

pub(in crate::source_verify) fn vec_admitted(types: &TypeTable<'_>, ty: &Type) -> bool {
    matches!(ty, Type::Named{name,arguments} if name=="Vec" && matches!(arguments.as_slice(),[element] if super::copy_record_collection::admitted(types,element)))
}
pub(in crate::source_verify) fn admitted(types: &TypeTable<'_>, ty: &Type) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    if !arguments.is_empty() {
        return false;
    }
    let Some(declaration) = types.declaration(name) else {
        return false;
    };
    declaration_admitted(types, declaration)
}
pub(in crate::source_verify) fn declaration_admitted(
    types: &TypeTable<'_>,
    declaration: &crate::ast::TypeDeclaration,
) -> bool {
    if !declaration.explicit_id || !declaration.type_parameters.is_empty() {
        return false;
    }
    let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
        return false;
    };
    if cases.len() != 2 {
        return false;
    }
    let mut owners = 0;
    for case in cases {
        if !(1..=8).contains(&case.fields.len()) {
            return false;
        }
        for field in &case.fields {
            if vec_admitted(types, &field.ty) {
                owners += 1;
            } else if !crate::vec_ops::ast_element_is_admitted(&field.ty) {
                return false;
            }
        }
    }
    (1..=2).contains(&owners)
}
