//! Trace the same guarded release that the exact v16 String renewal requires.
use super::*;
impl Executor<'_> {
    pub(super) fn release_string_replacement(
        &mut self,
        at: &crate::hir::ExpressionId,
        destination: &CleanupPlace,
    ) -> Result<(), CleanupExecutionError> {
        let Some(binding) = crate::string_ops::replacement::binding(self.function, at) else {
            return Ok(());
        };
        if *destination != CleanupPlace::whole(StorageId::Value(binding.id.clone())) {
            return Ok(());
        }
        let flags = self.flags_under(destination)?;
        if flags.len() != 1 {
            return Err(invariant("String replacement needs one owned leaf"));
        }
        let leaf = &self.leaves[&flags[0]];
        let action = super::super::FinalizeAction {
            source: destination.clone(),
            lifecycle_id: leaf.lifecycle.clone(),
            guard_flag: flags[0],
        };
        self.execute_finalizer_actions(&[action])
    }
}
