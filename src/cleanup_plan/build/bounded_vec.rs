//! Cleanup-plan leaf and synthetic-call handling for compiler-owned bounded Vec.

use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId, ResolvedParam, ResolvedType};
#[cfg(test)]
use crate::hir::{ResolvedExpr, ResolvedExprKind};

use super::{plan_error, CleanupPlace, LeafMetadata, PlanBuilder, StorageId};

impl PlanBuilder<'_> {
    pub(super) fn bounded_vec_shape(
        &mut self,
        ty: &ResolvedType,
        storage: &StorageId,
        projections: &[DeclarationId],
    ) -> Result<Option<FieldLivenessShape>, Diagnostic> {
        if !crate::cleanup::is_owned_bounded_vec_type(ty) {
            return Ok(None);
        }
        let flag = LivenessFlagId(self.next_flag);
        self.next_flag = self
            .next_flag
            .checked_add(1)
            .ok_or_else(|| plan_error("too many cleanup liveness flags"))?;
        let lifecycle = DeclarationId::new(crate::cleanup::VEC_DROP_LIFECYCLE_ID);
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
    op: crate::vec_ops::VecOp,
    has_instance: bool,
    argument_count: usize,
    type_arguments: &[ResolvedType],
    expression: &ExpressionId,
) -> Result<Vec<ResolvedParam>, Diagnostic> {
    if has_instance
        || argument_count != op.arity()
        || type_arguments.len() != 1
        || !crate::vec_ops::resolved_element_is_admitted(&type_arguments[0])
    {
        return Err(plan_error(format!(
            "cleanup bounded Vec call `{expression}` has inconsistent shape"
        )));
    }
    Ok(crate::vec_ops::resolved_params(op, &type_arguments[0]))
}

#[cfg(test)]
pub(super) fn type_arguments(expression: &ResolvedExpr) -> Result<&[ResolvedType], Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Call { type_arguments, .. } => Ok(type_arguments),
        _ => Err(plan_error("cleanup call expression has inconsistent shape")),
    }
}

/// Whether some cleanup region already owns `storage`.
///
/// A storage belongs to the region that first introduced it. The loop-carried
/// profile assigns an enclosing scope's binding from inside a bounded `while`
/// body — `values = vec_push<T>(values, value)` — and the body is its own
/// cleanup region. Re-homing the binding slot into that inner region would
/// make the body's scope exit finalize a vector the enclosing scope still
/// owns: the next iteration would read destroyed storage, and the plan would
/// instead fail closed because one linearized body pass no longer preserves
/// owned liveness.
pub(super) fn storage_is_placed(
    regions: &[crate::cleanup_plan::CleanupRegion],
    storage: &StorageId,
) -> bool {
    regions.iter().any(|region| region.slots.contains(storage))
}
