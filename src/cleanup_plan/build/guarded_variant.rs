//! Boolean guard decisions and scoped temporary settlement for Copy payloads.
use super::*;
impl PlanBuilder<'_> {
    pub(super) fn begin_variant_guard(
        &mut self,
        guard: &ResolvedExpr,
        entry: BlockId,
        parent: CleanupRegionId,
    ) -> Result<(BlockId, Option<CleanupRegionId>), Diagnostic> {
        if crate::variant_guards::scalar_guard_shape(guard) {
            return Ok((entry, None));
        }
        let region = self.new_region(parent)?;
        let guard_entry = self.new_block(region)?;
        let edge = self.new_edge(entry, guard_entry, EdgeCondition::Always)?;
        self.terminate(entry, CleanupTerminator::Goto(edge))?;
        Ok((guard_entry, Some(region)))
    }
    pub(super) fn finish_variant_guard(
        &mut self,
        guard: &ResolvedExpr,
        result: EvalResult,
        baseline: &FlowState,
        rejected: BlockId,
        region: CleanupRegionId,
        guard_region: Option<CleanupRegionId>,
    ) -> Result<BlockId, Diagnostic> {
        let mut result = result;
        if let Some(guard_region) = guard_region {
            (result.block, result.state) =
                self.exit_scope(result.block, result.state, guard_region)?;
        }
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
