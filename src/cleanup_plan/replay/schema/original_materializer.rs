//! Complete pre-adoption schema materializer, independently executable.
use super::*;
pub(super) fn selected_schema_with_index(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<&'static str, Diagnostic> {
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
    let inventory = crate::cleanup::build_inventory(program, function)?;
    let has_nested_owned_bytes = inventory.slots.iter().try_fold(false, |nested, slot| {
        crate::cleanup::cleanup_shape_profile(&slot.shape)
            .map(|profile| nested || profile.has_nested_owned_bytes)
    })?;
    let has_nested_record_destructure = record_destructure::function_contains(program, function);
    let has_nested_record_update =
        record_destructure::update::function_contains(program, function)?;
    Ok(if crate::byte_ops::requires_same_owner_set(function) {
        CLEANUP_PLAN_SCHEMA_V17
    } else if crate::string_ops::replacement::requires(function) {
        CLEANUP_PLAN_SCHEMA_V16
    } else if crate::hir::vec_loop_renewal::requires_in(program, function) {
        CLEANUP_PLAN_SCHEMA_V15
    } else if function_has_owner_admission(program, function)? {
        CLEANUP_PLAN_SCHEMA_V14
    } else if crate::iterator_ops::function_uses_owned_iterator(function)
        || crate::iterator_ops::function_uses_record_iterator_in(program.declarations(), function)
    {
        CLEANUP_PLAN_SCHEMA_V13
    } else if crate::hir::iterator_loop::function_requires_renewal(function)
        || crate::hir::iterator_loop::function_requires_record_renewal(program, function)
    {
        CLEANUP_PLAN_SCHEMA_V12
    } else if crate::hir::iterator_loop::function_contains(function) {
        CLEANUP_PLAN_SCHEMA_V11
    } else if inventory
        .flags
        .iter()
        .any(|flag| flag.lifecycle.as_str() == crate::cleanup::ITER_DROP_LIFECYCLE_ID)
    {
        CLEANUP_PLAN_SCHEMA_V10
    } else if has_nested_record_update {
        CLEANUP_PLAN_SCHEMA_V9
    } else if has_nested_record_destructure {
        CLEANUP_PLAN_SCHEMA_V8
    } else if has_nested_owned_bytes {
        CLEANUP_PLAN_SCHEMA_V7
    } else if inventory.schema == crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2
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
