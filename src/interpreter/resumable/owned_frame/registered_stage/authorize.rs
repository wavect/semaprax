//! Staged checked authorization. No durable ACK, grant mint, or public result.
use super::*;
use crate::interpreter::OwnedVariantValue;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedAuthorizeV2;

pub(crate) struct StagedOwnedAuthorizeV2 {
    state: CompletedOwnedAgentStateV2,
    plan: CheckedOwnedAuthorizeV2,
    decision: Option<Value>,
    failure: Option<OwnedFrameFailure>,
    provisional: bool,
    settlement_started: bool,
}
impl StagedOwnedAuthorizeV2 {
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.failure.as_ref()
    }
}
pub(crate) struct OwnedAuthorizeRejectionV2 {
    pub(crate) state: CompletedOwnedAgentStateV2,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) fn stage_owned_authorize_v2(
    state: CompletedOwnedAgentStateV2,
    plan: &CheckedOwnedAuthorizeV2,
    budget: &mut OwnedFrameBudget,
) -> Result<StagedOwnedAuthorizeV2, OwnedAuthorizeRejectionV2> {
    if state.creator != std::process::id()
        || !state.plan.same_helper(plan.helper())
        || !state
            .root
            .as_ref()
            .is_some_and(|root| root_valid(&state.plan, root))
        || !channel_v2::valid_copy_carrier(
            &state.plan.program().declarations,
            &state
                .plan
                .function()
                .yields
                .as_ref()
                .expect("checked yields")
                .response_type,
            &state.proposal,
        )
    {
        return Err(OwnedAuthorizeRejectionV2 {
            state,
            diagnostic: rejected("authorize helper/root/Proposal/process mismatch"),
        });
    }
    let mut staged = StagedOwnedAuthorizeV2 {
        state,
        plan: plan.clone(),
        decision: None,
        failure: None,
        provisional: false,
        settlement_started: false,
    };
    if budget.cancelled {
        staged.failure = Some(OwnedFrameFailure::HostAbandoned);
        return Ok(staged);
    }
    let f = staged.plan.function();
    let ResumableChannelValue::Record { fields, .. } = &staged.state.proposal else {
        unreachable!()
    };
    let mut frame = Environment::from(Vec::new());
    for (param, scalar) in f.params[1..].iter().zip(fields) {
        frame.push((
            param.id.clone(),
            super::super::super::scalar_of(&param.ty, scalar).expect("checked scalar"),
        ));
    }
    let functions = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&functions),
        BTreeMap::new(),
        &staged.state.plan.program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    let root = staged.state.root.as_ref().expect("consumed state");
    let result = (|| {
        evaluator.semantic_charge()?;
        contracts(&mut evaluator, f, root, &mut frame, true)?;
        let ResolvedExprKind::Block { statements, tail } = &f.body.kind else {
            unreachable!()
        };
        evaluator.charge()?; // actual outer block
        let ResolvedStatement::Let { binding, value, .. } = &statements[0] else {
            unreachable!()
        };
        let array = evaluator.evaluate(value, &mut frame, 0)?;
        frame.push((binding.id.clone(), array));
        evaluator.charge()?; // actual If node
        let ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } = &tail.kind
        else {
            unreachable!()
        };
        let branch = match copy(&mut evaluator, condition, f, root, &mut frame)? {
            Value::Bool(true) => then_branch,
            Value::Bool(false) => else_branch,
            _ => return Err(Flow::Guard("checked authorize condition")),
        };
        let mut constructor = branch.as_ref();
        while let ResolvedExprKind::Block { statements, tail } = &constructor.kind {
            debug_assert!(statements.is_empty());
            evaluator.charge()?;
            constructor = tail;
        }
        evaluator.charge()?; // actual ConstructVariant node, before fields
        let ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } = &constructor.kind
        else {
            unreachable!()
        };
        for field in fields {
            let value = if field.value.ty == ResolvedType::Bytes {
                evaluator.evaluate(&field.value, &mut frame, 0)?
            } else {
                copy(&mut evaluator, &field.value, f, root, &mut frame)?
            };
            let decision = staged.decision.get_or_insert_with(|| {
                Value::Variant(Arc::new(OwnedVariantValue {
                    ty: f.return_type.clone(),
                    variant: variant.clone(),
                    case: case.clone(),
                    fields: BTreeMap::new(),
                }))
            });
            let Value::Variant(decision) = decision else {
                unreachable!()
            };
            Arc::get_mut(decision)
                .expect("private staged fields")
                .fields
                .insert(field.field.clone(), value);
        }
        // No further fallible expression between the final field transfer and
        // the compiler's provisional-result transfer. Conditional result flags
        // now replace the unsealed constructor's unconditional first-field flag.
        staged.provisional = true;
        contracts(&mut evaluator, f, root, &mut frame, false)
    })();
    let steps = evaluator.steps;
    drop(evaluator);
    drop(frame); // only Copy parameters/array; never an owning State alias
    budget.remaining -= steps;
    budget.consumed += steps;
    if let Err(flow) = result {
        staged.failure = Some(failure(flow));
    }
    Ok(staged)
}
fn contracts(
    e: &mut Evaluator<'_>,
    f: &hir::ResolvedFunction,
    root: &Value,
    frame: &mut Environment,
    requires: bool,
) -> Result<(), Flow> {
    let clauses = if requires { &f.requires } else { &f.ensures };
    for (index, clause) in clauses.iter().enumerate() {
        e.charge()?;
        match copy(e, clause, f, root, frame)? {
            Value::Bool(true) => {}
            Value::Bool(false) => {
                return Err(e.contract_failure(
                    f,
                    frame,
                    if requires {
                        crate::cleanup_plan::ContractPhase::Requires
                    } else {
                        crate::cleanup_plan::ContractPhase::Ensures
                    },
                    index,
                ))
            }
            _ => return Err(Flow::Guard("checked authorize contract")),
        }
    }
    Ok(())
}
fn copy(
    e: &mut Evaluator<'_>,
    expression: &ResolvedExpr,
    f: &hir::ResolvedFunction,
    root: &Value,
    frame: &mut Environment,
) -> Result<Value, Flow> {
    let mut expression = expression.clone();
    project(&mut expression, f, root)?;
    e.evaluate(&expression, frame, 0)
}
fn project(
    expression: &mut ResolvedExpr,
    f: &hir::ResolvedFunction,
    root: &Value,
) -> Result<(), Flow> {
    if matches!(&expression.kind, ResolvedExprKind::Place(_)) {
        return project_copy(expression, f, root);
    }
    match &mut expression.kind {
        ResolvedExprKind::Unary { value, .. } => project(value, f, root)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            project(left, f, root)?;
            project(right, f, root)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            project(condition, f, root)?;
            project(then_branch, f, root)?;
            project(else_branch, f, root)?;
        }
        ResolvedExprKind::Block { tail, .. } => project(tail, f, root)?,
        _ => {}
    }
    Ok(())
}

