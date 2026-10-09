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

pub(in crate::source_verify) fn copy_or_leaf_admitted(
    types: &TypeTable<'_>,
    element: &Type,
) -> bool {
    super::copy_record_collection::admitted(types, element) || admitted(types, element)
}

pub(in crate::source_verify) fn vec_operation_admitted(
    types: &TypeTable<'_>,
    op: crate::vec_ops::VecOp,
    element: &Type,
) -> bool {
    crate::vec_ops::ast_operation_element_is_admitted(op, element)
        || (!op.owned_leaf_only() && super::copy_record_collection::admitted(types, element))
        || super::owned_record_collection::admits_vec_operation_element(types, op, element)
        || admits_operation(types, op, element)
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

pub(in crate::source_verify) fn runtime_element(types: &TypeTable<'_>, element: &Type) -> bool {
    admitted(types, element)
        || super::owned_record_collection::is_admitted_owned_record_collection_element(
            types, element,
        )
}

pub(in crate::source_verify) fn clone_capacity_flow(
    types: &TypeTable<'_>,
    expression: &Expr,
    path: &str,
) -> Option<crate::byte_data_capacity::CapacityFlow> {
    let ExprKind::Call {
        name,
        type_arguments,
        ..
    } = &expression.kind
    else {
        return None;
    };
    if name != crate::vec_ops::CLONE_AT_NAME {
        return None;
    }
    let [element] = type_arguments.as_slice() else {
        return None;
    };
    if !admitted(types, element) {
        return None;
    }
    let count = match element {
        Type::String => 0,
        Type::Named { name, .. } => {
            let TypeDeclarationKind::Record { fields } = &types.declaration(name)?.kind else {
                return None;
            };
            fields
                .iter()
                .filter(|field| field.ty == Type::Bytes)
                .count()
        }
        _ => return None,
    };
    Some(crate::hir::owned_leaf_collection::clone_byte_flow(
        path, count,
    ))
}

pub(crate) fn legacy_source_element(program: &Program, element: &Type) -> bool {
    super::owned_record_collection::is_admitted_authored_record_collection_element(program, element)
}
