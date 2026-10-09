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
    let uses = |ty: &ResolvedType| {
        is_vec(&program.declarations, ty)
            || super::collection_outcome::admitted(&program.declarations, ty)
    };
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            if f.params.iter().any(|p| uses(&p.ty)) || uses(&f.return_type) {
                return true;
            }
            let mut pending = std::iter::once(&f.body)
                .chain(f.requires.iter())
                .chain(f.ensures.iter())
                .collect::<Vec<_>>();
            while let Some(expression) = pending.pop() {
                if uses(&expression.ty) {
                    return true;
                }
                super::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut pending,
                );
            }
            false
        })
}
