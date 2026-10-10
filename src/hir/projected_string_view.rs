//! Authenticated nonescaping views of existing record String leaves.
use super::{
    DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, PlaceProjection, ResolvedType,
};

/// The root's ownership and availability belong to the caller's live scope.
/// This proof authenticates every nominal and field against ordinary declarations;
/// a path or cached result type alone never grants access to a String carrier.
pub(crate) fn admitted(
    index: &DeclarationIndex,
    root: &ResolvedType,
    path: &[PlaceProjection],
) -> bool {
    if path.is_empty()
        || path.len() > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
        || !super::owned_text_record::runtime_admitted(root, index)
    {
        return false;
    }
    let mut ty = root;
    for projection in path {
        let PlaceProjection::Field(id) = projection else {
            return false;
        };
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
                .is_none_or(|p| !p.is_empty())
            || index.declaration(declaration).is_none_or(|d| {
                d.kind != DeclarationKind::Record || d.identity_origin != IdentityOrigin::Explicit
            })
            || index
                .declaration(&DeclarationId::new(format!("{declaration}#invariant")))
                .is_some()
        {
            return false;
        }
        let Some(fields) = index.record_fields(declaration) else {
            return false;
        };
        let Some((position, field)) = fields.iter().enumerate().find(|(_, f)| &f.id == id) else {
            return false;
        };
        if usize::try_from(field.index) != Ok(position)
            || index.declaration(id).is_none_or(|d| {
                d.kind != DeclarationKind::Field
                    || d.identity_origin != IdentityOrigin::Explicit
                    || d.owner.as_ref() != Some(declaration)
                    || d.name != field.name
            })
        {
            return false;
        }
        ty = &field.ty;
    }
    *ty == ResolvedType::String
}

/// Loop shape checks have no local scope. Ordinary expression replay separately
/// binds the actual root and replays this same full path against that root.
pub(crate) fn path_admitted(index: &DeclarationIndex, path: &[PlaceProjection]) -> bool {
    let Some(PlaceProjection::Field(first)) = path.first() else {
        return false;
    };
    let Some(owner) = index.declaration(first).and_then(|d| d.owner.as_ref()) else {
        return false;
    };
    admitted(
        index,
        &ResolvedType::Nominal {
            declaration: owner.clone(),
            arguments: vec![],
        },
        path,
    )
}

#[cfg(test)]
mod tests;
