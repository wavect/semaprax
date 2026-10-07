//! Private text/collection imports; public and frozen Project selectors stay separate.
use super::*;

pub(super) fn admitted(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Some(function) = target.function else {
        return false;
    };
    if !function.type_parameters.is_empty()
        || function.yields.is_some()
        || function.follows.is_some()
    {
        return false;
    }
    let shape = |ty: &Type| shape(target.module, ty, caller, authored, programs);
    let added = |ty: &Type| {
        crate::map_ops::ast_collection(ty)
            || *ty == Type::String
            || (matches!(ty, Type::Named { .. }) && shape(ty) == Some(true))
    };
    let selected = added(&function.return_type) || function.params.iter().any(|p| added(&p.ty));
    selected
        && shape(&function.return_type).is_some()
        && function.params.iter().all(|p| {
            if crate::map_ops::ast_collection(&p.ty) {
                return matches!(p.mode, ParamMode::Own | ParamMode::Borrow);
            }
            match &p.ty {
                Type::String => matches!(p.mode, ParamMode::Value | ParamMode::Own),
                Type::Bytes => p.mode == ParamMode::Own,
                Type::Str | Type::SliceU8 => p.mode == ParamMode::Borrow,
                ty if scalar(ty) || matches!(ty, Type::ArrayU8(_)) => p.mode == ParamMode::Value,
                _ => match shape(&p.ty) {
                    Some(true) => matches!(p.mode, ParamMode::Own | ParamMode::Borrow),
                    Some(false) => p.mode == ParamMode::Value,
                    None => false,
                },
            }
        })
}

pub(super) fn record_import(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Some(declaration) = target.ty else {
        return false;
    };
    let ty = Type::Named {
        name: declaration.name.clone(),
        arguments: Vec::new(),
    };
    shape(target.module, &ty, caller, authored, programs) == Some(true)
}

// Re-derive a bounded explicit record closure, including each exposed type import.
// The boolean distinguishes owners from Copy records; no generic substitution occurs.
fn shape<'a>(
    module: &'a str,
    root: &'a Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<bool> {
    enum Frame<'a> {
        Enter(&'a str, &'a Type, usize),
        Leave(String),
    }
    let mut pending = vec![Frame::Enter(module, root, 1)];
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut owns = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(_, ty, _)
                if crate::map_ops::ast_collection(ty)
                    || matches!(ty, Type::String | Type::Bytes) =>
            {
                owns = true;
                leaves += 1;
            }
            Frame::Enter(_, ty, depth)
                if scalar(ty) || (depth == 1 && matches!(ty, Type::ArrayU8(_))) => {}
            Frame::Enter(module, Type::Named { name, arguments }, depth) => {
                if !arguments.is_empty() || depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return None;
                }
                let id = resolve_type_id(module, name, programs)?;
                let target = authored.get(id.as_str())?;
                let declaration = target.ty?;
                if !target.explicit
                    || !declaration.explicit_id
                    || !declaration.type_parameters.is_empty()
                    || !declaration.invariants().is_empty()
                    || !active.insert(id.clone())
                {
                    return None;
                }
                if caller.module != target.module
                    && !caller
                        .module_uses
                        .iter()
                        .any(|u| u.kind == ModuleUseKind::Type && u.persistent_id == id)
                {
                    return None;
                }
                let TypeDeclarationKind::Record { fields: declared } = &declaration.kind else {
                    return None;
                };
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return None;
                }
                pending.push(Frame::Leave(id));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|f| Frame::Enter(target.module, &f.ty, depth + 1)),
                );
            }
            Frame::Leave(id) => {
                active.remove(&id);
            }
            _ => return None,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return None;
        }
    }
    Some(owns)
}
