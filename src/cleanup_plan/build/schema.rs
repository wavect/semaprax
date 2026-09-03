use crate::cleanup::{CleanupInventory, FieldLivenessShape, CLEANUP_INVENTORY_SCHEMA_V2};
use crate::diagnostic::Diagnostic;

use super::super::{
    CLEANUP_PLAN_SCHEMA_V2, CLEANUP_PLAN_SCHEMA_V5, CLEANUP_PLAN_SCHEMA_V6, CLEANUP_PLAN_SCHEMA_V7,
    CLEANUP_PLAN_SCHEMA_V8, CLEANUP_PLAN_SCHEMA_V9,
};

pub(super) fn initial(inventory: &CleanupInventory) -> Result<&'static str, Diagnostic> {
    if inventory.slots.iter().try_fold(false, |nested, slot| {
        crate::cleanup::cleanup_shape_profile(&slot.shape)
            .map(|profile| nested || profile.has_nested_owned_bytes)
    })? {
        Ok(CLEANUP_PLAN_SCHEMA_V7)
    } else if inventory.schema == CLEANUP_INVENTORY_SCHEMA_V2 {
        Ok(CLEANUP_PLAN_SCHEMA_V6)
    } else {
        Ok(CLEANUP_PLAN_SCHEMA_V2)
    }
}

pub(super) fn shape_is_nested(shape: &FieldLivenessShape) -> Result<bool, Diagnostic> {
    Ok(crate::cleanup::cleanup_shape_profile(shape)?.has_nested_owned_bytes)
}

pub(super) fn includes_v5(schema: &str) -> bool {
    matches!(
        schema,
        CLEANUP_PLAN_SCHEMA_V5
            | CLEANUP_PLAN_SCHEMA_V6
            | CLEANUP_PLAN_SCHEMA_V7
            | CLEANUP_PLAN_SCHEMA_V8
            | CLEANUP_PLAN_SCHEMA_V9
    )
}

pub(super) fn includes_v6(schema: &str) -> bool {
    matches!(
        schema,
        CLEANUP_PLAN_SCHEMA_V6
            | CLEANUP_PLAN_SCHEMA_V7
            | CLEANUP_PLAN_SCHEMA_V8
            | CLEANUP_PLAN_SCHEMA_V9
    )
}

pub(super) fn promote_v6(schema: &mut &'static str) {
    if !matches!(
        *schema,
        CLEANUP_PLAN_SCHEMA_V7 | CLEANUP_PLAN_SCHEMA_V8 | CLEANUP_PLAN_SCHEMA_V9
    ) {
        *schema = CLEANUP_PLAN_SCHEMA_V6;
    }
}

pub(super) fn promote_v8(schema: &mut &'static str) {
    if *schema != CLEANUP_PLAN_SCHEMA_V9 {
        *schema = CLEANUP_PLAN_SCHEMA_V8;
    }
}

pub(super) fn promote_v9(schema: &mut &'static str) {
    *schema = CLEANUP_PLAN_SCHEMA_V9;
}

pub(super) fn promote_v7(schema: &mut &'static str) {
    if !matches!(*schema, CLEANUP_PLAN_SCHEMA_V8 | CLEANUP_PLAN_SCHEMA_V9) {
        *schema = CLEANUP_PLAN_SCHEMA_V7;
    }
}

pub(super) fn promote_match(schema: &mut &'static str, arms: &[crate::hir::ResolvedMatchArm]) {
    if arms.iter().any(|arm| {
        matches!(
            &arm.pattern,
            crate::hir::ResolvedMatchPattern::Record { fields, .. }
                if super::record_destructure::contains_nested(fields)
        )
    }) {
        promote_v8(schema);
    } else if arms.iter().any(|arm| {
        matches!(
            arm.pattern,
            crate::hir::ResolvedMatchPattern::Variant { .. }
        )
    }) {
        promote_v6(schema);
    } else if !includes_v6(schema) {
        *schema = CLEANUP_PLAN_SCHEMA_V5;
    }
}
