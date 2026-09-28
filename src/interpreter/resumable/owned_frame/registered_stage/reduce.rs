//! Actual consuming source reducer. The effect-completed input constructor is
//! supplied only by the ACK-gated physical effect holder, retaining its lease.
use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::resumable::clone_scalar;
use crate::interpreter::OwnedVariantValue;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedReduceV2;

pub(crate) struct PreparedOwnedReduceV2 {
    plan: CheckedOwnedReduceV2,
    state: Option<Value>,
    outcome: Option<Value>,
    proposal: ResumableChannelValue,
    allocations: OwnedAllocationProvenanceV2,
    creator: u32,
}
pub(crate) struct StagedOwnedReduceV2 {
    plan: CheckedOwnedReduceV2,
    state: Option<Value>,
    outcome: Option<Value>,
    step: Option<Value>,
    case: Option<usize>,
    transferred: usize,
    provisional: bool,
    failure: Option<OwnedFrameFailure>,
    allocations: OwnedAllocationProvenanceV2,
    creator: u32,
    settlement_started: bool,
}
pub(crate) struct OwnedReduceRejectionV2 {
    pub(crate) input: PreparedOwnedReduceV2,
    pub(crate) diagnostic: Diagnostic,
}
impl StagedOwnedReduceV2 {
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.failure.as_ref()
    }
}

