//! Explicit funded twin of `bindings` (explicit payer). Traversal order, the closure
//! skip, the admission predicate and the mutable filter are the original ones;
//! every Vec, set/map entry and tree node is reserved on the supplied payer.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

pub(crate) fn bindings_funded<'a, E>(
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut mutable = BTreeSet::new();
    let mut assignments = Vec::new();
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
                    } if admitted(binding, value)
                        && !super::super::is_same_owner_concat_hir(value, &binding.id) =>
                    {
                        funded::charge_clone(&value.id, reserve)?;
                        funded::push(&mut assignments, (value.id.clone(), binding), reserve)?;
                    }
                    _ => {}
                }
            }
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    let mut found = BTreeMap::new();
    for (at, binding) in assignments {
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

    const SOURCE: &str = r#"module directory_string;
@id("main") fn main()->i64 { let mut text="a"; let mut i=0;
 while i<1 {text=string_concat("prefix",text);i=i+1;0} 0 }
"#;

    #[test]
    fn funded_replacement_matches_the_original_and_refuses_at_every_boundary() {
        let checked = crate::check(SOURCE, "funded-string.spx").unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[0];
        let normalize = |found: BTreeMap<ExpressionId, &ResolvedBinding>| {
            found
                .into_iter()
                .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
                .collect::<Vec<_>>()
        };
        let old = normalize(super::bindings(function));
        let (value, calls, charged) =
            assert_every_boundary(|reserve| Ok(normalize(bindings_funded(function, reserve)?)));
        assert_eq!(value, old);
        assert!(calls > 0 && charged > 0);
    }
}
