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
    #[cfg(test)]
    pub(crate) fn live_test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        let mut leaves = super::super::snapshot::weak_leaves(self.state.root.as_ref().unwrap());
        if let Some(Value::Variant(decision)) = &self.decision {
            leaves.extend(decision.fields.values().filter_map(|value| match value {
                Value::Bytes(bytes) => Some(Arc::downgrade(&bytes.bytes)),
                _ => None,
            }));
        }
        leaves
    }
    /// Borrow checked full source facts only; no owner or restore authority.
    pub(crate) fn live_staged_facts(
        &self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value)> {
        if self.failure.is_some()
            || !self.provisional
            || self.settlement_started
            || self.state.creator != std::process::id()
            || !self.state.plan.same_helper(binding.helper())
            || self.plan.function().id != binding.authorize().function().id
        {
            return None;
        }
        let root = self.state.root.as_ref()?;
        let decision_root = self.decision.as_ref()?;
        if !exclusive(root)
            || !exclusive_decision(decision_root)
            || !self.state.allocations.validate(&[root, decision_root])
        {
            return None;
        }
        let state = super::live_run::root_facts(&self.state.plan, root)?;
        let Value::Variant(decision) = decision_root else {
            return None;
        };
        if decision.variant != *self.plan.decision()
            || (decision.case != *self.plan.granted() && decision.case != *self.plan.refused())
        {
            return None;
        }
        let fields = self
            .plan
            .helper()
            .program()
            .declarations
            .case_fields(&decision.case)?;
        if decision.fields.len() != fields.len() {
            return None;
        }
        let values = fields.iter().map(|field| {
            let value = decision.fields.get(&field.id)?;
            let value = match value {
                Value::Bytes(bytes) if field.ty == ResolvedType::Bytes && bytes.bytes.len() <= 1024 =>
                    serde_json::json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(&bytes.bytes)}),
                Value::Int(v) if field.ty == ResolvedType::I64 =>
                    serde_json::json!({"tag":"i64","value":v}),
                _ => return None,
            };
            Some(serde_json::json!({"identity":field.id.as_str(),"value":value}))
        }).collect::<Option<Vec<_>>>()?;
        Some((
            state,
            serde_json::json!({"declaration":decision.variant.as_str(),"case":decision.case.as_str(),"fields":values}),
        ))
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
        || !state
            .root
            .as_ref()
            .is_some_and(|r| state.allocations.validate(&[r]))
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
    evaluator.next_byte_allocation = staged
        .state
        .allocations
        .seed(&[root])
        .expect("checked allocation provenance");
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
    let next_allocation = evaluator.next_byte_allocation;
    drop(evaluator);
    drop(frame); // only Copy parameters/array; never an owning State alias
    budget.remaining -= steps;
    budget.consumed += steps;
    let mut roots = vec![staged.state.root.as_ref().expect("retained State")];
    if let Some(decision) = staged.decision.as_ref() {
        roots.push(decision);
    }
    if staged
        .state
        .allocations
        .record_frame(&roots, next_allocation)
        .is_err()
    {
        staged.failure = Some(OwnedFrameFailure::EvaluationRejected);
    }
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
impl ReadyOwnedAuthorizeV2 {
    /// Descriptive checked facts, retaining both actual physical roots here.
    pub(crate) fn live_checked_facts(
        &self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    ) -> Option<(serde_json::Value, serde_json::Value)> {
        self.staged.live_staged_facts(binding)
    }
    #[cfg(test)]
    pub(crate) fn live_test_weak(&self) -> Vec<std::sync::Weak<[u8]>> {
        self.staged.live_test_weak()
    }
}
/// Private consuming bridge boundary. This wrapper keeps the actual Ready
/// holder intact; inert metadata cannot construct it or extract its roots.
pub(super) struct HeldOwnedEffectAuthorizationV8 {
    ready: ReadyOwnedAuthorizeV2,
    release_started: bool,
}
pub(super) struct OwnedEffectDecisionReleaseV8 {
    pub(super) holder: HeldOwnedEffectAuthorizationV8,
    pub(super) operations: Vec<FinalizeAction>,
    pub(super) observations_succeeded: bool,
}
pub(super) struct OwnedEffectReleasedRootsV8 {
    pub(super) state: Option<Value>,
    pub(super) outcome: Option<Value>,
    pub(super) helper: CheckedOwnedFrameHelperV2,
    pub(super) proposal: ResumableChannelValue,
    pub(super) allocations: OwnedAllocationProvenanceV2,
    pub(super) creator: u32,
}
impl OwnedEffectDecisionReleaseV8 {
    /// The effect owner calls this only after matching the post-release receipt
    /// ACK. This primitive itself cannot attest persistence or mint an ACK.
    pub(super) fn into_outcome(
        mut self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
        payload: Vec<u8>,
        mut current: impl FnMut() -> bool,
    ) -> Result<OwnedEffectReleasedRootsV8, (Self, Vec<u8>, Diagnostic)> {
        let state = &self.holder.ready.staged.state;
        if !self.observations_succeeded
            || !self.holder.release_started
            || self.holder.ready.staged.decision.is_some()
            || !state.plan.same_helper(binding.helper())
            || !state
                .root
                .as_ref()
                .is_some_and(|r| root_valid(binding.helper(), r))
            || !effect_current(state.creator, &mut current)
        {
            return Err((
                self,
                payload,
                rejected("effect post-release handoff authority differs"),
            ));
        }
        let state = &mut self.holder.ready.staged.state;
        let bytes = match state
            .allocations
            .mint_accepted_bytes(&[state.root.as_ref().unwrap()], payload)
        {
            Ok(bytes) => bytes,
            Err((payload, diagnostic)) => return Err((self, payload, diagnostic)),
        };
        let metadata = binding.lifecycle().owned_wait_outcome_v8();
        let outcome = Value::Record(Arc::new(OwnedRecordValue {
            record: metadata.id.clone(),
            fields: BTreeMap::from([
                (metadata.bytes_field.clone(), bytes),
                (metadata.status_field.clone(), Value::Int(0)),
            ]),
        }));
        // No user callback between the validated guard, fresh allocation and
        // consuming transfer. The same surviving root/token move together.
        let state = self.holder.ready.staged.state;
        Ok(OwnedEffectReleasedRootsV8 {
            state: state.root,
            outcome: Some(outcome),
            helper: state.plan,
            proposal: state.proposal,
            allocations: state.allocations,
            creator: state.creator,
        })
    }
}

