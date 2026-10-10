//! Source-owned proof for nested records with ordinary bounded vector leaves.
use super::*;

pub(in crate::source_verify) fn vector(types: &TypeTable<'_>, ty: &Type) -> bool {
    matches!(ty, Type::Named { name, arguments }
        if name == "Vec"
        && types.declaration(name).is_some_and(|d| d.stable_id == crate::prelude::VEC_ID)
        && matches!(arguments.as_slice(), [element]
            if crate::vec_ops::ast_vec_element_is_admitted(element)
                || super::copy_record_collection::admitted(types, element)
                || super::owned_leaf_collection::runtime_element(types, element)))
}

pub(in crate::source_verify) fn admitted(root: &Type, types: &TypeTable<'_>) -> bool {
    if !matches!(root, Type::Named { name, arguments }
        if arguments.is_empty() && types.declaration(name).is_some_and(|d| matches!(d.kind, TypeDeclarationKind::Record { .. })))
    {
        return false;
    }
    enum Frame {
        Enter(Type, usize),
        Leave(String),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = HashSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut has_vector = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if vector(types, &ty) => {
                has_vector = true;
                leaves += 1;
            }
            Frame::Enter(Type::String | Type::Bytes, _) => leaves += 1,
            Frame::Enter(ty, _) if crate::vec_ops::ast_element_is_admitted(&ty) => {}
            Frame::Enter(Type::Named { name, arguments }, depth) => {
                let Some(declaration) = types.declaration(&name) else {
                    return false;
                };
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                    || !arguments.is_empty()
                    || !declaration.type_parameters.is_empty()
                    || !declaration.explicit_id
                    || !declaration.invariants().is_empty()
                    || !active.insert(name.clone())
                {
                    return false;
                }
                let TypeDeclarationKind::Record { fields: declared } = &declaration.kind else {
                    return false;
                };
                let Some(next) = fields.checked_add(declared.len()) else {
                    return false;
                };
                fields = next;
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS
                    || declared.iter().any(|field| !field.explicit_id)
                {
                    return false;
                }
                pending.push(Frame::Leave(name));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(field.ty.clone(), depth + 1)),
                );
            }
            Frame::Leave(name) => {
                active.remove(&name);
            }
            Frame::Enter(_, _) => return false,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return false;
        }
    }
    has_vector
}
