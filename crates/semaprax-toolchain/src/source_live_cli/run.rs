use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::opencode_host::source::OpenCodeDurableProposalSource;
use crate::opencode_host::{
    OpenCodeGrammar, OpenCodeHostConfig, OpenCodeModelHandler, OpenCodeRunner,
    ProcessOpenCodeRunner, OPENCODE_MODEL,
};
use semaprax::agent_lifecycle::iterative::source_live::{
    prepare_source_live_migration, prepare_source_live_migration_from_hot_reload_handoff,
    prepare_source_live_migration_with_io_limits, prepare_source_live_priced_migration,
    run_source_live_migration_from_hot_reload_session, SourceIoLimits, SourceLiveFailure,
    SourceLiveMigrationEndpoint, SourceLiveMigrationRequest, SourceLiveOutcome, SourceLivePolicy,
    SourceLiveRequest,
};
use semaprax::agent_lifecycle::iterative::{
    compile_project_agent_lifecycle_v2, CompiledIterativeLifecycle, IterativeBudget, IterativeRun,
};
use semaprax::agent_lifecycle::{AgentReadOperation, AuthorizedRequest, LifecycleTask};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::digest_hex::LowerHex;
use semaprax::live_invocation::source_journal::{
    recover_source_checkpoint, RecoveredSourceCheckpoint, SourceInvocationBinding,
    MAX_SOURCE_EFFECT_BYTES, MAX_SOURCE_REQUEST_BYTES,
};
use semaprax::live_invocation::{InvocationClock, ModelInvokeCapability, SourceInvocationClock};
use semaprax::project::{
    with_authenticated_project, HotReloadPlan, HotReloadSession, ProjectRevision,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::checkpoint::{bounded_read, CheckpointDir};
use super::options::{Command, SessionConfig};
use super::CliError;

const CLOCK_DOMAIN: &str = "unix_epoch_millis.v1";
const UNIT: &str = "fixed_model_attempt_units.v1";
const MAX_ONE_PROVIDER_CALL_MS: i64 = 30_000;

struct UnixClock;
impl InvocationClock for UnixClock {
    fn now_millis(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|duration| i64::try_from(duration.as_millis()).ok())
            .unwrap_or(i64::MIN)
    }
}
impl SourceInvocationClock for UnixClock {
    fn clock_domain(&self) -> &str {
        CLOCK_DOMAIN
    }
}

struct ReadSnapshot(Vec<u8>);
impl AgentReadOperation for ReadSnapshot {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        Some(self.0.clone())
    }
}

struct Endpoint {
    project_root: PathBuf,
    project: Arc<ProjectRevision>,
    compiled: CompiledIterativeLifecycle,
    config: SessionConfig,
    task: LifecycleTask,
    read: Vec<u8>,
    budget: IterativeBudget,
    policy: SourceLivePolicy,
}

impl Endpoint {
    fn load(config: SessionConfig) -> Result<Self, CliError> {
        let manifest = config
            .manifest
            .canonicalize()
            .map_err(|_| CliError::refused("Project manifest is unavailable"))?;
        let project_root = manifest
            .parent()
            .ok_or(CliError::refused("Project manifest has no root"))?
            .to_owned();
        let project =
            with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
                .map_err(|_| CliError::refused("Project authentication refused"))?;
        Self::from_authenticated(config, project_root, project)
    }

    /// Reuses the hot-reload supervisor's retained checked predecessor. The
    /// config still provides every host-selected file and policy input; only
    /// the source revision comes from the opaque in-process supervisor.
    fn load_retained(
        config: SessionConfig,
        project: Arc<ProjectRevision>,
    ) -> Result<Self, CliError> {
        let manifest = config
            .manifest
            .canonicalize()
            .map_err(|_| CliError::refused("Project manifest is unavailable"))?;
        let project_root = manifest
            .parent()
            .ok_or(CliError::refused("Project manifest has no root"))?
            .to_owned();
        Self::from_authenticated(config, project_root, project)
    }

