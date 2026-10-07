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
    Ok(if function_has_owner_admission(program, function)? {
        CLEANUP_PLAN_SCHEMA_V14
    } else if crate::iterator_ops::function_uses_owned_iterator(function)
        || crate::iterator_ops::function_uses_record_iterator_in(&program.declarations, function)
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

fn function_has_owner_admission(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<bool, Diagnostic> {
    for root in function
        .requires
        .iter()
        .chain(&function.ensures)
        .chain(std::iter::once(&function.body))
    {
        let mut stack = [None; 514];
        stack[0] = Some((root, 0usize));
        let mut len = 1usize;
        while len != 0 {
            len -= 1;
            let (expression, next) = stack[len].take().expect("owner admission frame retained");
            if next == 0 && super::super::owner_admission::required(program, expression) {
                return Ok(true);
            }
            if let Some(child) = replay_expression_child(expression, next) {
                if len + 2 > stack.len() {
                    return Err(replay_error(function, "owner admission depth exceeds 512"));
                }
                stack[len] = Some((expression, next + 1));
                stack[len + 1] = Some((child, 0));
                len += 2;
            }
        }
    }
    Ok(false)
}

fn expression_has_explicit_variant_match(expression: &ResolvedExpr) -> bool {
    expression_has_kind(expression, |kind| {
        matches!(
            kind,
            ResolvedExprKind::Match {
                mode: crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow,
                arms,
                ..
            }
                if arms.iter().any(|arm| matches!(arm.pattern, ResolvedMatchPattern::Variant { .. }))
        )
    })
}
