//! Explicit funded twin of `same_owner_set_bindings` (explicit payer). The census
//! order, closure skip and mutable filter are the original ones; every Vec,
//! set/map entry and tree node is reserved on the supplied payer first.
use crate::hir::funded::{self, FundedError, Reserve};
use crate::hir::{
    ExpressionId, ResolvedBinding, ResolvedExprKind, ResolvedFunction, ResolvedStatement,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn same_owner_set_bindings_funded<'a, E>(
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut mutable = BTreeSet::new();
    let mut updates = Vec::new();
    let mut pending = funded::singleton(&function.body, reserve)?;
    while let Some(expression) = pending.pop() {
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            continue;
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable: true,
                        ..
                    } => {
                        funded::insert_cloned_key(&mut mutable, &binding.id, reserve)?;
                    }
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } if super::is_same_owner_set_hir(value, &binding.id) => {
                        funded::charge_clone(&value.id, reserve)?;
                        funded::push(&mut updates, (value.id.clone(), binding), reserve)?;
                    }
                    _ => {}
                }
            }
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    // The original `collect` keeps the last value of a duplicate identity.
    let mut found = BTreeMap::new();
    for (at, binding) in updates {
        if mutable.contains(&binding.id) {
            funded::insert_moved(&mut found, at, binding, reserve)?;
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::assert_every_boundary;

    const SOURCE: &str = include_str!("../../tests/owned_data/byte_helper_renewal.spx");

    #[test]
    fn funded_same_owner_set_matches_the_original_and_refuses_at_every_boundary() {
        let checked = crate::check(SOURCE, "funded-byte-set.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut sites = 0;
        for function in &program.functions {
            let normalize = |found: BTreeMap<ExpressionId, &ResolvedBinding>| {
                found
                    .into_iter()
                    .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
                    .collect::<Vec<_>>()
            };
            let old = normalize(super::super::same_owner_set_bindings(function));
            sites += old.len();
            let (value, calls, _) = assert_every_boundary(|reserve| {
                Ok(normalize(same_owner_set_bindings_funded(
                    function, reserve,
                )?))
            });
            assert_eq!(value, old, "{}", function.id.as_str());
            assert!(calls > 0);
        }
        assert_eq!(sites, 1, "only the helper leaf republishes its own owner");
    }
}