    fn from_authenticated(
        config: SessionConfig,
        project_root: PathBuf,
        project: Arc<ProjectRevision>,
    ) -> Result<Self, CliError> {
        let compiled = compile_project_agent_lifecycle_v2(
            &project,
            &config.source_path,
            &config.agent_id,
            &config.step_id,
        )
        .map_err(|_| CliError::refused("checked Project Agent compilation refused"))?;
        let program_root = project
            .program_root()
            .map_err(|_| CliError::refused("Project ProgramRoot is unavailable"))?
            .program_root_digest()
            .to_owned();
        let objective = bounded_read(&config.task_path, MAX_SOURCE_REQUEST_BYTES)?;
        let read = bounded_read(&config.read_path, MAX_SOURCE_EFFECT_BYTES)?;
        let task = LifecycleTask {
            objective,
            budget: config.task_budget,
        };
        let budget = IterativeBudget {
            max_iterations: config.max_iterations,
            max_stages: config.max_stages,
            max_steps_per_stage: config.max_steps_per_stage,
        };
        let policy = SourceLivePolicy {
            deployment_binding: deployment_binding(&read),
            response_limit: config.response_limit,
            ceiling: config.ceiling,
            reservation_units: config.reservation_units,
            unit: UNIT.into(),
            clock_domain: CLOCK_DOMAIN.into(),
            initial_millis: 0,
            deadline_millis: config.deadline_millis,
            max_total_steps: config.max_total_steps,
            program_root: Some(program_root),
        };
        match config.io_limits.as_ref() {
            Some(io_limits) => policy.binding_with_io_limits(
                &compiled,
                &task,
                budget,
                config.pricing.as_ref(),
                io_limits,
            ),
            None => match config.pricing.as_ref() {
                Some(pricing) => policy.binding_priced(&compiled, &task, budget, pricing),
                None => policy.binding(&compiled, &task, budget),
            },
        }
        .map_err(|_| CliError::refused("source execution policy refused"))?;
        Ok(Self {
            project_root,
            project,
            compiled,
            config,
            task,
            read,
            budget,
            policy,
        })
    }

    /// Checks the retained migrated binding against the current checked
    /// predecessor and host-selected policy before it can authenticate a
    /// predecessor journal. The carried migration profile stays opaque.
    fn matches_retained_binding(&self, binding: &SourceInvocationBinding) -> bool {
        binding.matches_proposal_source(
            self.compiled.source_revision(),
            &self.policy.deployment_binding,
            &self.task.objective,
            self.task.budget,
            self.compiled.proposal_schema().schema().digest(),
        ) && binding.response_limit() == self.policy.response_limit
            && binding.ceiling() == self.policy.ceiling
            && binding.reservation_units() == self.policy.reservation_units
            && binding.unit() == self.policy.unit
            && binding.clock_domain() == self.policy.clock_domain
            && binding.deadline_millis() == self.policy.deadline_millis
            && binding.max_iterations() == self.budget.max_iterations as u32
            && binding.max_stages() == self.budget.max_stages as u32
            && binding.max_steps_per_stage() == Some(self.budget.max_steps_per_stage)
            && binding.max_total_steps() == Some(self.policy.max_total_steps)
    }

    fn binding(&self) -> Result<SourceInvocationBinding, CliError> {
        if let Some(io_limits) = self.config.io_limits.as_ref() {
            return self
                .policy
                .binding_with_io_limits(
                    &self.compiled,
                    &self.task,
                    self.budget,
                    self.config.pricing.as_ref(),
                    io_limits,
                )
                .map_err(|_| CliError::refused("source execution binding refused"));
        }
        match self.config.pricing.as_ref() {
            Some(pricing) => {
                self.policy
                    .binding_priced(&self.compiled, &self.task, self.budget, pricing)
            }
            None => self.policy.binding(&self.compiled, &self.task, self.budget),
        }
        .map_err(|_| CliError::refused("source execution binding refused"))
    }

    fn io_limits(&self) -> Option<&SourceIoLimits> {
        self.config.io_limits.as_ref()
    }

    fn migration_endpoint(&self) -> SourceLiveMigrationEndpoint<'_> {
        SourceLiveMigrationEndpoint {
            project: &self.project,
            source_path: &self.config.source_path,
            agent_id: &self.config.agent_id,
            lifecycle: &self.compiled,
            policy: &self.policy,
            budget: self.budget,
        }
    }
}

fn deployment_binding(read: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.source-live-cli.deployment.v1\0");
    hash.update(OPENCODE_MODEL.as_bytes());
    hash.update([0]);
    hash.update(read);
    format!("sha256:{:x}", LowerHex(hash.finalize()))
}

