//! Source classifier, independent of HIR nominal admission.
use super::*;

pub(in crate::source_verify) fn admitted(types: &TypeTable<'_>, ty: &Type) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    arguments.is_empty() && types.declaration(name).is_some_and(declaration_admitted)
}

pub(crate) fn declaration_admitted(declaration: &crate::ast::TypeDeclaration) -> bool {
    declaration.explicit_id
        && declaration.type_parameters.is_empty()
        && matches!(&declaration.kind, TypeDeclarationKind::Record { fields }
            if (1..=crate::hir::copy_record_collection::MAX_FIELDS).contains(&fields.len())
            && fields.iter().all(|field| crate::vec_ops::ast_element_is_admitted(&field.ty)))
}

pub(crate) fn source_admitted(program: &Program, ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments } if arguments.is_empty()
        && program.types.iter().any(|declaration| declaration.name == *name && declaration_admitted(declaration)))
}

pub(crate) fn resolved_source_admitted(program: &Program, ty: &crate::hir::ResolvedType) -> bool {
    matches!(ty, crate::hir::ResolvedType::Nominal { declaration, arguments } if arguments.is_empty()
        && program.types.iter().any(|item| item.stable_id == declaration.as_str() && declaration_admitted(item)))
}
