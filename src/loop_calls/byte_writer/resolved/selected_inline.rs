//! Exact helper census/proof twins carrying only finite error data.
//! Legacy Diagnostic producers remain independently compiled in parent files.
use super::super::{DEPTH, HelperBindings, WORK, assignment_with_target};
use super::{CENSUS_DEPTH, CensusCursor, ProofCursor, prove_with_frames};
use crate::cleanup_plan::ReplayPhysicalReservation;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::hir::{
    OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType,
};
fn error() -> Refusal {
    Refusal::HelperCensus
}
fn prepay<T>(
    physical: &mut ReplayPhysicalReservation,
    debit: &mut dyn FnMut(usize) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    physical
        .reserve(std::mem::size_of::<(
            Refusal,
            Result<T, Refusal>,
            Result<(), Refusal>,
            Result<bool, Refusal>,
            Option<Refusal>,
            (
                &mut ReplayPhysicalReservation,
                &mut dyn FnMut(usize) -> Result<(), Refusal>,
            ),
            CensusRefusal<(
                &mut ReplayPhysicalReservation,
                &mut dyn FnMut(usize) -> Result<(), Refusal>,
            )>,
        )>())
        .map_err(|_| error())?;
    debit(7)
}
pub(crate) fn checked_bindings_with_reservation_and_work_inline<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
    debit: &mut dyn FnMut(usize) -> Result<(), Refusal>,
) -> Result<HelperBindings<'a>, Refusal> {
    prepay::<HelperBindings<'a>>(physical, debit)?;
    let mut possible = false;
    for ordinal in 0..program.function_count() {
        debit(1)?;
        let member = program.function_at(ordinal).ok_or_else(error)?;
        if member.return_type == ResolvedType::Bytes {
            for parameter in &member.params {
                debit(1)?;
                if parameter.ownership == OwnershipMode::Own && parameter.ty == ResolvedType::Bytes
                {
                    possible = true;
                    break;
                }
            }
        }
        if possible {
            break;
        }
    }
    if !possible {
        return Ok(HelperBindings::default());
    }
    physical
        .reserve(std::mem::size_of::<CensusCursor<'_>>())
        .map_err(|_| error())?;
    let mut census = CensusCursor {
        frames: [(&function.body, 0, 0); CENSUS_DEPTH],
    };
    let mut proof: Option<ProofCursor<'a>> = None;
    let mut storage = (physical, debit);
    checked_bindings_with_storage(
        function,
        &mut census,
        &mut storage,
        |binding, value, storage| {
            let (physical, debit) = storage;
            debit(program.function_count())?;
            let ResolvedExprKind::Call {
                callee,
                instance: None,
                ..
            } = &value.kind
            else {
                return Ok(false);
            };
            let Some(target) = program.resolve_call_target(callee, None) else {
                return Ok(false);
            };
            if target.return_type != ResolvedType::Bytes
                || target
                    .params
                    .iter()
                    .filter(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
                    .count()
                    != 1
                || !target
                    .params
                    .iter()
                    .all(|p| primitive_param(p.ownership, &p.ty))
            {
                return Ok(false);
            }
            if proof.is_none() {
                physical
                    .reserve(std::mem::size_of::<Option<ProofCursor<'_>>>())
                    .map_err(|_| error())?;
                proof = Some(ProofCursor {
                    frames: [None; DEPTH],
                });
            }
            prepay::<bool>(physical, debit)?;
            let proved = nonallocating_with_cursor_and_work(
                target,
                |id, instance| program.resolve_call_target(id, instance),
                |_, mode, ty| primitive_param(mode, ty),
                |_, ty| crate::hir::is_scalar_resolved_type(ty),
                program
                    .function_count()
                    .saturating_add(program.function_instances().len()),
                proof.as_mut().ok_or_else(error)?,
                *debit,
            )?;
            Ok(assignment_with_target(
                program, binding, value, target, proved,
            ))
        },
        |bytes, storage| storage.0.reserve(bytes).is_ok(),
        |units, storage| (storage.1)(units),
        CensusRefusal {
            physical: |_| {},
            error: error,
        },
    )
}
fn nonallocating_with_cursor_and_work<'a>(
    target: &'a ResolvedFunction,
    lookup: impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
    cursor: &mut ProofCursor<'a>,
    debit: &mut dyn FnMut(usize) -> Result<(), Refusal>,
) -> Result<bool, Refusal> {
    cursor.frames.fill(None);
    let mut failure = None;
    let proved = prove_with_frames(
        target,
        &lookup,
        &param_admit,
        &result_admit,
        lookup_work,
        (&mut cursor.frames, WORK),
        &mut |units| match debit(units) {
            Ok(()) => Some(()),
            Err(error) => {
                failure = Some(error);
                None
            }
        },
    )
    .is_some();
    match failure {
        Some(error) => Err(error),
        None => Ok(proved),
    }
}

struct CensusRefusal<S> {
    physical: fn(&mut S),
    error: fn() -> Refusal,
}

fn checked_bindings_with_storage<'a, S>(
    function: &'a ResolvedFunction,
    cursor: &mut CensusCursor<'a>,
    storage: &mut S,
    mut admitted: impl FnMut(&ResolvedBinding, &ResolvedExpr, &mut S) -> Result<bool, Refusal>,
    mut reserve: impl FnMut(usize, &mut S) -> bool,
    mut debit: impl FnMut(usize, &mut S) -> Result<(), Refusal>,
    refusal: CensusRefusal<S>,
) -> Result<HelperBindings<'a>, Refusal> {
    let CensusRefusal { physical, error } = refusal;
    let mut found = HelperBindings::default();
    let frames = &mut cursor.frames;
    frames[0] = (&function.body, 0, 0);
    let mut depth = 1usize;
    let mut work = WORK;
    while depth != 0 {
        debit(1, storage)?;
        work = work.checked_sub(1).ok_or_else(error)?;
        let (expression, next, sub) = frames[depth - 1];
        if next == 0 && sub == 0 {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                debit(statements.len(), storage)?;
                work = work.checked_sub(statements.len()).ok_or_else(error)?;
                for statement in statements {
                    if let crate::hir::ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        if admitted(binding, value, storage)? {
                            // Binary search, insertion shift and replacement copy have
                            // this bounded linear upper in the current site count.
                            let insertion = found
                                .len()
                                .checked_mul(3)
                                .and_then(|n| n.checked_add(1))
                                .ok_or_else(|| {
                                    physical(storage);
                                    error()
                                })?;
                            debit(insertion, storage)?;
                            work = work.checked_sub(insertion).ok_or_else(error)?;
                            found
                                .insert_with(&value.id, binding, |bytes| reserve(bytes, storage))
                                .ok_or_else(|| {
                                    physical(storage);
                                    error()
                                })?;
                        }
                    }
                }
            }
        }
        // Block and Match cursors never repeatedly rescan a growing prefix.
        let child = match &expression.kind {
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                if let Some(statement) = statements.get(next) {
                    let child = statement.child(sub);
                    if sub + 1 >= statement.child_count() {
                        frames[depth - 1] = (expression, next + 1, 0);
                    } else {
                        frames[depth - 1].2 += 1;
                    }
                    if child.is_none() {
                        continue;
                    }
                    child
                } else {
                    frames[depth - 1].1 += 1;
                    (next == statements.len()).then_some(tail.as_ref())
                }
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                if next == 0 {
                    frames[depth - 1].1 = 1;
                    Some(scrutinee.as_ref())
                } else if let Some(arm) = arms.get(next - 1) {
                    if sub == 0 && arm.guard.is_some() {
                        frames[depth - 1].2 = 1;
                        arm.guard.as_deref()
                    } else {
                        frames[depth - 1] = (expression, next + 1, 0);
                        Some(&arm.value)
                    }
                } else {
                    None
                }
            }
            _ => {
                frames[depth - 1].1 += 1;
                crate::hir::lexical_exit::child(expression, next)
            }
        };
        if let Some(child) = child {
            if depth == CENSUS_DEPTH {
                return Err(error());
            }
            frames[depth] = (child, 0, 0);
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Ok(found)
}

fn primitive_param(mode: OwnershipMode, ty: &ResolvedType) -> bool {
    match mode {
        OwnershipMode::Own => *ty == ResolvedType::Bytes,
        OwnershipMode::Value => crate::hir::is_scalar_resolved_type(ty),
        OwnershipMode::Borrow => matches!(
            ty,
            ResolvedType::Bytes | ResolvedType::String | ResolvedType::Str | ResolvedType::SliceU8
        ),
        OwnershipMode::Shared => false,
    }
}
