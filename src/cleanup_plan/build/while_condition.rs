//! Per-iteration condition lifetimes, before both Boolean outcomes.
use super::*;
impl PlanBuilder<'_> {
    /// Linearize one admitted iteration: a scalar condition (including exact
    /// named String length inspection) branches to the body or continuation.
    /// Body cleanup must restore entry ownership, so every physical iteration
    /// repeats the same checked lifecycle without a CleanupPlan back-edge.
    pub(super) fn lower_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let entry_state = state.clone();
        let condition_region = if crate::string_ops::conditions::needs_cleanup(condition) {
            Some(self.new_region(region)?)
        } else {
            None
        };
        let (condition_entry, active_region) = if let Some(child) = condition_region {
            let entry = self.new_block(child)?;
            let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
            self.terminate(block, CleanupTerminator::Goto(edge))?;
            (entry, child)
        } else {
            (block, region)
        };
        let mut evaluated_condition =
            self.lower_expr(condition, condition_entry, state, active_region)?;
        if evaluated_condition.owned_source.is_some() {
            return Err(plan_error(
                "while condition owns a value, which no admitted program can express",
            ));
        }
        if let Some(child) = condition_region {
            let (block, state) =
                self.exit_scope(evaluated_condition.block, evaluated_condition.state, child)?;
            evaluated_condition.block = block;
            evaluated_condition.state = state;
        }
        if evaluated_condition.state != entry_state {
            return Err(plan_error(
                "while condition changes surrounding owned cleanup state",
            ));
        }
        let body_entry = self.new_block(region)?;
        let after = self.new_block(region)?;
        let true_edge = self.new_edge(
            evaluated_condition.block,
            body_entry,
            EdgeCondition::BooleanResult(condition.id.clone(), true),
        )?;
        let false_edge = self.new_edge(
            evaluated_condition.block,
            after,
            EdgeCondition::BooleanResult(condition.id.clone(), false),
        )?;
        self.terminate(
            evaluated_condition.block,
            CleanupTerminator::Branch(vec![true_edge, false_edge]),
        )?;

        // The body is an ordinary checked block; lowering it once yields the
        // exact per-iteration ownership events of any iteration count.
        let evaluated_body =
            self.lower_expr(body, body_entry, evaluated_condition.state.clone(), region)?;
        if evaluated_body.state != evaluated_condition.state
            || evaluated_body.owned_source.is_some()
        {
            return Err(plan_error(
                "while loop body changes owned liveness, which the Bounded While-Loops v1 admission profile forbids",
            ));
        }
        let join_edge = self.new_edge(evaluated_body.block, after, EdgeCondition::Always)?;
        self.terminate(evaluated_body.block, CleanupTerminator::Goto(join_edge))?;
        Ok(EvalResult {
            block: after,
            state: entry_state,
            owned_source: None,
        })
    }
}
