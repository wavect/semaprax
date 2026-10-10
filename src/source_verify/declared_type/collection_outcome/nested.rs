//! Source-owned proof for proper outcomes carrying finite owning records.
use super::*;

pub(super) fn admitted(types: &TypeTable<'_>, declaration: &crate::ast::TypeDeclaration) -> bool {
    if !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || !declaration.invariants().is_empty()
    {
        return false;
    }
    let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
        return false;
    };
    if cases.len() != 2
        || cases
            .iter()
            .any(|case| !case.explicit_id || case.fields.iter().any(|field| !field.explicit_id))
    {
        return false;
    }
    let success = |fields: &[crate::ast::FieldDeclaration]| matches!(fields, [value] if record_payload_admitted(&value.ty, types));
    let error = |fields: &[crate::ast::FieldDeclaration]| matches!(fields, [code, offset, field] if code.ty == Type::I64 && offset.ty == Type::Usize && field.ty == Type::I64);
    success(&cases[0].fields) && error(&cases[1].fields)
        || success(&cases[1].fields) && error(&cases[0].fields)
}

fn record_payload_admitted(root: &Type, types: &TypeTable<'_>) -> bool {
    if !matches!(root, Type::Named { name, arguments }
        if arguments.is_empty() && types.declaration(name).is_some_and(|d| matches!(d.kind, TypeDeclarationKind::Record { .. })))
    {
        return false;
    }
    enum Frame {
        Enter(Type, usize),
        Leave(String),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 2)];
    let mut active = HashSet::new();
    let mut fields = 4usize;
    let mut leaves = 0usize;
    let mut owned = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if super::super::collection_record::vector(types, &ty) => {
                owned = true;
                leaves += 1;
            }
            Frame::Enter(Type::String | Type::Bytes, _) => {
                owned = true;
                leaves += 1;
            }
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
    owned
}
