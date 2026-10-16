//! Explicit-payer funded twin of `checked_moving_string_operands`. Precedence is the
//! original's: same-owner concat operands first, then every `take` operand and
//! every direct owning String Place passed to a `LiteralFormat`, in
//! `function_value::walk` order. Each insertion debits its tree node (only for a
//! genuinely new identity) and its clone payload before the clone, on the
//! caller's payer; a payer refusal is returned unchanged, never as a smaller
//! set. Audit (source read, not measured): this route never calls `scan` or
//! builds `Piece`/template text, so no literal-format text parsing is reachable.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};
use std::collections::BTreeSet;

pub(crate) fn checked_moving_string_operands_funded<E>(
    function: &crate::hir::ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<crate::hir::ExpressionId>, FundedError<E>> {
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
    Ok(operands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::assert_every_boundary;

    const SOURCE: &str = r#"module moving.funded;
@id("m.main") fn main()->i64 {
 let mut text=string_from_i64(1);
 let mut count=0;
 while count<2 {
  text=string_concat(text,"x");
  count=count+1;
  0
 }
 string_len(text)
}"#;

    fn names(set: &BTreeSet<crate::hir::ExpressionId>) -> Vec<String> {
        set.iter().map(|id| id.as_str().to_owned()).collect()
    }

    #[test]
    fn funded_moving_operands_match_the_original_at_every_boundary() {
        let checked = crate::check(SOURCE, "funded-moving.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[0];
        let old = checked_moving_string_operands(function).unwrap();
        assert!(!old.is_empty(), "the append operand is moving");
        let (value, calls, charged) = assert_every_boundary(|reserve| {
            Ok(names(&checked_moving_string_operands_funded(
                function, reserve,
            )?))
        });
        assert_eq!(value, names(&old));
        assert!(calls > 0 && charged > 0);
    }

    #[test]
    fn funded_moving_operands_never_touch_the_production_ledger() {
        let checked = crate::check(SOURCE, "funded-moving-active.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let (calls, refused, active) = crate::bounded_output::with_limit_usage(0, || {
            let mut payer = funded::payer::Payer::unlimited();
            checked_moving_string_operands_funded(&program.functions[0], &mut |b| payer.pay(b))
                .unwrap();
            payer.calls
        });
        assert!(calls > 0 && !refused);
        assert_eq!(active, 0);
    }
}

#[cfg(test)]
pub(crate) mod work;
