//! Source-owned shape proof, independent of the resolved descriptor.
use super::*;

pub(crate) fn declaration_admitted(declaration: &crate::ast::TypeDeclaration) -> bool {
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return false;
    };
    if !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || !(1..=8).contains(&fields.len())
        || fields.iter().any(|field| !field.explicit_id)
    {
        return false;
    }
    let mut owned = 0;
    for field in fields {
        match field.ty {
            Type::String | Type::Bytes => owned += 1,
            _ if crate::vec_ops::ast_element_is_admitted(&field.ty) => {}
            _ => return false,
        }
    }
    (1..=2).contains(&owned)
}

pub(in crate::source_verify) fn admitted(types: &TypeTable<'_>, element: &Type) -> bool {
    *element == Type::String
        || matches!(element, Type::Named { name, arguments }
            if arguments.is_empty()
            && types.declaration(name).is_some_and(declaration_admitted))
}

pub(in crate::source_verify) fn admits_operation(
    types: &TypeTable<'_>,
    op: crate::vec_ops::VecOp,
    element: &Type,
) -> bool {
    op.admits_owned_leaf() && admitted(types, element)
}

pub(crate) fn source_admitted(program: &Program, element: &Type) -> bool {
    *element == Type::String
        || matches!(element, Type::Named { name, arguments }
            if arguments.is_empty()
            && program.types.iter().any(|item| item.name == *name && declaration_admitted(item)))
}

pub(crate) fn resolved_source_admitted(
    program: &Program,
    element: &crate::hir::ResolvedType,
) -> bool {
    *element == crate::hir::ResolvedType::String
        || matches!(element, crate::hir::ResolvedType::Nominal { declaration, arguments }
            if arguments.is_empty()
            && program.types.iter().any(|item|
                item.stable_id == declaration.as_str() && declaration_admitted(item)))
}
