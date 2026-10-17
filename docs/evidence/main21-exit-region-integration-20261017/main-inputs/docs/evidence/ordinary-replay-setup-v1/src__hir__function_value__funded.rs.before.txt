//! Test-only funded twin of `function_value::walk`. Root order (requires, body,
//! ensures), the pop-from-the-end discipline and the authored child order are
//! the original's. The pending root vector and every child growth are debited
//! on the caller's payer first; the visitor is fallible and its error comes
//! back unchanged as `Refused`, ending the walk at that expression.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

/// Visitor that owns no payer of its own: its error is the payer's `E`.
pub(crate) fn walk_funded<'a, E>(
    function: &'a ResolvedFunction,
    mut visit: impl FnMut(&'a ResolvedExpr) -> Result<(), E>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    walk_funded_paying(
        function,
        |expression, _| visit(expression).map_err(FundedError::Refused),
        reserve,
    )
}

/// Visitor that allocates too: it is handed the same payer between pushes, so
/// its own scratch is debited on one ledger in walk order.
pub(crate) fn walk_funded_paying<'a, E>(
    function: &'a ResolvedFunction,
    mut visit: impl FnMut(&'a ResolvedExpr, Reserve<'_, E>) -> Result<(), FundedError<E>>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut pending = Vec::new();
    funded::grow(
        &mut pending,
        function
            .requires
            .len()
            .saturating_add(1)
            .saturating_add(function.ensures.len()),
        reserve,
    )?;
    pending.extend(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
    );
    while let Some(expr) = pending.pop() {
        visit(expr, reserve)?;
        funded::push_children(expr, &mut pending, reserve)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::{assert_every_boundary, Payer, PayerRefusal};

    const SOURCE: &str = include_str!("../../../tests/owned_data/byte_helper_renewal.spx");

    fn order(function: &ResolvedFunction) -> Vec<String> {
        let mut seen = Vec::new();
        walk(function, |e| seen.push(e.id.as_str().to_owned()));
        seen
    }

    #[test]
    fn funded_walk_keeps_root_contract_and_child_order_at_every_boundary() {
        let checked = crate::check(SOURCE, "funded-walk.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut nodes = 0;
        for function in &program.functions {
            let old = order(function);
            nodes += old.len();
            let (value, calls, charged) = assert_every_boundary(|reserve| {
                let mut seen = Vec::new();
                walk_funded(
                    function,
                    |e| {
                        seen.push(e.id.as_str().to_owned());
                        Ok(())
                    },
                    reserve,
                )?;
                Ok(seen)
            });
            assert_eq!(value, old, "{}", function.id.as_str());
            assert!(calls > 0 && charged > 0);
        }
        assert!(nodes > 20);
    }

    #[test]
    fn funded_walk_orders_requires_body_then_ensures() {
        let source =
            "module walk; @id(\"w.main\") fn main(x:i64)->i64 requires x>0 ensures result>0 {x}";
        let diagnostics = crate::check(source, "funded-walk-contracts.spx").unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "SPX-T104");
        assert_eq!(
            diagnostics[0].message,
            "entry function must be monomorphic with signature `fn main() -> i64`"
        );
        let admitted = format!(
            "{} @id(\"app.main\") fn main()->i64 {{contract_helper(1)}}",
            source.replace("fn main(x:i64)", "fn contract_helper(x:i64)")
        );
        let checked = crate::check(&admitted, "funded-walk-contracts.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "w.main")
            .unwrap();
        assert!(!function.requires.is_empty() && !function.ensures.is_empty());
        let old = order(function);
        let mut payer = Payer::unlimited();
        let mut seen = Vec::new();
        walk_funded(
            function,
            |e| {
                seen.push(e.id.as_str().to_owned());
                Ok(())
            },
            &mut |b| payer.pay(b),
        )
        .unwrap();
        assert_eq!(seen, old);
    }

    #[test]
    fn visitor_failure_is_returned_unchanged_and_stops_the_walk() {
        let checked = crate::check(SOURCE, "funded-walk-visit.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .max_by_key(|f| order(f).len())
            .unwrap();
        let total = order(function).len();
        assert!(total > 3);
        for stop in [0, 1, total - 1] {
            let mut payer = Payer::unlimited();
            let mut visited = 0usize;
            let result = walk_funded(
                function,
                |_| {
                    visited += 1;
                    if visited == stop + 1 {
                        Err(PayerRefusal::Ordinal(stop))
                    } else {
                        Ok(())
                    }
                },
                &mut |b| payer.pay(b),
            );
            assert_eq!(
                result,
                Err(FundedError::Refused(PayerRefusal::Ordinal(stop)))
            );
            assert_eq!(visited, stop + 1);
        }
    }
}

#[cfg(test)]
pub(crate) mod work;
