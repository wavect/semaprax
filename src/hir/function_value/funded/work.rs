//! Test-only work-metered twin of `walk_funded_paying`. Root order, pop-from-the-end
//! and authored child order are the original's. Every pop, root copy, child
//! read and push is announced on `work` first; the pending vector's Drop is paid
//! before it is dropped.
use super::*;
use crate::hir::funded::work::{self as paid, Work};

pub(crate) fn walk_funded_paying_with_work<'a, E>(
    function: &'a ResolvedFunction,
    mut visit: impl FnMut(&'a ResolvedExpr, Work<'_, E>, Reserve<'_, E>) -> Result<(), FundedError<E>>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    funded::charge(reserve, 0)?;
    paid::tick(work, 2)?; // acquisition and receiving buffer release
    let roots = function
        .requires
        .len()
        .checked_add(1)
        .and_then(|n| n.checked_add(function.ensures.len()))
        .ok_or(FundedError::Layout("funded root count overflowed"))?;
    paid::tick(work, roots)?;
    let mut pending = Vec::new();
    funded::grow(&mut pending, roots, reserve)?;
    pending.extend(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
    );
    let outcome = (|| -> Result<(), FundedError<E>> {
        loop {
            paid::tick(work, 1)?;
            let Some(expr) = pending.pop() else {
                return Ok(());
            };
            visit(expr, work, reserve)?;
            paid::push_children_with_work(expr, &mut pending, work, reserve)?;
        }
    })();
    match outcome {
        Ok(()) => paid::retire(pending, (), work),
        Err(error) => Err(paid::abandon(pending, error, work)),
    }
}

#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(crate) fn walk_funded_with_work<'a, E>(
    function: &'a ResolvedFunction,
    mut visit: impl FnMut(&'a ResolvedExpr) -> Result<(), E>,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    walk_funded_paying_with_work(
        function,
        |expression, _, _| visit(expression).map_err(FundedError::Refused),
        work,
        reserve,
    )
}