fn source_error(failure: SourceLiveFailure) -> CliError {
    let diagnostic_code = failure
        .diagnostics
        .first()
        .map_or("unclassified", |diagnostic| diagnostic.code);
    let status = failure
        .selected
        .map_or("unselected", |status| status.as_str());
    let generation = failure
        .checkpoint
        .as_ref()
        .map_or(0, RecoveredSourceCheckpoint::generation);
    let units = failure
        .checkpoint
        .as_ref()
        .map_or(0, RecoveredSourceCheckpoint::committed_reserved_units);
    let fuel = failure
        .checkpoint
        .as_ref()
        .map_or(0, RecoveredSourceCheckpoint::committed_stage_fuel);
    let money = failure.checkpoint.as_ref().and_then(RecoveredSourceCheckpoint::priced_totals)
        .map(|totals| format!("; currency={}; minor_unit_exponent={}; reserved_minor={}; observed_charge_minor={}; unknown_charge_reservation_minor={}; observed_over_reservation_minor={}; remaining_admission_minor={}", totals.currency, totals.minor_unit_exponent, totals.reserved_minor, totals.observed_charge_minor, totals.unknown_charge_reservation_minor, totals.observed_over_reservation_minor, totals.remaining_admission_minor))
        .unwrap_or_default();
    let io = failure
        .checkpoint
        .as_ref()
        .and_then(RecoveredSourceCheckpoint::io_totals)
        .map(|totals| format!(
            "; reserved_request_bytes={}; reserved_response_bytes={}; observed_response_bytes={}; unknown_response_reservation_bytes={}",
            totals.reserved_request_bytes,
            totals.reserved_response_bytes,
            totals.observed_response_bytes,
            totals.unknown_response_reservation_bytes,
        ))
        .unwrap_or_default();
    CliError::detail(format!(
        "checked source run failed; code={diagnostic_code}; selected={status}; acknowledged_generation={generation}; committed_model_units={units}; committed_stage_fuel={fuel}{money}{io}"
    ))
}

/// The compiled reducer already renders a self-contained, revision-bound
/// evidence document (stage rows, authorization bindings, terminal value
/// digest) into `IterativeRun::evidence`. A fresh or resumed attempt that
/// dispatched this call carries it; a pure checkpoint replay (`checked_run`
/// is `None`) has none to add. Parsing is defensive only: the string is
/// compiler-produced canonical JSON and is expected to always parse.
fn iterative_evidence_value(checked_run: Option<&IterativeRun>) -> Value {
    checked_run.map_or(Value::Null, |run| {
        serde_json::from_str(run.evidence()).unwrap_or(Value::Null)
    })
}

