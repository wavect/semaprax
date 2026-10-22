//! Direct ordinary output producer; legacy returning producer remains intact.
use super::*;
pub(crate) fn moving_into<E>(
    function: &crate::hir::ResolvedFunction,
    output: &mut BTreeSet<crate::hir::ExpressionId>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    use crate::hir::{OwnershipMode, ResolvedExprKind, ResolvedType};
    let mut operands = crate::string_ops::same_owner_concat_operands_funded(function, reserve)?;
    crate::hir::function_value::walk_funded_paying(
        function,
        |expression, reserve| {
            if let Some(argument) = crate::string_ops::take::operand(expression) {
                funded::insert_cloned_key(&mut operands, &argument.id, reserve)?;
            }
            if let ResolvedExprKind::LiteralFormat { args, .. } = &expression.kind {
                for argument in args.iter().filter(|argument| {
                    argument.ty == ResolvedType::String
                        && argument.ownership == OwnershipMode::Own
                        && matches!(argument.kind, ResolvedExprKind::Place(_))
                }) {
                    funded::insert_cloned_key(&mut operands, &argument.id, reserve)?;
                }
            }
            Ok(())
        },
        reserve,
    )?;
    *output = operands;
    Ok(())
}