pub(crate) fn stage_owned_reduce_v2(
    mut input: PreparedOwnedReduceV2,
    budget: &mut OwnedFrameBudget,
) -> Result<StagedOwnedReduceV2, OwnedReduceRejectionV2> {
    let valid = input.creator == std::process::id()
        && input
            .state
            .as_ref()
            .is_some_and(|r| root_valid(input.plan.helper(), r))
        && input
            .outcome
            .as_ref()
            .is_some_and(|r| outcome_valid(&input.plan, r))
        && input.allocations.validate(&input_roots(&input))
        && channel_v2::value_of_copy(
            &input.plan.helper().program().declarations,
            &input
                .plan
                .helper()
                .function()
                .yields
                .as_ref()
                .expect("helper")
                .response_type,
            &input.proposal,
        )
        .is_some();
    if !valid {
        return Err(OwnedReduceRejectionV2 {
            input,
            diagnostic: rejected("reducer checked input/process differs"),
        });
    }
    let mut staged = StagedOwnedReduceV2 {
        plan: input.plan,
        state: input.state.take(),
        outcome: input.outcome.take(),
        step: None,
        case: None,
        transferred: 0,
        provisional: false,
        failure: None,
        allocations: input.allocations,
        creator: input.creator,
        settlement_started: false,
    };
    if budget.cancelled {
        staged.failure = Some(OwnedFrameFailure::HostAbandoned);
        return Ok(staged);
    }
    let functions = BTreeMap::new();
    let proof = staged.plan.clone();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&functions),
        BTreeMap::new(),
        &proof.helper().program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    evaluator.next_byte_allocation = staged
        .allocations
        .seed(&roots(&staged))
        .expect("checked provenance");
    let mut env = Environment::from(Vec::new());
    let copy = channel_v2::value_of_copy(
        &staged.plan.helper().program().declarations,
        &staged
            .plan
            .helper()
            .function()
            .yields
            .as_ref()
            .unwrap()
            .response_type,
        &input.proposal,
    )
    .expect("checked Proposal");
    let Value::Record(proposal) = copy else {
        unreachable!()
    };
    let fields = staged
        .plan
        .helper()
        .program()
        .declarations
        .record_fields(
            staged
                .plan
                .helper()
                .function()
                .yields
                .as_ref()
                .unwrap()
                .response_type
                .nominal_id()
                .unwrap(),
        )
        .unwrap();
    for (p, field) in staged.plan.function().params[1..staged.plan.function().params.len() - 1]
        .iter()
        .zip(fields)
    {
        env.push((
            p.id.clone(),
            clone_scalar(&proposal.fields[&field.id]).expect("Copy Proposal"),
        ));
    }
    drop(proposal);
    let body = staged.plan.function().body.clone();
    let result = (|| {
        evaluator.semantic_charge()?;
        contracts(&mut evaluator, &mut env, &staged, true)?;
        evaluate(&mut evaluator, &mut env, &mut staged, &body)?;
        staged.provisional = true;
        contracts(&mut evaluator, &mut env, &staged, false)
    })();
    let steps = evaluator.steps;
    drop(env);
    drop(evaluator);
    budget.remaining -= steps;
    budget.consumed += steps;
    if let Err(flow) = result {
        staged.failure = Some(failure(flow));
    }
    Ok(staged)
}
fn input_roots(input: &PreparedOwnedReduceV2) -> Vec<&Value> {
    [&input.state, &input.outcome]
        .into_iter()
        .filter_map(Option::as_ref)
        .collect()
}
fn roots(staged: &StagedOwnedReduceV2) -> Vec<&Value> {
    [&staged.state, &staged.outcome, &staged.step]
        .into_iter()
        .filter_map(Option::as_ref)
        .collect()
}
pub(super) fn outcome_valid(plan: &CheckedOwnedReduceV2, root: &Value) -> bool {
    if !exclusive(root) {
        return false;
    }
    let Value::Record(record) = root else {
        return false;
    };
    let ty = &plan.function().params.last().unwrap().ty;
    let Some(id) = ty.nominal_id() else {
        return false;
    };
    let Some(fields) = plan.helper().program().declarations.record_fields(id) else {
        return false;
    };
    record.record == *id
        && fields.len() == record.fields.len()
        && fields.iter().all(|f| {
            record.fields.get(&f.id).is_some_and(|v| match v {
                Value::Bytes(b) => {
                    f.ty == ResolvedType::Bytes && b.bytes.len() <= 1024 && b.allocation != 0
                }
                v => super::super::super::argument_of(v).is_some_and(|a| scalar_valid(&f.ty, &a)),
            })
        })
}
fn evaluate(
    evaluator: &mut Evaluator<'_>,
    env: &mut Environment,
    staged: &mut StagedOwnedReduceV2,
    expr: &ResolvedExpr,
) -> Result<(), Flow> {
    evaluator.charge()?;
    match &expr.kind {
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
            evaluate(evaluator, env, staged, tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let branch = match scalar(evaluator, env, staged, condition)? {
                Value::Bool(true) => then_branch,
                Value::Bool(false) => else_branch,
                _ => return Err(Flow::Guard("checked reducer condition")),
            };
            evaluate(evaluator, env, staged, branch)
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => {
            let index = staged
                .plan
                .transfers()
                .cases
                .iter()
                .position(|p| p.constructor == expr.id && p.case == *case)
                .ok_or(Flow::Guard("checked reducer constructor"))?;
            staged.case = Some(index);
            staged.step = Some(Value::Variant(Arc::new(OwnedVariantValue {
                ty: expr.ty.clone(),
                variant: variant.clone(),
                case: case.clone(),
                fields: BTreeMap::new(),
            })));
            for field in fields {
                let value = if field.value.ty == ResolvedType::Bytes {
                    evaluator.charge()?; // owning Place, BEFORE its actual take
                    let transfer = &staged.plan.transfers().cases[index].fields[staged.transferred];
                    if transfer.at != field.value.id {
                        return Err(Flow::Guard("checked transfer order"));
                    }
                    let root = if transfer.source.storage
                        == crate::cleanup_plan::StorageId::Value(
                            staged.plan.function().params[0].id.clone(),
                        ) {
                        &mut staged.state
                    } else {
                        &mut staged.outcome
                    };
                    let Value::Record(record) = root.as_mut().expect("consumed input") else {
                        unreachable!()
                    };
                    Arc::get_mut(record)
                        .expect("exclusive input")
                        .fields
                        .remove(&transfer.source.projections[0])
                        .ok_or(Flow::Guard("missing transfer leaf"))?
                } else {
                    scalar(evaluator, env, staged, &field.value)?
                };
                let Value::Variant(record) = staged.step.as_mut().unwrap() else {
                    unreachable!()
                };
                Arc::get_mut(record)
                    .expect("exclusive constructor")
                    .fields
                    .insert(field.field.clone(), value);
                if field.value.ty == ResolvedType::Bytes {
                    staged.transferred += 1;
                }
            }
            Ok(())
        }
        _ => Err(Flow::Guard("checked reducer source body")),
    }
}
fn scalar(
    evaluator: &mut Evaluator<'_>,
    env: &mut Environment,
    staged: &StagedOwnedReduceV2,
    expression: &ResolvedExpr,
) -> Result<Value, Flow> {
    let mut projected = expression.clone();
    project(&mut projected, staged)?;
    evaluator.evaluate(&projected, env, 0)
}
fn project(expr: &mut ResolvedExpr, staged: &StagedOwnedReduceV2) -> Result<(), Flow> {
    match &mut expr.kind {
        ResolvedExprKind::Place(place) if !place.projections.is_empty() => {
            let f = staged.plan.function();
            let root = if place.root == f.params[0].id {
                staged.state.as_ref()
            } else if place.root == f.params.last().unwrap().id {
                staged.outcome.as_ref()
            } else {
                None
            }
            .ok_or(Flow::Guard("foreign reducer Copy root"))?;
            let [hir::PlaceProjection::Field(field)] = place.projections.as_slice() else {
                return Err(Flow::Guard("reducer projection depth"));
            };
            let Value::Record(root) = root else {
                unreachable!()
            };
            expr.kind = scalar_expression(
                root.fields
                    .get(field)
                    .ok_or(Flow::Guard("missing reducer Copy field"))?,
            )?;
        }
        ResolvedExprKind::Place(place) if place.root == staged.plan.function().result_id => {
            return Err(Flow::Guard("owned Step contract read outside profile"))
        }
        ResolvedExprKind::Unary { value, .. } => project(value, staged)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            project(left, staged)?;
            project(right, staged)?;
        }
        _ => {}
    }
    Ok(())
}
fn scalar_expression(value: &Value) -> Result<ResolvedExprKind, Flow> {
    Ok(match value {
        Value::Int(v) => ResolvedExprKind::Int(*v),
        Value::Int32(v) => ResolvedExprKind::Int32(*v),
        Value::Uint8(v) => ResolvedExprKind::Uint8(*v),
        Value::Usize(v) => ResolvedExprKind::Usize(*v),
        Value::Char(v) => ResolvedExprKind::Char(*v),
        Value::Float32(v) => ResolvedExprKind::Float32(v.to_bits()),
        Value::Float64(v) => ResolvedExprKind::Float64(v.to_bits()),
        Value::Bool(v) => ResolvedExprKind::Bool(*v),
        _ => return Err(Flow::Guard("owning scalar read")),
    })
}
fn contracts(
    evaluator: &mut Evaluator<'_>,
    env: &mut Environment,
    staged: &StagedOwnedReduceV2,
    requires: bool,
) -> Result<(), Flow> {
    let f = staged.plan.function();
    let clauses = if requires { &f.requires } else { &f.ensures };
    for (index, clause) in clauses.iter().enumerate() {
        evaluator.charge()?;
        match scalar(evaluator, env, staged, clause)? {
            Value::Bool(true) => {}
            Value::Bool(false) => {
                return Err(evaluator.contract_failure(
                    f,
                    env,
                    if requires {
                        crate::cleanup_plan::ContractPhase::Requires
                    } else {
                        crate::cleanup_plan::ContractPhase::Ensures
                    },
                    index,
                ))
            }
            _ => return Err(Flow::Guard("checked reducer contract")),
        }
    }
    Ok(())
}

