//! Full original owner candidate order with a separate explicit work receiver.
//! Existing byte payer, cached ValueId fingerprint and first ambiguity remain.
use super::*;

pub(crate) fn owner_funded_with_work<E>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
    work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Result<FunctionExecutionId, OwnerVerdict>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    work(1).map_err(FundedError::Refused)?;
    let mut found = None;
    for declaration in program.functions() {
        work(2).map_err(FundedError::Refused)?;
        funded::charge(reserve, 0)?;
        let candidate = FunctionExecutionId::Monomorphic(declaration.id.clone());
        if crate::hir::ids::oracle_owner_matches_with_work(function, &candidate, work)
            .map_err(FundedError::Refused)?
            && found.replace(candidate).is_some()
        {
            return Ok(Err(OwnerVerdict::Ambiguous));
        }
    }
    for instance in program.function_instances() {
        work(2).map_err(FundedError::Refused)?;
        funded::charge(reserve, instance.id.as_str().len())?;
        work(instance.id.as_str().len()).map_err(FundedError::Refused)?;
        let candidate = FunctionExecutionId::Generic(instance.id.clone());
        if crate::hir::ids::oracle_owner_matches_with_work(function, &candidate, work)
            .map_err(FundedError::Refused)?
            && found.replace(candidate).is_some()
        {
            return Ok(Err(OwnerVerdict::Ambiguous));
        }
    }
    Ok(found.ok_or(OwnerVerdict::Missing))
}

/// Complete old lexical scan, with work before each cursor operation. Its
/// admitted fixed depth and the old overflow-as-false compatibility remain.
pub(crate) fn requires_funded_with_work<E>(
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
    work: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, FundedError<E>> {
    const DEPTH_TEXT: &str = "lexical control scan exceeds admitted expression depth";
    funded::charge(
        reserve,
        std::mem::size_of::<
            [Option<super::super::ChildCursor<'_>>; super::super::walk::PRESENCE_DEPTH],
        >(),
    )?;
    funded::charge(reserve, DEPTH_TEXT.len())?;
    work(1).map_err(FundedError::Refused)?;
    let mut stack = [None; super::super::walk::PRESENCE_DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        work(1).map_err(FundedError::Refused)?;
        stack[0] = Some(super::super::ChildCursor::new(root));
        let mut len = 1usize;
        while len != 0 {
            work(1).map_err(FundedError::Refused)?;
            let frame = stack[len - 1]
                .as_mut()
                .expect("occupied lexical scan frame");
            let expression = frame.expression();
            if !frame.is_started()
                && matches!(
                    &expression.kind,
                    crate::hir::ResolvedExprKind::LexicalExit { .. }
                )
            {
                return Ok(len <= crate::cache_codec::MAX_DEPTH + 1);
            }
            if let Some(next) = frame.next_with_work(work).map_err(FundedError::Refused)? {
                if len == stack.len() {
                    return Ok(false);
                }
                stack[len] = Some(super::super::ChildCursor::new(next));
                len += 1;
            } else {
                stack[len - 1] = None;
                len -= 1;
            }
        }
    }
    Ok(false)
}
