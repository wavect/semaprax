//! Additive private owned-collection outcome; the Copy-only v29 shape is frozen.
use super::*;

pub(crate) fn admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    if !arguments.is_empty()
        || index
            .type_parameters(declaration)
            .is_none_or(|parameters| !parameters.is_empty())
        || index.declaration(declaration).is_none_or(|item| {
            item.kind != DeclarationKind::Variant
                || item.identity_origin != IdentityOrigin::Explicit
        })
    {
        return false;
    }
    let Some(cases) = index.variant_cases(declaration) else {
        return false;
    };
    if cases.len() != 2
        || cases.iter().any(|case| {
            index
                .declaration(&case.id)
                .is_none_or(|item| item.identity_origin != IdentityOrigin::Explicit)
                || case.fields.iter().any(|field| {
                    index
                        .declaration(&field.id)
                        .is_none_or(|item| item.identity_origin != IdentityOrigin::Explicit)
                })
        })
    {
        return false;
    }
    let success = |fields: &[ResolvedFieldDeclaration]| {
        (1..=2).contains(&fields.len())
            && fields
                .iter()
                .all(|field| owned_leaf_collection::is_vec(index, &field.ty))
    };
    let error = |fields: &[ResolvedFieldDeclaration]| {
        matches!(fields, [code, offset, field]
        if code.ty == ResolvedType::I64 && offset.ty == ResolvedType::Usize && field.ty == ResolvedType::I64)
    };
    success(&cases[0].fields) && error(&cases[1].fields)
        || success(&cases[1].fields) && error(&cases[0].fields)
}

#[cfg(test)]
mod tests;