pub(crate) struct ReadyOwnedAuthorizeV2 {
    staged: StagedOwnedAuthorizeV2,
}
pub(crate) struct OwnedAuthorizeSettlementRejectionV2 {
    pub(crate) staged: StagedOwnedAuthorizeV2,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) enum OwnedAuthorizeSettledV2 {
    Ready(ReadyOwnedAuthorizeV2),
    Failed {
        failure: OwnedFrameFailure,
        decision_operations: Vec<FinalizeAction>,
        state_receipt: OwnedFrameReleaseReceipt,
        observations_succeeded: bool,
    },
}
fn exclusive_decision(value: &Value) -> bool {
    let Value::Variant(v) = value else {
        return false;
    };
    Arc::strong_count(v) == 1
        && v.fields.values().all(|v| match v {
            Value::Bytes(b) => Arc::strong_count(&b.bytes) == 1,
            _ => true,
        })
}
pub(crate) fn settle_owned_authorize_v2(
    mut staged: StagedOwnedAuthorizeV2,
    mut current: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<OwnedAuthorizeSettledV2, OwnedAuthorizeSettlementRejectionV2> {
    if staged.settlement_started
        || !current_in_creator(staged.state.creator, &mut current)
        || !staged
            .state
            .root
            .as_ref()
            .is_some_and(|r| root_valid(&staged.state.plan, r))
        || staged
            .decision
            .as_ref()
            .is_some_and(|v| !exclusive_decision(v))
    {
        return Err(OwnedAuthorizeSettlementRejectionV2 {
            staged,
            diagnostic: rejected("authorize owner/authority changed"),
        });
    }
    if staged.failure.is_none() {
        if !staged.provisional || staged.decision.is_none() {
            return Err(OwnedAuthorizeSettlementRejectionV2 {
                staged,
                diagnostic: rejected("authorize result not staged"),
            });
        }
        return Ok(OwnedAuthorizeSettledV2::Ready(ReadyOwnedAuthorizeV2 {
            staged,
        }));
    }
    let mut operations = Vec::new();
    let mut observations_succeeded = true;
    let creator = staged.state.creator;
    staged.settlement_started = true;
    if let Some(Value::Variant(value)) = staged.decision.as_mut() {
        let value = Arc::get_mut(value).expect("exclusive Decision");
        let actions = if staged.provisional {
            staged.plan.disposal()
        } else {
            staged.plan.partial_disposal()
        };
        let selected: Vec<_> = actions
            .iter()
            .filter(|a| a.active_case.as_ref().is_none_or(|c| c.case == value.case))
            .collect();
        let bytes: Vec<_> = value
            .fields
            .iter()
            .filter(|(_, v)| matches!(v, Value::Bytes(_)))
            .map(|(id, _)| id)
            .collect();
        if selected.len() != bytes.len()
            || selected.iter().any(|a| {
                a.source.projections.len() != 2
                    || a.source.projections[0] != value.case
                    || !bytes.contains(&&a.source.projections[1])
            })
        {
            return Err(OwnedAuthorizeSettlementRejectionV2 {
                staged,
                diagnostic: rejected("authorize partial field inventory changed"),
            });
        }
        drop(bytes);
        for action in selected {
            if !current_in_creator(creator, &mut current) {
                return Err(OwnedAuthorizeSettlementRejectionV2 {
                    staged,
                    diagnostic: rejected("authorize authority changed"),
                });
            }
            drop(
                value
                    .fields
                    .remove(&action.source.projections[1])
                    .expect("checked staged leaf"),
            );
            operations.push(action.clone());
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_err() {
                observations_succeeded = false;
            }
            if !current_in_creator(creator, &mut current) {
                return Err(OwnedAuthorizeSettlementRejectionV2 {
                    staged,
                    diagnostic: rejected("authorize authority changed"),
                });
            }
        }
    }
    drop(staged.decision.take());
    let actions = &staged.state.plan.liveness().result_disposal;
    let receipt = release_guarded(
        &mut staged.state.root,
        actions,
        || current_in_creator(creator, &mut current),
        |a| {
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(a))).is_err() {
                observations_succeeded = false;
            }
        },
    );
    match receipt {
        Ok(state_receipt) => Ok(OwnedAuthorizeSettledV2::Failed {
            failure: staged.failure.take().expect("sticky failure"),
            decision_operations: operations,
            state_receipt,
            observations_succeeded,
        }),
        Err(diagnostic) => Err(OwnedAuthorizeSettlementRejectionV2 { staged, diagnostic }),
    }
}

#[cfg(test)]
mod tests;
