//! Guarded compiler cleanup precedes the sealed, consuming Step field move.
use super::*;
use crate::interpreter::OwnedRecordValue;

pub(crate) struct ReadyOwnedStepV2 {
    plan: CheckedOwnedReduceV2,
    pub(super) root: Option<Value>,
    allocations: Option<OwnedAllocationProvenanceV2>,
    creator: u32,
    nonresult_operations: Vec<FinalizeAction>,
    observations_succeeded: bool,
}
impl ReadyOwnedStepV2 {
    pub(crate) fn nonresult_operations(&self) -> &[FinalizeAction] {
        &self.nonresult_operations
    }
    pub(crate) fn observations_succeeded(&self) -> bool {
        self.observations_succeeded
    }
}
impl Drop for ReadyOwnedStepV2 {
    fn drop(&mut self) {
        if self.creator == std::process::id()
            && self
                .root
                .as_ref()
                .is_some_and(|r| self.allocations.as_ref().is_some_and(|p| p.validate(&[r])))
        {
            for action in &self.plan.transfers().result_disposal {
                let Some(Value::Variant(record)) = self.root.as_mut() else {
                    break;
                };
                if action
                    .active_case
                    .as_ref()
                    .is_some_and(|a| a.case == record.case)
                {
                    let Some(record) = Arc::get_mut(record) else {
                        break;
                    };
                    drop(
                        record
                            .fields
                            .remove(action.source.projections.last().expect("compiler leaf")),
                    );
                }
            }
        }
        drop(self.root.take()); // foreign process: backing only, no semantic receipt
    }
}
pub(crate) struct OwnedReducedReportV2 {
    plan: CheckedOwnedReduceV2,
    pub(super) root: Option<Value>,
    source_case: DeclarationId,
    allocations: OwnedAllocationProvenanceV2,
    creator: u32,
}
impl Drop for OwnedReducedReportV2 {
    fn drop(&mut self) {
        if self.creator == std::process::id()
            && self
                .root
                .as_ref()
                .is_some_and(|r| self.allocations.validate(&[r]))
        {
            let map = self
                .plan
                .mappings()
                .iter()
                .find(|m| m.case == self.source_case)
                .expect("checked Complete map");
            for action in &self.plan.transfers().result_disposal {
                if action
                    .active_case
                    .as_ref()
                    .is_some_and(|a| a.case == self.source_case)
                {
                    let source = action.source.projections.last().expect("compiler leaf");
                    let target = &map
                        .fields
                        .iter()
                        .find(|(id, _)| id == source)
                        .expect("checked field map")
                        .1;
                    let Some(Value::Record(record)) = self.root.as_mut() else {
                        break;
                    };
                    let Some(record) = Arc::get_mut(record) else {
                        break;
                    };
                    drop(record.fields.remove(target));
                }
            }
        }
        drop(self.root.take());
    }
}
pub(crate) enum OwnedStepTransferV2 {
    Continue(OwnedAgentStateArgument),
    Suspend(OwnedAgentStateArgument),
    Complete(OwnedReducedReportV2),
    Fail(i64),
}
pub(crate) struct OwnedReduceSettlementRejectionV2 {
    pub(crate) staged: StagedOwnedReduceV2,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) enum OwnedReduceSettledV2 {
    Ready(ReadyOwnedStepV2),
    Failed {
        failure: OwnedFrameFailure,
        operations: Vec<FinalizeAction>,
        observations_succeeded: bool,
    },
}
pub(crate) fn settle_owned_reduce_v2(
    mut staged: StagedOwnedReduceV2,
    mut current: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<OwnedReduceSettledV2, OwnedReduceSettlementRejectionV2> {
    let reject = |staged, diagnostic| OwnedReduceSettlementRejectionV2 { staged, diagnostic };
    if staged.settlement_started || !current_in_creator(staged.creator, &mut current) {
        return Err(reject(
            staged,
            rejected("reducer settlement process/authority differs"),
        ));
    }
    let actions = actions(&staged);
    let active = active_flags(&staged);
    if !pending_matches(&staged, &actions, &active) {
        return Err(reject(
            staged,
            rejected("reducer physical/guard cleanup basis differs"),
        ));
    }
    staged.settlement_started = true;
    let mut operations = Vec::new();
    let mut observations_succeeded = true;
    for action in &actions {
        // Preserve the original vector and implement its compiler guard. A
        // false guard does not attempt a physical operation or observation.
        if !active.contains(&action.guard_flag) {
            continue;
        }
        if !current_in_creator(staged.creator, &mut current) {
            return Err(reject(staged, rejected("reducer cleanup authority lost")));
        }
        let root = root_for(&mut staged, &action.source.storage);
        match root.as_mut().expect("checked root") {
            Value::Record(record) => drop(
                Arc::get_mut(record)
                    .expect("exclusive")
                    .fields
                    .remove(action.source.projections.last().unwrap())
                    .expect("checked leaf"),
            ),
            Value::Variant(record) => drop(
                Arc::get_mut(record)
                    .expect("exclusive")
                    .fields
                    .remove(action.source.projections.last().unwrap())
                    .expect("checked leaf"),
            ),
            _ => unreachable!(),
        }
        operations.push(action.clone());
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_err() {
            observations_succeeded = false;
        }
        if !current_in_creator(staged.creator, &mut current) {
            return Err(reject(staged, rejected("reducer cleanup authority lost")));
        }
    }
    if let Some(failure) = staged.failure.take() {
        return Ok(OwnedReduceSettledV2::Failed {
            failure,
            operations,
            observations_succeeded,
        });
    }
    if !staged.provisional
        || !staged
            .allocations
            .validate(&[staged.step.as_ref().expect("checked Step")])
        || !current_in_creator(staged.creator, &mut current)
    {
        return Err(reject(
            staged,
            rejected("reducer Step handoff basis differs"),
        ));
    }
    drop(staged.state.take());
    drop(staged.outcome.take());
    Ok(OwnedReduceSettledV2::Ready(ReadyOwnedStepV2 {
        plan: staged.plan,
        root: staged.step.take(),
        allocations: Some(staged.allocations),
        creator: staged.creator,
        nonresult_operations: operations,
        observations_succeeded,
    }))
}
pub(super) fn actions(staged: &StagedOwnedReduceV2) -> Vec<FinalizeAction> {
    if staged.provisional {
        if staged.failure.is_some() {
            staged.plan.transfers().provisional_failure.clone()
        } else {
            staged.plan.transfers().completion_cleanup.clone()
        }
    } else if let Some(case) = staged.case {
        staged.plan.transfers().cases[case].failure_by_prefix[staged.transferred].clone()
    } else {
        staged.plan.transfers().initial_disposal.clone()
    }
}
pub(super) fn active_flags(staged: &StagedOwnedReduceV2) -> Vec<crate::cleanup::LivenessFlagId> {
    if !staged.provisional {
        return actions(staged).iter().map(|a| a.guard_flag).collect();
    }
    let mut flags = staged.plan.transfers().cases[staged.case.expect("selected constructor")]
        .completion_live_flags
        .clone();
    if staged.failure.is_some() {
        let Value::Variant(root) = staged.step.as_ref().unwrap() else {
            unreachable!()
        };
        flags.extend(
            staged
                .plan
                .transfers()
                .result_disposal
                .iter()
                .filter(|a| a.active_case.as_ref().is_some_and(|c| c.case == root.case))
                .map(|a| a.guard_flag),
        );
    }
    flags
}
fn root_for<'a>(
    staged: &'a mut StagedOwnedReduceV2,
    storage: &crate::cleanup_plan::StorageId,
) -> &'a mut Option<Value> {
    if *storage
        == crate::cleanup_plan::StorageId::Value(staged.plan.function().params[0].id.clone())
    {
        &mut staged.state
    } else if *storage
        == crate::cleanup_plan::StorageId::Value(
            staged.plan.function().params.last().unwrap().id.clone(),
        )
    {
        &mut staged.outcome
    } else {
        &mut staged.step
    }
}
fn pending_matches(
    staged: &StagedOwnedReduceV2,
    actions: &[FinalizeAction],
    active: &[crate::cleanup::LivenessFlagId],
) -> bool {
    if !staged.allocations.validate(&roots(staged)) || !partial_types_match(staged) {
        return false;
    }
    let mut actual = Vec::new();
    for (storage, value) in [
        (
            crate::cleanup_plan::StorageId::Value(staged.plan.function().params[0].id.clone()),
            &staged.state,
        ),
        (
            crate::cleanup_plan::StorageId::Value(
                staged.plan.function().params.last().unwrap().id.clone(),
            ),
            &staged.outcome,
        ),
        (
            if staged.provisional {
                crate::cleanup_plan::StorageId::ProvisionalResult
            } else if let Some(case) = staged.case {
                crate::cleanup_plan::StorageId::Temporary(
                    staged.plan.transfers().cases[case].constructor.clone(),
                )
            } else {
                crate::cleanup_plan::StorageId::ProvisionalResult
            },
            &staged.step,
        ),
    ] {
        if let Some(value) = value {
            match value {
                Value::Record(record) => {
                    for (field, value) in &record.fields {
                        if matches!(value, Value::Bytes(_)) {
                            actual.push(crate::cleanup_plan::CleanupPlace {
                                storage: storage.clone(),
                                projections: vec![field.clone()],
                            });
                        }
                    }
                }
                Value::Variant(record) => {
                    for (field, value) in &record.fields {
                        if matches!(value, Value::Bytes(_)) {
                            actual.push(crate::cleanup_plan::CleanupPlace {
                                storage: storage.clone(),
                                projections: vec![record.case.clone(), field.clone()],
                            });
                        }
                    }
                }
                _ => return false,
            }
        }
    }
    let mut expected = actions
        .iter()
        .filter(|a| active.contains(&a.guard_flag))
        .map(|a| a.source.clone())
        .collect::<Vec<_>>();
    // Successful nonresult settlement retains every active provisional result
    // leaf, authenticated by the compiler's conditional result inventory.
    if staged.provisional && staged.failure.is_none() {
        let Some(Value::Variant(root)) = staged.step.as_ref() else {
            return false;
        };
        expected.extend(
            staged
                .plan
                .transfers()
                .result_disposal
                .iter()
                .filter(|a| a.active_case.as_ref().is_some_and(|c| c.case == root.case))
                .map(|a| a.source.clone()),
        );
    }
    expected.len() == actual.len()
        && expected
            .iter()
            .enumerate()
            .all(|(i, p)| actual.contains(p) && !expected[..i].contains(p))
}

