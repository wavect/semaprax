//! Direct ordinary output producer; legacy returning producer remains intact.
use super::*;
pub(crate) fn requires_into<E>(
    function: &ResolvedFunction,
    output: &mut bool,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    funded::charge(
        reserve,
        std::mem::size_of::<Vec<super::super::ChildCursor<'_>>>(),
    )?;
    let mut stack = Vec::new();
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        funded::push(&mut stack, super::super::ChildCursor::new(root), reserve)?;
        while !stack.is_empty() {
            let len = stack.len();
            let frame = stack.last_mut().expect("occupied lexical scan frame");
            if !frame.is_started()
                && matches!(
                    &frame.expression().kind,
                    crate::hir::ResolvedExprKind::LexicalExit { .. }
                )
            {
                *output = len <= crate::cache_codec::MAX_DEPTH + 1;
                return Ok(());
            }
            if let Some(child) = frame.next() {
                if len == super::super::walk::PRESENCE_DEPTH {
                    *output = false;
                    return Ok(());
                }
                funded::push(&mut stack, super::super::ChildCursor::new(child), reserve)?;
            } else {
                stack.pop();
            }
        }
    }
    *output = false;
    Ok(())
}

pub(crate) fn owner_into<E>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    output: &mut Option<FunctionExecutionId>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut found = None;
    for declaration in program.functions() {
        funded::charge(reserve, 0)?;
        let candidate = FunctionExecutionId::Monomorphic(declaration.id.clone());
        if function.result_id.matches_result(&candidate)
            && function.body.id.matches(&candidate, "body")
            && found.replace(candidate).is_some()
        {
            *output = None;
            return Ok(());
        }
    }
    for instance in program.function_instances() {
        funded::charge(reserve, instance.id.as_str().len())?;
        let candidate = FunctionExecutionId::Generic(instance.id.clone());
        if function.result_id.matches_result(&candidate)
            && function.body.id.matches(&candidate, "body")
            && found.replace(candidate).is_some()
        {
            *output = None;
            return Ok(());
        }
    }
    *output = found;
    Ok(())
}
