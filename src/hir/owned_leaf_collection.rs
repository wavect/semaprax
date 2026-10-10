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

pub(crate) fn copy_or_leaf_admitted(index: &DeclarationIndex, element: &ResolvedType) -> bool {
    super::copy_record_collection::admitted(index, element) || layout(index, element).is_some()
}

pub(crate) fn is_copy_or_leaf_vec(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    super::copy_record_collection::is_vec(index, ty) || is_vec(index, ty)
}

pub(crate) fn vec_operation_admitted(
    index: &DeclarationIndex,
    op: crate::vec_ops::VecOp,
    element: &ResolvedType,
) -> bool {
    crate::vec_ops::resolved_operation_element_is_admitted(op, element)
        || (!op.owned_leaf_only() && super::copy_record_collection::admitted(index, element))
        || super::owned_record_collection::admits_vec_operation_element(index, op, element)
        || admits_operation(index, op, element)
}

/// Shared runtime admission preserves the older independently owned profile.
pub(crate) fn runtime_element(index: &DeclarationIndex, element: &ResolvedType) -> bool {
    layout(index, element).is_some()
        || super::owned_record_collection::is_admitted_owned_record_collection_element(
            index, element,
        )
}

pub(crate) fn capacity(index: &DeclarationIndex, element: &ResolvedType) -> u64 {
    layout(index, element).map_or_else(
        || super::copy_record_collection::capacity(index, element),
        |layout| layout.capacity(),
    )
}

/// Conservative static charge for the actual independent Bytes copies.
pub(crate) fn clone_capacity_flow(
    program: &super::ResolvedProgram,
    expression: &super::ResolvedExpr,
) -> Option<crate::byte_data_capacity::CapacityFlow> {
    let super::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        ..
    } = &expression.kind
    else {
        return None;
    };
    if callee.as_str() != crate::vec_ops::CLONE_AT_ID {
        return None;
    }
    let [element] = type_arguments.as_slice() else {
        return None;
    };
    let layout = layout(&program.declarations, element)?;
    let count = layout
        .owned_fields
        .iter()
        .flatten()
        .filter(|field| field.kind == OwnedLeafKind::Bytes)
        .count();
    Some(clone_byte_flow(expression.id.as_str(), count))
}
pub(crate) fn clone_byte_flow(site: &str, count: usize) -> crate::byte_data_capacity::CapacityFlow {
    crate::byte_data_capacity::CapacityFlow::Sequence(
        (0..count)
            .map(|index| crate::byte_data_capacity::CapacityFlow::BytesCopy {
                site: format!("{site}.owned-clone.{index}"),
                conservative_payload_bytes: crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES,
            })
            .collect(),
    )
}

pub(crate) fn function_requires_profile(
    program: &super::ResolvedProgram,
    f: &super::ResolvedFunction,
) -> bool {
    super::workspace_link::stream_owned_function_requires_profile(program, f)
}

/// Independent old-profile replay also checks scalar-signature bodies and
/// materialized callees; a Project source guard is not backend authority.
pub(crate) fn program_requires_profile(program: &super::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| function_requires_profile(program, function))
}

/// Retained source modules do not hold a DeclarationIndex. Re-derive the
/// negative old-profile guard from their authenticated declarations instead.
/// Unknown nominal elements fail closed, including imported declarations
/// omitted by an incomplete lookup. No declaration table is cloned.
pub(crate) fn function_requires_profile_by<'a>(
    function: &super::ResolvedFunction,
    lookup: impl Fn(&super::DeclarationId) -> Option<&'a super::ResolvedTypeDeclaration>,
) -> bool {
    fn carrier<'a>(
        ty: &ResolvedType,
        lookup: &impl Fn(&super::DeclarationId) -> Option<&'a super::ResolvedTypeDeclaration>,
    ) -> bool {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return false;
        };
        if !matches!(
            declaration.as_str(),
            crate::prelude::VEC_ID | crate::iterator_ops::ITER_ID | crate::iterator_ops::STEP_ID
        ) {
            return arguments.is_empty()
                && lookup(declaration).is_some_and(|item| {
                    matches!(&item.kind, super::ResolvedTypeDeclarationKind::Variant { cases }
                    if cases.iter().flat_map(|case| &case.fields).any(|field|
                        matches!(&field.ty, ResolvedType::Nominal { declaration, .. }
                            if declaration.as_str() == crate::prelude::VEC_ID)
                        && carrier(&field.ty, lookup)))
                });
        }
        let [element] = arguments.as_slice() else {
            return true;
        };
        if *element == ResolvedType::String {
            return true;
        }
        if crate::vec_ops::resolved_vec_element_is_admitted(element) {
            return false;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = element
        else {
            return true;
        };
        let Some(declared) = lookup(declaration) else {
            return true;
        };
        let super::ResolvedTypeDeclarationKind::Record { fields } = &declared.kind else {
            return true;
        };
        if !arguments.is_empty() || !declared.type_parameters.is_empty() {
            return true;
        }
        let scalar = fields
            .iter()
            .filter(|f| crate::vec_ops::resolved_element_is_admitted(&f.ty))
            .count();
        !((1..=8).contains(&fields.len()) && scalar == fields.len()
            || fields.len() == 3
                && scalar == 1
                && fields
                    .iter()
                    .filter(|f| f.ty == ResolvedType::Bytes)
                    .count()
                    == 2)
    }
    if carrier(&function.return_type, &lookup)
        || function.params.iter().any(|p| carrier(&p.ty, &lookup))
    {
        return true;
    }
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    while let Some(expr) = pending.pop() {
        if carrier(&expr.ty, &lookup)
            || matches!(expr.kind, super::ResolvedExprKind::VecFieldRead { .. })
            || matches!(&expr.kind, super::ResolvedExprKind::Call { callee, .. } if crate::vec_ops::by_id(callee.as_str()).is_some_and(|op| op.owned_leaf_only()))
        {
            return true;
        }
        super::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
    false
}

#[cfg(test)]
mod tests;

/// Direct owned-field access in a loop still receives ordinary place and loan
/// replay. The field's authenticated owner provides the bounded shape proof.
pub(crate) fn projected_leaf_admitted(
    index: &DeclarationIndex,
    place: &super::Place,
    expected: &ResolvedType,
) -> bool {
    let [super::PlaceProjection::Field(field)] = place.projections.as_slice() else {
        return false;
    };
    let Some(owner) = index
        .declaration(field)
        .and_then(|field| field.owner.as_ref())
    else {
        return false;
    };
    let record = ResolvedType::Nominal {
        declaration: owner.clone(),
        arguments: Vec::new(),
    };
    layout(index, &record).is_some_and(|layout| {
        layout.fields.is_some_and(|fields| {
            fields
                .iter()
                .any(|candidate| &candidate.id == field && &candidate.ty == expected)
        })
    })
}

#[cfg(test)]
#[path = "owned_leaf_collection/native_identity_tests.rs"]
mod native_identity_tests;
