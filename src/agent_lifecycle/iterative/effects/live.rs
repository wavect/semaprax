//! Live source-proposal entry for the checked typed-effect runner.
//!
//! Proposal acquisition remains outside the typed effect boundary. This
//! adapter receives only the source driver's accepted canonical proposal at
//! its `before_effect` boundary, then uses the same Dispatch as frozen input.

use super::*;
#[path = "live/model_wait.rs"]
mod model_wait;
pub(crate) mod owned_wait_v8;
use crate::agent_lifecycle::authorization::target_protocol::{
    self, TargetAccounting, TargetGrant, TargetHostHandler, TargetLimits, TargetOperation,
    TypedCarrier,
};
use crate::agent_lifecycle::iterative::driver::{EffectContext, IterativeDriver, ProposalSource};
use crate::agent_lifecycle::iterative::source_live::{
    PreparedSourceLiveMigration, SourceLiveFailure, SourceLiveOutcome, SourceLiveRequest,
};
use crate::agent_lifecycle::CheckpointStore;
use crate::live_invocation::source_journal::{SourceIoLimits, SourcePolicyBindingV6};
use serde_json::Value;

pub(super) fn target_backend_identity(
    backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
) -> String {
    match backend {
        crate::agent_lifecycle::authorization::StageBackend::Metered {
            backend,
            fuel_limit,
            ..
        } => {
            format!(
                "semantic-work-v1:{fuel_limit}:{}",
                target_backend_identity(*backend)
            )
        }
        crate::agent_lifecycle::authorization::StageBackend::Interpreter => "interpreter".into(),
        crate::agent_lifecycle::authorization::StageBackend::Native { host } => {
            format!("native:-O0:{}", host.identity())
        }
        crate::agent_lifecycle::authorization::StageBackend::NativeAtOptimization {
            host,
            optimization,
        } => {
            format!("native:{optimization}:{}", host.identity())
        }
        #[cfg(test)]
        crate::agent_lifecycle::authorization::StageBackend::Wasm { source } => format!(
            "core-wasm:test:{}",
            digest(
                b"semaprax.agent-target-stage-backend.wasm.v2\0",
                source.as_bytes(),
            )
        ),
        crate::agent_lifecycle::authorization::StageBackend::WasmHeld { host, source } => format!(
            "core-wasm:{}:{}",
            host.identity(),
            digest(
                b"semaprax.agent-target-stage-backend.wasm.v2\0",
                source.as_bytes(),
            )
        ),
    }
}

struct LiveDispatch<'a> {
    dispatch: Dispatch<'a>,
    proposal: Option<String>,
}

struct LiveEffectPlan {
    operation: String,
    request_digest: String,
    argument_bytes: usize,
}

/// Private #182 adapter for the already checked source-live typed-effect
/// route.  It has no host discovery or provider construction: the only host
/// is the caller-injected protocol handler, and it sees the protocol's closed
/// request rather than a lifecycle or source handle.
struct TargetLiveDispatch<'a> {
    compiled: &'a CompiledTypedEffects,
    handler: &'a mut dyn TargetHostHandler,
    limits: TargetLimits,
    accounting: TargetAccounting,
    evidence: Vec<target_protocol::TargetEvidence>,
    failure: Option<&'static str>,
    execution_binding: Option<String>,
}

impl TargetLiveDispatch<'_> {
    fn carrier_type(operation: &EffectOperation, role: &str) -> String {
        let fields = match role {
            "argument" => operation
                .arguments
                .iter()
                .map(|field| format!("{}:{}", field.argument_id, field.kind.name()))
                .collect::<Vec<_>>(),
            "result" => operation
                .results
                .iter()
                .map(|field| format!("{}:{}", field.result_id, field.kind.name()))
                .collect::<Vec<_>>(),
            _ => unreachable!("closed target carrier role"),
        };
        let identity = digest(
            b"semaprax.agent-typed-effect.target-carrier.v1\0",
            format!(
                "{}\0{}\0{}\0{}",
                operation.operation_id,
                operation.effect_id,
                role,
                fields.join("\0")
            )
            .as_bytes(),
        );
        let suffix = identity.strip_prefix("sha256:").unwrap_or(&identity);
        format!("semaprax.agent-typed-effect.{role}.{suffix}")
    }

    fn planned_call(
        &self,
        proposal: &str,
        projected: &[RetainedValue],
    ) -> Result<(usize, TargetOperation, TypedCarrier), Vec<Diagnostic>> {
        let lifecycle = &self.compiled.lifecycle;
        let decoded = lifecycle
            .inner
            .proposal
            .decode(proposal)
            .map_err(|_| error("target.proposal_decode"))?;
        let Some(ProposalValue::Unsigned(selector)) = decoded.field(&self.compiled.selector) else {
            return Err(error("target.selector_type"));
        };
        let index = usize::try_from(*selector).map_err(|_| error("target.selector_range"))?;
        owned_wait_v8::planned_call_projected(self.compiled, index, projected)
    }

    fn accepted_result(&self, index: usize, payload: &[u8]) -> Option<Vec<u8>> {
        owned_wait_v8::accepted_result(self.compiled, index, payload)
    }
}

