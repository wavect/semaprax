use super::*;

mod accounting;
mod admission;
mod deadline;
#[cfg(test)]
mod economic_tests;
mod evidence;
mod execution;
mod replay;
mod request;

#[cfg(test)]
pub(super) use accounting::replay_accounting_receipt;
use admission::reserve_parse_bound;
pub(super) use admission::{admit_profile, parse_task};
#[cfg(test)]
pub(super) use admission::{parse_profile, render_task};
use evidence::render_bundle;
#[cfg(test)]
pub(super) use evidence::{preflight_terminal_for_test, terminal_diagnostics_for_test};
use execution::run_bounded;
#[cfg(test)]
pub(super) use replay::{replay_evidence, replay_trace};

#[derive(Clone, Default, Eq, PartialEq)]
struct EvidenceBudget {
    used_models: u64,
    used_tools: u64,
    used_capabilities: u64,
    used_turns: u64,
    used_provider_attempts: u64,
    used_provider_input_bytes: u64,
    used_provider_output_bytes: u64,
    used_reported_model_input_tokens: u64,
    used_reported_model_output_tokens: u64,
    used_usd_microunits: u64,
    used_tool_calls: u64,
    used_tool_argument_bytes: u64,
    used_tool_result_bytes: u64,
    used_retained_state_bytes: u64,
    used_trace_events: u64,
    used_trace_bytes: u64,
    used_evidence_bytes: u64,
    used_builder_bytes: u64,
    used_elapsed_ms: u64,
    used_concurrency: u64,
}

#[derive(Clone)]
struct RunState {
    run_id: String,
    events: Vec<TraceEvent>,
    usage: Usage,
    history: Vec<(String, Option<String>)>,
    final_message: Option<String>,
    last_turn: u64,
    termination: Termination,
    task_digest: String,
    task_bytes: u64,
    task_nonce: String,
    external_effect_crossed: bool,
    provider_accounting: Vec<accounting::ProviderAccounting>,
}

pub(super) struct EvidenceReplay {
    state: RunState,
    budget: EvidenceBudget,
}

impl EvidenceReplay {
    pub(super) fn final_message(&self) -> Option<&str> {
        self.state.final_message.as_deref()
    }

    pub(super) fn run_id(&self) -> &str {
        &self.state.run_id
    }
}

struct Route {
    model_index: usize,
    request: String,
    request_digest: String,
    input_tokens: u64,
    output_token_reservation: u64,
    reserved_cost: u64,
}

#[cfg(test)]
pub(super) struct TestAgent<H: AgentHost>(Agent<H>);

#[cfg(test)]
impl<H: AgentHost> TestAgent<H> {
    pub(super) fn run(mut self, task: &str) -> Result<AgentRun, Vec<Diagnostic>> {
        self.0.run(task)
    }
}

#[cfg(test)]
pub(super) fn new_agent<H: AgentHost>(profile_source: &str, host: H) -> TestAgent<H> {
    TestAgent(Agent::new(profile_source, host, AgentCancellation::new()).unwrap())
}

#[cfg(test)]
pub(crate) fn completed_run_for_economic_test(message: &str) -> AgentRun {
    struct Probe;
    impl AgentBoundaryProbe for Probe {
        fn policy_epoch(&self) -> u64 {
            1
        }
        fn elapsed_ms(&self) -> u64 {
            0
        }
    }
    struct Host {
        response: Vec<u8>,
    }
    impl AgentHost for Host {
        fn policy_epoch(&self) -> u64 {
            1
        }
        fn elapsed_ms(&self) -> u64 {
            0
        }
        fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
            Box::new(Probe)
        }
        fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
            Some(request.len() as u64)
        }
        fn attempt_provider(
            &mut self,
            _: &str,
            _: &str,
            request: &str,
            _: u64,
            sink: &mut AgentProviderSink,
        ) -> AgentProviderAttempt {
            assert!(sink.push(&self.response));
            AgentProviderAttempt::new(
                AgentProviderDisposition::Succeeded,
                AgentProviderUsage::new(request.len() as u64, self.response.len() as u64, 0),
            )
        }
        fn invoke_tool(&mut self, _: &str, _: &str, _: &str, _: &mut AgentToolResultSink) -> bool {
            false
        }
    }
    let profile = Profile {
        agent_id: "economic.fixture.agent".to_owned(),
        models: vec![Model {
            provider_id: "fixture.local".to_owned(),
            model_id: "fixture-economic".to_owned(),
            locality: Locality::Local,
            quality_tier: QualityTier::Basic,
            tokenizer_id: "fixture.bytes-v1".to_owned(),
            max_context_tokens: 1_048_576,
            input_price: 0,
            output_price: 0,
            capabilities: vec!["text".to_owned()],
        }],
        tools: vec![],
        policy: Policy {
            allowed_provider_ids: vec!["fixture.local".to_owned()],
            allowed_model_ids: vec!["fixture-economic".to_owned()],
            required_locality: RequiredLocality::LocalOnly,
            minimum_quality_tier: QualityTier::Basic,
            required_model_capabilities: vec!["text".to_owned()],
            granted_capabilities: vec![],
            allowed_tool_ids: vec![],
        },
        limits: EffectiveLimits {
            max_turns: 1,
            max_provider_attempts: 1,
            max_retries_per_turn: 0,
            max_concurrency: 1,
            max_elapsed_ms: 10_000,
            max_provider_request_bytes: 2_097_152,
            max_provider_response_bytes: 1_048_576,
            max_stream_chunks: 4,
            max_total_provider_input_bytes: 2_097_152,
            max_total_provider_output_bytes: 1_048_576,
            max_reported_model_input_tokens: 2_097_152,
            max_reported_model_output_tokens: 262_144,
            max_usd_microunits: 0,
            max_tool_calls: 0,
            max_tool_arguments_bytes: 1,
            max_tool_result_bytes: 1,
            max_total_tool_bytes: 1,
            max_retained_state_bytes: 2_097_152,
            max_trace_events: 32,
            max_trace_bytes: 262_144,
            max_evidence_bytes: 2_097_152,
            max_builder_bytes: 67_108_864,
        },
        source: String::new(),
        digest: String::new(),
    };
    let profile_source = render_profile(&profile);
    let task = Task {
        nonce: "0".repeat(64),
        objective: "Return the exact economic proposal.".to_owned(),
        context: vec![],
        source: String::new(),
        digest: String::new(),
    };
    let task_source = render_task(&task);
    let response = format!(
        "{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"final\",\"message\":{}}}\n",
        quote_json(message)
    )
    .into_bytes();
    economic_tests::run(&profile_source, &task_source, Host { response })
}

