//! Reconstruct each scoped Vec field read from ordinary explicit declarations.
use super::{
    DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration,
    ResolvedType,
};

pub(crate) struct Field<'a> {
    pub(crate) position: usize,
    pub(crate) declaration: &'a ResolvedFieldDeclaration,
    pub(crate) layout: super::owned_leaf_collection::ElementLayout<'a>,
}
impl Field<'_> {
    pub(crate) fn result_type(&self, bytes: bool) -> Option<ResolvedType> {
        match (&self.declaration.ty, bytes) {
            (ResolvedType::String, false) => Some(ResolvedType::Str),
            (ResolvedType::String | ResolvedType::Bytes, true) => {
                (self.declaration.ty == ResolvedType::String).then_some(ResolvedType::SliceU8)
            }
            (ResolvedType::Bytes, false) => Some(ResolvedType::SliceU8),
            (ty, false) if crate::vec_ops::resolved_element_is_admitted(ty) => Some(ty.clone()),
            _ => None,
        }
    }
}

pub(crate) fn field<'a>(
    index: &'a DeclarationIndex,
    element: &ResolvedType,
    selected: &DeclarationId,
) -> Option<Field<'a>> {
    let layout = super::owned_leaf_collection::layout(index, element)?;
    let fields = layout.fields?; // Primitive String has no record field selector.
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = element
    else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    for (position, field) in fields.iter().enumerate() {
        let fact = index.declaration(&field.id)?;
        if field.index as usize != position
            || fact.kind != DeclarationKind::Field
            || fact.identity_origin != IdentityOrigin::Explicit
            || fact.owner.as_ref() != Some(declaration)
            || fact.name != field.name
        {
            return None;
        }
    }
    let position = fields
        .iter()
        .position(|candidate| &candidate.id == selected)?;
    Some(Field {
        position,
        declaration: &fields[position],
        layout,
    })
}

#[cfg(test)]
mod tests;
