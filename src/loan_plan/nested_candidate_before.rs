//! Exact complete candidate before v2 caller adoption; cfg(test) only.
use super::*;

pub(super) fn has_own_root_candidate(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<bool, Diagnostic> {
    let roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(function.ensures.iter())
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut pending = roots.iter().rev().copied().collect::<Vec<_>>();
    while let Some(expression) = pending.pop() {
        expressions.push(expression);
        push_children(expression, &mut pending);
    }

    let mut aliases = BTreeMap::<ValueId, Place>::new();
    let mut bound = BTreeMap::<ExpressionId, ValueId>::new();
    let mut ownership = function
        .params
        .iter()
        .map(|param| (param.id.clone(), param.ownership))
        .collect::<BTreeMap<_, _>>();
    for expression in &expressions {
        native_view::inventory_aliases(program, expression, &mut aliases);
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                inventory_pattern_ownership(&arm.pattern, &mut ownership);
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    ownership.insert(binding.id.clone(), binding.ownership);
                    if matches!(binding.ty, ResolvedType::SliceU8 | ResolvedType::Str) {
                        if let Some(place) = expression_place(value) {
                            aliases.insert(binding.id.clone(), place);
                            bound.insert(value.id.clone(), binding.id.clone());
                        }
                    }
                }
            }
        }
    }

    let provisional = provisional_result::root(function);
    let mut root_ownership = BTreeMap::<ValueId, bool>::new();
    for expression in expressions {
        if let Some(proof) = traversal::inspect(program, expression)? {
            if expression_place(proof.source)
                .is_some_and(|place| ownership.get(&place.root) == Some(&OwnershipMode::Own))
            {
                return Ok(true);
            }
        }
        // A borrowed byte traversal freezes its view's ultimate owner.
        if let Some(region) = crate::hir::byte_traversal_region(expression)? {
            if let Some(place) = expression_place(region.view) {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                )? {
                    return Ok(true);
                }
            }
        }
        let view = match &expression.kind {
            ResolvedExprKind::Call { .. } => {
                crate::vec_ops::string_slot::place(expression).cloned()
            }
            ResolvedExprKind::BorrowPlace { place, .. } => Some(place.clone()),
            ResolvedExprKind::VecFieldRead { args, .. }
                if matches!(
                    expression.ty,
                    crate::hir::ResolvedType::Str | crate::hir::ResolvedType::SliceU8
                ) =>
            {
                args.first().and_then(expression_place)
            }
            ResolvedExprKind::ByteRange { source, .. } => expression_place(source),
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                call.args.first().and_then(expression_place)
            }
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(place.clone())
            }
            _ => None,
        };
        if let Some(place) = view {
            if ultimate_root_is_own(
                &place.root,
                &aliases,
                &ownership,
                &mut root_ownership,
                provisional,
            )? {
                return Ok(true);
            }
        }
        for (_, argument) in borrowed_call_arguments(program, expression) {
            if let Some(place) = expression_place(argument) {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                )? {
                    return Ok(true);
                }
            }
        }
        if let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            scrutinee,
            ..
        } = &expression.kind
        {
            if let Some(place) = expression_place(scrutinee) {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                )? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