impl IterativeDriver for TargetLiveDispatch<'_> {
    fn read(&mut self, _: &AuthorizedRequest) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
        Err(error("target.read_bypass"))
    }

    fn target_effect(
        &mut self,
        context: crate::agent_lifecycle::iterative::driver::TargetEffectContext<'_>,
        authorization: Authorized,
    ) -> Result<crate::agent_lifecycle::iterative::driver::TargetEffect, Vec<Diagnostic>> {
        let (index, operation, argument) =
            self.planned_call(context.proposal_canonical, context.projected)?;
        let turn = u64::try_from(context.turn).map_err(|_| error("target.turn"))?;
        // Source-stage evaluator steps are already bounded by the outer
        // lifecycle. The target protocol meters one additional host-call work
        // unit here; treating the stage-step ceiling as grant spend would
        // conflate two independent budgets and refuse every ordinary grant.
        let fuel = u64::from(context.max_steps > 0);
        let grant = TargetGrant::bind(
            authorization,
            context.invocation_root,
            self.execution_binding.as_deref(),
            turn,
            operation,
            &argument,
        )
        .map_err(|_| error("target.grant"))?;
        let dispatched = target_protocol::dispatch(
            grant,
            argument,
            fuel,
            self.limits,
            &mut self.accounting,
            context.cancellation,
            self.handler,
        );
        let settlement = dispatched.evidence().settlement();
        self.evidence.push(dispatched.evidence().clone());
        let result = dispatched
            .result()
            .and_then(|carrier| self.accepted_result(index, carrier.payload()));
        if settlement != target_protocol::Settlement::Returned {
            self.failure = Some(settlement.text());
        } else if dispatched.result().is_some() && result.is_none() {
            self.failure = Some("target_result_shape");
        }
        Ok(
            crate::agent_lifecycle::iterative::driver::TargetEffect::Dispatched {
                dispatch: dispatched,
                result,
            },
        )
    }
}

