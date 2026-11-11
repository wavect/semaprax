//! Explicit funded twin of `renewal_bindings` (explicit payer). Record-then-default
//! precedence, traversal order and the admission predicates are the original
//! ones; every Vec, map entry and tree node is reserved on the supplied payer
//! first, and a refusal is returned instead of a smaller binding set.
//!
//! Source audit of the original predicates this module still calls (read in
//! full, no execution; a source-only claim, not a measured one). Each only
//! borrows and compares; none constructs a Vec/String/map/set/clone/type key:
//! - `recognize_scoped` -> `zero`, `place`, `cases`, `replacement`,
//!   `step_element`, `iter_matches` (`matches!`/`==`/`as_str`/`from_ref`),
//!   `iterator_ops::{item_ownership, resolved_element_is_admitted}`,
//!   `is_scalar_resolved_type`, `generic_collection::parameter` (pattern
//!   matches and integer compare). It returns `IteratorLoop` of three borrows.
//! - `is_record_owner_renewal` -> `is_owner_renewal_record`
//!   (`DeclarationIndex::{declaration, type_parameters, record_fields}` are
//!   `BTreeMap::get` + `Vec::as_slice`; iterator `filter/count/all` only),
//!   `ReplayContext::resolve_call_target` (linear `iter().find`, borrowed
//!   result), `loop_calls::resolved_renewal_view` (`matches!`).
//!
//! The only owning expression on this route was `same_owner`'s
//! `[element.clone()]`, which `same_owner_funded` replaces. If any of the
//! callees above gains an owning temporary, this audit must be redone.
use super::*;
use crate::hir::funded::{self, FundedError, Reserve};

pub(crate) fn renewal_bindings_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut found = record_bindings_funded(program, &function.body, reserve)?;
    // Same historical default-profile precedence as the original `extend`.
    let defaults = bindings_funded(&function.body, None, reserve)?;
    funded::extend_map(&mut found, defaults, reserve)?;
    Ok(found)
}

/// Funded single-site twin of `renewal_binding`: default profile first, then
/// record owners, each collector fully funded; the first map is dropped before
/// the second is built, as in the original.
#[cfg(test)]
pub(crate) fn renewal_binding_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    at: &ExpressionId,
    reserve: Reserve<'_, E>,
) -> Result<Option<&'a ResolvedBinding>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    if let Some(found) = bindings_funded(&function.body, None, reserve)?.remove(at) {
        return Ok(Some(found));
    }
    Ok(record_bindings_funded(program, &function.body, reserve)?.remove(at))
}

fn record_bindings_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    body: &'a ResolvedExpr,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    let mut found = BTreeMap::new();
    let mut pending = funded::singleton(body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::While { body, .. }
                | ResolvedStatement::LexicalWhile { body, .. } = statement
                {
                    collect_record_bindings_funded(program, body, &mut found, reserve)?;
                }
            }
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    Ok(found)
}

fn collect_record_bindings_funded<'a, E>(
    program: &'a dyn crate::cleanup::ReplayContext,
    body: &'a ResolvedExpr,
    found: &mut BTreeMap<ExpressionId, &'a ResolvedBinding>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let mut pending = funded::singleton(body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if is_record_owner_renewal(program, binding, value) {
                        funded::insert_cloned(found, &value.id, binding, reserve)?;
                    }
                }
            }
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    Ok(())
}

fn bindings_funded<'a, E>(
    body: &'a ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeMap<ExpressionId, &'a ResolvedBinding>, FundedError<E>> {
    let mut found = BTreeMap::new();
    let mut pending = funded::singleton(body, reserve)?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::While {
                    condition, body, ..
                }
                | ResolvedStatement::LexicalWhile {
                    condition, body, ..
                } = statement
                {
                    if let Some(protocol) = recognize_scoped(condition, body, owner) {
                        conditional_bindings_funded(
                            protocol.authored_body,
                            owner,
                            &mut found,
                            reserve,
                        )?;
                    }
                }
            }
        }
        funded::push_children(expression, &mut pending, reserve)?;
    }
    Ok(found)
}

