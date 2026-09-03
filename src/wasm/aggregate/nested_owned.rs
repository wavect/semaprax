//! Closed bounded classification for executable owned-byte records.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, DeclarationKind, ResolvedProgram, ResolvedType};

enum Frame {
    Enter(ResolvedType, usize),
    Leave(DeclarationId),
}

pub(super) fn record_contains_owned_bytes(
    program: &ResolvedProgram,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    if !is_exact_record(program, root)? {
        return Ok(false);
    }
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut visited_fields = 0usize;
    let mut owned_leaves = 0usize;
    let mut contains = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ResolvedType::Bytes, _) => {
                contains = true;
                owned_leaves = owned_leaves
                    .checked_add(1)
                    .ok_or_else(|| super::error("Wasm owned-byte leaf count overflowed"))?;
                if owned_leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return Err(super::error(
                        "nested owned Wasm lowering exceeds its owned-leaf limit",
                    ));
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
                    return Err(super::error(
                        "nested owned Wasm lowering exceeds its record-depth limit",
                    ));
                }
                if !is_exact_record(program, &ty)? {
                    return Err(super::error(
                        "non-record nominal reached nested owned Wasm lowering",
                    ));
                }
                let ResolvedType::Nominal { declaration, .. } = &ty else {
                    unreachable!()
                };
                if !active.insert(declaration.clone()) {
                    return Err(super::error(
                        "cyclic record reached nested owned Wasm lowering",
                    ));
                }
                let fields = program
                    .declarations
                    .record_fields(declaration)
                    .ok_or_else(|| super::error("nested owned record field inventory is absent"))?;
                visited_fields = visited_fields
                    .checked_add(fields.len())
                    .ok_or_else(|| super::error("Wasm owned-record field count overflowed"))?;
                if visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Err(super::error(
                        "nested owned Wasm lowering exceeds its field limit",
                    ));
                }
                pending.push(Frame::Leave(declaration.clone()));
                for field in fields.iter().rev() {
                    pending.push(Frame::Enter(field.ty.clone(), depth + 1));
                }
            }
            Frame::Enter(_, _) => {
                return Err(super::error(
                    "closed field kind reached nested owned Wasm lowering",
                ));
            }
            Frame::Leave(declaration) => {
                active.remove(&declaration);
            }
        }
    }
    Ok(contains)
}

fn is_exact_record(program: &ResolvedProgram, ty: &ResolvedType) -> Result<bool, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    let item = program
        .declarations
        .declaration(declaration)
        .ok_or_else(|| super::error("nested owned nominal declaration is absent"))?;
    Ok(arguments.is_empty()
        && program
            .declarations
            .type_parameters(declaration)
            .is_some_and(|parameters| parameters.is_empty())
        && item.kind == DeclarationKind::Record)
}