impl LiveDispatch<'_> {
    fn effect_plan(
        &self,
        authorization: &AuthorizedRequest,
    ) -> Result<LiveEffectPlan, Vec<Diagnostic>> {
        let source = self
            .proposal
            .as_deref()
            .ok_or_else(|| error("live.effect_context"))?;
        let lifecycle = &self.dispatch.compiled.lifecycle;
        let decoded = lifecycle
            .inner
            .proposal
            .decode(source)
            .map_err(|_| error("live.proposal_decode"))?;
        let Some(ProposalValue::Unsigned(selector)) =
            decoded.field(&self.dispatch.compiled.selector)
        else {
            return Err(error("live.selector_type"));
        };
        let index = usize::try_from(*selector).map_err(|_| error("live.selector_range"))?;
        let operation = self
            .dispatch
            .compiled
            .operations
            .get(index)
            .ok_or_else(|| error("live.selector_range"))?;
        let projected = lifecycle
            .inner
            .project(&decoded)
            .ok_or_else(|| error("live.projection"))?;
        let mut arguments = Vec::new();
        for argument in &operation.arguments {
            let field = lifecycle
                .inner
                .binding
                .proposal
                .iter()
                .position(|value| value.field.as_str() == argument.proposal_field_id)
                .ok_or_else(|| error("live.argument_identity"))?;
            let value = projected
                .get(field)
                .ok_or_else(|| error("live.argument_index"))?;
            if !argument.kind.accepts(value) {
                return Err(error("live.argument_type"));
            }
            arguments.push((argument.argument_id.clone(), value.clone()));
        }
        for ((_, value), limit) in arguments
            .iter()
            .zip(&self.dispatch.compiled.field_limits[index].0)
        {
            if scalar_bytes(value).is_none_or(|size| size > *limit) {
                return Err(error("live.argument_field_budget"));
            }
        }
        let encoded_arguments = encode_fields(&arguments);
        let results = operation
            .results
            .iter()
            .map(|result| {
                format!(
                    "[{},{}]",
                    quote_json(&result.result_id),
                    quote_json(result.kind.name())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let identity = format!(
            "{{\"schema\":\"semaprax.source-typed-effect-request.v1\",\"registry\":{},\"operation\":{},\"effect\":{},\"authorization\":{},\"budget\":{},\"seal\":{},\"arguments\":{},\"results\":[{}]}}",
            quote_json(self.dispatch.compiled.digest()),
            quote_json(&operation.operation_id),
            quote_json(&operation.effect_id),
            quote_json(authorization.binding()),
            authorization.budget(),
            crate::agent_lifecycle::canonical_retained_value_json(&RetainedValue::Bytes(
                authorization.seal().to_vec(),
            )),
            encoded_arguments,
            results,
        );
        Ok(LiveEffectPlan {
            operation: operation.operation_id.clone(),
            request_digest: digest(
                b"semaprax.source-typed-effect-request.v1\0",
                identity.as_bytes(),
            ),
            argument_bytes: encoded_arguments.len(),
        })
    }

    fn restore_replayed_effect(
        &mut self,
        authorization: &AuthorizedRequest,
        observation: Option<&[u8]>,
    ) -> Result<(), Vec<Diagnostic>> {
        let plan = self.effect_plan(authorization)?;
        if self.dispatch.dispatched >= self.dispatch.budget.max_calls {
            return Err(error("live.replay_call_budget"));
        }
        let arguments = self
            .dispatch
            .arguments
            .checked_add(plan.argument_bytes)
            .ok_or_else(|| error("live.replay_argument_overflow"))?;
        if plan.argument_bytes > self.dispatch.budget.max_argument_bytes
            || arguments
                .checked_add(self.dispatch.results)
                .is_none_or(|total| total > self.dispatch.budget.max_total_bytes)
        {
            return Err(error("live.replay_argument_budget"));
        }
        let result_bytes = observation.map_or(0, <[u8]>::len);
        let results = self
            .dispatch
            .results
            .checked_add(result_bytes)
            .ok_or_else(|| error("live.replay_result_overflow"))?;
        if result_bytes > self.dispatch.budget.max_result_bytes
            || arguments
                .checked_add(results)
                .is_none_or(|total| total > self.dispatch.budget.max_total_bytes)
        {
            return Err(error("live.replay_result_budget"));
        }
        self.dispatch.dispatched += 1;
        self.dispatch.arguments = arguments;
        self.dispatch.results = results;
        self.proposal = None;
        Ok(())
    }
}

impl IterativeDriver for LiveDispatch<'_> {
    fn before_effect(&mut self, context: EffectContext<'_>) -> Result<(), Vec<Diagnostic>> {
        self.proposal = Some(context.proposal_canonical.to_owned());
        Ok(())
    }

    fn source_effect_identity(
        &mut self,
        authorization: &AuthorizedRequest,
    ) -> Result<Option<(String, String)>, Vec<Diagnostic>> {
        let plan = self.effect_plan(authorization)?;
        Ok(Some((plan.operation, plan.request_digest)))
    }

    fn validate_replayed_read(
        &mut self,
        authorization: &AuthorizedRequest,
        observation: Option<&[u8]>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.restore_replayed_effect(authorization, observation)
    }

    fn read(
        &mut self,
        authorization: &AuthorizedRequest,
    ) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
        let proposal = self
            .proposal
            .take()
            .ok_or_else(|| error("live.effect_context"))?;
        match self.dispatch.invoke_proposal(authorization, &proposal) {
            Ok(value) => Ok(Some(value)),
            Err(reason) => {
                self.dispatch.failure = Some(reason);
                Ok(None)
            }
        }
    }
}

impl CompiledTypedEffects {
    /// Execute the checked source-live typed-effect route through the #182
    /// target protocol. The supplied handler is the sole target host;
    /// this method neither discovers a provider nor serializes a grant.
    pub fn run_target_live(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TargetHostHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
    ) -> Result<TargetEffectRun, Vec<Diagnostic>> {
        self.run_target_live_inner(
            task,
            source,
            handler,
            stages,
            effects,
            cancellation,
            crate::agent_lifecycle::authorization::StageBackend::Interpreter,
            None,
        )
    }