fn receipt(outcome: SourceLiveOutcome) -> Result<String, CliError> {
    let iterative_evidence = iterative_evidence_value(outcome.checked_run.as_ref());
    let terminal = outcome
        .checkpoint
        .terminal_snapshot()
        .ok_or(CliError::refused("checked run has no terminal checkpoint"))?;
    if let Some(io) = outcome.checkpoint.io_totals() {
        let money = outcome
            .checkpoint
            .priced_totals()
            .ok_or(CliError::refused("I/O checkpoint has no priced receipt"))?;
        let receipt = serde_json::json!({
            "schema": "semaprax.source-live-cli.receipt.v3",
            "status": terminal.status().as_str(),
            "invocation": outcome.checkpoint.invocation(),
            "generation": outcome.checkpoint.generation(),
            "chain": outcome.checkpoint.chain(),
            "committed_model_units": outcome.checkpoint.committed_reserved_units(),
            "committed_stage_fuel": outcome.checkpoint.committed_stage_fuel(),
            "model_dispatches": outcome.model_dispatches,
            "effect_dispatches": outcome.effect_dispatches,
            "money": {
                "currency": money.currency,
                "minor_unit_exponent": money.minor_unit_exponent,
                "reserved_minor": money.reserved_minor,
                "observed_charge_minor": money.observed_charge_minor,
                "unknown_charge_reservation_minor": money.unknown_charge_reservation_minor,
                "observed_over_reservation_minor": money.observed_over_reservation_minor,
                "remaining_admission_minor": money.remaining_admission_minor,
            },
            "io": {
                "reserved_request_bytes": io.reserved_request_bytes,
                "reserved_response_bytes": io.reserved_response_bytes,
                "observed_response_bytes": io.observed_response_bytes,
                "unknown_response_reservation_bytes": io.unknown_response_reservation_bytes,
            },
            "iterative_evidence": iterative_evidence,
        });
        return Ok(format!("{receipt}\n"));
    }
    if let Some(money) = outcome.checkpoint.priced_totals() {
        let receipt = serde_json::json!({
            "schema": "semaprax.source-live-cli.receipt.v2",
            "status": terminal.status().as_str(),
            "invocation": outcome.checkpoint.invocation(),
            "generation": outcome.checkpoint.generation(),
            "chain": outcome.checkpoint.chain(),
            "committed_model_units": outcome.checkpoint.committed_reserved_units(),
            "committed_stage_fuel": outcome.checkpoint.committed_stage_fuel(),
            "model_dispatches": outcome.model_dispatches,
            "effect_dispatches": outcome.effect_dispatches,
            "money": {
                "currency": money.currency,
                "minor_unit_exponent": money.minor_unit_exponent,
                "reserved_minor": money.reserved_minor,
                "observed_charge_minor": money.observed_charge_minor,
                "unknown_charge_reservation_minor": money.unknown_charge_reservation_minor,
                "observed_over_reservation_minor": money.observed_over_reservation_minor,
                "remaining_admission_minor": money.remaining_admission_minor,
            },
            "iterative_evidence": iterative_evidence,
        });
        return Ok(format!("{receipt}\n"));
    }
    let receipt = serde_json::json!({
        "schema": "semaprax.source-live-cli.receipt.v1",
        "status": terminal.status().as_str(),
        "invocation": outcome.checkpoint.invocation(),
        "generation": outcome.checkpoint.generation(),
        "chain": outcome.checkpoint.chain(),
        "committed_model_units": outcome.checkpoint.committed_reserved_units(),
        "committed_stage_fuel": outcome.checkpoint.committed_stage_fuel(),
        "model_dispatches": outcome.model_dispatches,
        "effect_dispatches": outcome.effect_dispatches,
        "iterative_evidence": iterative_evidence,
    });
    Ok(format!("{receipt}\n"))
}

fn provider<R: OpenCodeRunner>(
    endpoint: &Endpoint,
    executable: PathBuf,
    scratch: PathBuf,
    runner: R,
    clock: &dyn SourceInvocationClock,
) -> Result<
    (
        OpenCodeModelHandler<R>,
        OpenCodeGrammar,
        ModelInvokeCapability,
    ),
    CliError,
> {
    let grammar = OpenCodeGrammar::from_proposal(endpoint.compiled.proposal_schema())
        .map_err(|_| CliError::refused("OpenCode grammar admission refused"))?;
    let remaining = endpoint
        .policy
        .deadline_millis
        .saturating_sub(clock.now_millis());
    let call_ms = remaining.clamp(1, MAX_ONE_PROVIDER_CALL_MS) as u64;
    let config = OpenCodeHostConfig::new(
        executable,
        scratch,
        Duration::from_millis(call_ms),
        grammar.clone(),
    )
    .map_err(|_| CliError::refused("private OpenCode host configuration refused"))?;
    Ok((
        OpenCodeModelHandler::new(config, runner),
        grammar,
        ModelInvokeCapability::grant("source-live CLI fixed free provider"),
    ))
}

pub(super) fn execute(command: Command) -> Result<String, CliError> {
    execute_with_runner(command, ProcessOpenCodeRunner)
}

pub(super) fn execute_with_runner<R: OpenCodeRunner>(
    command: Command,
    runner: R,
) -> Result<String, CliError> {
    match command {
        Command::Run {
            config,
            checkpoint,
            executable,
            scratch,
        } => execute_run(config, checkpoint, executable, scratch, true, runner),
        Command::Resume {
            config,
            checkpoint,
            executable,
            scratch,
        } => execute_run(config, checkpoint, executable, scratch, false, runner),
        Command::Migrate {
            previous_config,
            previous_checkpoint,
            destination_config,
            destination_checkpoint,
            function,
            steps,
            executable,
            scratch,
        } => execute_migrate(
            previous_config,
            previous_checkpoint,
            destination_config,
            destination_checkpoint,
            &function,
            steps,
            executable,
            scratch,
            runner,
        ),
    }
}

