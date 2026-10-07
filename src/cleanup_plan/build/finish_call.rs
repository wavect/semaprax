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
            || callee.as_str() == crate::stdin_stream_ops::EOF_ID
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

impl PlanBuilder<'_> {
    pub(super) fn finish_host_command(
        &mut self,
        expression: &ResolvedExpr,
        operation: crate::hir::ResolvedHostCommandOperation,
        flow: EvalResult,
        active_region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        self.push_transition(
            flow.block,
            CleanupTransition::CallCommit {
                call: expression.id.clone(),
                arguments: Vec::new(),
            },
        );
        let state = flow.state;
        let (block, mut state) = if crate::command_io_ops::failure(operation)
            == crate::command_io_ops::CommandIoFailure::Status
        {
            let source = StatusSourceId {
                expression: expression.id.clone(),
                lane: StatusLane::OperationFailure,
            };
            self.add_status_source(
                source.clone(),
                StatusProducer::PropagatedCall {
                    callee: DeclarationId::new(crate::command_io_ops::id(operation)),
                },
            )?;
            self.split_status(flow.block, state, active_region, source)?
        } else {
            (flow.block, state)
        };
        let destination = self.expression_slot(expression, active_region)?;
        if let Some(destination) = destination.clone() {
            self.initialize(block, expression.id.clone(), destination, &mut state)?;
        }
        Ok(EvalResult {
            block,
            state,
            owned_source: destination,
        })
    }
}

/// Authenticate the sole owned host-call profile before ordinary call staging.
/// The availability/loan proof remains the independent HIR validator's job.
pub(super) fn stream_next_signature(
    expression: &ResolvedExpr,
) -> Result<(&'static DeclarationId, Vec<crate::hir::ResolvedParam>), Diagnostic> {
    let ResolvedExprKind::HostCommandCall(call) = &expression.kind else {
        return Err(plan_error("streaming advancement has no host-call shape"));
    };
    if call.operation != crate::hir::ResolvedHostCommandOperation::StdinStreamNext
        || call.expression != expression.id
        || expression.ownership != OwnershipMode::Own
        || !crate::stdin_stream_ops::is_reader(&expression.ty)
        || !matches!(call.args.as_slice(), [argument] if argument.ownership == OwnershipMode::Own
            && crate::stdin_stream_ops::is_reader(&argument.ty)
            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
    {
        return Err(plan_error(
            "streaming advancement requires one exact owned reader",
        ));
    }
    static NEXT: std::sync::LazyLock<DeclarationId> =
        std::sync::LazyLock::new(|| DeclarationId::new(crate::stdin_stream_ops::NEXT_ID));
    Ok((
        &NEXT,
        crate::stdin_stream_ops::resolved_host_params(call.operation),
    ))
}
