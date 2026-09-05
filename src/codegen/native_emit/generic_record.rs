//! Independent native admission for fully substituted generic record storage.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind};

use super::backend_error;

enum Frame {
    Enter(ResolvedType, usize),
    Leave(String),
}

pub(super) fn is_admitted(
    program: &ResolvedProgram,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut visited_fields = 0usize;
    let mut owned_leaves = 0usize;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ResolvedType::Bytes, _) => {
                owned_leaves = owned_leaves
                    .checked_add(1)
                    .ok_or_else(|| backend_error("native generic owned-leaf count overflowed"))?;
                if owned_leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return Ok(false);
                }
            }
            Frame::Enter(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return Ok(false);
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    unreachable!()
                };
                let Some(item) = program.types.iter().find(|item| item.id == *declaration) else {
                    return Ok(false);
                };
                let ResolvedTypeDeclarationKind::Record { fields } = &item.kind else {
                    return Ok(false);
                };
                if arguments.len() != item.type_parameters.len() {
                    return Ok(false);
                }
                let identity = ty.identity_key();
                if !active.insert(identity.clone()) {
                    return Ok(false);
                }
                visited_fields = visited_fields.checked_add(fields.len()).ok_or_else(|| {
                    backend_error("native generic visited-field count overflowed")
                })?;
                if visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Ok(false);
                }
                pending.push(Frame::Leave(identity));
                for field in fields.iter().rev() {
                    pending.push(Frame::Enter(
                        crate::hir::substitute_type(&field.ty, declaration, arguments)?,
                        depth + 1,
                    ));
                }
            }
            Frame::Enter(_, _) => return Ok(false),
            Frame::Leave(identity) => {
                active.remove(&identity);
            }
        }
    }
    Ok(true)
}
