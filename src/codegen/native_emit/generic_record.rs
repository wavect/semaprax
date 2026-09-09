//! Independent native admission for fully substituted generic record storage.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedMatchPattern,
    ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::backend_error;
use super::{c_field_symbol, CEmitter, COutput, CValue};

enum Frame {
    Enter(ResolvedType, usize),
    Leave(String),
}

pub(super) fn match_result_is_admitted(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> bool {
    let generic = hir::bounded_owned_record_template_for_function(program, function).is_some();
    let ordinary = program.functions.iter().any(|item| item.id == function.id);
    if !generic && !ordinary {
        return false;
    }
    let ResolvedExprKind::Match {
        mode,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return false;
    };
    let [arm] = arms.as_slice() else { return false };
    let ResolvedMatchPattern::Record {
        record, instance, ..
    } = &arm.pattern
    else {
        return false;
    };
    let ordinary_shape = ordinary
        && hir::is_admitted_nested_owned_byte_record(&program.declarations, &scrutinee.ty)
        && (expression.ty == ResolvedType::Bytes
            || hir::is_admitted_nested_owned_byte_record(&program.declarations, &expression.ty));
    *mode == hir::ResolvedMatchMode::Own
        && expression.ty == arm.value.ty
        && expression.ownership == OwnershipMode::Own
        && scrutinee.ownership == OwnershipMode::Own
        && arm.value.ownership == OwnershipMode::Own
        && instance == &scrutinee.ty
        && matches!(&scrutinee.ty, ResolvedType::Nominal { declaration, .. }
            if declaration == record)
        && (ordinary_shape || (generic && is_admitted(program, &expression.ty).unwrap_or(false)))
}

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn finish_record_match_phase(
        &mut self,
        expression: &ResolvedExpr,
        anchors: &BTreeSet<crate::cleanup_plan::StorageId>,
        value: &mut CValue,
    ) -> Result<(), Diagnostic> {
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("owned match has no cleanup plan"))?;
        let preflight = plan.record_match_phase(&expression.id, anchors, false, true)?;
        let transfers = plan.record_match_phase(&expression.id, anchors, false, false)?;
        for line in preflight.lines().chain(transfers.lines()) {
            self.line(line);
        }
        if expression.ty == ResolvedType::Bytes {
            if let Some(result) = plan.result_at(&expression.id) {
                value.code = result.to_owned();
            }
        }
        Ok(())
    }

    pub(super) fn generic_projected_bytes_value(
        &self,
        root: &crate::hir::ValueId,
        storage: &crate::cleanup_plan::StorageId,
        path: &[crate::hir::DeclarationId],
    ) -> Result<String, Diagnostic> {
        self.bytes_plan
            .and_then(|plan| plan.projected_value(storage, path).ok())
            .map(str::to_owned)
            .or_else(|| {
                self.borrowed_aggregate_bytes
                    .get(&(root.clone(), path.to_vec()))
                    .cloned()
            })
            .ok_or_else(|| backend_error("projected Bytes place has no authenticated storage"))
    }

    pub(super) fn finish_generic_owned_match_result(
        &mut self,
        expression: &ResolvedExpr,
        arm_value: &ResolvedExpr,
        value: &mut CValue,
    ) -> Result<(), Diagnostic> {
        if self.bytes_plan.is_none() {
            return Err(backend_error("owned record match has no cleanup plan"));
        }
        if expression.ty == ResolvedType::Bytes {
            let plan = self.bytes_plan.expect("owned match plan checked above");
            if matches!(arm_value.kind, ResolvedExprKind::Place(_)) {
                for line in plan.apply_at(&arm_value.id)?.lines() {
                    self.line(line);
                }
            }
            value.code = plan
                .result_at(&arm_value.id)
                .ok_or_else(|| backend_error("owned Bytes match has no result transfer"))?
                .to_owned();
            return Ok(());
        }
        let layout = self.record_layout(&expression.ty)?.clone();
        let destination = self.temporary(&expression.ty)?;
        self.initialize_record_carrier(&destination, &layout);
        self.zero_owned_record_bytes(&destination, &expression.ty)?;
        // Copy scalar fields even when their enclosing record also owns Bytes.
        // Cleanup-plan transitions alone publish the owned leaves.
        let mut pending = vec![(destination.clone(), value.code.clone(), layout)];
        while let Some((target, source, layout)) = pending.pop() {
            for field in &layout.fields {
                if field.size == 0 {
                    continue;
                }
                let symbol = c_field_symbol(&field.field);
                if !self.record_contains_owned_bytes(&field.ty)? {
                    self.line(&format!("{target}.{symbol} = {source}.{symbol};"));
                } else if field.ty != ResolvedType::Bytes {
                    pending.push((
                        format!("{target}.{symbol}"),
                        format!("{source}.{symbol}"),
                        self.record_layout(&field.ty)?.clone(),
                    ));
                }
            }
        }
        value.code = destination;
        Ok(())
    }
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