/// Executes an explicitly configured source-live migration through the
/// retained hot-reload supervisor. This is private-host glue: the public CLI
/// cannot construct the hook or gain its provider authority.
pub(super) fn execute_hot_reload_migration(
    supervisor: &mut HotReloadSession,
    plan: HotReloadPlan,
    arguments: &[String],
) -> Result<(), CliError> {
    execute_hot_reload_migration_with_runner(supervisor, plan, arguments, ProcessOpenCodeRunner)
}

pub(super) fn execute_hot_reload_migration_with_runner<R: OpenCodeRunner>(
    supervisor: &mut HotReloadSession,
    plan: HotReloadPlan,
    arguments: &[String],
    runner: R,
) -> Result<(), CliError> {
    let command = Command::parse(arguments)?;
    let Command::Migrate {
        previous_config,
        previous_checkpoint,
        destination_config,
        destination_checkpoint,
        function,
        steps,
        executable,
        scratch,
    } = command
    else {
        return Err(CliError::usage(
            "source-Agent dev requires explicit source-live migrate operands",
        ));
    };
    let previous = Endpoint::load_retained(
        SessionConfig::load(&previous_config)?,
        supervisor.retained_active_project(),
    )?;
    let mut destination = Endpoint::load(SessionConfig::load(&destination_config)?)?;
    if previous.config.pricing.is_some()
        || destination.config.pricing.is_some()
        || previous.io_limits().is_some()
        || destination.io_limits().is_some()
    {
        return Err(CliError::refused(
            "source-Agent dev migration requires the unpriced source-live profile",
        ));
    }
    if previous.task.objective != destination.task.objective
        || previous.task.budget != destination.task.budget
    {
        return Err(CliError::refused("migration task changed"));
    }
    let previous_binding = match supervisor.retained_source_agent_binding() {
        Some(binding) => {
            if !previous.matches_retained_binding(binding) {
                return Err(CliError::refused(
                    "retained source-Agent binding no longer matches predecessor authority",
                ));
            }
            binding.clone()
        }
        None => previous.binding()?,
    };
    let previous_store = CheckpointDir::existing(&previous_checkpoint, &previous.project_root)?;
    let previous_document = previous_store
        .latest()?
        .ok_or(CliError::refused("predecessor has no latest checkpoint"))?;
    let predecessor = recover_source_checkpoint(&previous_document, &previous_binding)
        .map_err(|_| CliError::refused("predecessor checkpoint refused"))?;
    destination.policy.initial_millis = predecessor.last_checked_millis();
    let request = SourceLiveMigrationRequest {
        previous: previous.migration_endpoint(),
        previous_binding: &previous_binding,
        previous_checkpoint: &previous_document,
        destination: destination.migration_endpoint(),
        task: &destination.task,
        migration_function: &function,
        max_migration_steps: steps,
        expected_handoff_digest: None,
    };
    let handoff = supervisor
        .wait_for_source_agent_handoff(&plan, previous.config.agent_id.as_str())
        .map_err(|_| CliError::refused("hot-reload source-Agent handoff refused"))?;
    let prepared = prepare_source_live_migration_from_hot_reload_handoff(request, &handoff)
        .map_err(source_error)?;
    let path_was_new = !destination_checkpoint.exists();
    let mut destination_store = if path_was_new {
        CheckpointDir::fresh(&destination_checkpoint, &destination.project_root)?
    } else {
        CheckpointDir::existing(&destination_checkpoint, &destination.project_root)?
    };
    if destination_store.path() == previous_store.path() {
        return Err(CliError::refused(
            "migration destination equals predecessor store",
        ));
    }
    if destination_store.latest()?.is_some() {
        return Err(CliError::refused(
            "source-Agent dev requires a fresh destination checkpoint",
        ));
    }
    let clock = UnixClock;
    let (mut handler, grammar, capability) =
        provider(&destination, executable, scratch, runner, &clock)?;
    previous_store.claim_handoff(
        prepared.handoff_digest(),
        destination_store.path(),
        prepared.binding().invocation(),
    )?;
    drop(prepared);
    let request = SourceLiveMigrationRequest {
        previous: previous.migration_endpoint(),
        previous_binding: &previous_binding,
        previous_checkpoint: &previous_document,
        destination: destination.migration_endpoint(),
        task: &destination.task,
        migration_function: &function,
        max_migration_steps: steps,
        expected_handoff_digest: None,
    };
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        destination.policy.deployment_binding.clone(),
        grammar,
        destination.policy.response_limit,
        destination.policy.reservation_units,
    )
    .map_err(|_| CliError::refused("durable OpenCode source refused"))?;
    let mut read = ReadSnapshot(destination.read.clone());
    let cancellation = AgentCancellation::new();
    run_source_live_migration_from_hot_reload_session(
        supervisor,
        plan,
        request,
        &mut source,
        &mut read,
        &mut destination_store,
        &clock,
        &cancellation,
    )
    .map_err(|_| CliError::refused("source-Agent hot-reload migration refused"))?;
    Ok(())
}

