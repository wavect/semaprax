//! Complete ordinary schema selection on the replay invocation payer.
use super::*;
mod iterator_presence;
mod renewal_selectors;

pub(in crate::cleanup_plan::replay) fn selected_with_budget(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<&'static str, Diagnostic> {
    #[cfg(test)]
    if shipping_controls::previous_selected() {
        let _phase = crate::test_allocation_observer::function_phase_guard("schema-previous");
        return before_selectors::selected_with_budget(program, function, helpers, budget);
    }
    #[cfg(test)]
    let _phase = crate::test_allocation_observer::function_phase_guard("schema-current");
    // Exactly the prior unfunded/reservation-only compatibility API remains.
    // The new constructor is selected only by the existing builder-like payer;
    // no selected admission, credits, session policy or quota is introduced.
    if !budget.pays_builder() {
        return super::selected_schema_with_index(program, function, helpers);
    }
    budget.reserve_bytes(function, 0, "summary schema live sticky authority")?;
    // Independently replayed lexical exits select the lexical certificate
    // profile regardless of any attached inventory or plan.
    let helper_sites = match helpers {
        Some(index) => index.has_sites(function),
        None => !crate::loop_calls::byte_writer::checked_bindings(program, function)?.is_empty(),
    };
    if helper_sites {
        return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19);
    }
    if lexical_cert::requires_lexical(function) {
        return Ok(CLEANUP_PLAN_SCHEMA_V18);
    }
    // Complete ordinary inventory validation precedes the selector scans.
    // The attached plan has no authority over either decision.
    let inventory = paid_type_facts::inventory_summary::collect(program, function, budget)?;
    let has_nested_owned_bytes = inventory.has_nested_owned_bytes;
    let has_nested_record_destructure = record_destructure::function_contains(program, function);
    let mut payer = paid_type_facts::Payer::capture(function, budget)?;
    let has_nested_record_update =
        record_destructure::update::paid::contains(program, function, &mut payer)?;
    if renewal_selectors::includes(program, function, &mut payer, renewal_selectors::Kind::Byte)? {
        return Ok(CLEANUP_PLAN_SCHEMA_V17);
    }
    if renewal_selectors::includes(program, function, &mut payer, renewal_selectors::Kind::Text)? {
        return Ok(CLEANUP_PLAN_SCHEMA_V16);
    }
    if renewal_selectors::includes(
        program,
        function,
        &mut payer,
        renewal_selectors::Kind::Vector,
    )? {
        return Ok(CLEANUP_PLAN_SCHEMA_V15);
    }
    if function_has_owner_admission(program, function)? {
        return Ok(CLEANUP_PLAN_SCHEMA_V14);
    }
    {
        let mut presence = iterator_presence::Walk::new(function, &mut payer)?;
        if presence.uses_owned_iterator()?
            || presence.uses_record_iterator(program.declarations())?
        {
            return Ok(CLEANUP_PLAN_SCHEMA_V13);
        }
        if renewal_selectors::includes(
            program,
            function,
            presence.payer,
            renewal_selectors::Kind::Iterator,
        )? {
            return Ok(CLEANUP_PLAN_SCHEMA_V12);
        }
        if presence.contains_iterator_loop()? {
            return Ok(CLEANUP_PLAN_SCHEMA_V11);
        }
    }
    Ok(if inventory.has_iterator {
        CLEANUP_PLAN_SCHEMA_V10
    } else if has_nested_record_update {
        CLEANUP_PLAN_SCHEMA_V9
    } else if has_nested_record_destructure {
        CLEANUP_PLAN_SCHEMA_V8
    } else if has_nested_owned_bytes {
        CLEANUP_PLAN_SCHEMA_V7
    } else if inventory.has_variant
        || function
            .requires
            .iter()
            .any(expression_has_explicit_variant_match)
        || function
            .ensures
            .iter()
            .any(expression_has_explicit_variant_match)
        || expression_has_explicit_variant_match(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V6
    } else if function
        .requires
        .iter()
        .any(expression_has_explicit_record_match)
        || function
            .ensures
            .iter()
            .any(expression_has_explicit_record_match)
        || expression_has_explicit_record_match(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V5
    } else if function.requires.iter().any(expression_has_byte_range)
        || function.ensures.iter().any(expression_has_byte_range)
        || expression_has_byte_range(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V4
    } else if function.requires.iter().any(expression_has_option_try)
        || function.ensures.iter().any(expression_has_option_try)
        || expression_has_option_try(&function.body)
    {
        CLEANUP_PLAN_SCHEMA_V3
    } else {
        CLEANUP_PLAN_SCHEMA_V2
    })
}

#[cfg(test)]
#[rustfmt::skip]
mod before_selectors;
#[cfg(test)]
mod original_receiver;
#[cfg(test)]
mod positive_schema_controls;
#[cfg(test)]
mod shipping_controls;
#[cfg(test)]
mod tests;
