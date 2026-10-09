//! Authenticated, target-neutral layout of the additive owned-leaf Vec slice.
//!
//! This descriptor does not widen the frozen two-Bytes record predicate or
//! primitive Vec<Bytes> operation set. Call sites choose admission separately.
use super::{
    DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration, ResolvedType,
};

pub(crate) const MAX_FIELDS: usize = 8;
pub(crate) const MAX_OWNED_FIELDS: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnedLeafKind {
    String,
    Bytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnedField {
    pub(crate) index: usize,
    pub(crate) kind: OwnedLeafKind,
}

/// Positions are declaration positions, never owner IDs or physical handles.
/// `fields == None` denotes primitive String; its sole leaf has position zero.
#[derive(Clone, Copy)]
pub(crate) struct ElementLayout<'a> {
    pub(crate) fields: Option<&'a [ResolvedFieldDeclaration]>,
    pub(crate) owned_fields: [Option<OwnedField>; MAX_OWNED_FIELDS],
    pub(crate) owned_count: usize,
    pub(crate) scalar_count: usize,
}

impl ElementLayout<'_> {
    pub(crate) fn capacity(self) -> u64 {
        // Preserve the existing independent scalar-storage and owned-carrier
        // envelopes. Payload contents retain their ordinary String/Bytes caps.
        let scalar = if self.scalar_count == 0 {
            crate::vec_ops::MAX_CAPACITY
        } else {
            crate::vec_ops::MAX_CAPACITY / self.scalar_count as u64
        };
        let owned = crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
            / (self.owned_count as u64 * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT);
        crate::vec_ops::MAX_CAPACITY.min(scalar).min(owned)
    }
}

pub(crate) fn layout<'a>(
    index: &'a DeclarationIndex,
    element: &ResolvedType,
) -> Option<ElementLayout<'a>> {
    if *element == ResolvedType::String {
        return Some(ElementLayout {
            fields: None,
            owned_fields: [
                Some(OwnedField {
                    index: 0,
                    kind: OwnedLeafKind::String,
                }),
                None,
            ],
            owned_count: 1,
            scalar_count: 0,
        });
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = element
    else {
        return None;
    };
    let declared = index.declaration(declaration)?;
    if !arguments.is_empty()
        || declared.kind != DeclarationKind::Record
        || declared.identity_origin != IdentityOrigin::Explicit
        || !index.type_parameters(declaration)?.is_empty()
    {
        return None;
    }
    let fields = index.record_fields(declaration)?;
    if !(1..=MAX_FIELDS).contains(&fields.len()) {
        return None;
    }
    let mut owned_fields = [None; MAX_OWNED_FIELDS];
    let mut owned_count = 0;
    let mut scalar_count = 0;
    for (position, field) in fields.iter().enumerate() {
        let declared_field = index.declaration(&field.id)?;
        if declared_field.kind != DeclarationKind::Field
            || declared_field.identity_origin != IdentityOrigin::Explicit
        {
            return None;
        }
        let kind = match field.ty {
            ResolvedType::String => Some(OwnedLeafKind::String),
            ResolvedType::Bytes => Some(OwnedLeafKind::Bytes),
            _ if crate::vec_ops::resolved_element_is_admitted(&field.ty) => {
                scalar_count += 1;
                None
            }
            _ => return None,
        };
        if let Some(kind) = kind {
            *owned_fields.get_mut(owned_count)? = Some(OwnedField {
                index: position,
                kind,
            });
            owned_count += 1;
        }
    }
    (owned_count != 0).then_some(ElementLayout {
        fields: Some(fields),
        owned_fields,
        owned_count,
        scalar_count,
    })
}

pub(crate) fn is_vec(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::VEC_ID
        && matches!(arguments.as_slice(), [element] if layout(index, element).is_some()))
}

pub(crate) fn admits_operation(
    index: &DeclarationIndex,
    op: crate::vec_ops::VecOp,
    element: &ResolvedType,
) -> bool {
    op.admits_owned_leaf() && layout(index, element).is_some()
}
