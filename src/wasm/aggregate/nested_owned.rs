//! Closed bounded classification for executable owned-byte records.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, DeclarationKind, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedType,
};

use super::{error, is_aggregate, layout, require_type, value_type, Emitter, Pointer, Value};

enum Frame {
    Enter(ResolvedType, usize),
    Leave(String),
}

pub(super) fn record_contains_owned_bytes(
    program: &ResolvedProgram,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    if !is_exact_record(program, root)? {
        return Ok(false);
    }
    if !crate::hir::resolved_type_contains_owned_bytes(program, root) {
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
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    unreachable!()
                };
                let identity = ty.identity_key();
                if !active.insert(identity.clone()) {
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
                pending.push(Frame::Leave(identity));
                for field in fields.iter().rev() {
                    pending.push(Frame::Enter(
                        crate::hir::substitute_type(&field.ty, declaration, arguments)?,
                        depth + 1,
                    ));
                }
            }
            Frame::Enter(_, _) => {
                return Err(super::error(
                    "closed field kind reached nested owned Wasm lowering",
                ));
            }
            Frame::Leave(identity) => {
                active.remove(&identity);
            }
        }
    }
    Ok(contains)
}

pub(super) fn record_is_nested_owned(
    program: &ResolvedProgram,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    if !record_contains_owned_bytes(program, root)? {
        return Ok(false);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = root
    else {
        return Ok(false);
    };
    let fields = program
        .declarations
        .record_fields(declaration)
        .ok_or_else(|| super::error("nested update record inventory is absent"))?;
    for field in fields {
        let field_ty = crate::hir::substitute_type(&field.ty, declaration, arguments)?;
        if record_contains_owned_bytes(program, &field_ty)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) fn record_update_uses_owned_plan(
    program: &ResolvedProgram,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    Ok(record_is_nested_owned(program, root)?
        || is_concrete_generic_flat_owned_record(program, root))
}

fn is_concrete_generic_flat_owned_record(program: &ResolvedProgram, root: &ResolvedType) -> bool {
    matches!(root, ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
        && crate::hir::is_flat_owned_byte_record(&program.declarations, root)
}

pub(super) fn emit_update_scope_cleanup(
    emitter: &mut super::Emitter<'_>,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind else {
        return Ok(());
    };
    let nested = record_is_nested_owned(emitter.program, &expression.ty)?;
    if !nested && !is_concrete_generic_flat_owned_record(emitter.program, &expression.ty) {
        return Ok(());
    }
    if nested && emitter.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V9 {
        return Err(super::error(
            "nested record update cleanup requires CleanupPlan v9",
        ));
    }
    let anchor = crate::cleanup_plan::StorageId::Temporary(base.id.clone());
    let mut regions = emitter
        .cleanup_plan
        .regions
        .iter()
        .filter(|region| region.slots.contains(&anchor));
    let region = regions
        .next()
        .cloned()
        .ok_or_else(|| super::error("nested record update has no cleanup region"))?;
    if regions.next().is_some() || region.parent.is_none() {
        return Err(super::error(
            "nested record update cleanup region is not exact",
        ));
    }
    let exit = emitter
        .cleanup_plan
        .exits
        .get(region.normal_scope_end.0 as usize)
        .filter(|exit| exit.id == region.normal_scope_end)
        .cloned()
        .ok_or_else(|| super::error("nested record update region has no normal exit"))?;
    if !matches!(
        exit.continuation,
        crate::cleanup_plan::ExitContinuation::Continue(_)
    ) || exit.leaves_regions.as_slice() != [region.id]
    {
        return Err(super::error(
            "nested record update cleanup exit is not canonical",
        ));
    }
    emitter.emit_cleanup_actions(&exit.finalize_in_order)
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
    Ok(program
        .declarations
        .type_parameters(declaration)
        .is_some_and(|parameters| parameters.len() == arguments.len())
        && item.kind == DeclarationKind::Record)
}

pub(super) fn owned_record_pattern_anchors(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
) -> Result<BTreeSet<crate::cleanup_plan::StorageId>, Diagnostic> {
    let mut pending = fields
        .iter()
        .map(|field| (&field.pattern, 1usize))
        .collect::<Vec<_>>();
    let mut anchors = BTreeSet::new();
    let mut visited = 0usize;
    while let Some((pattern, depth)) = pending.pop() {
        if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
            return Err(super::error(
                "owned record match cleanup exceeds the pattern depth limit",
            ));
        }
        visited = visited
            .checked_add(1)
            .ok_or_else(|| super::error("owned record match field count overflowed"))?;
        if visited > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
            return Err(super::error(
                "owned record match cleanup exceeds the field limit",
            ));
        }
        match pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding)
                if binding.ty == ResolvedType::Bytes =>
            {
                anchors.insert(crate::cleanup_plan::StorageId::Value(binding.id.clone()));
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                pending.extend(fields.iter().rev().map(|field| (&field.pattern, depth + 1)));
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(_)
            | crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
    }
    Ok(anchors)
}

