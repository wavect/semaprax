//! Test-only work-metered twins of `condition_reads`, `function_reads` and
//! `needs_cleanup`. Same pop discipline, same While-condition-only block rule,
//! same interpreter child order. Statement slot probes, pops, operand reads,
//! identity-set searches/inserts and every owned collection's Drop are paid on
//! `work` before they happen; bytes still go through `reserve`.
use super::*;
use crate::hir::funded::work::{self as paid, Work};

fn scan<E>(
    reads: &mut BTreeSet<ExpressionId>,
    pending: &mut Vec<&ResolvedExpr>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    loop {
        paid::tick(work, 1)?;
        let Some(expression) = pending.pop() else {
            return Ok(());
        };
        paid::tick(work, 1)?;
        // REQUIRED source (section 4): work twin of `resolved_operands`.
        if let Some(arguments) = super::super::resolved_operands_with_work(expression, work)? {
            for argument in arguments {
                paid::tick(work, 1)?;
                paid::insert_cloned_key_with_work(reads, &argument.id, work, reserve)?;
            }
            continue;
        }
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            let mut needed = 1usize;
            let mut probes = statements.len();
            for statement in statements {
                paid::tick(work, 1)?;
                if matches!(statement, ResolvedStatement::While { .. }) {
                    needed += 1;
                } else {
                    let slots = statement.child_count();
                    paid::tick(work, slots)?;
                    probes = probes.saturating_add(slots);
                    needed += (0..slots)
                        .filter(|index| statement.child(*index).is_some())
                        .count();
                }
            }
            // The second statement pass and the pushes themselves.
            paid::tick(work, probes.saturating_add(needed))?;
            funded::grow(pending, needed, reserve)?;
            pending.push(tail);
            for statement in statements {
                if let ResolvedStatement::While { condition, .. } = statement {
                    pending.push(condition);
                } else {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
        } else {
            paid::push_trace_children_with_work(expression, pending, work, reserve)?;
        }
    }
}

pub(crate) fn condition_reads_funded_with_work<E>(
    condition: &ResolvedExpr,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<ExpressionId>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    paid::tick(work, 2)?;
    let mut reads = BTreeSet::new(); // empty: no backing to drop if seeding refuses
    paid::tick(work, 2)?;
    let mut pending = funded::singleton(condition, reserve)?;
    match scan(&mut reads, &mut pending, work, reserve) {
        Ok(()) => paid::retire(pending, reads, work),
        Err(error) => Err(paid::abandon((pending, reads), error, work)),
    }
}

pub(crate) fn function_reads_funded_with_work<E>(
    function: &ResolvedFunction,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<ExpressionId>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    paid::tick(work, 2)?;
    let mut reads = BTreeSet::new();
    paid::tick(work, 2)?;
    let mut pending = funded::singleton(&function.body, reserve)?;
    let outcome = (|| -> Result<(), FundedError<E>> {
        loop {
            paid::tick(work, 1)?;
            let Some(expression) = pending.pop() else {
                return Ok(());
            };
            paid::tick(work, 1)?;
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                for statement in statements {
                    paid::tick(work, 1)?;
                    if let ResolvedStatement::While { condition, .. } = statement {
                        let found = condition_reads_funded_with_work(condition, work, reserve)?;
                        paid::extend_set_with_work(&mut reads, found, work, reserve)?;
                    }
                }
            }
            paid::push_trace_children_with_work(expression, &mut pending, work, reserve)?;
        }
    })();
    match outcome {
        Ok(()) => paid::retire(pending, reads, work),
        Err(error) => Err(paid::abandon((pending, reads), error, work)),
    }
}

#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(crate) fn needs_cleanup_funded_with_work<E>(
    condition: &ResolvedExpr,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    let reads = condition_reads_funded_with_work(condition, work, reserve)?;
    if let Err(error) = paid::tick(work, 1) {
        return Err(paid::abandon(reads, error, work));
    }
    let mut pending = match funded::singleton(condition, reserve) {
        Ok(pending) => pending,
        Err(error) => return Err(paid::abandon(reads, error, work)),
    };
    let outcome = (|| -> Result<bool, FundedError<E>> {
        loop {
            paid::tick(work, 1)?;
            let Some(expression) = pending.pop() else {
                return Ok(false);
            };
            paid::tick(work, 1)?; // discriminant test against `String`
            if expression.ty == ResolvedType::String
                && !paid::contains_with_work(&reads, &expression.id, work)?
            {
                return Ok(true);
            }
            paid::push_children_with_work(expression, &mut pending, work, reserve)?;
        }
    })();
    match outcome {
        Ok(verdict) => paid::retire((pending, reads), verdict, work),
        Err(error) => Err(paid::abandon((pending, reads), error, work)),
    }
}
