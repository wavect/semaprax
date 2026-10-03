use super::*;

impl PlanBuilder<'_> {
    pub(super) fn finish_call(
        &mut self,
        expression: &ResolvedExpr,
        callee: &DeclarationId,
        flow: EvalResult,
        commits: Vec<CallArgumentTransfer>,
        active_region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let (block, mut state) =
            self.admit_owners(expression, callee, flow.block, flow.state, active_region)?;
        let (vec_op, defer_commit) = super::super::deferred_commit::call_behavior(expression);
        if !defer_commit {
            for commit in &commits {
                self.consume_place(&commit.source, &mut state, &expression.id)?;
            }
            self.push_transition(
                block,
                CleanupTransition::CallCommit {
                    call: expression.id.clone(),
                    arguments: commits.clone(),
                },
            );
        }
        if super::super::deferred_commit::is_total_byte_operation(callee)
            || crate::host_io_ops::by_id(callee.as_str()).is_some()
            || super::super::deferred_commit::is_infallible_vec_operation(vec_op)
            || super::super::deferred_commit::is_infallible_box_operation(callee)
        {
            let destination = self.expression_slot(expression, active_region)?;
            if let Some(destination) = destination.clone() {
                self.initialize_owned_result(block, expression, destination, &mut state)?;
            }
            return Ok(EvalResult {
                block,
                state,
                owned_source: destination,
            });
        }
        let source = StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::OperationFailure,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::PropagatedCall {
                callee: callee.clone(),
            },
        )?;
        let (success, mut success_state) =
            self.split_status(block, state, active_region, source)?;
        if defer_commit {
            for commit in &commits {
                self.consume_place(&commit.source, &mut success_state, &expression.id)?;
            }
            self.push_transition(
                success,
                CleanupTransition::CallCommit {
                    call: expression.id.clone(),
                    arguments: commits,
                },
            );
        }
        let destination = self.expression_slot(expression, active_region)?;
        if let Some(destination) = destination.clone() {
            self.initialize_owned_result(success, expression, destination, &mut success_state)?;
        }
        Ok(EvalResult {
            block: success,
            state: success_state,
            owned_source: destination,
        })
    }

    pub(super) fn admit_owners(
        &mut self,
        expression: &ResolvedExpr,
        callee: &DeclarationId,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        if !super::super::owner_admission::required(self.program, expression) {
            return Ok((block, state));
        }
        let source = StatusSourceId {
            expression: expression.id.clone(),
            lane: StatusLane::OwnerAdmission,
        };
        self.add_status_source(
            source.clone(),
            StatusProducer::PropagatedCall {
                callee: callee.clone(),
            },
        )?;
        self.split_status(block, state, region, source)
    }
}