/// Pure field move after guarded stage settlement. This grants no journal ACK,
/// external effect permission, public Report delivery or restoration authority.
pub(crate) fn consume_owned_step_v2(
    mut ready: ReadyOwnedStepV2,
) -> Result<OwnedStepTransferV2, ReadyOwnedStepV2> {
    if ready.creator != std::process::id()
        || !ready.observations_succeeded
        || !ready
            .root
            .as_ref()
            .is_some_and(|r| ready.allocations.as_ref().is_some_and(|p| p.validate(&[r])))
    {
        return Err(ready);
    }
    let Some(Value::Variant(root)) = ready.root.as_ref() else {
        return Err(ready);
    };
    let Some(mapping) = ready
        .plan
        .mappings()
        .iter()
        .find(|m| m.case == root.case)
        .cloned()
    else {
        return Err(ready);
    };
    if root.variant != *ready.plan.function().return_type.nominal_id().unwrap()
        || root.fields.len() != mapping.fields.len()
        || mapping
            .fields
            .iter()
            .any(|(id, _)| !root.fields.contains_key(id))
    {
        return Err(ready);
    }
    let declared = ready
        .plan
        .helper()
        .program()
        .declarations
        .case_fields(&root.case)
        .expect("checked Step case");
    if declared.iter().any(|f| {
        !root
            .fields
            .get(&f.id)
            .is_some_and(|v| leaf_type_matches(&f.ty, v))
    }) {
        return Err(ready);
    }
    if mapping.role == "Fail" {
        let Value::Int(code) = &root.fields[&mapping.fields[0].0] else {
            return Err(ready);
        };
        let code = *code;
        drop(ready.root.take());
        return Ok(OwnedStepTransferV2::Fail(code));
    }
    let Some(Value::Variant(root)) = ready.root.as_mut() else {
        unreachable!()
    };
    let mut fields = BTreeMap::new();
    for (source, target) in &mapping.fields {
        fields.insert(
            target.clone(),
            Arc::get_mut(root)
                .expect("sealed Step")
                .fields
                .remove(source)
                .expect("checked map"),
        );
    }
    drop(ready.root.take()); // Copy-only shell
    let root = Value::Record(Arc::new(OwnedRecordValue {
        record: mapping.target.clone(),
        fields,
    }));
    let allocations = ready.allocations.take().expect("consumed provenance");
    debug_assert!(allocations.validate(&[&root]));
    match mapping.role {
        "Continue" | "Suspend" => {
            debug_assert!(root_valid(ready.plan.helper(), &root));
            let state = OwnedAgentStateArgument {
                plan: ready.plan.helper().clone(),
                root: Some(root),
                allocations: Some(allocations),
                creator: ready.creator,
            };
            Ok(if mapping.role == "Continue" {
                OwnedStepTransferV2::Continue(state)
            } else {
                OwnedStepTransferV2::Suspend(state)
            })
        }
        "Complete" => Ok(OwnedStepTransferV2::Complete(OwnedReducedReportV2 {
            plan: ready.plan.clone(),
            root: Some(root),
            source_case: mapping.case,
            allocations,
            creator: ready.creator,
        })),
        _ => unreachable!("checked four-case mapping"),
    }
}