fn conditional_bindings_funded<'a, E>(
    body: &'a ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
    found: &mut BTreeMap<ExpressionId, &'a ResolvedBinding>,
    reserve: Reserve<'_, E>,
) -> Result<(), FundedError<E>> {
    let mut pending = funded::singleton((body, false), reserve)?;
    while let Some((expression, conditional)) = pending.pop() {
        if let ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &expression.kind
        {
            funded::grow(&mut pending, 3, reserve)?;
            pending.push((condition, conditional));
            pending.push((then_branch, true));
            pending.push((else_branch, true));
            continue;
        }
        if conditional {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                for statement in statements {
                    if let ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        if same_owner_funded(binding, value, owner) {
                            funded::insert_cloned(found, &value.id, binding, reserve)?;
                        }
                    }
                }
            }
        }
        funded::extend_children(
            expression,
            &mut pending,
            |child| (child, conditional),
            reserve,
        )?;
    }
    Ok(())
}

/// `same_owner` with the allocating `[element.clone()]` comparand replaced by
/// the equal borrowed comparison, so the funded route owns no unpaid clone.
fn same_owner_funded(
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
    owner: Option<(&DeclarationId, usize)>,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &binding.ty
    else {
        return false;
    };
    let [element] = arguments.as_slice() else {
        return false;
    };
    if declaration.as_str() != crate::prelude::VEC_ID
        || binding.ownership != OwnershipMode::Own
        || value.ty != binding.ty
        || value.ownership != OwnershipMode::Own
        || !(generic_collection::scalar(element)
            || owner
                .is_some_and(|(owner, count)| generic_collection::parameter(element, owner, count)))
    {
        return false;
    }
    matches!(&value.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
        if crate::vec_ops::by_id(callee.as_str()).is_some_and(|op| op.reopens_same_owner() && args.len() == op.arity())
        && type_arguments.as_slice() == std::slice::from_ref(element)
        && args.first().is_some_and(|argument| place(argument, &binding.id) && argument.ty == binding.ty && argument.ownership == OwnershipMode::Own))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::funded::payer::assert_every_boundary;

    const SOURCE: &str = r#"module record.renewal;
@id("cursor") record Cursor { @id("cursor.bytes") bytes: Bytes, @id("cursor.index") index: usize, }
@id("advance") fn advance(value: own Cursor)->Cursor { value }
@id("main") fn main()->i64 {
 let mut cursor=Cursor{bytes:bytes_zeroed(0usize),index:0usize};
 while false { cursor=advance(cursor); continue; }
 0
}"#;

    #[test]
    fn funded_receiver_matches_the_original_and_refuses_at_every_boundary() {
        let parsed = crate::check(SOURCE, "funded-record-renewal.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        crate::hir::validate(&program).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "main")
            .unwrap();
        let old = renewal_bindings(&program, function)
            .into_iter()
            .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
            .collect::<Vec<_>>();
        assert_eq!(old.len(), 1);
        let (value, calls, charged) = assert_every_boundary(|reserve| {
            Ok(renewal_bindings_funded(&program, function, reserve)?
                .into_iter()
                .map(|(at, binding)| (at.as_str().to_owned(), binding.id.as_str().to_owned()))
                .collect::<Vec<_>>())
        });
        assert_eq!(value, old, "binding set and order are the original's");
        assert!(calls > 4 && charged > 0, "both collectors were funded");
        // Single-site lookup: the recorded site and an absent site.
        let site = renewal_bindings(&program, function)
            .into_keys()
            .next()
            .unwrap();
        for at in [site.clone(), program.functions[0].body.id.clone()] {
            let old = renewal_binding(&program, function, &at).map(|b| b.id.as_str().to_owned());
            let (value, calls, _) = assert_every_boundary(|reserve| {
                Ok(renewal_binding_funded(&program, function, &at, reserve)?
                    .map(|b| b.id.as_str().to_owned()))
            });
            assert_eq!(value, old);
            assert!(calls > 0);
        }
    }

    #[test]
    fn funded_same_owner_agrees_with_the_cloning_original() {
        // Both predicates are exercised through the Vec renewal loop sources of
        // the enclosing profile controls; here the borrowed comparand is pinned
        // on a non-Vec binding, which both must reject identically.
        let parsed = crate::check(SOURCE, "funded-same-owner.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "main")
            .unwrap();
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    if let ResolvedStatement::Assign { binding, value, .. } = statement {
                        assert_eq!(
                            same_owner_funded(binding, value, None),
                            same_owner(binding, value, None)
                        );
                    }
                }
            }
            push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
    }
}
