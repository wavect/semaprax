//! Exact bounded lexical scan with a closed inline depth refusal.
use super::ChildCursor;
use crate::cleanup_plan::renewal_profile::child_refusal::Refusal;
use crate::hir::{ResolvedExprKind as K, ResolvedFunction};
/// Admits exactly what the ordinary `requires` scan admits: the same 513-frame
/// presence stack, and the narrower cache depth only when an actual exit is
/// reached beyond it (87612d690). Only the refusal type differs.
pub(crate) fn requires_inline(function: &ResolvedFunction) -> Result<bool, Refusal> {
    let mut stack = [None; super::walk::PRESENCE_DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some(ChildCursor::new(root));
        let mut len = 1usize;
        while len != 0 {
            let frame = stack[len - 1]
                .as_mut()
                .expect("occupied lexical scan frame");
            let expr = frame.expression();
            if !frame.is_started() && matches!(&expr.kind, K::LexicalExit { .. }) {
                if len > crate::cache_codec::MAX_DEPTH + 1 {
                    return Err(Refusal::LexicalDepth);
                }
                return Ok(true);
            }
            if let Some(next) = frame.next() {
                if len == stack.len() {
                    return Err(Refusal::LexicalDepth);
                }
                stack[len] = Some(ChildCursor::new(next));
                len += 1;
            } else {
                stack[len - 1] = None;
                len -= 1;
            }
        }
    }
    Ok(false)
}

pub(crate) fn validate_owner_inline(
    function: &ResolvedFunction,
    owner: &crate::hir::FunctionExecutionId,
) -> Result<(), Refusal> {
    if function.result_id.matches_result(owner) && function.body.id.matches(owner, "body") {
        Ok(())
    } else {
        Err(Refusal::Profile(crate::cleanup_plan::renewal_profile::profile_refusal::ProfileRefusal::LexicalFunctionOwnerDiffersFromCanonicalResultBodyIdentities))
    }
}
