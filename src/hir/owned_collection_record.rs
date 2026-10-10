//! Independent internal record composition; Vec element profiles stay frozen.
use super::{DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedType};
use std::collections::BTreeSet;

pub(crate) fn vector(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::VEC_ID
        && index.declaration(declaration).is_some_and(|d|
            d.kind == DeclarationKind::Record && d.identity_origin == IdentityOrigin::CompilerOwned)
        && matches!(arguments.as_slice(), [element]
            if crate::vec_ops::resolved_vec_element_is_admitted(element)
                || super::copy_record_collection::admitted(index, element)
                || super::owned_leaf_collection::runtime_element(index, element)))
}

pub(crate) fn admitted(root: &ResolvedType, index: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.declaration(declaration).is_some_and(|d| d.kind == DeclarationKind::Record))
    {
        return false;
    }
    enum Frame {
        Enter(ResolvedType, usize),
        Leave(DeclarationId),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut has_vector = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if vector(index, &ty) => {
                has_vector = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String | ResolvedType::Bytes, _) => leaves += 1,
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
    has_vector
}

/// Negative frozen-profile guard over authenticated module type declarations.
/// This grants no admission; missing or excessive reachable record inventories fail closed.
/// Ordinary declaration replay independently rejects recursive record cycles.
pub(crate) fn function_requires_profile_by<'a>(
    function: &super::ResolvedFunction,
    lookup: impl Fn(&DeclarationId) -> Option<&'a super::ResolvedTypeDeclaration>,
) -> bool {
    fn contains<'a>(
        root: &ResolvedType,
        lookup: &impl Fn(&DeclarationId) -> Option<&'a super::ResolvedTypeDeclaration>,
    ) -> bool {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = root
        else {
            return false;
        };
        if crate::prelude::is_compiler_owned_id(declaration.as_str()) {
            return false;
        }
        let Some(item) = lookup(declaration) else {
            return true;
        };
        if !matches!(item.kind, super::ResolvedTypeDeclarationKind::Record { .. }) {
            return false;
        }
        let mut pending = vec![(declaration.clone(), arguments.clone())];
        let mut visited = BTreeSet::new();
        let mut work = 0usize;
        while let Some((id, arguments)) = pending.pop() {
            if !visited.insert((id.clone(), arguments.clone())) {
                continue;
            }
            let Some(item) = lookup(&id) else { return true };
            let super::ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
                continue;
            };
            work = match work.checked_add(fields.len()) {
                Some(n) if n <= crate::cleanup::MAX_CLEANUP_VISITED_FIELDS => n,
                _ => return true,
            };
            for field in fields {
                let Ok(ty) = super::substitute_type(&field.ty, &id, &arguments) else {
                    return true;
                };
                if let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty
                {
                    if declaration.as_str() == crate::prelude::VEC_ID {
                        return true;
                    }
                    if !crate::prelude::is_compiler_owned_id(declaration.as_str()) {
                        pending.push((declaration, arguments));
                    }
                }
            }
        }
        false
    }
    if contains(&function.return_type, &lookup)
        || function.params.iter().any(|p| contains(&p.ty, &lookup))
    {
        return true;
    }
    let mut found = false;
    super::function_value::walk(function, |expr| found |= contains(&expr.ty, &lookup));
    found
}

pub(crate) fn function_requires_profile(
    program: &super::ResolvedProgram,
    function: &super::ResolvedFunction,
) -> bool {
    function_requires_profile_by(function, |id| {
        program.types.iter().find(|item| item.id == *id)
    })
}

pub(crate) fn program_requires_profile(program: &super::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|function| function_requires_profile(program, function))
}

/// Authenticate the full stable field path. The ordinary place validator also
/// binds the first field's record to the expression's lexical root binding.
pub(crate) fn projected_field(
    index: &DeclarationIndex,
    place: &super::Place,
    expected: &ResolvedType,
) -> bool {
    let Some(super::PlaceProjection::Field(first)) = place.projections.first() else {
        return false;
    };
    let Some(root) = index
        .declaration(first)
        .and_then(|field| field.owner.as_ref())
    else {
        return false;
    };
    let mut ty = ResolvedType::Nominal {
        declaration: root.clone(),
        arguments: Vec::new(),
    };
    if !admitted(&ty, index) {
        return false;
    }
    for projection in &place.projections {
        let super::PlaceProjection::Field(id) = projection else {
            return false;
        };
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &ty
        else {
            return false;
        };
        let Some(field) = index
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|f| f.id == *id))
        else {
            return false;
        };
        let Ok(next) = super::substitute_type(&field.ty, declaration, arguments) else {
            return false;
        };
        ty = next;
    }
    ty == *expected
}

pub(crate) fn borrowed_vec_argument(
    program: &super::ResolvedProgram,
    call: &super::ResolvedExpr,
    position: usize,
    argument: &super::ResolvedExpr,
) -> bool {
    let super::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        ..
    } = &call.kind
    else {
        return false;
    };
    let (Some(op), [element]) = (
        crate::vec_ops::by_id(callee.as_str()),
        type_arguments.as_slice(),
    ) else {
        return false;
    };
    op.param_ownership_for(position, element) == super::OwnershipMode::Borrow
        && vector(&program.declarations, &argument.ty)
        && matches!(&argument.kind, super::ResolvedExprKind::Place(place)
            if projected_field(&program.declarations, place, &argument.ty))
}

#[cfg(test)]
mod tests;
