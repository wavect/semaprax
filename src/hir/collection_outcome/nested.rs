//! Proper owning outcomes over independently authenticated acyclic records.
use crate::hir::{DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedType};
use std::collections::BTreeSet;
mod profile;
pub(crate) use profile::{function_requires_profile_by, program_requires_profile};

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
            .is_none_or(|p| !p.is_empty())
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
    let success = |fields: &[crate::hir::ResolvedFieldDeclaration]| matches!(fields, [value] if record_payload_admitted(index, &value.ty));
    let error = |fields: &[crate::hir::ResolvedFieldDeclaration]| {
        matches!(fields, [code,offset,field]
        if code.ty == ResolvedType::I64 && offset.ty == ResolvedType::Usize && field.ty == ResolvedType::I64)
    };
    success(&cases[0].fields) && error(&cases[1].fields)
        || success(&cases[1].fields) && error(&cases[0].fields)
}

pub(crate) fn record_payload_admitted(index: &DeclarationIndex, root: &ResolvedType) -> bool {
    record_shape_admitted(index, root, true)
}

/// Copy-only nested helper records in the new profile use the same explicit
/// declaration walk, never cached Copy facts as authority.
pub(crate) fn copy_record_helper_admitted(index: &DeclarationIndex, root: &ResolvedType) -> bool {
    record_shape_admitted(index, root, false)
}

fn record_shape_admitted(
    index: &DeclarationIndex,
    root: &ResolvedType,
    expected_owning: bool,
) -> bool {
    if !matches!(root, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.declaration(declaration).is_some_and(|d| d.kind == DeclarationKind::Record))
    {
        return false;
    }
    enum Frame {
        Enter(ResolvedType, usize),
        Leave(DeclarationId),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 2)];
    let mut active = BTreeSet::new();
    let mut fields = 4usize; // One success field plus the three scalar error fields.
    let mut leaves = 0usize;
    let mut owned = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if super::super::owned_collection_record::vector(index, &ty) => {
                owned = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String | ResolvedType::Bytes, _) => {
                owned = true;
                leaves += 1;
            }
            Frame::Enter(ty, _) if crate::vec_ops::resolved_element_is_admitted(&ty) => {}
            Frame::Enter(
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                },
                depth,
            ) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                    || !arguments.is_empty()
                    || index
                        .type_parameters(&declaration)
                        .is_none_or(|p| !p.is_empty())
                    || index.declaration(&declaration).is_none_or(|d| {
                        d.kind != DeclarationKind::Record
                            || d.identity_origin != IdentityOrigin::Explicit
                    })
                    || index
                        .declaration(&DeclarationId::new(format!("{declaration}#invariant")))
                        .is_some()
                    || !active.insert(declaration.clone())
                {
                    return false;
                }
                let Some(declared) = index.record_fields(&declaration) else {
                    return false;
                };
                let Some(next) = fields.checked_add(declared.len()) else {
                    return false;
                };
                fields = next;
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return false;
                }
                for (position, field) in declared.iter().enumerate() {
                    if usize::try_from(field.index) != Ok(position)
                        || index.declaration(&field.id).is_none_or(|d| {
                            d.kind != DeclarationKind::Field
                                || d.identity_origin != IdentityOrigin::Explicit
                                || d.owner.as_ref() != Some(&declaration)
                                || d.name != field.name
                        })
                    {
                        return false;
                    }
                }
                pending.push(Frame::Leave(declaration));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(field.ty.clone(), depth + 1)),
                );
            }
            Frame::Leave(declaration) => {
                active.remove(&declaration);
            }
            Frame::Enter(_, _) => return false,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return false;
        }
    }
    owned == expected_owning
}

/// Every cleanup/layout use must bind the exact active case and payload field.
pub(crate) fn record_field(
    index: &DeclarationIndex,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
) -> bool {
    admitted(index, container)
        && record_payload_admitted(index, ty)
        && matches!(container, ResolvedType::Nominal { declaration, arguments }
            if arguments.is_empty() && index.variant_cases(declaration).is_some_and(|cases|
                cases.iter().any(|candidate| &candidate.id == case && candidate.fields.iter().any(|candidate| &candidate.id == field && &candidate.ty == ty))))
}

#[cfg(test)]
mod tests;
