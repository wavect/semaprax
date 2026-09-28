//! Sealed consuming two-parameter helper foundation; not a durable/store route.
use super::*;
mod provenance;
use crate::interpreter::resumable::ResumableChannelValue;
use crate::resumable_effects::owned_frame::v2::CheckedOwnedFrameHelperV2;
use provenance::OwnedAllocationProvenanceV2;

pub(crate) struct OwnedAgentStateArgument {
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    creator: u32,
    allocations: Option<OwnedAllocationProvenanceV2>,
}
pub(crate) fn admit_owned_agent_state_input(
    plan: &CheckedOwnedFrameHelperV2,
    input: OwnedFrameInput,
) -> Result<OwnedAgentStateArgument, OwnedFrameInputRejection> {
    let result = validate_fields_for(
        &plan.program().declarations,
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
    );
    if let Err(diagnostic) = result {
        return Err(OwnedFrameInputRejection { input, diagnostic });
    }
    let root = stage_root_for(
        &plan.program().declarations,
        &plan.function().params[0].ty,
        input,
    );
    let allocations = OwnedAllocationProvenanceV2::fresh(&root).expect("checked fresh staging");
    Ok(OwnedAgentStateArgument {
        plan: plan.clone(),
        root: Some(root),
        creator: std::process::id(),
        allocations: Some(allocations),
    })
}
impl Drop for OwnedAgentStateArgument {
    fn drop(&mut self) {
        if self.creator == std::process::id() && self.root.is_some() {
            let _ = release(&mut self.root, &self.plan.liveness().failure_cleanup);
        } else {
            drop(self.root.take());
        }
    }
}
pub(crate) struct PreparedOwnedCopyWaitV2 {
    argument: OwnedAgentStateArgument,
    observation: ResumableChannelValue,
}
pub(crate) struct OwnedCopyWaitPreparationRejection {
    pub(crate) argument: OwnedAgentStateArgument,
    pub(crate) observation: ResumableChannelValue,
    pub(crate) diagnostic: Diagnostic,
}
pub(crate) fn prepare_owned_copy_wait_v2(
    argument: OwnedAgentStateArgument,
    observation: ResumableChannelValue,
) -> Result<PreparedOwnedCopyWaitV2, OwnedCopyWaitPreparationRejection> {
    if argument.creator != std::process::id()
        || !channel_v2::valid_copy_carrier(
            &argument.plan.program().declarations,
            &argument.plan.function().params[1].ty,
            &observation,
        )
    {
        return Err(OwnedCopyWaitPreparationRejection {
            argument,
            observation,
            diagnostic: rejected("v2 observation/process mismatch"),
        });
    }
    Ok(PreparedOwnedCopyWaitV2 {
        argument,
        observation,
    })
}
pub(crate) struct OwnedCopyWaitParkedV2 {
    allocations: OwnedAllocationProvenanceV2,
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    observation: ResumableChannelValue,
    creator: u32,
}
impl OwnedCopyWaitParkedV2 {
    pub(crate) fn request(&self) -> &ResumableChannelValue {
        &self.observation
    }
}
pub(crate) struct OwnedCopyWaitTerminalV2 {
    allocations: OwnedAllocationProvenanceV2,
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    proposal: Option<ResumableChannelValue>,
    failure: Option<OwnedFrameFailure>,
    provisional: bool,
    creator: u32,
}
impl OwnedCopyWaitTerminalV2 {
    pub(crate) fn failure(&self) -> Option<&OwnedFrameFailure> {
        self.failure.as_ref()
    }
    pub(crate) fn proposal(&self) -> Option<&ResumableChannelValue> {
        self.proposal.as_ref()
    }
}
pub(crate) enum OwnedCopyWaitStepV2 {
    Parked(OwnedCopyWaitParkedV2),
    Terminal(OwnedCopyWaitTerminalV2),
}
fn terminal(
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    failure: OwnedFrameFailure,
    provisional: bool,
    creator: u32,
    allocations: OwnedAllocationProvenanceV2,
) -> OwnedCopyWaitStepV2 {
    OwnedCopyWaitStepV2::Terminal(OwnedCopyWaitTerminalV2 {
        allocations,
        plan,
        root,
        proposal: None,
        failure: Some(failure),
        provisional,
        creator,
    })
}
pub(crate) fn begin_owned_copy_wait_v2(
    mut prepared: PreparedOwnedCopyWaitV2,
    budget: &mut OwnedFrameBudget,
) -> Result<OwnedCopyWaitStepV2, PreparedOwnedCopyWaitV2> {
    if prepared.argument.creator != std::process::id()
        || !prepared.argument.root.as_ref().is_some_and(|r| {
            prepared
                .argument
                .allocations
                .as_ref()
                .is_some_and(|p| p.validate(&[r]))
        })
    {
        return Err(prepared);
    }
    let allocations = prepared
        .argument
        .allocations
        .take()
        .expect("checked provenance");
    let root = prepared.argument.root.take();
    let plan = prepared.argument.plan.clone();
    let creator = prepared.argument.creator;
    if budget.cancelled {
        return Ok(terminal(
            plan,
            root,
            OwnedFrameFailure::HostAbandoned,
            false,
            creator,
            allocations,
        ));
    }
    let mut env = Environment::from(Vec::new());
    let copy = channel_v2::value_of_copy(
        &plan.program().declarations,
        &plan.function().params[1].ty,
        &prepared.observation,
    )
    .expect("checked observation");
    env.push((plan.function().params[1].id.clone(), copy));
    let (result, provisional) = evaluate_contract_phase(
        &plan,
        root.as_ref().expect("consumed state"),
        &mut env,
        true,
        budget,
    );
    drop(env);
    if let Err(flow) = result {
        return Ok(terminal(
            plan,
            root,
            failure(flow),
            provisional,
            creator,
            allocations,
        ));
    }
    Ok(OwnedCopyWaitStepV2::Parked(OwnedCopyWaitParkedV2 {
        allocations,
        plan,
        root,
        observation: prepared.observation,
        creator,
    }))
}
pub(crate) fn resume_owned_copy_wait_v2(
    parked: OwnedCopyWaitParkedV2,
    proposal: ResumableChannelValue,
    budget: &mut OwnedFrameBudget,
) -> Result<OwnedCopyWaitStepV2, (OwnedCopyWaitParkedV2, ResumableChannelValue)> {
    if parked.creator != std::process::id()
        || !parked
            .root
            .as_ref()
            .is_some_and(|r| parked.allocations.validate(&[r]))
    {
        return Err((parked, proposal));
    }
    let OwnedCopyWaitParkedV2 {
        allocations,
        plan,
        root,
        observation,
        creator,
    } = parked;
    if budget.cancelled {
        return Ok(terminal(
            plan,
            root,
            OwnedFrameFailure::HostAbandoned,
            false,
            creator,
            allocations,
        ));
    }
    let ty = &plan
        .function()
        .yields
        .as_ref()
        .expect("checked yields")
        .response_type;
    let Some(answer) = channel_v2::value_of_copy(&plan.program().declarations, ty, &proposal)
    else {
        return Ok(terminal(
            plan,
            root,
            OwnedFrameFailure::AnswerTypeMismatch,
            false,
            creator,
            allocations,
        ));
    };
    let mut env = Environment::from(Vec::new());
    env.push((
        plan.function().params[1].id.clone(),
        channel_v2::value_of_copy(
            &plan.program().declarations,
            &plan.function().params[1].ty,
            &observation,
        )
        .expect("saved checked observation"),
    ));
    let ResolvedExprKind::Block { statements, .. } = &plan.function().body.kind else {
        unreachable!()
    };
    let ResolvedStatement::Let { binding, .. } = &statements[0] else {
        unreachable!()
    };
    env.push((binding.id.clone(), answer));
    let (result, provisional) = evaluate_contract_phase(
        &plan,
        root.as_ref().expect("parked state"),
        &mut env,
        false,
        budget,
    );
    drop(env);
    if let Err(flow) = result {
        return Ok(terminal(
            plan,
            root,
            failure(flow),
            provisional,
            creator,
            allocations,
        ));
    }
    Ok(OwnedCopyWaitStepV2::Terminal(OwnedCopyWaitTerminalV2 {
        allocations,
        plan,
        root,
        proposal: Some(proposal),
        failure: None,
        provisional: true,
        creator,
    }))
}
fn evaluate_contract_phase(
    plan: &CheckedOwnedFrameHelperV2,
    root: &Value,
    env: &mut Environment,
    start: bool,
    budget: &mut OwnedFrameBudget,
) -> (Result<(), Flow>, bool) {
    let admitted = BTreeMap::new();
    let mut evaluator = Evaluator::new_prepared(
        FunctionLookup::Borrowed(&admitted),
        BTreeMap::new(),
        &plan.program().declarations,
        budget.remaining,
        0,
        PreparedCancellation::Never,
    );
    let mut provisional = false;
    let evaluated = (|| {
        if start {
            evaluator.charge()?; // helper entry
            check_contracts(&mut evaluator, plan.function(), root, env, true)?;
            evaluator.charge()?; // actual yield node
            let ResolvedExprKind::Block { statements, .. } = &plan.function().body.kind else {
                unreachable!()
            };
            let ResolvedStatement::Let { value, .. } = &statements[0] else {
                unreachable!()
            };
            let ResolvedExprKind::Yield { request } = &value.kind else {
                unreachable!()
            };
            // Execute the checked identity request expression, borrowing only
            // the Copy Observation. The owned State is never installed in env.
            drop(evaluate_copy(
                &mut evaluator,
                request,
                plan.function(),
                root,
                env,
            )?);
        } else {
            evaluator.charge()?; // resumed yield node
            evaluator.charge()?; // whole identity State tail
            provisional = true;
            check_contracts(&mut evaluator, plan.function(), root, env, false)?;
        }
        Ok(())
    })();
    let steps = evaluator.steps;
    drop(evaluator);
    budget.remaining -= steps;
    budget.consumed += steps;
    (evaluated, provisional)
}

