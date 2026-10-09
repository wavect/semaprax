//! Independently authenticated flat Copy-record Vec element profile.
use super::{
    DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration, ResolvedType,
};

pub(crate) const MAX_FIELDS: usize = 8;

pub(crate) fn fields<'a>(
    index: &'a DeclarationIndex,
    ty: &ResolvedType,
) -> Option<&'a [ResolvedFieldDeclaration]> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    let item = index.declaration(declaration)?;
    if !arguments.is_empty()
        || item.kind != DeclarationKind::Record
        || item.identity_origin != IdentityOrigin::Explicit
        || !index.type_parameters(declaration)?.is_empty()
    {
        return None;
    }
    let fields = index.record_fields(declaration)?;
    if !(1..=MAX_FIELDS).contains(&fields.len())
        || fields
            .iter()
            .any(|field| !crate::vec_ops::resolved_element_is_admitted(&field.ty))
    {
        return None;
    }
    Some(fields)
}

pub(crate) fn admitted(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    fields(index, ty).is_some()
}

pub(crate) fn is_vec(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::VEC_ID
        && matches!(arguments.as_slice(), [element] if admitted(index, element)))
}

pub(crate) fn capacity(index: &DeclarationIndex, ty: &ResolvedType) -> u64 {
    fields(index, ty).map_or(crate::vec_ops::MAX_CAPACITY, |fields| {
        crate::vec_ops::MAX_CAPACITY / fields.len() as u64
    })
}

pub(crate) fn program_uses(program: &super::ResolvedProgram) -> bool {
    program.functions.iter().chain(program.function_instances.iter().map(|instance| &instance.function)).any(|function| {
        function.params.iter().any(|param| is_vec(&program.declarations, &param.ty))
            || is_vec(&program.declarations, &function.return_type)
            || std::iter::once(&function.body).chain(function.requires.iter()).chain(function.ensures.iter()).any(|root| {
                let mut found = false;
                super::visit_resolved_calls(root, &mut |callee, instance, types| {
                    found |= instance.is_none() && crate::vec_ops::by_id(callee.as_str()).is_some()
                        && matches!(types, [element] if admitted(&program.declarations, element));
                });
                found
            })
    })
}
