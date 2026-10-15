//! Retain actual complete candidate inventories for the same immutable function.
use super::*;
mod inventory;
pub(in crate::loan_plan) use inventory::Inventory;
const RESULT_BYTES: usize = std::mem::size_of::<Result<Option<Inventory<'static, 'static>>, Diagnostic>>()
    + std::mem::size_of::<Option<Inventory<'static, 'static>>>();

pub(in crate::loan_plan) fn inspect<'a, 'p>(
    subject: funded_flat::CurrentSubject<'a>,
    payer: &'p crate::bounded_output::cfg_reservation::CfgReservation,
    work: &mut WorkCounter<'_>,
) -> Result<Option<Inventory<'a, 'p>>, Diagnostic> {
    payer.reserve(RESULT_BYTES).map_err(|_| storage::marker())?;
    let (program, function) = subject.into_refs();
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

    // One expansion governs all five positive branches, so every initializer
    // and its affine consuming/receiving carriers have the same full payment.
    macro_rules! retain {
        () => {{
            payer.reserve(std::mem::size_of::<Inventory<'_, '_>>() + inventory::PARTS_BYTES)
                .map_err(|_| storage::marker())?;
            return Ok(Some(Inventory { program, function, payer, aliases, bound, ownership }));
        }};
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
                retain!();
            }
        }
        // A borrowed byte traversal freezes its view's ultimate owner.
        if let Some(region) = byte_census.region(program, function, expression)? {
            if let Some(place) = storage::expression_place(region.view, Some(payer))? {
                if ultimate_root_is_own(
                    &place.root,
                    &aliases,
                    &ownership,
                    &mut root_ownership,
                    provisional,
                    payer,
                )? {
                    retain!();
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
                retain!();
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
                    retain!();
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
                    retain!();
                }
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(in crate::loan_plan) fn before_inventory_source(path: &str, source: &str) -> String {
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/docs/evidence/loan-candidate-inventory-reuse/exact-source-inverses.json"
    ))).unwrap();
    let row = rows.as_array().unwrap().iter().find(|row| row["source"] == path).unwrap();
    let mut restored = source.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    restored
}

#[cfg(test)]
mod nested_tests;
