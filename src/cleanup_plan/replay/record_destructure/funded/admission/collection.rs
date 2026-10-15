use super::*;
use std::collections::BTreeSet;
pub(super) fn collection(
    root: &ResolvedType,
    index: &DeclarationIndex,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if !matches!(root, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.declaration(declaration).is_some_and(|d| d.kind == DeclarationKind::Record))
    {
        return Ok(false);
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType, usize),
        Leave(&'a DeclarationId),
    }
    let mut pending = Vec::new();
    budget.reserve_vec(function, &mut pending, 1, "collection admission root")?;
    pending.push(Frame::Enter(root, 1));
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut has_vector = false;
    while let Some(frame) = pending.pop() {
        budget.charge(function, 1, "collection admission frame")?;
        match frame {
            Frame::Enter(ty, _) if vector(index, ty) => {
                has_vector = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String | ResolvedType::Bytes, _) => leaves += 1,
            Frame::Enter(ty, _) if crate::vec_ops::resolved_element_is_admitted(ty) => {}
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
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || index.declaration(declaration).is_none_or(|d| {
                        d.kind != DeclarationKind::Record
                            || d.identity_origin != IdentityOrigin::Explicit
                    })
                    || index.has_record_invariant(declaration)
                    || active.contains(declaration)
                {
                    return Ok(false);
                }
                budget.reserve_btree_node::<&DeclarationId, ()>(
                    function,
                    "collection admission active node",
                )?;
                active.insert(declaration);
                let Some(declared) = index.record_fields(declaration) else {
                    return Ok(false);
                };
                let Some(next) = fields.checked_add(declared.len()) else {
                    return Ok(false);
                };
                fields = next;
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Ok(false);
                }
                for (position, field) in declared.iter().enumerate() {
                    if usize::try_from(field.index) != Ok(position)
                        || index.declaration(&field.id).is_none_or(|d| {
                            d.kind != DeclarationKind::Field
                                || d.identity_origin != IdentityOrigin::Explicit
                                || d.owner.as_ref() != Some(declaration)
                                || d.name != field.name
                        })
                    {
                        return Ok(false);
                    }
                }
                budget.reserve_vec(
                    function,
                    &mut pending,
                    declared.len().saturating_add(1),
                    "collection admission child frames",
                )?;
                pending.push(Frame::Leave(declaration));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(&field.ty, depth + 1)),
                );
            }
            Frame::Leave(declaration) => {
                active.remove(declaration);
            }
            Frame::Enter(_, _) => return Ok(false),
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return Ok(false);
        }
    }
    Ok(has_vector)
}