pub(crate) struct CompletedOwnedAgentStateV2 {
    allocations: OwnedAllocationProvenanceV2,
    plan: CheckedOwnedFrameHelperV2,
    root: Option<Value>,
    proposal: ResumableChannelValue,
    creator: u32,
}
pub(crate) enum OwnedCopyWaitSettledV2 {
    Completed(CompletedOwnedAgentStateV2),
    Failed {
        failure: OwnedFrameFailure,
        receipt: OwnedFrameReleaseReceipt,
        observations_succeeded: bool,
    },
}
pub(crate) struct OwnedCopyWaitSettlementRejectionV2 {
    pub(crate) terminal: OwnedCopyWaitTerminalV2,
    pub(crate) diagnostic: Diagnostic,
}
fn root_valid(plan: &CheckedOwnedFrameHelperV2, root: &Value) -> bool {
    if !exclusive(root) {
        return false;
    }
    let ResolvedType::Nominal { declaration, .. } = &plan.function().params[0].ty else {
        return false;
    };
    let Value::Record(record) = root else {
        return false;
    };
    let Some(fields) = plan.program().declarations.record_fields(declaration) else {
        return false;
    };
    record.record == *declaration
        && record.fields.len() == fields.len()
        && fields.iter().all(|f| {
            let Some(value) = record.fields.get(&f.id) else {
                return false;
            };
            match value {
                Value::Bytes(bytes) => f.ty == ResolvedType::Bytes && bytes.bytes.len() <= 1024,
                value => super::super::argument_of(value)
                    .is_some_and(|scalar| scalar_valid(&f.ty, &scalar)),
            }
        })
}
fn current_in_creator<F: FnMut() -> bool + ?Sized>(creator: u32, current: &mut F) -> bool {
    if creator != std::process::id() {
        return false;
    }
    let allowed = current();
    allowed && creator == std::process::id()
}
pub(crate) fn settle_owned_copy_wait_v2(
    mut terminal: OwnedCopyWaitTerminalV2,
    mut current_authority: impl FnMut() -> bool,
    mut observe: impl FnMut(&FinalizeAction),
) -> Result<OwnedCopyWaitSettledV2, OwnedCopyWaitSettlementRejectionV2> {
    if !current_in_creator(terminal.creator, &mut current_authority)
        || !terminal
            .root
            .as_ref()
            .is_some_and(|r| terminal.allocations.validate(&[r]))
        || !terminal
            .root
            .as_ref()
            .is_some_and(|root| root_valid(&terminal.plan, root))
    {
        return Err(OwnedCopyWaitSettlementRejectionV2 {
            terminal,
            diagnostic: rejected("v2 root/authority changed"),
        });
    }
    if let Some(failure) = terminal.failure.clone() {
        let actions = if terminal.provisional {
            &terminal.plan.liveness().result_disposal
        } else {
            &terminal.plan.liveness().failure_cleanup
        };
        let mut observations_succeeded = true;
        let creator = terminal.creator;
        let released = release_guarded(
            &mut terminal.root,
            actions,
            || current_in_creator(creator, &mut current_authority),
            |action| {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action)))
                    .is_err()
                {
                    observations_succeeded = false;
                }
            },
        );
        return match released {
            Ok(receipt) => Ok(OwnedCopyWaitSettledV2::Failed {
                failure,
                receipt,
                observations_succeeded,
            }),
            Err(diagnostic) => Err(OwnedCopyWaitSettlementRejectionV2 {
                terminal,
                diagnostic,
            }),
        };
    }
    if !terminal.provisional
        || !terminal.plan.liveness().completion_cleanup.is_empty()
        || terminal.proposal.is_none()
        || !current_in_creator(terminal.creator, &mut current_authority)
    {
        return Err(OwnedCopyWaitSettlementRejectionV2 {
            terminal,
            diagnostic: rejected("v2 result not ready"),
        });
    }
    Ok(OwnedCopyWaitSettledV2::Completed(
        CompletedOwnedAgentStateV2 {
            allocations: terminal.allocations,
            plan: terminal.plan,
            root: terminal.root,
            proposal: terminal.proposal.expect("checked answer"),
            creator: terminal.creator,
        },
    ))
}
#[cfg(test)]
mod tests;

pub(crate) mod authorize;
pub(crate) mod initialize;
pub(crate) mod observe;
pub(crate) mod reduce;

pub(crate) mod effect;