fn execute_run<R: OpenCodeRunner>(
    config: PathBuf,
    checkpoint: PathBuf,
    executable: PathBuf,
    scratch: PathBuf,
    fresh: bool,
    runner: R,
) -> Result<String, CliError> {
    let endpoint = Endpoint::load(SessionConfig::load(&config)?)?;
    let binding = endpoint.binding()?;
    let mut store = if fresh {
        CheckpointDir::fresh(&checkpoint, &endpoint.project_root)?
    } else {
        CheckpointDir::existing(&checkpoint, &endpoint.project_root)?
    };
    let latest = store.latest()?;
    if fresh && latest.is_some() || !fresh && latest.is_none() {
        return Err(CliError::refused(
            "checkpoint mode does not match latest journal",
        ));
    }
    if let Some(document) = latest.as_deref() {
        let recovered = recover_source_checkpoint(document, &binding)
            .map_err(|_| CliError::refused("latest source checkpoint binding refused"))?;
        store.set_generation(recovered.generation());
        if recovered.terminal_snapshot().is_some() {
            return receipt(SourceLiveOutcome {
                checked_run: None,
                checkpoint: recovered,
                model_dispatches: 0,
                effect_dispatches: 0,
            });
        }
    }
    let clock = UnixClock;
    let (mut handler, grammar, capability) =
        provider(&endpoint, executable, scratch, runner, &clock)?;
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        endpoint.policy.deployment_binding.clone(),
        grammar,
        endpoint.policy.response_limit,
        endpoint.policy.reservation_units,
    )
    .map_err(|_| CliError::refused("durable OpenCode source refused"))?;
    let mut read = ReadSnapshot(endpoint.read.clone());
    let cancellation = AgentCancellation::new();
    let request = SourceLiveRequest {
        task: &endpoint.task,
        budget: endpoint.budget,
        policy: &endpoint.policy,
        clock: &clock,
        cancellation: &cancellation,
        checkpoint: latest.as_deref(),
    };
    let result = match endpoint.io_limits() {
        Some(io_limits) => endpoint.compiled.run_live_durable_with_io_limits(
            request,
            endpoint.config.pricing.as_ref(),
            io_limits,
            &mut source,
            &mut read,
            &mut store,
        ),
        None => match endpoint.config.pricing.as_ref() {
            Some(pricing) => endpoint.compiled.run_live_durable_priced(
                request,
                pricing,
                &mut source,
                &mut read,
                &mut store,
            ),
            None => endpoint
                .compiled
                .run_live_durable(request, &mut source, &mut read, &mut store),
        },
    };
    drop(source);
    result.map_err(source_error).and_then(receipt)
}

