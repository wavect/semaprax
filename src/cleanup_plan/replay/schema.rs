//! Semantic cleanup classification, independent of plan construction.
//! Attached inventories and plans carry no authority for this selection.

use super::*;

pub(crate) fn selected_schema(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<&'static str, Diagnostic> {
    let inventory = crate::cleanup::build_inventory(program, function)?;
    let has_nested_owned_bytes = inventory.slots.iter().try_fold(false, |nested, slot| {
        crate::cleanup::cleanup_shape_profile(&slot.shape)
            .map(|profile| nested || profile.has_nested_owned_bytes)
    })?;
    let has_nested_record_destructure = record_destructure::function_contains(function);
    let has_nested_record_update =
        record_destructure::update::function_contains(program, function)?;
    Ok(
        if inventory
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
        },
    )
}
