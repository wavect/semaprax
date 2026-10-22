//! Direct ordinary output producer; legacy returning producer remains intact.
use super::*;
pub(crate) fn reads_into<E>(
    function: &ResolvedFunction,
    output: &mut BTreeSet<ExpressionId>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut reads = BTreeSet::new();
    let mut pending = funded::singleton(&function.body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::While { condition, .. } = statement {
                    let found = condition_reads_funded(condition, reserve)?;
                    funded::extend_set(&mut reads, found, reserve)?;
                }
            }
        }
        funded::push_trace_children(expression, &mut pending, reserve)?;
    }
    *output = reads;
    Ok(())
}
