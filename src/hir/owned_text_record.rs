//! Independent additive internal-record profile; public Bytes ABIs remain frozen.
use super::{DeclarationIndex, DeclarationKind, ResolvedType};
use std::collections::BTreeSet;

pub(crate) fn admitted(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { .. }) {
        return false;
    }
    enum Frame {
        Enter(ResolvedType, usize),
        Leave(String),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut text = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ResolvedType::String, _) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::Bytes, _) => {
                leaves += 1;
            }
            Frame::Enter(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::Char
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return false;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    unreachable!()
                };
                if declarations
                    .declaration(&super::DeclarationId::new(format!(
                        "{declaration}#invariant"
                    )))
                    .is_some()
                    || !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|d| d.kind != DeclarationKind::Record)
                {
                    return false;
                }
                let Some(declared) = declarations.record_fields(declaration) else {
                    return false;
                };
                let identity = ty.identity_key();
                if !active.insert(identity.clone()) {
                    return false;
                }
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return false;
                }
                pending.push(Frame::Leave(identity));
                pending.extend(
                    declared
                        .iter()
                        .rev()
                        .map(|field| Frame::Enter(field.ty.clone(), depth + 1)),
                );
            }
            Frame::Leave(identity) => {
                active.remove(&identity);
            }
            Frame::Enter(_, _) => return false,
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return false;
        }
    }
    text
}

/// Select a String-bearing record from exact declaration fields. This is a
/// separate walk from admission: hostile fields cannot hide behind a failed
/// or truncated admission traversal.
pub(crate) fn contains_string(root: &ResolvedType, declarations: &DeclarationIndex) -> bool {
    if !matches!(root, ResolvedType::Nominal { declaration, .. }
        if declarations.declaration(declaration).is_some_and(|item| item.kind == DeclarationKind::Record))
    {
        return false;
    }
    let mut pending = vec![root.clone()];
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if ty == ResolvedType::String {
            return true;
        }
        if !visited.insert(ty.clone()) {
            continue;
        }
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        {
            if let Some(fields) = declarations.record_fields(&declaration) {
                for field in fields {
                    if let Ok(ty) = super::substitute_type(&field.ty, &declaration, &arguments) {
                        pending.push(ty);
                    }
                }
            }
        }
    }
    false
}

pub(crate) fn validate_use(
    root: &ResolvedType,
    declarations: &DeclarationIndex,
) -> Result<(), crate::diagnostic::Diagnostic> {
    if contains_string(root, declarations) && !admitted(root, declarations) {
        return Err(super::hir_error(
            "String-bearing executable record is outside the bounded owned-text profile",
        ));
    }
    Ok(())
}

/// Additive owning-record selection for runtime String helpers. The ordinary
/// scalar signature traversal stays unchanged.
pub(crate) fn program_uses_strings(program: &super::ResolvedProgram) -> bool {
    program.types.iter().any(|declaration| {
        admitted(
            &ResolvedType::Nominal {
                declaration: declaration.id.clone(),
                arguments: Vec::new(),
            },
            &program.declarations,
        )
    })
}
