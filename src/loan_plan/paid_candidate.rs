//! Current candidate order with explicit caller-paid discovery containers.
use super::*;

#[cfg(test)]
pub(super) fn inspect(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
    work: &mut WorkCounter<'_>,
) -> Result<bool, Diagnostic> {
    // Complete discovery still precedes every alias/ownership diagnostic.
    // Retain its actual paid spill and replay the immutable borrowed traversal.
    let mut discovery = paid_cursor::Cursor::new(function, payer);
    while discovery.next(work)?.is_some() {}
    discovery.reset()?;

    let mut aliases = cfg_directory::Map::<ValueId, Place>::new(payer)?;
    let mut bound = cfg_directory::Map::<ExpressionId, ValueId>::new(payer)?;
    let mut ownership = cfg_directory::Map::new(payer)?;
    for parameter in &function.params {
        cfg_directory::map_insert(
            &mut ownership,
            parameter.id.clone(),
            parameter.ownership,
            Some(payer),
        )?;
    }
    while let Some(expression) = discovery.next(work)? {
        native_view::inventory_aliases_paid(program, expression, &mut aliases, Some(payer))?;
        if let ResolvedExprKind::Match { arms, .. } = &expression.kind {
            for arm in arms {
                paid_patterns::inventory(&arm.pattern, &mut ownership, Some(payer))?;
            }
        }
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    cfg_directory::map_insert(
                        &mut ownership,
                        binding.id.clone(),
                        binding.ownership,
                        Some(payer),
                    )?;
                    if matches!(binding.ty, ResolvedType::SliceU8 | ResolvedType::Str) {
                        if let Some(place) = storage::expression_place(value, Some(payer))? {
                            cfg_directory::map_insert(
                                &mut aliases,
                                binding.id.clone(),
                                place,
                                Some(payer),
                            )?;
                            cfg_directory::map_insert(
                                &mut bound,
                                value.id.clone(),
                                binding.id.clone(),
                                Some(payer),
                            )?;
                        }
                    }
                }
            }
        }
    }

    let mut byte_census = crate::hir::ByteTraversalSlot::new(program, function)?;
    let provisional = provisional_result::root(function);
    let mut root_ownership = cfg_directory::Map::<ValueId, bool>::new(payer)?;
    discovery.reset()?;
    while let Some(expression) = discovery.next(work)? {
        if let Some(proof) = traversal::paid::inspect(program, expression, payer)? {
            if storage::expression_place(proof.source, Some(payer))?
                .is_some_and(|place| ownership.get(&place.root) == Some(&OwnershipMode::Own))
            {
                return Ok(true);
            }
        }
        // A borrowed byte traversal freezes its view's ultimate owner.
        if let Some(region) = byte_census.region_for(program, expression)? {
            if let Some(place) = storage::expression_place(region.view, Some(payer))? {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                    payer,
                )? {
                    return Ok(true);
                }
            }
        }
        let view = match &expression.kind {
            ResolvedExprKind::Call { .. } => paid_predicates::string_slot(expression)
                .map(|place| storage::clone_place(place, Some(payer)))
                .transpose()?,
            ResolvedExprKind::BorrowPlace { place, .. } => {
                Some(storage::clone_place(place, Some(payer))?)
            }
            ResolvedExprKind::VecFieldRead { args, .. }
                if matches!(
                    expression.ty,
                    crate::hir::ResolvedType::Str | crate::hir::ResolvedType::SliceU8
                ) =>
            {
                args.first()
                    .map(|source| storage::expression_place(source, Some(payer)))
                    .transpose()?
                    .flatten()
            }
            ResolvedExprKind::ByteRange { source, .. } => {
                storage::expression_place(source, Some(payer))?
            }
            ResolvedExprKind::NativeRustImportCall(call)
                if matches!(
                    call.result,
                    crate::hir::ResolvedImportResultKind::BorrowedStr { .. }
                ) =>
            {
                call.args
                    .first()
                    .map(|source| storage::expression_place(source, Some(payer)))
                    .transpose()?
                    .flatten()
            }
            ResolvedExprKind::Place(place) if bound.contains_key(&expression.id) => {
                Some(storage::clone_place(place, Some(payer))?)
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
                payer,
            )? {
                return Ok(true);
            }
        }
        for (_, argument) in
            call_arguments::borrowed_call_arguments_paid(program, expression, Some(payer))?
        {
            if let Some(place) = storage::expression_place(argument, Some(payer))? {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                    payer,
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
            if let Some(place) = storage::expression_place(scrutinee, Some(payer))? {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                    payer,
                )? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn ultimate_root_is_own(
    root: &ValueId,
    aliases: &cfg_directory::Map<ValueId, Place>,
    ownership: &cfg_directory::Map<ValueId, OwnershipMode>,
    memo: &mut cfg_directory::Map<ValueId, bool>,
    provisional: Option<&ValueId>,
    payer: &crate::bounded_output::cfg_reservation::CfgReservation,
) -> Result<bool, Diagnostic> {
    if let Some(result) = memo.get(root) {
        return Ok(*result);
    }
    let mut current = root.clone();
    let mut chain = Vec::new();
    let mut seen = cfg_directory::Set::new(payer)?;
    let result = loop {
        if let Some(result) = memo.get(&current) {
            break *result;
        }
        if !cfg_directory::set_insert(&mut seen, current.clone(), Some(payer))? {
            return Err(error("shared-loan alias provenance contains a cycle"));
        }
        let Some(parent) = aliases.get(&current) else {
            break ownership.get(&current) == Some(&OwnershipMode::Own)
                || provisional == Some(&current);
        };
        storage::push(&mut chain, current, Some(payer))?;
        current = parent.root.clone();
    };
    cfg_directory::map_insert(memo, current, result, Some(payer))?;
    for alias in chain {
        cfg_directory::map_insert(memo, alias, result, Some(payer))?;
    }
    Ok(result)
}

mod retained;
pub(super) use retained::{inspect as inspect_retained, Inventory};

#[cfg(test)]
pub(super) use retained::before_inventory_source;