    /// Execute deterministic Agent stages on one admitted public backend
    /// while retaining the same proposal source, target host protocol,
    /// authorization ordering, budgets, settlement, and replay evidence.
    ///
    /// `CoreWasm` never accepts caller-supplied source bytes. It lowers the
    /// exact checked source retained when this value was compiled. A linked
    /// project lifecycle currently has no canonical single-module Wasm source,
    /// so selecting `CoreWasm` for one is refused before proposal or host work.
    pub fn run_target_live_with_backend(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TargetHostHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        selected: TargetStageBackend<'_>,
    ) -> Result<TargetEffectRun, Vec<Diagnostic>> {
        let backend = self.selected_target_backend(selected)?;
        let execution_binding = self.target_execution_binding(backend);
        self.run_target_live_inner(
            task,
            source,
            handler,
            stages,
            effects,
            cancellation,
            backend,
            Some(execution_binding),
        )
    }

    pub(super) fn selected_target_backend<'a>(
        &'a self,
        selected: TargetStageBackend<'a>,
    ) -> Result<crate::agent_lifecycle::authorization::StageBackend<'a>, Vec<Diagnostic>> {
        Ok(match selected {
            TargetStageBackend::Interpreter => {
                crate::agent_lifecycle::authorization::StageBackend::Interpreter
            }
            TargetStageBackend::Native(host) => {
                crate::agent_lifecycle::authorization::StageBackend::Native { host: &host.host }
            }
            #[cfg(test)]
            TargetStageBackend::CoreWasm => {
                let source = self
                    .target_source
                    .as_deref()
                    .ok_or_else(|| error("target_backend.core_wasm_source"))?;
                crate::agent_lifecycle::authorization::StageBackend::Wasm { source }
            }
            TargetStageBackend::CoreWasmHeld(host) => {
                let source = self
                    .target_source
                    .as_deref()
                    .ok_or_else(|| error("target_backend.core_wasm_source"))?;
                crate::agent_lifecycle::authorization::StageBackend::WasmHeld {
                    host: &host.host,
                    source,
                }
            }
        })
    }

    /// Local parity-only entry. It does not select a production target: the
    /// caller must supply one sealed stage executor backend and an explicitly
    /// injected host handler, while the target protocol remains unchanged.
    #[cfg(test)]
    pub(in crate::agent_lifecycle) fn run_target_live_on(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TargetHostHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    ) -> Result<TargetEffectRun, Vec<Diagnostic>> {
        let execution_binding = self.target_execution_binding(backend);
        self.run_target_live_inner(
            task,
            source,
            handler,
            stages,
            effects,
            cancellation,
            backend,
            Some(execution_binding),
        )
    }

    pub(super) fn target_execution_binding(
        &self,
        backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    ) -> String {
        let backend = target_backend_identity(backend);
        digest(
            b"semaprax.agent-target-stage-execution-binding.v1\0",
            format!("{}\0{}", self.digest(), backend).as_bytes(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_target_live_inner(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TargetHostHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
        execution_binding: Option<String>,
    ) -> Result<TargetEffectRun, Vec<Diagnostic>> {
        let effects = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let max_calls = u64::try_from(effects.max_calls).map_err(|_| error("target.calls"))?;
        let limits = TargetLimits {
            max_calls,
            // Target protocol meters its complete framed request/result wire,
            // including the nominal carrier identities and request metadata.
            max_request_bytes: u64::try_from(effects.max_argument_bytes)
                .map_err(|_| error("target.request_budget"))?,
            max_result_bytes: u64::try_from(effects.max_result_bytes)
                .map_err(|_| error("target.result_budget"))?,
            max_total_bytes: u64::try_from(effects.max_total_bytes)
                .map_err(|_| error("target.total_budget"))?,
            max_fuel: max_calls,
        };
        let mut dispatch = TargetLiveDispatch {
            compiled: self,
            handler,
            limits,
            accounting: TargetAccounting::default(),
            evidence: Vec::new(),
            failure: None,
            execution_binding,
        };
        let stages = IterativeBudget {
            max_iterations: stages.max_iterations.min(self.max_iterations),
            ..stages
        };
        // Cancellation is the first target-stage settlement check. In
        // particular, a pre-cancelled invocation carrying an otherwise
        // invalid (or arithmetic-overflowing) stage-fuel request must settle
        // as cancellation without admitting a compiler, Node, proposal, or
        // host effect. For every non-cancelled run, reuse the retained-call
        // interval before converting or multiplying it, so all selectors
        // reject the exact same invalid fuel range.
        let cancelled_before_stage_accounting = cancellation.is_cancelled();
        if !cancelled_before_stage_accounting {
            crate::interpreter::retained_call::validate_step_limit(stages.max_steps_per_stage)?;
        }
        let per_stage_limit = u64::try_from(stages.max_steps_per_stage)
            .map_err(|_| error("target.stage_work.per_stage"))?;
        let run_stage_limit =
            u64::try_from(stages.max_stages).map_err(|_| error("target.stage_work.stages"))?;
        // This is an admission envelope, not a synthetic instruction count:
        // every selector receives the same checked stage cap, and each
        // recorded stage reserves that cap before any later work can run.
        let max_reserved_steps = if cancelled_before_stage_accounting {
            // No target stage is admitted after a pre-dispatch cancellation,
            // so there is no reservation to multiply or overflow.
            0
        } else {
            per_stage_limit
                .checked_mul(run_stage_limit)
                .ok_or_else(|| error("target.stage_work.capacity"))?
        };
        let lifecycle = self
            .lifecycle
            .run_with_target_driver_live_on(
                task,
                source,
                &mut dispatch,
                stages,
                cancellation,
                backend,
            )
            .map_err(crate::agent_lifecycle::iterative::driver::DriverFailure::into_diagnostics)?;
        let recorded_stages = u64::try_from(lifecycle.stages().len())
            .map_err(|_| error("target.stage_work.recorded"))?;
        let reserved_steps = if cancelled_before_stage_accounting {
            0
        } else {
            per_stage_limit
                .checked_mul(recorded_stages)
                .ok_or_else(|| error("target.stage_work.reservation"))?
        };
        if recorded_stages > run_stage_limit
            || (!cancelled_before_stage_accounting && reserved_steps > max_reserved_steps)
        {
            return Err(error("target.stage_work.bound"));
        }
        let stage_work = TargetStageWork {
            per_stage_limit,
            run_stage_limit,
            recorded_stages,
            reserved_steps,
        };
        let evidence = format!(
            "{{\"schema\":\"semaprax.agent-target-effects-evidence.v1\",\"registry\":{},\"lifecycle_evidence\":{},\"limits\":[{},{},{},{},{}],\"stage_work\":[{},{},{},{}],\"accounting\":[{},{},{},{}],\"target_evidence\":[{}],\"failure\":{}}}\n",
            quote_json(self.digest()),
            quote_json(lifecycle.evidence_digest()),
            limits.max_calls,
            limits.max_request_bytes,
            limits.max_result_bytes,
            limits.max_total_bytes,
            limits.max_fuel,
            stage_work.per_stage_limit(),
            stage_work.run_stage_limit(),
            stage_work.recorded_stages(),
            stage_work.reserved_steps(),
            dispatch.accounting.calls(),
            dispatch.accounting.request_bytes(),
            dispatch.accounting.result_bytes(),
            dispatch.accounting.fuel(),
            dispatch.evidence.iter().map(|evidence| quote_json(evidence.digest())).collect::<Vec<_>>().join(","),
            dispatch.failure.map(quote_json).unwrap_or_else(|| "null".into()),
        );
        let digest = digest(
            b"semaprax.agent-target-effects-evidence.v1\0",
            evidence.as_bytes(),
        );
        Ok(TargetEffectRun {
            lifecycle,
            accounting: dispatch.accounting,
            stage_work,
            target_evidence: dispatch.evidence,
            failure: dispatch.failure,
            evidence,
            digest,
        })
    }

    /// Execute with an injected source proposal stream instead of a submitted
    /// proposal inventory. The lifecycle still authorizes before `Dispatch`
    /// can invoke any typed effect.
    pub fn run_live(
        &self,
        task: &LifecycleTask,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
    ) -> Result<TypedEffectRun, Vec<Diagnostic>> {
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let mut dispatch = LiveDispatch {
            dispatch: Dispatch {
                compiled: self,
                proposals: &[],
                handler,
                budget,
                dispatched: 0,
                arguments: 0,
                results: 0,
                failure: None,
            },
            proposal: None,
        };
        let stages = IterativeBudget {
            max_iterations: stages.max_iterations.min(self.max_iterations),
            ..stages
        };
        let lifecycle = self
            .lifecycle
            .run_with_driver_live(task, source, &mut dispatch, stages, cancellation)
            .map_err(crate::agent_lifecycle::iterative::driver::DriverFailure::into_diagnostics)?;
        let evidence = format!(
            "{{\"schema\":\"semaprax.agent-typed-effects-evidence.v3\",\"registry\":{},\"lifecycle_evidence\":{},\"limits\":[{},{},{},{}],\"dispatched\":{},\"argument_bytes\":{},\"result_bytes\":{},\"failure\":{}}}\n",
            quote_json(self.digest()),
            quote_json(lifecycle.evidence_digest()),
            budget.max_calls,
            budget.max_argument_bytes,
            budget.max_result_bytes,
            budget.max_total_bytes,
            dispatch.dispatch.dispatched,
            dispatch.dispatch.arguments,
            dispatch.dispatch.results,
            dispatch.dispatch.failure.map(quote_json).unwrap_or_else(|| "null".into())
        );
        Ok(TypedEffectRun {
            lifecycle,
            dispatched: dispatch.dispatch.dispatched,
            argument_bytes: dispatch.dispatch.arguments,
            result_bytes: dispatch.dispatch.results,
            failure: dispatch.dispatch.failure,
            digest: digest(
                b"semaprax.agent-typed-effects-evidence.v3\0",
                evidence.as_bytes(),
            ),
            evidence,
        })
    }

    /// Durable live proposals use the existing Source Live Journal v2 cursor.
    /// The typed registry remains the sole effect dispatcher; this does not
    /// construct the frozen-operation checkpoint profile.
    pub(crate) fn run_live_durable_source(
        &self,
        request: SourceLiveRequest<'_>,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TypedEffectHandler,
        effects: EffectBudget,
        store: &mut dyn CheckpointStore,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let mut dispatch = LiveDispatch {
            dispatch: Dispatch {
                compiled: self,
                proposals: &[],
                handler,
                budget,
                dispatched: 0,
                arguments: 0,
                results: 0,
                failure: None,
            },
            proposal: None,
        };
        self.lifecycle
            .run_live_durable_with_driver(request, source, &mut dispatch, store)
    }

    /// V6 durable source route with an explicit, host-validated model policy.
    /// It keeps the existing typed-effect dispatcher and one source cursor.
    pub(crate) fn run_live_durable_source_with_model_policy(
        &self,
        request: SourceLiveRequest<'_>,
        policy: SourcePolicyBindingV6,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TypedEffectHandler,
        effects: EffectBudget,
        store: &mut dyn CheckpointStore,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let mut dispatch = LiveDispatch {
            dispatch: Dispatch {
                compiled: self,
                proposals: &[],
                handler,
                budget,
                dispatched: 0,
                arguments: 0,
                results: 0,
                failure: None,
            },
            proposal: None,
        };
        self.lifecycle.run_live_durable_with_model_policy(
            request,
            policy,
            source,
            &mut dispatch,
            store,
        )
    }

    /// V6 model policy and V5 cumulative I/O limits share this typed-effect
    /// dispatcher and one journal cursor.
    pub(crate) fn run_live_durable_source_with_model_policy_and_io_limits(
        &self,
        request: SourceLiveRequest<'_>,
        policy: SourcePolicyBindingV6,
        limits: &SourceIoLimits,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TypedEffectHandler,
        effects: EffectBudget,
        store: &mut dyn CheckpointStore,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let mut dispatch = LiveDispatch {
            dispatch: Dispatch {
                compiled: self,
                proposals: &[],
                handler,
                budget,
                dispatched: 0,
                arguments: 0,
                results: 0,
                failure: None,
            },
            proposal: None,
        };
        self.lifecycle
            .run_live_durable_with_model_policy_and_io_limits(
                request,
                policy,
                limits,
                source,
                &mut dispatch,
                store,
            )
    }

    /// Continues an already checked Source Live migration with the same typed
    /// effect identity and replay validation as the direct durable route.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_prepared_source_migration(
        &self,
        prepared: PreparedSourceLiveMigration<'_>,
        source: &mut dyn ProposalSource,
        handler: &mut dyn TypedEffectHandler,
        effects: EffectBudget,
        store: &mut dyn CheckpointStore,
        clock: &dyn crate::live_invocation::SourceInvocationClock,
        cancellation: &AgentCancellation,
    ) -> Result<SourceLiveOutcome, SourceLiveFailure> {
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let mut dispatch = LiveDispatch {
            dispatch: Dispatch {
                compiled: self,
                proposals: &[],
                handler,
                budget,
                dispatched: 0,
                arguments: 0,
                results: 0,
                failure: None,
            },
            proposal: None,
        };
        prepared.run_with_driver(source, &mut dispatch, store, clock, cancellation)
    }
}
