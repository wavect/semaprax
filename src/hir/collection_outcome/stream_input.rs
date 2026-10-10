//! Independent replay of the exact normalization-owner to nested outcome join.
use super::*;

pub(super) fn match_result(
    index: &DeclarationIndex,
    input: &ResolvedType,
    mode: ResolvedMatchMode,
    result: &ResolvedType,
    ownership: OwnershipMode,
) -> bool {
    mode == ResolvedMatchMode::Own
        && ownership == OwnershipMode::Own
        && admitted(index, input)
        && super::nested::admitted(index, result)
}

fn admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
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
        || index.declaration(declaration).is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
        || index
            .declaration(&DeclarationId::new(format!("{declaration}#invariant")))
            .is_some()
    {
        return false;
    }
    let Some(cases) = index.variant_cases(declaration) else {
        return false;
    };
    if cases.len() != 2 {
        return false;
    }
    for (position, case) in cases.iter().enumerate() {
        if usize::try_from(case.index) != Ok(position)
            || index.declaration(&case.id).is_none_or(|d| {
                d.kind != DeclarationKind::VariantCase
                    || d.identity_origin != IdentityOrigin::Explicit
                    || d.owner.as_ref() != Some(declaration)
                    || d.name != case.name
            })
        {
            return false;
        }
        for (position, field) in case.fields.iter().enumerate() {
            if usize::try_from(field.index) != Ok(position)
                || index.declaration(&field.id).is_none_or(|d| {
                    d.kind != DeclarationKind::CaseField
                        || d.identity_origin != IdentityOrigin::Explicit
                        || d.owner.as_ref() != Some(&case.id)
                        || d.name != field.name
                })
            {
                return false;
            }
        }
    }
    let ready = |fields: &[ResolvedFieldDeclaration]| matches!(fields, [bytes, length] if bytes.ty==ResolvedType::Bytes && length.ty==ResolvedType::Usize);
    let error = |fields: &[ResolvedFieldDeclaration]| matches!(fields, [code, offset, field] if code.ty==ResolvedType::I64 && offset.ty==ResolvedType::Usize && field.ty==ResolvedType::I64);
    ready(&cases[0].fields) && error(&cases[1].fields)
        || ready(&cases[1].fields) && error(&cases[0].fields)
}

#[cfg(test)]
mod tests;