pub(super) fn bind_record_match_pattern(
    emitter: &mut Emitter<'_>,
    base: &Value,
    mode: crate::hir::ResolvedMatchMode,
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
) -> Result<(), Diagnostic> {
    struct Frame<'a> {
        base: Value,
        record: &'a DeclarationId,
        instance: &'a ResolvedType,
        fields: &'a [crate::hir::ResolvedRecordMatchPatternField],
        index: usize,
        seen: std::collections::BTreeSet<DeclarationId>,
        depth: usize,
    }
    let mut pending = vec![Frame {
        base: base.clone(),
        record,
        instance,
        fields,
        index: 0,
        seen: std::collections::BTreeSet::new(),
        depth: 1,
    }];
    let mut visited_fields = 0usize;
    while let Some(mut frame) = pending.pop() {
        if frame.depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
            return Err(error("nested record pattern exceeds the Wasm depth limit"));
        }
        require_type(
            value_type(&frame.base),
            frame.instance,
            "record pattern instance",
        )?;
        let record_layout = layout(emitter.program, frame.instance)?;
        if record_layout.record != *frame.record || record_layout.fields.len() != frame.fields.len()
        {
            return Err(error(
                "record pattern disagrees with its exact aggregate layout",
            ));
        }
        if frame.index == 0 {
            visited_fields = visited_fields
                .checked_add(frame.fields.len())
                .ok_or_else(|| error("record pattern field count overflowed"))?;
            if visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                return Err(error("nested record pattern exceeds the Wasm field limit"));
            }
        }
        let Some(field) = frame.fields.get(frame.index) else {
            continue;
        };
        if !frame.seen.insert(field.field.clone()) {
            return Err(error(format!(
                "record pattern `{}` repeats field `{}`",
                frame.record, field.field
            )));
        }
        let projected = emitter.project_value(&frame.base, &field.field)?;
        match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                require_type(
                    &binding.ty,
                    value_type(&projected),
                    "record pattern binding",
                )?;
                if !nested_record_binding_is_exact(record_contains_owned_bytes(
                    emitter.program,
                    &binding.ty,
                )?) {
                    return Err(error(
                        "owning nested record binding reached exact destructuring lowering",
                    ));
                }
                if binding.ty == ResolvedType::Bytes {
                    if !byte_binding_mode_is_exact(mode, binding.ownership) {
                        return Err(error(
                            "record Bytes binding ownership disagrees with match mode",
                        ));
                    }
                } else if binding.ownership != crate::hir::OwnershipMode::Value {
                    return Err(error("Copy record binding has non-Value ownership"));
                }
                let destination = if is_aggregate(emitter.program, &binding.ty)? {
                    let offset = emitter
                        .plan
                        .aggregate_bindings
                        .get(&binding.id)
                        .copied()
                        .ok_or_else(|| {
                            error(format!(
                                "missing aggregate record match binding `{}`",
                                binding.id
                            ))
                        })?;
                    Value::Aggregate {
                        pointer: Pointer {
                            local: emitter.plan.frame_base,
                            offset,
                        },
                        ty: binding.ty.clone(),
                    }
                } else {
                    Value::Scalar {
                        local: emitter
                            .plan
                            .scalar_bindings
                            .get(&binding.id)
                            .copied()
                            .ok_or_else(|| {
                                error(format!(
                                    "missing scalar record match binding `{}`",
                                    binding.id
                                ))
                            })?,
                        ty: binding.ty.clone(),
                    }
                };
                if binding.ty == ResolvedType::Bytes
                    && binding.ownership == crate::hir::OwnershipMode::Borrow
                {
                    emitter.copy_borrowed_scalar_alias(&destination, &projected)?;
                } else {
                    emitter.copy_value(&destination, &projected, "record pattern binding")?;
                }
                if emitter
                    .bindings
                    .insert(binding.id.clone(), destination)
                    .is_some()
                {
                    return Err(error("record match binding is not fresh"));
                }
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                let ty = value_type(&projected);
                let nested = record_contains_owned_bytes(emitter.program, ty)?;
                let direct = *ty == ResolvedType::Bytes;
                // The resolver admits a wildcard over a direct droppable leaf
                // under a borrow and rejects a nested owning subtree in either
                // mode. Match it exactly, so a program the front end admits is
                // never refused by a backend.
                if !wildcard_is_exact(mode, nested)
                    || (direct && matches!(mode, crate::hir::ResolvedMatchMode::Own))
                {
                    return Err(error(
                        "owned Bytes subtree reached a record pattern wildcard",
                    ));
                }
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => {
                let child_depth = frame.depth + 1;
                let next = Frame {
                    base: frame.base,
                    record: frame.record,
                    instance: frame.instance,
                    fields: frame.fields,
                    index: frame.index + 1,
                    seen: frame.seen,
                    depth: frame.depth,
                };
                pending.push(next);
                pending.push(Frame {
                    base: projected,
                    record,
                    instance,
                    fields,
                    index: 0,
                    seen: std::collections::BTreeSet::new(),
                    depth: child_depth,
                });
                continue;
            }
        }
        frame.index += 1;
        pending.push(frame);
    }
    Ok(())
}

