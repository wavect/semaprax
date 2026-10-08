//! Exact whole String replacement release, before publication of its new carrier.
use super::*;
impl Emitter<'_> {
    pub(super) fn is_string_replacement(
        &self,
        transition: &crate::cleanup_plan::CleanupTransition,
    ) -> bool {
        let crate::cleanup_plan::CleanupTransition::Renew {
            at, destination, ..
        } = transition
        else {
            return false;
        };
        matches!(
            self.cleanup_plan.schema,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V16
                | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17
        ) && crate::string_ops::replacement::binding(self.function, at).is_some_and(|binding| {
            destination.projections.is_empty()
                && destination.storage == crate::cleanup_plan::StorageId::Value(binding.id.clone())
        })
    }
    pub(super) fn release_replaced_string(
        &mut self,
        transition: &crate::cleanup_plan::CleanupTransition,
    ) -> Result<(), Diagnostic> {
        if !self.is_string_replacement(transition) {
            return Ok(());
        }
        let crate::cleanup_plan::CleanupTransition::Renew { destination, .. } = transition else {
            unreachable!()
        };
        let slot = self
            .cleanup_plan
            .slots
            .iter()
            .find(|slot| slot.storage == destination.storage)
            .ok_or_else(|| error("String replacement has no canonical slot"))?;
        let crate::cleanup::FieldLivenessShape::Leaf {
            flag, lifecycle, ..
        } = &slot.field_liveness_shape
        else {
            return Err(error("String replacement is not one owned leaf"));
        };
        if slot.ty != ResolvedType::String
            || lifecycle.as_str() != crate::cleanup::STRING_DROP_LIFECYCLE_ID
        {
            return Err(error("String replacement leaf type disagrees"));
        }
        let action = crate::cleanup_plan::FinalizeAction {
            source: destination.clone(),
            lifecycle_id: lifecycle.clone(),
            guard_flag: *flag,
            active_case: None,
        };
        self.emit_cleanup_actions(&[action])
    }
}