pub(super) struct OwnedEffectDecisionReleaseRejectionV8 {
    pub(super) holder: HeldOwnedEffectAuthorizationV8,
    pub(super) diagnostic: Diagnostic,
}
fn effect_current<F: FnMut() -> bool + ?Sized>(creator: u32, current: &mut F) -> bool {
    current_in_creator(creator, current) && creator == std::process::id()
}
impl ReadyOwnedAuthorizeV2 {
    /// No root is taken until source identity, State schema, Proposal bits and
    /// both physical inventories agree with the actual sealed Agent binding.
    pub(super) fn hold_for_effect(
        self,
        binding: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
        proposal: &crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitProposalV8,
        scope: &crate::resumable_effects::source_checkpoint::SourceCheckpointScope,
        mut current: impl FnMut() -> bool,
    ) -> Result<HeldOwnedEffectAuthorizationV8, Self> {
        let staged = &self.staged;
        let scope = serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()});
        let valid = effect_current(staged.state.creator, &mut current)
            && staged.failure.is_none()
            && staged.provisional
            && !staged.settlement_started
            && staged.state.plan.same_helper(binding.helper())
            && staged.plan.function().id == binding.authorize().function().id
            && proposal.matches(binding.binding(), &scope)
            && staged
                .state
                .root
                .as_ref()
                .is_some_and(|r| root_valid(binding.helper(), r));
        let copies_equal = || {
            let ty = &binding
                .helper()
                .function()
                .yields
                .as_ref()
                .expect("checked yields")
                .response_type;
            let declarations = &binding.helper().program().declarations;
            let left = channel_v2::value_of_copy(declarations, ty, &staged.state.proposal)?;
            let right = channel_v2::value_of_copy(declarations, ty, proposal.carrier())?;
            Some(crate::interpreter::resumable::scalar_values_equal(
                &left, &right,
            ))
        };
        if !valid || copies_equal() != Some(true) || !effect_roots_valid(staged) {
            return Err(self);
        }
        Ok(HeldOwnedEffectAuthorizationV8 {
            ready: self,
            release_started: false,
        })
    }
}
fn effect_roots_valid(staged: &StagedOwnedAuthorizeV2) -> bool {
    let Some(Value::Variant(decision)) = staged.decision.as_ref() else {
        return false;
    };
    let Some(state) = staged.state.root.as_ref() else {
        return false;
    };
    let Some(fields) = staged
        .plan
        .helper()
        .program()
        .declarations
        .case_fields(staged.plan.granted())
    else {
        return false;
    };
    decision.ty == staged.plan.function().return_type
        && decision.variant == *staged.plan.decision()
        && decision.case == *staged.plan.granted()
        && fields.len() == 2
        && decision.fields.len() == 2
        && matches!(decision.fields.get(&fields[0].id), Some(Value::Bytes(_)))
        && matches!(decision.fields.get(&fields[1].id), Some(Value::Int(_)))
        && exclusive_decision(staged.decision.as_ref().unwrap())
        && root_valid(&staged.state.plan, state)
        && staged
            .state
            .allocations
            .validate(&[state, staged.decision.as_ref().unwrap()])
}
#[cfg(test)]
impl ReadyOwnedAuthorizeV2 {
    pub(super) fn effect_facts(&self) -> Option<(serde_json::Value, serde_json::Value, i64)> {
        if self.staged.state.creator != std::process::id() || !effect_roots_valid(&self.staged) {
            return None;
        }
        effect_facts(&self.staged)
    }
}
fn effect_facts(
    staged: &StagedOwnedAuthorizeV2,
) -> Option<(serde_json::Value, serde_json::Value, i64)> {
    let Value::Record(state) = staged.state.root.as_ref()? else {
        return None;
    };
    let Value::Variant(decision) = staged.decision.as_ref()? else {
        return None;
    };
    let state_fields = staged
        .state
        .plan
        .program()
        .declarations
        .record_fields(&state.record)?;
    let decision_fields = staged
        .plan
        .helper()
        .program()
        .declarations
        .case_fields(&decision.case)?;
    let field = |id: &hir::DeclarationId, value: &Value| -> Option<serde_json::Value> {
        let value = match value {
            Value::Bytes(bytes) => {
                let mut hex = String::with_capacity(bytes.bytes.len() * 2);
                use std::fmt::Write;
                for byte in bytes.bytes.iter() {
                    write!(hex, "{byte:02x}").ok()?;
                }
                serde_json::json!({"kind":"bytes","hex":hex})
            }
            Value::Int(value) => serde_json::json!({"tag":"i64","value":value}),
            _ => return None,
        };
        Some(serde_json::json!({"identity":id.as_str(),"value":value}))
    };
    let state_values = state_fields
        .iter()
        .map(|f| field(&f.id, state.fields.get(&f.id)?))
        .collect::<Option<Vec<_>>>()?;
    let decision_values = decision_fields
        .iter()
        .map(|f| field(&f.id, decision.fields.get(&f.id)?))
        .collect::<Option<Vec<_>>>()?;
    let Value::Int(budget) = decision.fields.get(&decision_fields[1].id)? else {
        return None;
    };
    Some((
        serde_json::json!({"declaration":state.record.as_str(),"fields":state_values}),
        serde_json::json!({"declaration":decision.variant.as_str(),"case":decision.case.as_str(),"fields":decision_values}),
        *budget,
    ))
}
impl HeldOwnedEffectAuthorizationV8 {
    pub(super) fn into_ready(self) -> ReadyOwnedAuthorizeV2 {
        assert!(
            !self.release_started,
            "released effect owner cannot become Ready"
        );
        self.ready
    }
    /// Bounded inert snapshots borrowed from the actual checked physical roots.
    /// Bytes become hex text, never a second Bytes/RetainedValue owner.
    pub(super) fn facts(&self) -> Option<(serde_json::Value, serde_json::Value, i64)> {
        if self.release_started
            || self.ready.staged.state.creator != std::process::id()
            || !effect_roots_valid(&self.ready.staged)
        {
            return None;
        }
        effect_facts(&self.ready.staged)
    }
    /// This private primitive is called only after the effect owner validates
    /// its distinct settlement and cleanup-start ACKs. It mints no ACK.
    pub(super) fn release_decision(
        mut self,
        mut current: impl FnMut() -> bool,
        mut observe: impl FnMut(&FinalizeAction),
    ) -> Result<OwnedEffectDecisionReleaseV8, OwnedEffectDecisionReleaseRejectionV8> {
        if self.release_started
            || !effect_roots_valid(&self.ready.staged)
            || !effect_current(self.ready.staged.state.creator, &mut current)
        {
            return Err(OwnedEffectDecisionReleaseRejectionV8 {
                holder: self,
                diagnostic: rejected("effect Decision release authority/inventory differs"),
            });
        }
        let creator = self.ready.staged.state.creator;
        let actions: Vec<_> = self
            .ready
            .staged
            .plan
            .disposal()
            .iter()
            .filter(|a| {
                a.active_case
                    .as_ref()
                    .is_some_and(|c| c.case == *self.ready.staged.plan.granted())
            })
            .cloned()
            .collect();
        let Value::Variant(value) = self.ready.staged.decision.as_ref().unwrap() else {
            unreachable!()
        };
        let leaves: Vec<_> = value
            .fields
            .iter()
            .filter(|(_, v)| matches!(v, Value::Bytes(_)))
            .map(|(id, _)| id)
            .collect();
        if actions.len() != leaves.len()
            || actions.iter().any(|a| {
                a.source.projections.len() != 2
                    || a.source.projections[0] != value.case
                    || !leaves.contains(&&a.source.projections[1])
            })
        {
            return Err(OwnedEffectDecisionReleaseRejectionV8 {
                holder: self,
                diagnostic: rejected("effect Decision compiler vector differs"),
            });
        }
        self.release_started = true;
        let mut observations_succeeded = true;
        for action in &actions {
            if !effect_current(creator, &mut current) {
                return Err(OwnedEffectDecisionReleaseRejectionV8 {
                    holder: self,
                    diagnostic: rejected("effect cleanup authority changed"),
                });
            }
            let Some(Value::Variant(value)) = self.ready.staged.decision.as_mut() else {
                unreachable!()
            };
            let value = Arc::get_mut(value).expect("checked exclusive Decision");
            drop(
                value
                    .fields
                    .remove(&action.source.projections[1])
                    .expect("checked actual Decision leaf"),
            );
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_err() {
                observations_succeeded = false;
            }
            if !effect_current(creator, &mut current) {
                return Err(OwnedEffectDecisionReleaseRejectionV8 {
                    holder: self,
                    diagnostic: rejected("effect cleanup authority changed"),
                });
            }
        }
        drop(self.ready.staged.decision.take());
        Ok(OwnedEffectDecisionReleaseV8 {
            holder: self,
            operations: actions,
            observations_succeeded,
        })
    }
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
    let mut roots = vec![staged.state.root.as_ref().expect("retained State")];
    if let Some(decision) = staged.decision.as_ref() {
        roots.push(decision);
    }
    if !staged.state.allocations.validate(&roots) {
        return Err(OwnedAuthorizeSettlementRejectionV2 {
            staged,
            diagnostic: rejected("authorize allocation witnesses differ"),
        });
    }
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

#[cfg(test)]
#[path = "authorize/effect_tests.rs"]
pub(super) mod effect_tests;
