//! Private checked borrowed request and one-use actual model Intent boundary.
use super::*;
use crate::execution_revision::typed::AgentRuntimeV2;
use crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::LiveParkedStateV8;
use crate::live_invocation::source_journal::LiveModelIntentPermitV8;
use crate::resumable_effects::owned_frame::v2::{
    ordinary_state_bytes, CheckedOwnedWaitObservationV8,
};
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
pub(crate) struct CheckedOwnedModelRequestV8 {
    request: AdapterRequest,
    identity: SourceAttemptIdentity,
    binding: SourceModelBinding,
    response_limit: usize,
    reservation_units: i64,
}
impl CheckedOwnedModelRequestV8 {
    pub(crate) fn identity(&self) -> &SourceAttemptIdentity {
        &self.identity
    }
    fn matches(&self, adapter: &StreamingSourceProposalAdapter<'_>) -> Result<(), Vec<Diagnostic>> {
        adapter.validate_model_binding(&self.binding)?;
        if !adapter.ordinary_checkpoint_matches(&SourceProposalPolicy {
            deployment_binding: self.binding.digest(),
            response_limit: self.response_limit,
            reservation_units: self.reservation_units,
        }) {
            return Err(StreamingSourceProposalAdapter::refusal(
                "source.owned_wait_profile",
            ));
        }
        Ok(())
    }
}
pub(crate) struct OwnedModelDispatchV8<'j> {
    permit: LiveModelIntentPermitV8<'j>,
    result: SourceAdapterDispatch,
    usage: Option<(u64, u64, i64)>,
}
impl<'j> OwnedModelDispatchV8<'j> {
    pub(crate) fn into_parts(self) -> (LiveModelIntentPermitV8<'j>, OwnedModelSettlementV8) {
        let result = match self.result {
            SourceAdapterDispatch::Settled {
                decoded,
                response,
                usage,
                ..
            } => OwnedModelSettlementV8::Settled {
                decoded,
                response,
                usage,
            },
            SourceAdapterDispatch::Failed {
                diagnostics,
                reason,
                attempted_bytes,
            } => OwnedModelSettlementV8::Failed {
                diagnostics,
                reason,
                attempted_bytes,
                usage: self.usage,
            },
        };
        (self.permit, result)
    }
}
pub(crate) enum OwnedModelSettlementV8 {
    Settled {
        decoded: DecodedProposal,
        response: Vec<u8>,
        usage: Option<(u64, u64, i64)>,
    },
    Failed {
        diagnostics: Vec<Diagnostic>,
        reason: SourceAttemptFailure,
        attempted_bytes: usize,
        usage: Option<(u64, u64, i64)>,
    },
}
enum OwnedModelRequestOriginV8<'p, 'j> {
    Initial {
        parked: &'p LiveParkedStateV8,
        observation: &'p CheckedOwnedWaitObservationV8,
    },
    Continued(
        &'p crate::live_invocation::source_journal::LiveContinuedModelRequestOriginV8<'p, 'j>,
    ),
}
impl OwnedModelRequestOriginV8<'_, '_> {
    fn checked_facts(
        &self,
        b: &crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8,
    ) -> Option<serde_json::Value> {
        match self {
            Self::Initial { parked, .. } => parked.checked_facts(b),
            Self::Continued(o) => o.checked_facts(b),
        }
    }
    fn request(&self) -> Option<&crate::interpreter::resumable::ResumableChannelValue> {
        match self {
            Self::Initial { parked, .. } => Some(parked.request()),
            Self::Continued(o) => o.request(),
        }
    }
    fn observation(&self) -> &CheckedOwnedWaitObservationV8 {
        match self {
            Self::Initial { observation, .. } => observation,
            Self::Continued(o) => o.observation(),
        }
    }
    fn prompt_coordinates(&self) -> Result<(u32, Option<Vec<u8>>), Vec<Diagnostic>> {
        match self {
            Self::Initial { .. } => Ok((0, None)),
            Self::Continued(o) => {
                let (_, previous) = o.coordinates().map_err(|_| {
                    StreamingSourceProposalAdapter::refusal("source.owned_wait_history")
                })?;
                Ok((o.turn(), previous))
            }
        }
    }
}
/// A fixed dispatcher accepts exactly one of these sealed actual Intent owners.
/// No external callback can implement this guard or substitute a boolean.
pub(super) enum OwnedModelGuardV8<'p, 'j> {
    Initial(&'p LiveModelIntentPermitV8<'j>),
    Continued(&'p crate::live_invocation::source_journal::LiveContinuedModelIntentPermitV8<'p, 'j>),
}
impl OwnedModelGuardV8<'_, '_> {
    pub(super) fn validate_guard(
        &self,
    ) -> Result<(), crate::live_invocation::source_journal::SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_guard(),
            Self::Continued(p) => p.validate_guard(),
        }
    }
    pub(super) fn validate_store(
        &self,
    ) -> Result<(), crate::live_invocation::source_journal::SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_store(),
            Self::Continued(p) => p.validate_store(),
        }
    }
    pub(super) fn guard_failure(&self) -> SourceAttemptFailure {
        match self {
            Self::Initial(p) => p.guard_failure(),
            Self::Continued(p) => p.guard_failure(),
        }
    }
    pub(super) fn quarantine(&self) {
        match self {
            Self::Initial(p) => p.quarantine(),
            Self::Continued(p) => p.quarantine(),
        }
    }
}
impl StreamingSourceProposalAdapter<'_> {
    pub(crate) fn checked_owned_model_request_v8(
        &self,
        runtime: &AgentRuntimeV2,
        execution: &CheckedTypedOwnedWaitExecutionV8,
        scope: &SourceCheckpointScope,
        parked: &LiveParkedStateV8,
        observation: &CheckedOwnedWaitObservationV8,
    ) -> Result<CheckedOwnedModelRequestV8, Vec<Diagnostic>> {
        self.checked_model_request_origin_v8(
            runtime,
            execution,
            scope,
            &OwnedModelRequestOriginV8::Initial {
                parked,
                observation,
            },
        )
    }
    pub(crate) fn checked_continued_model_request_v8(
        &self,
        runtime: &AgentRuntimeV2,
        execution: &CheckedTypedOwnedWaitExecutionV8,
        scope: &SourceCheckpointScope,
        origin: &crate::live_invocation::source_journal::LiveContinuedModelRequestOriginV8<'_, '_>,
    ) -> Result<CheckedOwnedModelRequestV8, Vec<Diagnostic>> {
        origin
            .validate_guard()
            .map_err(|_| Self::refusal("source.owned_wait_guard"))?;
        let result = self.checked_model_request_origin_v8(
            runtime,
            execution,
            scope,
            &OwnedModelRequestOriginV8::Continued(origin),
        );
        origin
            .validate_guard()
            .map_err(|_| Self::refusal("source.owned_wait_guard"))?;
        result
    }
    fn checked_model_request_origin_v8(
        &self,
        runtime: &AgentRuntimeV2,
        execution: &CheckedTypedOwnedWaitExecutionV8,
        scope: &SourceCheckpointScope,
        origin: &OwnedModelRequestOriginV8<'_, '_>,
    ) -> Result<CheckedOwnedModelRequestV8, Vec<Diagnostic>> {
        let observation = origin.observation();
        self.validate_model_binding(execution.model())?;
        let ordinary = execution.ordinary();
        if !self.ordinary_checkpoint_matches(&SourceProposalPolicy {
            deployment_binding: execution.model().digest(),
            response_limit: ordinary.response_limit(),
            reservation_units: ordinary.reservation_units(),
        }) {
            return Err(Self::refusal("source.owned_wait_profile"));
        }
        let task = runtime
            .owned_wait_task_v8(execution)
            .map_err(|_| Self::refusal("source.owned_wait_runtime"))?;
        let binding = execution.wait();
        let expected_invocation=crate::live_invocation::identity::digest(b"semaprax.live-invocation.source-id.v8\0",serde_json::to_string(&serde_json::json!({"execution":ordinary.invocation(),"owned_wait_binding":binding.binding()})).expect("inert JSON").as_bytes());
        if scope.program_root()!=binding.lifecycle().source_revision() || scope.invocation_id()!=expected_invocation
            || !observation.matches(binding.binding(),&serde_json::json!({"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()})) {return Err(Self::refusal("source.owned_wait_scope"));}
        let state = origin
            .checked_facts(binding)
            .ok_or_else(|| Self::refusal("source.owned_wait_state"))?;
        let actual = crate::resumable_effects::owned_frame::v2::bind_owned_wait_observation_v8(
            binding,
            scope,
            origin
                .request()
                .ok_or_else(|| Self::refusal("source.owned_wait_state"))?,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        if actual.copy_arguments() != observation.copy_arguments() {
            return Err(Self::refusal("source.owned_wait_observation"));
        }
        let state = ordinary_state_bytes(binding, &state)
            .map_err(|_| Self::refusal("source.owned_wait_state"))?;
        let schema = self.schema.schema().canonical_json();
        let max_request = execution.model().max_request_bytes();
        if schema.len() > max_request || task.objective.len() > max_request {
            return Err(Self::refusal("source.adapter_request_bound"));
        }
        let (turn, previous_effect) = origin.prompt_coordinates()?;
        let remaining_iterations = ordinary
            .max_iterations()
            .checked_sub(turn)
            .ok_or_else(|| Self::refusal("source.owned_wait_turn"))?;
        if remaining_iterations == 0 {
            return Err(Self::refusal("source.owned_wait_turn"));
        }
        let prompt = canonical_prompt_parts(PromptParts {
            task,
            source_revision: self.schema.source_revision(),
            turn: turn as usize,
            attempt: 0,
            remaining_iterations: remaining_iterations as usize,
            state: &state,
            observation: observation.ordinary_bytes(),
            previous_effect: previous_effect.as_deref(),
            previous_rejection: None,
            schema,
        });
        if prompt.len() > max_request {
            return Err(Self::refusal("source.adapter_request_bound"));
        }
        let request = AdapterRequest {
            request_bytes: prompt.into_bytes(),
            max_response_bytes: ordinary.response_limit(),
        };
        let digest = source_request_digest(&request.request_bytes);
        Ok(CheckedOwnedModelRequestV8 {
            identity: SourceAttemptIdentity {
                request_digest: digest.clone(),
                prompt_digest: digest,
                request_bytes: request.request_bytes.len(),
            },
            request,
            binding: execution.model().clone(),
            response_limit: ordinary.response_limit(),
            reservation_units: ordinary.reservation_units(),
        })
    }
    pub(crate) fn dispatch_owned_wait_v8<'j>(
        &mut self,
        permit: LiveModelIntentPermitV8<'j>,
    ) -> OwnedModelDispatchV8<'j> {
        self.last_dispatch = None;
        self.last_owned_usage = None;
        let clock = permit.clock();
        let live = OwnedModelGuardV8::Initial(&permit);
        let outcome = (|| {
            permit
                .validate_guard()
                .map_err(|_| Self::refusal("source.owned_wait_guard"))?;
            permit.request().matches(self)?;
            self.check_deadline_live_v8(Some(clock), Some(&live))?;
            let request = AdapterRequest {
                request_bytes: permit.request().request.request_bytes.clone(),
                max_response_bytes: permit.request().request.max_response_bytes,
            };
            self.propose_adapter_inner(request, Some(clock), false, Some(&live))
        })();
        if outcome.is_err() && self.last_dispatch.is_none() {
            let failure = if permit.guard_failure() == SourceAttemptFailure::Cancelled {
                SourceAttemptFailure::Cancelled
            } else if outcome.as_ref().err().is_some_and(|ds| {
                ds.iter()
                    .any(|d| d.message.contains("source.adapter_timeout"))
            }) {
                SourceAttemptFailure::Timeout
            } else {
                SourceAttemptFailure::Refused
            };
            self.dispatch_failure(failure, 0);
        }
        let result = self.finish_dispatch(outcome);
        OwnedModelDispatchV8 {
            permit,
            result,
            usage: self.last_owned_usage,
        }
    }
    pub(crate) fn dispatch_continued_wait_v8(
        &mut self,
        permit: &crate::live_invocation::source_journal::LiveContinuedModelIntentPermitV8<'_, '_>,
    ) -> OwnedModelSettlementV8 {
        self.last_dispatch = None;
        self.last_owned_usage = None;
        let live = OwnedModelGuardV8::Continued(permit);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            permit
                .validate_guard()
                .map_err(|_| Self::refusal("source.owned_wait_guard"))?;
            permit.request().matches(self)?;
            let clock = permit.clock();
            self.check_deadline_live_v8(Some(clock), Some(&live))?;
            let request = AdapterRequest {
                request_bytes: permit.request().request.request_bytes.clone(),
                max_response_bytes: permit.request().request.max_response_bytes,
            };
            self.propose_adapter_inner(request, Some(clock), false, Some(&live))
        }));
        let outcome = match outcome {
            Ok(v) => v,
            Err(_) => {
                permit.quarantine();
                Err(Self::refusal("source.owned_wait_callback_panic"))
            }
        };
        if outcome.is_err() && self.last_dispatch.is_none() {
            let failure = if permit.guard_failure() == SourceAttemptFailure::Cancelled {
                SourceAttemptFailure::Cancelled
            } else if outcome.as_ref().err().is_some_and(|ds| {
                ds.iter()
                    .any(|d| d.message.contains("source.adapter_timeout"))
            }) {
                SourceAttemptFailure::Timeout
            } else {
                SourceAttemptFailure::Refused
            };
            self.dispatch_failure(failure, 0);
        }
        match self.finish_dispatch(outcome) {
            SourceAdapterDispatch::Settled {
                decoded,
                response,
                usage,
                ..
            } => OwnedModelSettlementV8::Settled {
                decoded,
                response,
                usage,
            },
            SourceAdapterDispatch::Failed {
                diagnostics,
                reason,
                attempted_bytes,
            } => OwnedModelSettlementV8::Failed {
                diagnostics,
                reason,
                attempted_bytes,
                usage: self.last_owned_usage,
            },
        }
    }
}

#[cfg(test)]
mod tests;
