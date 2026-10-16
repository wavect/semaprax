//! Test-only Oracle routing through the complete paid ordinary inventory fold.
//! The lease is a backing/work payer, never semantic admission. The full
//! original classifier and outer cleanup receiver remain separate and unchanged.
use super::*;
use crate::hir::TypeFlags;

pub(super) fn flags(
    index: &DeclarationIndex,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<Option<TypeFlags>, Diagnostic> {
    inventory_flags::flags(
        index,
        ty,
        &mut Payer {
            function,
            budget,
            ledger: ClassifierLedger::oracle_marker(),
        },
    )
}

pub(super) fn needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let mut payer = Payer {
        function,
        budget,
        ledger: ClassifierLedger::oracle_marker(),
    };
    match inventory_drop::needs_drop(program, ty, &mut payer) {
        Ok(value) => Ok(value),
        Err(error)
            if payer
                .budget
                .oracle_lease()
                .is_some_and(|lease| lease.refused()) =>
        {
            Err(error)
        }
        // The raw paid receiver already owns its complete semantic message.
        // Borrow it while the replay diagnostic adds the original prefix;
        // the held raw message and replacement both remain on this account.
        Err(error) => Err(droppability::replay_diagnostic(
            function,
            &error.message,
            &mut payer,
        )?),
    }
}

fn receive<T>(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    body: impl FnOnce(&mut Payer<'_>) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    body(&mut Payer {
        function,
        budget,
        ledger: ClassifierLedger::oracle_marker(),
    })
}

pub(in crate::cleanup_plan::replay) fn primitive(
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<&'static str>, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_lifecycle::variants::primitive(ty, payer)
    })
}

pub(in crate::cleanup_plan::replay) fn vector(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_metadata::step(payer, 1)?;
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Ok(false);
        };
        if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::VEC_ID, payer)? {
            return Ok(false);
        }
        inventory_metadata::step(payer, 1)?;
        let [element] = arguments.as_slice() else {
            return Ok(false);
        };
        inventory_metadata::step(payer, 1)?;
        if crate::vec_ops::resolved_vec_element_is_admitted(element) {
            return Ok(true);
        }
        // Preserve the original scalar / owned-record / copy / leaf order.
        // The complete ordinary predicate does not demand a Vec declaration
        // row here; this receiver adds no metadata authority or restriction.
        let probes = index.inventory_probes();
        inventory_metadata::with_work(payer, |work| {
            Ok(
                crate::hir::owned_record_collection::element_with_work(&probes, element, work)?
                    || crate::hir::copy_record_collection::fields_with_work(
                        &probes, element, work,
                    )?
                    .is_some()
                    || crate::hir::owned_leaf_collection::layout_with_work(&probes, element, work)?
                        .is_some(),
            )
        })
    })
}

pub(in crate::cleanup_plan::replay) fn iterator(
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_lifecycle::variants::is_iter(ty, payer)
    })
}

pub(in crate::cleanup_plan::replay) fn boxed(
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_metadata::step(payer, 1)?;
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Ok(false);
        };
        if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::BOX_ID, payer)? {
            return Ok(false);
        }
        inventory_metadata::step(payer, 1)?;
        let [element] = arguments.as_slice() else {
            return Ok(false);
        };
        inventory_metadata::step(payer, 1)?;
        Ok(crate::box_ops::resolved_box_element_is_admitted(element))
    })
}

pub(in crate::cleanup_plan::replay) fn record_field(
    program: &dyn crate::cleanup::ReplayContext,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_lifecycle::variants::record(program, container, case, field, ty, payer)
    })
}

pub(in crate::cleanup_plan::replay) fn leaf<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a str>, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_lifecycle::variants::leaf(program, container, case, field, ty, payer)
    })
}

pub(in crate::cleanup_plan::replay) fn type_item<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    declaration: &DeclarationId,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a crate::hir::ResolvedTypeDeclaration>, Diagnostic> {
    receive(function, budget, |payer| {
        inventory_text::current(payer)?;
        for item in program.types() {
            inventory_metadata::step(payer, 1)?;
            if inventory_text::owner_matches(&item.id, declaration, payer)? {
                return Ok(Some(item));
            }
        }
        Ok(None)
    })
}

#[cfg(test)]
mod tests;
