//! Complete original admission predicate, including owning invariant query.
use super::*;

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
