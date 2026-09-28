//! Consuming actual source initializer. Partial moves are pending owners, never
//! rolled back or harvested through a public/inert carrier.
use super::*;
use crate::interpreter::OwnedRecordValue;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedInitializeV2;

pub(crate) struct OwnedTaskArgumentV2 {
    plan: CheckedOwnedInitializeV2,
    root: Option<Value>,
    creator: u32,
    allocations: Option<OwnedAllocationProvenanceV2>,
}
#[cfg(test)]
impl OwnedTaskArgumentV2 {
    pub(super) fn test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        super::super::snapshot::weak_leaves(self.root.as_ref().expect("actual admitted Task"))
    }
}
impl Drop for OwnedTaskArgumentV2 {
    fn drop(&mut self) {
        if self.creator == std::process::id() && self.root.is_some() {
            let _ = release(&mut self.root, &self.plan.transfers().failure_by_prefix[0]);
        } else {
            drop(self.root.take());
        }
    }
}
pub(crate) fn admit_owned_task_input_v2(
    plan: &CheckedOwnedInitializeV2,
    input: OwnedFrameInput,
) -> Result<OwnedTaskArgumentV2, OwnedFrameInputRejection> {
    if let Err(diagnostic) = validate_owned_task_input_v2(plan, &input) {
        return Err(OwnedFrameInputRejection { input, diagnostic });
    }
    let root = stage_root_for(
        &plan.helper().program().declarations,
        &plan.function().params[0].ty,
        input,
    );
    let allocations =
        OwnedAllocationProvenanceV2::fresh(&root).expect("checked fresh Task staging");
    Ok(OwnedTaskArgumentV2 {
        plan: plan.clone(),
        root: Some(root),
        creator: std::process::id(),
        allocations: Some(allocations),
    })
}
pub(crate) fn validate_owned_task_input_v2(
    plan: &CheckedOwnedInitializeV2,
    input: &OwnedFrameInput,
) -> Result<(), Diagnostic> {
    validate_fields_for(
        &plan.helper().program().declarations,
        &plan.function().params[0].ty,
        &input.declaration,
        input.fields.len(),
        input.fields.iter().map(|f| {
            (
                &f.identity,
                match &f.value {
                    OwnedFrameInputValue::Bytes(v) => InputRef::Bytes(v),
                    OwnedFrameInputValue::Scalar(v) => InputRef::Scalar(v),
                },
            )
        }),
    )
}
pub(crate) struct StagedOwnedInitializeV2 {
    plan: CheckedOwnedInitializeV2,
    task: Option<Value>,
    state: Option<Value>,
    transferred: usize,
    provisional: bool,
    failure: Option<OwnedFrameFailure>,
    creator: u32,
    settlement_started: bool,
    allocations: OwnedAllocationProvenanceV2,
}
impl StagedOwnedInitializeV2 {
    pub(super) fn abandon_after_live_guard(&mut self) {
        if self.failure.is_none() {
            self.failure = Some(OwnedFrameFailure::HostAbandoned);
        }
    }
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.failure.as_ref()
    }
}
pub(crate) struct OwnedInitializeRejectionV2 {
    pub(crate) argument: OwnedTaskArgumentV2,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) fn stage_owned_initialize_v2(
    mut argument: OwnedTaskArgumentV2,
    plan: &CheckedOwnedInitializeV2,
    budget: &mut OwnedFrameBudget,
) -> Result<StagedOwnedInitializeV2, OwnedInitializeRejectionV2> {
    if argument.creator != std::process::id()
        || !argument.root.as_ref().is_some_and(|r| {
            argument
                .allocations
                .as_ref()
                .is_some_and(|p| p.validate(&[r]))
        })
        || !argument.plan.helper().same_helper(plan.helper())
        || argument.plan.function().id != plan.function().id
        || !argument.root.as_ref().is_some_and(|r| task_valid(plan, r))
    {
        return Err(OwnedInitializeRejectionV2 {
            argument,
            diagnostic: rejected("initializer input/proof/process differs"),
        });
    }
    let mut staged = StagedOwnedInitializeV2 {
        allocations: argument
            .allocations
            .take()
            .expect("consumed allocation provenance"),
        plan: plan.clone(),
        task: argument.root.take(),
        state: None,
        transferred: 0,
        provisional: false,
        failure: None,
        creator: argument.creator,
        settlement_started: false,
    };
    if budget.cancelled {
        staged.failure = Some(OwnedFrameFailure::HostAbandoned);
        return Ok(staged);
    }
    let functions = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&functions),
        BTreeMap::new(),
        &plan.helper().program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    evaluator.next_byte_allocation = staged
        .allocations
        .seed(&[staged.task.as_ref().expect("consumed Task")])
        .expect("validated Task allocation provenance");
    let mut frame = Environment::from(Vec::new());
    let outcome = (|| {
        evaluator.semantic_charge()?; // actual checked call frame
        contracts(&mut evaluator, plan, &staged, &mut frame, true)?;
        evaluator.charge()?; // actual empty source Block
        evaluator.charge()?; // actual ConstructRecord, before any field
        let ResolvedExprKind::ConstructRecord { fields, .. } = &plan.constructor().kind else {
            unreachable!()
        };
        let ResolvedType::Nominal { declaration, .. } = &plan.function().return_type else {
            unreachable!()
        };
        staged.state = Some(Value::Record(Arc::new(OwnedRecordValue {
            record: declaration.clone(),
            fields: BTreeMap::new(),
        })));
        for (index, field) in fields.iter().enumerate() {
            let target = if field.value.ty == ResolvedType::Bytes {
                plan.transfers().fields[staged.transferred]
                    .destination
                    .projections[0]
                    .clone()
            } else {
                field.field.clone()
            };
            let value = if field.value.ty == ResolvedType::Bytes {
                evaluator.charge()?; // actual owning Place: charge BEFORE take
                let transfer = &plan.transfers().fields[staged.transferred];
                debug_assert_eq!(transfer.field_index, index);
                debug_assert_eq!(transfer.at, field.value.id);
                let Value::Record(task) = staged.task.as_mut().expect("pending Task") else {
                    unreachable!()
                };
                let source = &transfer.source.projections[0];
                let value = Arc::get_mut(task)
                    .expect("sealed unique Task")
                    .fields
                    .remove(source)
                    .ok_or(Flow::Guard("missing compiler-owned transfer source"))?;
                let Value::Bytes(_) = value else {
                    return Err(Flow::Guard("non-Bytes transfer source"));
                };
                value
            } else {
                scalar(&mut evaluator, &field.value, plan, &staged, &mut frame)?
            };
            let Value::Record(state) = staged.state.as_mut().expect("pending State") else {
                unreachable!()
            };
            Arc::get_mut(state)
                .expect("sealed constructor")
                .fields
                .insert(target, value);
            if field.value.ty == ResolvedType::Bytes {
                staged.transferred += 1;
            }
        }
        // The query authenticated the actual enclosing whole-root transfer
        // chain. No source evaluation/charge separates complete construction
        // from its provisional-result transfer; physical backing stays put.
        staged.provisional = true;
        contracts(&mut evaluator, plan, &staged, &mut frame, false)?;
        Ok(())
    })();
    let steps = evaluator.steps;
    drop(frame);
    drop(evaluator);
    budget.remaining -= steps;
    budget.consumed += steps;
    if let Err(flow) = outcome {
        staged.failure = Some(failure(flow));
    }
    Ok(staged)
}
fn task_valid(plan: &CheckedOwnedInitializeV2, root: &Value) -> bool {
    if !exclusive(root) {
        return false;
    }
    let Value::Record(record) = root else {
        return false;
    };
    let ResolvedType::Nominal { declaration, .. } = &plan.function().params[0].ty else {
        return false;
    };
    let Some(fields) = plan
        .helper()
        .program()
        .declarations
        .record_fields(declaration)
    else {
        return false;
    };
    let count = plan.transfers().fields.len() as u32;
    let mut allocations = Vec::new();
    record.record == *declaration
        && record.fields.len() == fields.len()
        && fields.iter().all(|f| {
            let Some(v) = record.fields.get(&f.id) else {
                return false;
            };
            match v {
                Value::Bytes(b) => {
                    let valid = f.ty == ResolvedType::Bytes
                        && b.bytes.len() <= 1024
                        && b.allocation != 0
                        && b.allocation <= count
                        && !allocations.contains(&b.allocation);
                    allocations.push(b.allocation);
                    valid
                }
                v => super::super::super::argument_of(v).is_some_and(|a| scalar_valid(&f.ty, &a)),
            }
        })
}
fn scalar(
    evaluator: &mut Evaluator<'_>,
    expression: &ResolvedExpr,
    plan: &CheckedOwnedInitializeV2,
    staged: &StagedOwnedInitializeV2,
    frame: &mut Environment,
) -> Result<Value, Flow> {
    let mut e = expression.clone();
    project(&mut e, plan, staged)?;
    evaluator.evaluate(&e, frame, 0)
}
fn project(
    e: &mut ResolvedExpr,
    plan: &CheckedOwnedInitializeV2,
    staged: &StagedOwnedInitializeV2,
) -> Result<(), Flow> {
    match &mut e.kind {
        ResolvedExprKind::Place(place) => {
            let root = if place.root == plan.function().params[0].id {
                staged.task.as_ref()
            } else if place.root == plan.function().result_id {
                staged.state.as_ref()
            } else {
                return Err(Flow::Guard("foreign initializer Copy root"));
            };
            project_copy(
                e,
                plan.function(),
                root.ok_or(Flow::Guard("missing initializer Copy root"))?,
            )?;
        }
        ResolvedExprKind::Unary { value, .. } => project(value, plan, staged)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            project(left, plan, staged)?;
            project(right, plan, staged)?;
        }
        _ => {}
    }
    Ok(())
}
fn contracts(
    evaluator: &mut Evaluator<'_>,
    plan: &CheckedOwnedInitializeV2,
    staged: &StagedOwnedInitializeV2,
    frame: &mut Environment,
    requires: bool,
) -> Result<(), Flow> {
    let f = plan.function();
    let clauses = if requires { &f.requires } else { &f.ensures };
    for (index, clause) in clauses.iter().enumerate() {
        evaluator.charge()?;
        match scalar(evaluator, clause, plan, staged, frame)? {
            Value::Bool(true) => {}
            Value::Bool(false) => {
                return Err(evaluator.contract_failure(
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
            _ => return Err(Flow::Guard("checked initializer contract type")),
        }
    }
    Ok(())
}
pub(crate) struct OwnedInitializeSettlementRejectionV2 {
    pub(crate) staged: StagedOwnedInitializeV2,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) enum OwnedInitializeSettledV2 {
    Initialized(OwnedAgentStateArgument),
    Failed {
        failure: OwnedFrameFailure,
        operations: Vec<FinalizeAction>,
        observations_succeeded: bool,
    },
}
pub(crate) fn settle_owned_initialize_v2(
    mut staged: StagedOwnedInitializeV2,
    mut current: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<OwnedInitializeSettledV2, OwnedInitializeSettlementRejectionV2> {
    let reject = |staged, diagnostic| OwnedInitializeSettlementRejectionV2 { staged, diagnostic };
    if staged.settlement_started || !current_in_creator(staged.creator, &mut current) {
        return Err(reject(
            staged,
            rejected("initializer settlement authority/process differs"),
        ));
    }
    if staged.failure.is_none() {
        if !staged.provisional
            || !staged.plan.transfers().completion_cleanup.is_empty()
            || !staged
                .state
                .as_ref()
                .is_some_and(|r| root_valid(staged.plan.helper(), r))
            || !pending_matches(&staged, &staged.plan.transfers().result_disposal)
            || !current_in_creator(staged.creator, &mut current)
        {
            return Err(reject(
                staged,
                rejected("initializer publication basis differs"),
            ));
        }
        drop(staged.task.take()); // only Copy fields remain, no semantic cleanup
        return Ok(OwnedInitializeSettledV2::Initialized(
            OwnedAgentStateArgument {
                plan: staged.plan.helper().clone(),
                root: staged.state.take(),
                creator: staged.creator,
                allocations: Some(staged.allocations),
            },
        ));
    }
    let actions = if staged.provisional {
        staged.plan.transfers().result_disposal.clone()
    } else {
        staged.plan.transfers().failure_by_prefix[staged.transferred].clone()
    };
    if !pending_matches(&staged, &actions) {
        return Err(reject(
            staged,
            rejected("initializer partial cleanup basis differs"),
        ));
    }
    staged.settlement_started = true;
    let mut operations = Vec::new();
    let mut observations_succeeded = true;
    for action in &actions {
        if !current_in_creator(staged.creator, &mut current) {
            return Err(reject(
                staged,
                rejected("initializer cleanup authority lost"),
            ));
        }
        let task_storage =
            crate::cleanup_plan::StorageId::Value(staged.plan.function().params[0].id.clone());
        let root = if action.source.storage == task_storage {
            &mut staged.task
        } else {
            &mut staged.state
        };
        let Value::Record(root) = root.as_mut().expect("preflight actual root") else {
            unreachable!()
        };
        drop(
            Arc::get_mut(root)
                .expect("sealed cleanup root")
                .fields
                .remove(&action.source.projections[0])
                .expect("preflight actual leaf"),
        );
        operations.push(action.clone());
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_err() {
            observations_succeeded = false;
        }
        if !current_in_creator(staged.creator, &mut current) {
            return Err(reject(
                staged,
                rejected("initializer cleanup authority lost"),
            ));
        }
    }
    Ok(OwnedInitializeSettledV2::Failed {
        failure: staged.failure.take().expect("sticky failure"),
        operations,
        observations_succeeded,
    })
}
fn pending_matches(staged: &StagedOwnedInitializeV2, actions: &[FinalizeAction]) -> bool {
    let task_storage =
        crate::cleanup_plan::StorageId::Value(staged.plan.function().params[0].id.clone());
    let state_storage = if staged.provisional {
        crate::cleanup_plan::StorageId::ProvisionalResult
    } else {
        crate::cleanup_plan::StorageId::Temporary(staged.plan.constructor().id.clone())
    };
    let roots = [&staged.task, &staged.state]
        .into_iter()
        .filter_map(|r| r.as_ref())
        .collect::<Vec<_>>();
    if !staged.allocations.validate(&roots) {
        return false;
    }
    let mut actual = Vec::new();
    for (storage, value) in [
        (&task_storage, &staged.task),
        (&state_storage, &staged.state),
    ] {
        if let Some(value) = value {
            if !exclusive(value) {
                return false;
            }
            let Value::Record(record) = value else {
                return false;
            };
            for (id, v) in &record.fields {
                if let Value::Bytes(_) = v {
                    actual.push(crate::cleanup_plan::CleanupPlace {
                        storage: storage.clone(),
                        projections: vec![id.clone()],
                    });
                }
            }
        }
    }
    actual.len() == actions.len()
        && actions.iter().enumerate().all(|(i, a)| {
            a.active_case.is_none()
                && a.lifecycle_id.as_str() == crate::cleanup::BYTES_DROP_LIFECYCLE_ID
                && actual.contains(&a.source)
                && !actions[..i].iter().any(|p| p.source == a.source)
        })
}

#[cfg(test)]
mod tests;