fn leaf_type_matches(ty: &ResolvedType, value: &Value) -> bool {
    match value {
        Value::Bytes(bytes) => {
            *ty == ResolvedType::Bytes && bytes.bytes.len() <= 1024 && bytes.allocation != 0
        }
        value => {
            super::super::super::super::argument_of(value).is_some_and(|v| scalar_valid(ty, &v))
        }
    }
}
fn partial_types_match(staged: &StagedOwnedReduceV2) -> bool {
    let declarations = &staged.plan.helper().program().declarations;
    for (value, ty) in [
        (&staged.state, &staged.plan.function().params[0].ty),
        (
            &staged.outcome,
            &staged.plan.function().params.last().unwrap().ty,
        ),
    ] {
        if let Some(value) = value {
            let Value::Record(record) = value else {
                return false;
            };
            let Some(id) = ty.nominal_id() else {
                return false;
            };
            let Some(fields) = declarations.record_fields(id) else {
                return false;
            };
            if record.record != *id
                || record.fields.iter().any(|(id, v)| {
                    fields
                        .iter()
                        .find(|f| f.id == *id)
                        .is_none_or(|f| !leaf_type_matches(&f.ty, v))
                })
                || fields
                    .iter()
                    .any(|f| f.ty != ResolvedType::Bytes && !record.fields.contains_key(&f.id))
            {
                return false;
            }
        }
    }
    if let Some(value) = &staged.step {
        let Value::Variant(record) = value else {
            return false;
        };
        let Some(case) = staged.case else {
            return false;
        };
        let actual = &staged.plan.transfers().cases[case];
        let Some(fields) = declarations.case_fields(&actual.case) else {
            return false;
        };
        if record.variant != *staged.plan.function().return_type.nominal_id().unwrap()
            || record.case != actual.case
            || record.fields.iter().any(|(id, v)| {
                fields
                    .iter()
                    .find(|f| f.id == *id)
                    .is_none_or(|f| !leaf_type_matches(&f.ty, v))
            })
            || staged.provisional && record.fields.len() != fields.len()
        {
            return false;
        }
    }
    true
}
