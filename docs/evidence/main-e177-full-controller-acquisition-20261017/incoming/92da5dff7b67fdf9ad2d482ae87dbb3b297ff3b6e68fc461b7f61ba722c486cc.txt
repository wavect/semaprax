use super::*;
mod funded;
mod selected;
pub(super) mod validation;
#[cfg(test)]
pub(crate) use funded::checked_bindings_funded;
pub(crate) use funded::checked_bindings_funded_with_work;
#[cfg(test)]
pub(crate) use selected::checked_absent_with_work_reservation;
pub(crate) use selected::checked_bindings_with_reservation_and_work;
#[derive(Clone, Copy)]
enum Frame<'a> {
    Function(&'a ResolvedFunction, usize),
    Expression(
        &'a ResolvedFunction,
        crate::hir::lexical_exit::ChildCursor<'a>,
        bool,
        usize,
    ),
}
pub(crate) fn nonallocating(
    program: &dyn crate::cleanup::ReplayContext,
    target: &ResolvedFunction,
) -> bool {
    nonallocating_with(
        target,
        |id, instance| program.resolve_call_target(id, instance),
        |_, mode, ty| resolved_param(program, mode, ty),
        |_, ty| {
            crate::hir::is_scalar_resolved_type(ty)
                || crate::hir::copy_record_collection::admitted(program.declarations(), ty)
                || super::super::resolved_match_scrutinee_admitted(program.declarations(), ty)
        },
        program
            .functions()
            .len()
            .saturating_add(program.function_instances().len()),
    )
}
pub(super) fn checked_target<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    target: &'a ResolvedFunction,
    cursor: &mut Option<ProofCursor<'a>>,
) -> Result<bool, crate::diagnostic::Diagnostic> {
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
            .all(|p| resolved_param(program, p.ownership, &p.ty))
    {
        return Ok(false);
    }
    if cursor.is_none() {
        *cursor = Some(ProofCursor::new()?);
    }
    Ok(nonallocating_with_cursor(
        target,
        |id, instance| program.resolve_call_target(id, instance),
        |_, mode, ty| resolved_param(program, mode, ty),
        |_, ty| {
            crate::hir::is_scalar_resolved_type(ty)
                || crate::hir::copy_record_collection::admitted(program.declarations(), ty)
                || super::super::resolved_match_scrutinee_admitted(program.declarations(), ty)
        },
        program
            .functions()
            .len()
            .saturating_add(program.function_instances().len()),
        cursor.as_mut().ok_or_else(census_error)?,
    ))
}
pub(crate) fn nonallocating_with<'a>(
    target: &'a ResolvedFunction,
    lookup: impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
) -> bool {
    prove(
        target,
        &lookup,
        &param_admit,
        &result_admit,
        lookup_work,
        true,
    )
    .is_some()
}
fn prove<'a>(
    target: &'a ResolvedFunction,
    lookup: &impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: &impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: &impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
    reserve: bool,
) -> Option<()> {
    if reserve
        && !crate::bounded_output::reserve_active_required(std::mem::size_of::<
            [Option<Frame<'_>>; DEPTH],
        >())
    {
        return None;
    }
    let mut frames = [None; DEPTH];
    prove_with_frames(
        target,
        lookup,
        param_admit,
        result_admit,
        lookup_work,
        (&mut frames, WORK),
        &mut |_| Some(()),
    )
}

/// One phase-owned physical proof stack; its lifetime cannot carry authority
/// across immutable provider contexts. The caller owns exact positive memo keys.
pub(crate) struct ProofCursor<'a> {
    frames: [Option<Frame<'a>>; DEPTH],
}
impl<'a> ProofCursor<'a> {
    pub(crate) fn new() -> Result<Self, crate::diagnostic::Diagnostic> {
        if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Self>()) {
            return Err(census_error());
        }
        Ok(Self {
            frames: [None; DEPTH],
        })
    }
}

pub(crate) fn nonallocating_with_cursor<'a>(
    target: &'a ResolvedFunction,
    lookup: impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
    cursor: &mut ProofCursor<'a>,
) -> bool {
    cursor.frames.fill(None);
    prove_with_frames(
        target,
        &lookup,
        &param_admit,
        &result_admit,
        lookup_work,
        (&mut cursor.frames, WORK),
        &mut |_| Some(()),
    )
    .is_some()
}

