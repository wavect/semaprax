//! Recursive cleanup-call oracle, including indirect scalar invocation.
use super::*;

impl PlanBuilder<'_> {
    #[cfg(test)]
    pub(super) fn lower_call(
        &mut self,
        expression: &ResolvedExpr,
        callee: &DeclarationId,
        instance: Option<&crate::hir::FunctionInstanceId>,
        args: &[ResolvedExpr],
        flow: (BlockId, FlowState, CleanupRegionId),
    ) -> Result<EvalResult, Diagnostic> {
        let (block, state, region) = flow;
        let type_arguments = bounded_vec::type_arguments(expression)?;
        let params = if matches!(expression.kind, ResolvedExprKind::Invoke { .. }) {
            crate::hir::function_value::invocation_params(expression)?
        } else if instance.is_none() {
            if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                crate::string_ops::resolved_params(op)
            } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                crate::str_ops::resolved_params(op)
            } else if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                crate::byte_ops::resolved_params(op)
            } else if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
                crate::host_io_ops::resolved_params(op)
            } else if let Some(op) = crate::command_io_ops::by_id(callee.as_str()) {
                crate::command_io_ops::resolved_params(op)
            } else if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
                let [element] = type_arguments else {
                    return Err(plan_error(
                        "cleanup bounded Vec call has incorrect type arity",
                    ));
                };
                crate::vec_ops::resolved_params(op, element)
            } else if let Some(op) = crate::box_ops::by_id(callee.as_str()) {
                let [element] = type_arguments else {
                    return Err(plan_error(
                        "cleanup bounded Box call has incorrect type arity",
                    ));
                };
                crate::box_ops::resolved_params(op, element)
            } else {
                let target = self
                    .program
                    .resolve_call_target(callee, instance)
                    .ok_or_else(|| plan_error(format!("unknown cleanup call target `{callee}`")))?;
                target.params.clone()
            }
        } else {
            let target = self
                .program
                .resolve_call_target(callee, instance)
                .ok_or_else(|| plan_error(format!("unknown cleanup call target `{callee}`")))?;
            target.params.clone()
        };
        if params.len() != args.len() {
            return Err(plan_error(format!(
                "cleanup call `{}` has inconsistent arity",
                expression.id
            )));
        }
        let mut current = block;
        let mut current_state = state;
        let mut commits = Vec::new();

        for (index, (argument, parameter)) in args.iter().zip(&params).enumerate() {
            let evaluated =
                self.lower_expr_recursive_reference(argument, current, current_state, region)?;
            current = evaluated.block;
            current_state = evaluated.state;
            if parameter.ownership == OwnershipMode::Own && self.needs_drop(&parameter.ty)? {
                let source = evaluated.owned_source.ok_or_else(|| {
                    plan_error(format!(
                        "owned call argument {} at `{}` has no cleanup source",
                        index, expression.id
                    ))
                })?;
                let epoch = self.call_argument_slot(expression, index, argument, region)?;
                self.transfer(
                    current,
                    argument.id.clone(),
                    source,
                    epoch.clone(),
                    &mut current_state,
                    true,
                )?;
                commits.push(CallArgumentTransfer {
                    parameter_index: u32::try_from(index)
                        .map_err(|_| plan_error("too many call arguments"))?,
                    source: epoch,
                });
            }
        }

        // This boundary lists every owned parameter epoch in signature
        // order; once emitted, even a nonzero call status cannot restore them.
        let (vec_op, defer_commit) = super::super::deferred_commit::call_behavior(expression);
        if !defer_commit {
            for commit in &commits {
                self.consume_place(&commit.source, &mut current_state, &expression.id)?;
            }
            self.push_transition(
                current,
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
            || crate::command_io_ops::by_id(callee.as_str()).is_some_and(|op| {
                crate::command_io_ops::failure(op)
                    == crate::command_io_ops::CommandIoFailure::Infallible
            })
        {
            let destination = self.expression_slot(expression, region)?;
            if let Some(destination) = destination.clone() {
                self.initialize_owned_result(current, expression, destination, &mut current_state)?;
            }
            return Ok(EvalResult {
                block: current,
                state: current_state,
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
            self.split_status(current, current_state, region, source)?;
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
        let destination = self.expression_slot(expression, region)?;
        if let Some(destination) = destination.clone() {
            // Caller result/out storage remains uninitialized until the
            // propagated status is known to be zero.
            self.initialize_owned_result(success, expression, destination, &mut success_state)?;
        }
        Ok(EvalResult {
            block: success,
            state: success_state,
            owned_source: destination,
        })
    }
}