impl<H: AgentHost> Agent<H> {
    /// Parses and owns one canonical Agent Runtime Profile before observing the host.
    pub fn new(
        profile_source: &str,
        host: H,
        cancellation: AgentCancellation,
    ) -> Result<Self, Vec<Diagnostic>> {
        let admitted = admit_profile(profile_source)?;
        Ok(Self {
            profile: admitted.profile,
            profile_builder_bytes: admitted.builder_bytes,
            host,
            cancellation,
        })
    }

    /// Runs one canonical task through the bounded injected-host state machine.
    pub fn run(&mut self, task_source: &str) -> Result<AgentRun, Vec<Diagnostic>> {
        let (result, overflowed, _) = with_limit_usage(MAX_BUILDER_BYTES, || {
            if !reserve_active(self.profile_builder_bytes as usize) {
                return Err(g208("builder_bytes", self.profile.limits.max_builder_bytes));
            }
            reserve_parse_bound(task_source)?;
            let task = parse_task(task_source)?;
            let parse_used = MAX_BUILDER_BYTES
                .saturating_sub(crate::bounded_output::active_remaining().unwrap_or(0))
                as u64;
            if parse_used > self.profile.limits.max_builder_bytes {
                return Err(g208("builder_bytes", self.profile.limits.max_builder_bytes));
            }
            let remaining = usize::try_from(self.profile.limits.max_builder_bytes - parse_used)
                .map_err(|_| g208("builder_bytes", self.profile.limits.max_builder_bytes))?;
            let (run, child_overflowed, child_used) = with_limit_usage(remaining, || {
                let admitted_policy_epoch = self.host.policy_epoch();
                let state = run_bounded(
                    &self.profile,
                    &mut self.host,
                    &self.cancellation,
                    admitted_policy_epoch,
                    task,
                )?;
                render_bundle(&self.profile, state, parse_used, remaining as u64)
            });
            let artifact = run?;
            let expected_builder_message = format!(
                "builder_bytes exceeds {}",
                self.profile.limits.max_builder_bytes
            );
            if child_overflowed
                && !(artifact.status == RunStatus::BudgetExhausted
                    && artifact.replay.state.termination.code == Some("SPX-G208")
                    && artifact.replay.state.termination.message.as_deref()
                        == Some(expected_builder_message.as_str()))
            {
                return Err(g208("builder_bytes", self.profile.limits.max_builder_bytes));
            }
            let sealed_builder_overflow = artifact.status == RunStatus::BudgetExhausted
                && artifact.replay.state.termination.code == Some("SPX-G208")
                && artifact.replay.state.termination.message.as_deref()
                    == Some(expected_builder_message.as_str());
            Ok((
                artifact,
                parse_used.saturating_add(child_used as u64),
                self.profile.limits.max_builder_bytes,
                sealed_builder_overflow,
            ))
        });
        let result = result.map_err(|diagnostic| vec![diagnostic])?;
        let outer_builder_message = format!("builder_bytes exceeds {}", result.2);
        if overflowed
            && !result.3
            && !(result.0.status == RunStatus::BudgetExhausted
                && result.0.replay.state.termination.code == Some("SPX-G208")
                && result.0.replay.state.termination.message.as_deref()
                    == Some(outer_builder_message.as_str()))
        {
            return Err(vec![g208("builder_bytes", MAX_BUILDER_BYTES as u64)]);
        }
        if result.1 > result.2
            && !(result.0.status == RunStatus::BudgetExhausted
                && result.0.replay.state.termination.code == Some("SPX-G208")
                && result.0.replay.state.termination.message.as_deref()
                    == Some(outer_builder_message.as_str()))
        {
            return Err(vec![g208("builder_bytes", result.2)]);
        }
        Ok(result.0)
    }
}
