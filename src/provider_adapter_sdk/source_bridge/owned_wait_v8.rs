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
impl StreamingSourceProposalAdapter<'_> {
    pub(crate) fn checked_owned_model_request_v8(
        &self,
        runtime: &AgentRuntimeV2,
        execution: &CheckedTypedOwnedWaitExecutionV8,
        scope: &SourceCheckpointScope,
        parked: &LiveParkedStateV8,
        observation: &CheckedOwnedWaitObservationV8,
    ) -> Result<CheckedOwnedModelRequestV8, Vec<Diagnostic>> {
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
        let state = parked
            .checked_facts(binding)
            .ok_or_else(|| Self::refusal("source.owned_wait_state"))?;
        let actual = crate::resumable_effects::owned_frame::v2::bind_owned_wait_observation_v8(
            binding,
            scope,
            parked.request(),
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
        let prompt = canonical_prompt_parts(PromptParts {
            task,
            source_revision: self.schema.source_revision(),
            turn: 0,
            attempt: 0,
            remaining_iterations: ordinary.max_iterations() as usize,
            state: &state,
            observation: observation.ordinary_bytes(),
            previous_effect: None,
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
        let outcome = (|| {
            permit
                .validate_guard()
                .map_err(|_| Self::refusal("source.owned_wait_guard"))?;
            permit.request().matches(self)?;
            self.check_deadline_live_v8(Some(clock), Some(&permit))?;
            let request = AdapterRequest {
                request_bytes: permit.request().request.request_bytes.clone(),
                max_response_bytes: permit.request().request.max_response_bytes,
            };
            self.propose_adapter_inner(request, Some(clock), false, Some(&permit))
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
}

#[cfg(test)]
mod tests;
