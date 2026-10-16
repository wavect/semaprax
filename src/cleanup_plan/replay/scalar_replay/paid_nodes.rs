//! Complete selected scalar traversal; legacy generic body stays independent.
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::renewal_profile::{child_refusal::Refusal, scalar_refusal::ScalarRefusal};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{ArrayReceiving, Permit};
use crate::cleanup_plan::replay::{ReplayBudget, ResolvedExprKind, ResolvedFunction};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;
pub(super) fn validate(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
    type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<Frame<'_>>(
            function,
            514,
            "selected scalar body profile cursor"
        )
    );
    let mut stack = [None; 514];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "selected scalar body profile node")?;
            let (mut cursor, entered) = match stack[depth - 1].take() {
                Some(value) => value,
                None => return Err(permit.refuse(Refusal::Scalar(ScalarRefusal::Cursor))),
            };
            let record = if !entered {
                super::records::paid::expression(program, function, cursor.expression(), budget)?
            } else {
                None
            };
            if !entered
                && !match record {
                    Some(admitted) => admitted,
                    None => {
                        // The unchanged generic helper can fail only through
                        // this exact real ReplayBudget charge callback. Keep
                        // its original Diagnostic Result; no extra Deferred
                        // primitive wrapper is introduced per root probe.
                        let _borrowed_permit = Permit::acquire::<bool>(function, budget)?;
                        crate::hir::borrowed_scalar::expression(
                            function,
                            cursor.expression(),
                            &mut |units| {
                                budget.charge(
                                    function,
                                    units,
                                    "selected borrowed helper root identity",
                                )
                            },
                        )
                        .map_err(Error::Owned)?
                    }
                }
            {
                return Err(permit.refuse(Refusal::Scalar(ScalarRefusal::NonCopy)));
            }
            if !entered
                && matches!(
                    &cursor.expression().kind,
                    ResolvedExprKind::LexicalExit { .. }
                )
            {
                return Err(permit.refuse(Refusal::Scalar(ScalarRefusal::Lexical)));
            }
            if let Some(child) = cursor.next() {
                if depth == 514 {
                    return Err(permit.refuse(Refusal::Scalar(ScalarRefusal::Depth)));
                }
                stack[depth - 1] = Some((cursor, true));
                stack[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}