#[allow(clippy::too_many_arguments)]
fn execute_migrate<R: OpenCodeRunner>(
    previous_config: PathBuf,
    previous_checkpoint: PathBuf,
    destination_config: PathBuf,
    destination_checkpoint: PathBuf,
    function: &str,
    steps: usize,
    executable: PathBuf,
    scratch: PathBuf,
    runner: R,
) -> Result<String, CliError> {
    let previous = Endpoint::load(SessionConfig::load(&previous_config)?)?;
    let mut destination = Endpoint::load(SessionConfig::load(&destination_config)?)?;
    if previous.config.pricing.is_some() != destination.config.pricing.is_some() {
        return Err(CliError::refused("migration cannot change pricing profile"));
    }
    if previous.config.io_limits.is_some() != destination.config.io_limits.is_some() {
        return Err(CliError::refused("migration cannot change I/O profile"));
    }
    if previous.task.objective != destination.task.objective
        || previous.task.budget != destination.task.budget
    {
        return Err(CliError::refused("migration task changed"));
    }
    let previous_binding = previous.binding()?;
    // A migrated predecessor needs its independently retained handoff binding.
    // This CLI accepts a fresh predecessor only; submitted checkpoint bytes
    // cannot supply the previous handoff authority.
    let previous_store = CheckpointDir::existing(&previous_checkpoint, &previous.project_root)?;
    let previous_document = previous_store
        .latest()?
        .ok_or(CliError::refused("predecessor has no latest checkpoint"))?;
    let predecessor = recover_source_checkpoint(&previous_document, &previous_binding)
        .map_err(|_| CliError::refused("predecessor checkpoint refused"))?;
    // The destination clock origin is carried from the authenticated latest
    // predecessor, not reset by the operator's destination CONFIG. Repeating
    // this handoff derives the same origin from the same held source journal.
    destination.policy.initial_millis = predecessor.last_checked_millis();
    let request = SourceLiveMigrationRequest {
        previous: previous.migration_endpoint(),
        previous_binding: &previous_binding,
        previous_checkpoint: &previous_document,
        destination: destination.migration_endpoint(),
        task: &destination.task,
        migration_function: function,
        max_migration_steps: steps,
        expected_handoff_digest: None,
    };
    let prepared = match (
        &previous.config.pricing,
        &destination.config.pricing,
        previous.io_limits(),
        destination.io_limits(),
    ) {
        (previous_price, destination_price, Some(previous_limits), Some(destination_limits)) => {
            prepare_source_live_migration_with_io_limits(
                request,
                previous_price.as_ref(),
                destination_price.as_ref(),
                previous_limits,
                destination_limits,
            )
        }
        (Some(previous_price), Some(destination_price), None, None) => {
            prepare_source_live_priced_migration(request, previous_price, destination_price)
        }
        (None, None, None, None) => prepare_source_live_migration(request),
        _ => unreachable!("pricing profiles checked before store access"),
    }
    .map_err(source_error)?;
    let path_was_new = !destination_checkpoint.exists();
    let mut destination_store = if path_was_new {
        CheckpointDir::fresh(&destination_checkpoint, &destination.project_root)?
    } else {
        CheckpointDir::existing(&destination_checkpoint, &destination.project_root)?
    };
    if destination_store.path() == previous_store.path() {
        return Err(CliError::refused(
            "migration destination equals predecessor store",
        ));
    }
    let destination_document = destination_store.latest()?;
    if let Some(document) = destination_document.as_deref() {
        let recovered = recover_source_checkpoint(document, prepared.binding())
            .map_err(|_| CliError::refused("destination latest checkpoint binding refused"))?;
        destination_store.set_generation(recovered.generation());
    }
    let clock = UnixClock;
    // Check provider configuration before making the irreversible claim, but
    // permit a terminal receipt to be retrieved without opening a new child.
    let terminal = destination_document.as_deref().and_then(|document| {
        recover_source_checkpoint(document, prepared.binding())
            .ok()
            .filter(|checkpoint| checkpoint.terminal_snapshot().is_some())
    });
    let prepared_provider = if terminal.is_none() {
        Some(provider(&destination, executable, scratch, runner, &clock)?)
    } else {
        None
    };
    previous_store.claim_handoff(
        prepared.handoff_digest(),
        destination_store.path(),
        prepared.binding().invocation(),
    )?;
    if let Some(checkpoint) = terminal {
        return receipt(SourceLiveOutcome {
            checked_run: None,
            checkpoint,
            model_dispatches: 0,
            effect_dispatches: 0,
        });
    }
    let (mut handler, grammar, capability) = prepared_provider.expect("nonterminal prepared");
    let mut source = OpenCodeDurableProposalSource::new(
        &mut handler,
        &capability,
        destination.policy.deployment_binding.clone(),
        grammar,
        destination.policy.response_limit,
        destination.policy.reservation_units,
    )
    .map_err(|_| CliError::refused("durable OpenCode source refused"))?;
    let mut read = ReadSnapshot(destination.read.clone());
    let cancellation = AgentCancellation::new();
    let prepared = if let Some(document) = destination_document.as_deref() {
        prepared.with_checkpoint(document)
    } else {
        prepared
    };
    let outcome = prepared.run(
        &mut source,
        &mut read,
        &mut destination_store,
        &clock,
        &cancellation,
    );
    drop(source);
    outcome.map_err(source_error).and_then(receipt)
}
