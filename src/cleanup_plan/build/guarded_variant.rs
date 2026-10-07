//! Boolean guard decisions for cleanup-inert Copy variant payloads.
use super::*;
impl PlanBuilder<'_> {
    pub(super) fn finish_variant_guard(
        &mut self,
        guard: &ResolvedExpr,
        result: EvalResult,
        baseline: &FlowState,
        rejected: BlockId,
        region: CleanupRegionId,
    ) -> Result<BlockId, Diagnostic> {
        if !crate::variant_guards::guard_shape(guard)
            || guard.ty != ResolvedType::Bool
            || result.owned_source.is_some()
            || &result.state != baseline
        {
            return Err(plan_error("Copy variant guard changed owned cleanup state"));
        }
        let selected = self.new_block(region)?;
        let yes = self.new_edge(
            result.block,
            selected,
            EdgeCondition::BooleanResult(guard.id.clone(), true),
        )?;
        let no = self.new_edge(
            result.block,
            rejected,
            EdgeCondition::BooleanResult(guard.id.clone(), false),
        )?;
        self.terminate(result.block, CleanupTerminator::Branch(vec![yes, no]))?;
        Ok(selected)
    }
}
