//! Cleanup-plan leaf and synthetic-call handling for compiler-owned bounded Box.

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{ExpressionId, ResolvedParam, ResolvedType};

use super::{plan_error, CleanupPlace, LeafMetadata, PlanBuilder, StorageId};

impl PlanBuilder<'_> {
    pub(super) fn bounded_box_shape(
        &mut self,
        ty: &ResolvedType,
        storage: &StorageId,
        projections: &[crate::hir::DeclarationId],
    ) -> Result<Option<FieldLivenessShape>, Diagnostic> {
        if !crate::cleanup::is_owned_bounded_box_type(ty) {
            return Ok(None);
        }
        let flag = LivenessFlagId(self.next_flag);
        self.next_flag = self
            .next_flag
            .checked_add(1)
            .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
        let lifecycle = crate::hir::DeclarationId::new(crate::cleanup::BOX_DROP_LIFECYCLE_ID);
        self.leaves.insert(
            flag,
            LeafMetadata {
                place: CleanupPlace {
                    storage: storage.clone(),
                    projections: projections.to_vec(),
                },
                lifecycle: lifecycle.clone(),
            },
        );
        Ok(Some(FieldLivenessShape::Leaf { flag, lifecycle }))
    }
}

pub(super) fn resolved_params(
    op: crate::box_ops::BoxOp,
    has_instance: bool,
    argument_count: usize,
    type_arguments: &[ResolvedType],
    expression: &ExpressionId,
) -> Result<Vec<ResolvedParam>, Diagnostic> {
    if has_instance
        || argument_count != 1
        || !matches!(type_arguments, [element] if crate::box_ops::resolved_element_is_admitted(element))
    {
        return Err(plan_error(format!(
            "cleanup bounded Box call `{expression}` has inconsistent shape"
        )));
    }
    Ok(crate::box_ops::resolved_params(op, &type_arguments[0]))
}