fn byte_binding_mode_is_exact(
    mode: crate::hir::ResolvedMatchMode,
    ownership: crate::hir::OwnershipMode,
) -> bool {
    matches!(
        (mode, ownership),
        (
            crate::hir::ResolvedMatchMode::Own,
            crate::hir::OwnershipMode::Own
        ) | (
            crate::hir::ResolvedMatchMode::Borrow,
            crate::hir::OwnershipMode::Borrow
        )
    )
}

fn wildcard_is_exact(mode: crate::hir::ResolvedMatchMode, owns_bytes: bool) -> bool {
    !owns_bytes
        || !matches!(
            mode,
            crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow
        )
}

fn nested_record_binding_is_exact(contains_owned_bytes: bool) -> bool {
    !contains_owned_bytes
}

#[cfg(test)]
mod tests {
    use super::{byte_binding_mode_is_exact, nested_record_binding_is_exact, wildcard_is_exact};
    use crate::hir::{OwnershipMode, ResolvedMatchMode};

    #[test]
    fn hostile_hir_cannot_alias_an_owned_record_match_terminal() {
        assert!(!byte_binding_mode_is_exact(
            ResolvedMatchMode::Own,
            OwnershipMode::Borrow,
        ));
        assert!(byte_binding_mode_is_exact(
            ResolvedMatchMode::Own,
            OwnershipMode::Own,
        ));
    }

    #[test]
    fn hostile_hir_cannot_move_a_borrowed_record_match_terminal() {
        assert!(!byte_binding_mode_is_exact(
            ResolvedMatchMode::Borrow,
            OwnershipMode::Own,
        ));
        assert!(!byte_binding_mode_is_exact(
            ResolvedMatchMode::Value,
            OwnershipMode::Own,
        ));
        assert!(byte_binding_mode_is_exact(
            ResolvedMatchMode::Borrow,
            OwnershipMode::Borrow,
        ));
    }

    #[test]
    fn hostile_hir_cannot_hide_owned_subtrees_with_wildcards() {
        assert!(!wildcard_is_exact(ResolvedMatchMode::Own, true));
        assert!(!wildcard_is_exact(ResolvedMatchMode::Borrow, true));
    }

    #[test]
    fn hostile_hir_cannot_bind_an_owning_record_as_one_terminal() {
        assert!(!nested_record_binding_is_exact(true));
        assert!(nested_record_binding_is_exact(false));
    }

    #[test]
    fn copy_only_wildcards_remain_admitted() {
        assert!(wildcard_is_exact(ResolvedMatchMode::Own, false));
        assert!(wildcard_is_exact(ResolvedMatchMode::Borrow, false));
    }
}
