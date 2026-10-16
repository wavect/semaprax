//! Explicit funded twin of `append_bindings` (explicit payer). The scan, its nested
//! candidate/declaration walks and the one-before-publish discipline are the
//! original ones. Every fixed cursor and entry is reserved on the supplied
//! payer first, and a refusal is returned instead of skipping the entry.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

fn frames_funded<'a, E>(reserve: Reserve<'_, E>) -> Result<Frames<'a>, FundedError<E>> {
    funded::charge(reserve, std::mem::size_of::<Frames<'_>>())?;
    Ok([None; crate::cache_codec::MAX_DEPTH + 1])
}

/// Same one-entry bound as `reserve_entry`, as a charge instead of an ACTIVE
/// debit; a parity control pins the two numbers together.
fn entry_bytes<T>(id: &ExpressionId) -> usize {
    (std::mem::size_of::<ExpressionId>() + std::mem::size_of::<T>())
        .saturating_mul(11)
        .saturating_add(16 * std::mem::size_of::<usize>())
        .saturating_add(id.as_str().len())
}

fn declares_funded<E>(
    root: &ResolvedExpr,
    target: &ResolvedBinding,
    mutable: bool,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    let mut frames = frames_funded(reserve)?;
    let mut found = false;
    let complete = visit_with(
        root,
        |expr| {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expr.kind
            {
                found |= statements.iter().any(|statement| {
                    matches!(statement,
                    ResolvedStatement::Let { binding, mutable: actual, .. }
                        if binding.id == target.id && binding.ty == target.ty
                            && binding.ownership == target.ownership && (!mutable || *actual))
                });
            }
        },
        &mut frames,
    );
    Ok(complete && found)
}

fn scan_with_funded<'a, E>(
    function: &'a ResolvedFunction,
    admission: &ElementAdmission,
    mut emit: impl FnMut(Site<'a>, Reserve<'_, E>) -> Result<(), FundedError<E>>,
    frames: &mut Frames<'a>,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    let mut complete_all = true;
    let mut failure: Option<FundedError<E>> = None;
    let complete = visit_with(
        &function.body,
        |expr| {
            if failure.is_some() {
                return;
            }
            let ResolvedExprKind::Block { statements, tail } = &expr.kind else {
                return;
            };
            for statement in statements {
                let (condition, body) = match statement {
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => (condition.as_ref(), body.as_ref()),
                    _ => continue,
                };
                let Some(protocol) = crate::hir::iterator_loop::recognize(condition, body) else {
                    continue;
                };
                let Some(item) = protocol.owned_item else {
                    continue;
                };
                let [ResolvedStatement::Let {
                    binding: step,
                    mutable: true,
                    value: seed,
                    ..
                }, ResolvedStatement::While { .. } | ResolvedStatement::LexicalWhile { .. }] =
                    statements.as_slice()
                else {
                    continue;
                };
                if step != protocol.step
                    || !(tail.ownership == OwnershipMode::Value
                        && matches!(
                            tail.kind,
                            ResolvedExprKind::Int(0) | ResolvedExprKind::Usize(0)
                        ))
                    || seed.ty != step.ty
                    || seed.ownership != OwnershipMode::Own
                    || !matches!(&seed.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
                    if callee.as_str() == crate::iterator_ops::NEXT_ID
                    && type_arguments.as_slice() == std::slice::from_ref(&item.ty)
                    && matches!(args.as_slice(), [source] if source.ownership == OwnershipMode::Own
                        && matches!(&source.ty, ResolvedType::Nominal { declaration, arguments }
                            if declaration.as_str() == crate::iterator_ops::ITER_ID && arguments.as_slice() == std::slice::from_ref(&item.ty))))
                {
                    continue;
                }
                if item.ownership != OwnershipMode::Own
                    || !matches!(&item.ty,
                ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
                {
                    continue;
                }
                // The nested body walk keeps its own overlapping cursor, as the
                // original `visit` did, but pays for it on the supplied payer.
                let mut inner = match frames_funded(reserve) {
                    Ok(inner) => inner,
                    Err(error) => {
                        failure = Some(error);
                        return;
                    }
                };
                complete_all &= visit_with(
                    protocol.authored_body,
                    |node| {
                        if failure.is_some() {
                            return;
                        }
                        let ResolvedExprKind::Block { statements, .. } = &node.kind else {
                            return;
                        };
                        for statement in statements {
                            let ResolvedStatement::Assign {
                                binding,
                                field: None,
                                value,
                                ..
                            } = statement
                            else {
                                continue;
                            };
                            let ResolvedType::Nominal { arguments, .. } = &binding.ty else {
                                continue;
                            };
                            let [element] = arguments.as_slice() else {
                                continue;
                            };
                            let ResolvedExprKind::Call { callee, .. } = &value.kind else {
                                continue;
                            };
                            let Some(op) = crate::vec_ops::by_id(callee.as_str()) else {
                                continue;
                            };
                            if op != crate::vec_ops::VecOp::Push
                                || admission(op, &item.ty) != Some(OwnershipMode::Own)
                                || admission(op, element) != Some(OwnershipMode::Own)
                                || !super::super::same_cell(binding, value, Some(admission))
                            {
                                continue;
                            }
                            // `!declares(whole) || declares(authored)` keeps its
                            // short circuit, so each nested cursor is charged
                            // exactly when the original would have built it.
                            match declares_funded(&function.body, binding, true, reserve) {
                                Ok(true) => {}
                                Ok(false) => continue,
                                Err(error) => {
                                    failure = Some(error);
                                    return;
                                }
                            }
                            match declares_funded(protocol.authored_body, binding, false, reserve) {
                                Ok(false) => {}
                                Ok(true) => continue,
                                Err(error) => {
                                    failure = Some(error);
                                    return;
                                }
                            }
                            let site = Site {
                                condition,
                                body,
                                item,
                                at: value,
                                binding,
                            };
                            if let Err(error) = emit(site, reserve) {
                                failure = Some(error);
                                return;
                            }
                        }
                    },
                    &mut inner,
                );
                if failure.is_some() {
                    return;
                }
            }
        },
        frames,
    );
    match failure {
        Some(error) => Err(error),
        None => Ok(complete && complete_all),
    }
}

pub(crate) fn append_bindings_funded<'a, E>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    found: &mut BTreeMap<ExpressionId, &'a ResolvedBinding>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    funded::charge(reserve, 0)?;
    if !has_record_candidate(program) {
        return Ok(());
    }
    let mut frames = frames_funded(reserve)?;
    // Scan once before publishing: an incomplete walk leaves nothing behind.
    if !scan_with_funded(
        function,
        &|op, element| super::super::element_mode(program.declarations(), op, element),
        |_, _| Ok(()),
        &mut frames,
        reserve,
    )? {
        return Ok(());
    }
    scan_with_funded(
        function,
        &|op, element| super::super::element_mode(program.declarations(), op, element),
        |site, reserve| {
            if !found.contains_key(&site.at.id) {
                funded::charge(reserve, entry_bytes::<&ResolvedBinding>(&site.at.id))?;
                found.insert(site.at.id.clone(), site.binding);
            }
            Ok(())
        },
        &mut frames,
        reserve,
    )?;
    Ok(())
}

#[cfg(test)]
pub(super) fn entry_bytes_for_test<T>(id: &ExpressionId) -> usize {
    entry_bytes::<T>(id)
}