mod owned_execute;
pub(crate) use owned_execute::{
    prepare_executed_owned_reduce_v2, stage_executed_owned_reduce_v2,
    ExecutedOwnedReducePreparationRejectionV2, ExecutedOwnedReduceRejectionV2,
    PreparedExecutedOwnedReduceV2, StagedExecutedOwnedReduceV2,
};

mod physical_step;
pub(crate) use physical_step::{
    consume_executed_owned_step_v2, settle_executed_owned_reduce_v2,
    CommittedExecutedOwnedReduceCleanupV2, CommittedExecutedOwnedStepTransferV2,
    ExecutedOwnedReduceSettledV2, HeldExecutedOwnedStepV2, ReadyExecutedOwnedStepV2,
};

mod step;
pub(crate) use step::{
    consume_owned_step_v2, settle_owned_reduce_v2, OwnedReduceSettledV2,
    OwnedReduceSettlementRejectionV2, OwnedReducedReportV2, OwnedStepTransferV2, ReadyOwnedStepV2,
};
#[cfg(test)]
mod tests;

mod live_stage;
pub(crate) use live_stage::{
    evaluate_live_executed_owned_reduce_v2, CheckedLiveOwnedReduceStageFactsV8,
    LiveReduceEvaluationFailureV8,
};

pub(crate) use physical_step::{
    consume_live_owned_step_v8, settle_live_owned_reduce_v8, LiveOwnedReduceCleanupFailureV8,
    LiveOwnedStepTransferFailureV8, OwnedReduceCleanupOriginV8,
};

pub(crate) use physical_step::{
    observe_live_continued_state_v8, ContinuedOwnedObserveV2, LiveContinuedObserveFailureV8,
};

#[cfg(test)]
pub(crate) use physical_step::{test_continue_observe_entries_v8, test_continue_observe_oracle_v8};

pub(crate) use physical_step::checked_continued_observe_facts_v8;