#[allow(clippy::too_many_arguments)]
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
    debit: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<bool, crate::diagnostic::Diagnostic> {
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

fn prove_with_frames<'a>(
    target: &'a ResolvedFunction,
    lookup: &impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: &impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: &impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
    cursor: (&mut [Option<Frame<'a>>; DEPTH], usize),
    debit: &mut dyn FnMut(usize) -> Option<()>,
) -> Option<()> {
    let (frames, mut work) = cursor;
    frames[0] = Some(Frame::Function(target, 0));
    let mut depth = 1usize;
    while depth != 0 {
        debit(1)?;
        work = work.checked_sub(1)?;
        let current = depth - 1;
        let child = match frames[current]? {
            Frame::Function(function, root) => {
                if root == 0
                    && (!function.effects.is_empty()
                        || function.yields.is_some()
                        || !function
                            .params
                            .iter()
                            .all(|p| param_admit(function, p.ownership, &p.ty))
                        || !(function.return_type == ResolvedType::Bytes
                            || result_admit(function, &function.return_type)
                                && function.return_type != ResolvedType::String))
                {
                    return None;
                }
                let expression = if root < function.requires.len() {
                    function.requires.get(root)
                } else if root == function.requires.len() {
                    Some(&function.body)
                } else {
                    function.ensures.get(root - function.requires.len() - 1)
                };
                frames[current] = Some(Frame::Function(function, root + 1));
                expression.map(|e| {
                    Frame::Expression(
                        function,
                        crate::hir::lexical_exit::ChildCursor::new(e),
                        false,
                        0,
                    )
                })
            }
            Frame::Expression(function, mut cursor, entered, next) => {
                let expression = cursor.expression();
                if !entered {
                    frames[current] = Some(Frame::Expression(function, cursor, true, 0));
                    match &expression.kind {
                        ResolvedExprKind::String(_)
                        | ResolvedExprKind::LiteralFormat { .. }
                        | ResolvedExprKind::Closure { .. }
                        | ResolvedExprKind::Invoke { .. }
                        | ResolvedExprKind::FunctionReference { .. }
                        | ResolvedExprKind::NativeRustImportCall(_)
                        | ResolvedExprKind::HostCommandCall(_)
                        | ResolvedExprKind::Yield { .. }
                        | ResolvedExprKind::Try { .. }
                        | ResolvedExprKind::TryOption { .. }
                        | ResolvedExprKind::LexicalExit { .. } => return None,
                        ResolvedExprKind::ConstructRecord { .. }
                        | ResolvedExprKind::ConstructVariant { .. }
                        | ResolvedExprKind::UpdateRecord { .. } => {
                            if expression.ownership != OwnershipMode::Value
                                || !result_admit(function, &expression.ty)
                            {
                                return None;
                            }
                        }
                        ResolvedExprKind::Block { statements, .. }
                        | ResolvedExprKind::VecConstructor { statements, .. } => {
                            debit(statements.len())?;
                            work = work.checked_sub(statements.len())?;
                            if statements
                                .iter()
                                .any(|s| matches!(s, crate::hir::ResolvedStatement::Unsafe { .. }))
                            {
                                return None;
                            }
                        }
                        ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } => {
                            if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                                if !byte_operation(op)
                                    || !type_arguments.is_empty()
                                    || args.len() != op.arity()
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                                if !borrowed_text_operation(op)
                                    || instance.is_some()
                                    || !type_arguments.is_empty()
                                    || args.len() != op.arity()
                                    || expression.ty != op.return_type()
                                    || expression.ownership != OwnershipMode::Value
                                    || args.iter().zip(op.param_types()).any(|(arg, ty)| {
                                        arg.ty != *ty
                                            || arg.ownership
                                                != if *ty == ResolvedType::Str {
                                                    OwnershipMode::Borrow
                                                } else {
                                                    OwnershipMode::Value
                                                }
                                    })
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                                if !text_operation(op) {
                                    return None;
                                }
                            } else if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
                                if !vec_operation(op) {
                                    return None;
                                }
                            } else {
                                let units = lookup_work.checked_add(depth)?;
                                debit(units)?;
                                work = work.checked_sub(units)?;
                                let target = lookup(callee, instance.as_ref())?;
                                if instance.is_some() || !type_arguments.is_empty() || args.len() != target.params.len()
                                    || expression.ty != target.return_type
                                    || args.iter().zip(&target.params).any(|(a, p)| a.ty != p.ty || !match p.ownership {
                                        OwnershipMode::Own => a.ownership == OwnershipMode::Own,
                                        OwnershipMode::Value => a.ownership == OwnershipMode::Value,
                                        OwnershipMode::Borrow => matches!(a.ownership, OwnershipMode::Own | OwnershipMode::Borrow),
                                        OwnershipMode::Shared => false,
                                    })
                                    || frames[..depth].iter().any(|f| matches!(f, Some(Frame::Function(active, _)) if active.id == target.id)) { return None; }
                                if depth == DEPTH {
                                    return None;
                                }
                                frames[depth] = Some(Frame::Function(target, 0));
                                depth += 1;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                // Two conservative constant units for the saved advance;
                // growing indexed-prefix cost belongs only to indexed walks.
                debit(2)?;
                work = work.checked_sub(2)?;
                let child = cursor.next();
                frames[current] = Some(Frame::Expression(function, cursor, true, next + 1));
                child.map(|e| {
                    Frame::Expression(
                        function,
                        crate::hir::lexical_exit::ChildCursor::new(e),
                        false,
                        0,
                    )
                })
            }
        };
        if let Some(child) = child {
            if depth == DEPTH {
                return None;
            }
            frames[depth] = Some(child);
            depth += 1;
        } else {
            frames[current] = None;
            depth -= 1;
        }
    }
    Some(())
}
pub(crate) fn bindings<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
) -> HelperBindings<'a> {
    checked_bindings(program, function).unwrap_or_default()
}

pub(crate) fn checked_bindings<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
    if !potential(program) {
        return Ok(HelperBindings::default());
    }
    let mut cursor = CensusCursor::new(function)?;
    checked_bindings_with(function, &mut cursor, |binding, value| {
        Ok(assignment(program, binding, value))
    })
}
const CENSUS_DEPTH: usize = 514;
#[cfg(test)]
const CENSUS_CODE: &str = "SPX-H006";
#[cfg(test)]
const CENSUS_MESSAGE: &str = "byte helper renewal census exceeds admitted scratch, work or depth";
pub(super) fn census_error() -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io(
        "SPX-H006",
        "byte helper renewal census exceeds admitted scratch, work or depth",
    )
}
pub(super) fn potential(program: &dyn crate::cleanup::ReplayContext) -> bool {
    program.functions().iter().any(|f| {
        f.return_type == ResolvedType::Bytes
            && f.params
                .iter()
                .any(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
    })
}
pub(super) struct CensusCursor<'a> {
    frames: [(&'a ResolvedExpr, usize, usize); CENSUS_DEPTH],
}
impl<'a> CensusCursor<'a> {
    pub(super) fn new(
        function: &'a ResolvedFunction,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Self>()) {
            return Err(census_error());
        }
        Ok(Self {
            frames: [(&function.body, 0, 0); CENSUS_DEPTH],
        })
    }
}
pub(super) fn checked_bindings_with<'a>(
    function: &'a ResolvedFunction,
    cursor: &mut CensusCursor<'a>,
    mut admitted: impl FnMut(
        &ResolvedBinding,
        &ResolvedExpr,
    ) -> Result<bool, crate::diagnostic::Diagnostic>,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
    checked_bindings_with_storage(
        function,
        cursor,
        &mut (),
        |binding, value, _| admitted(binding, value),
        |bytes, _| crate::bounded_output::reserve_active_required(bytes),
        |_, _| Ok(()),
        CensusRefusal {
            physical: |_| {},
            error: census_error,
        },
    )
}
// Physical failure notification and its diagnostic constructor travel together.
// Work/depth refusals never call physical; each route retains its original error.
struct CensusRefusal<S> {
    physical: fn(&mut S),
    error: fn() -> crate::diagnostic::Diagnostic,
}

fn checked_bindings_with_storage<'a, S>(
    function: &'a ResolvedFunction,
    cursor: &mut CensusCursor<'a>,
    storage: &mut S,
    mut admitted: impl FnMut(
        &ResolvedBinding,
        &ResolvedExpr,
        &mut S,
    ) -> Result<bool, crate::diagnostic::Diagnostic>,
    mut reserve: impl FnMut(usize, &mut S) -> bool,
    mut debit: impl FnMut(usize, &mut S) -> Result<(), crate::diagnostic::Diagnostic>,
    refusal: CensusRefusal<S>,
) -> Result<HelperBindings<'a>, crate::diagnostic::Diagnostic> {
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

#[cfg(test)]
mod cursor_work_tests;
