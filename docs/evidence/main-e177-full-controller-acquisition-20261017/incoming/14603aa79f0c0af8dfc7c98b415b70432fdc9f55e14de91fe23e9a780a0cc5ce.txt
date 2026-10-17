//! Explicit funded twin of `bindings_in` (explicit payer). Collector order, same-cell
//! admission and duplicate-identity precedence are the original ones; every
//! Vec, set/map entry and tree node is reserved on the supplied payer first and
//! a refusal is returned rather than a smaller binding set.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

pub(crate) fn bindings_in_funded<'a, E>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut found = collect_funded(
        function,
        Some(&|op, element| element_mode(program.declarations(), op, element)),
        reserve,
    )?;
    consuming::append_bindings_funded(program, function, &mut found, reserve)?;
    Ok(found)
}

fn collect_funded<'a, E>(
    function: &'a ResolvedFunction,
    admission: Option<&ElementAdmission>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    let mut mutable = BTreeSet::new();
    let mut pending = funded::singleton(&function.body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let {
                    binding,
                    mutable: true,
                    ..
                } = statement
                {
                    funded::insert_cloned_key(&mut mutable, &binding.id, reserve)?;
                }
            }
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    let mut found = BTreeMap::new();
    let mut pending = funded::singleton((&function.body, false), reserve)?;
    while let Some((expression, in_loop)) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            continue;
        }
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            funded::grow(&mut pending, 1, reserve)?;
            pending.push((tail, in_loop));
            for statement in statements {
                match statement {
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } if in_loop
                        && mutable.contains(&binding.id)
                        && same_cell(binding, value, admission) =>
                    {
                        funded::insert_cloned(&mut found, &value.id, binding, reserve)?;
                        funded::grow(&mut pending, 1, reserve)?;
                        pending.push((value, in_loop));
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => {
                        let ordinary =
                            crate::hir::iterator_loop::recognize(condition, body).is_none();
                        funded::grow(&mut pending, 2, reserve)?;
                        pending.push((body, ordinary));
                        pending.push((condition, false));
                    }
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. }
                    | ResolvedStatement::CopyDiscard { value, .. } => {
                        funded::grow(&mut pending, 1, reserve)?;
                        pending.push((value, in_loop))
                    }
                    _ => {}
                }
            }
        } else {
            funded::extend_children(expression, &mut pending, |child| (child, in_loop), reserve)?;
        }
    }
    Ok(found)
}
