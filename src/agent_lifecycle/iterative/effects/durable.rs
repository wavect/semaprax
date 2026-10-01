//! Trusted-store recovery around the exact typed registry and checked driver.
//! Hashes bind context; retained observations are trusted host input, not proof.
use super::*;
use crate::agent_lifecycle::iterative::driver::{
    DriverFailure, EffectContext as LiveContext, IterativeDriver,
};
use crate::agent_lifecycle::CheckpointStore;
use crate::agent_runtime_v2::checkpoint::{
    CheckpointIdentity, CheckpointLimits, CheckpointUsage, EffectContext, JournalEvent,
    OperationCheckpoint, RecoveryDisposition,
};
use crate::execution_revision::typed::migration::MigrationSeed;

/// The checked registry already commits to one exact retained module. A
/// selected Wasm executor is handed its source text directly (never a
/// filesystem lookup), so nothing else would otherwise stop a caller from
/// pairing this registry with different Wasm bytes at dispatch time. This
/// refuses that mismatch before any checkpoint identity, decode, store
/// write, or handler dispatch, on both the fresh and the resumed leg, so a
/// checkpoint produced or consumed under a wrong Wasm source is refused
/// identically to any other tampered input.
fn wasm_source_mismatch(
    compiled: &CompiledTypedEffects,
    backend: Option<crate::agent_lifecycle::authorization::StageBackend<'_>>,
) -> bool {
    match backend {
        #[cfg(test)]
        Some(crate::agent_lifecycle::authorization::StageBackend::Wasm { source }) => {
            compiled.target_source.as_deref() != Some(source)
        }
        Some(crate::agent_lifecycle::authorization::StageBackend::WasmHeld { source, .. }) => {
            compiled.target_source.as_deref() != Some(source)
        }
        _ => false,
    }
}

pub struct DurableTypedRun {
    run: TypedEffectRun,
    checkpoint: String,
    digest: String,
    usage: CheckpointUsage,
    iterations: usize,
    stages: usize,
}
impl DurableTypedRun {
    pub fn run(&self) -> &TypedEffectRun {
        &self.run
    }
    pub fn checkpoint(&self) -> &str {
        &self.checkpoint
    }
    pub fn checkpoint_digest(&self) -> &str {
        &self.digest
    }
    pub fn usage(&self) -> CheckpointUsage {
        self.usage
    }
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    pub fn stages(&self) -> usize {
        self.stages
    }
}
pub struct DurableTypedFailure {
    diagnostics: Vec<Diagnostic>,
    terminal: Option<IterativeRun>,
    checkpoint: String,
}
impl DurableTypedFailure {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn terminal(&self) -> Option<&IterativeRun> {
        self.terminal.as_ref()
    }
    pub fn checkpoint(&self) -> &str {
        &self.checkpoint
    }
}
impl std::fmt::Debug for DurableTypedFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableTypedFailure")
            .field("diagnostics", &self.diagnostics)
            .field(
                "terminal",
                &self.terminal.as_ref().map(IterativeRun::status),
            )
            .finish()
    }
}
struct Pending {
    turn: usize,
    state: RetainedValue,
    proposal: String,
    authorization: String,
}
struct DurableDriver<'a> {
    compiled: &'a CompiledTypedEffects,
    proposals: &'a [String],
    handler: &'a mut dyn TypedEffectHandler,
    store: &'a mut dyn CheckpointStore,
    journal: OperationCheckpoint,
    replay: Vec<JournalEvent>,
    cursor: usize,
    pending: Option<Pending>,
    context: Option<EffectContext>,
    budget: EffectBudget,
    persistence_error: Option<Vec<Diagnostic>>,
    failure: Option<&'static str>,
    physical_calls: usize,
    seed_binding: Option<String>,
    metered_observations:
        Option<&'a std::cell::RefCell<Vec<super::metered::StageSemanticObservation>>>,
    semantic_replay: Vec<JournalEvent>,
    semantic_cursor: usize,
}
fn diagnostic(reason: &str) -> Vec<Diagnostic> {
    error(&format!("durable.{reason}"))
}
pub(super) fn semantic_refusal(reason: &str, checkpoint: &str) -> DurableTypedFailure {
    DurableTypedFailure {
        diagnostics: diagnostic(reason),
        terminal: None,
        checkpoint: checkpoint.to_owned(),
    }
}
fn checked_sum(left: u64, right: usize) -> Result<u64, Vec<Diagnostic>> {
    left.checked_add(u64::try_from(right).map_err(|_| diagnostic("usage.overflow"))?)
        .ok_or_else(|| diagnostic("usage.overflow"))
}
impl DurableDriver<'_> {
    fn persist_semantic_work(&mut self) -> Result<(), Vec<Diagnostic>> {
        let Some(observations) = self.metered_observations else {
            return Ok(());
        };
        loop {
            let Some(observation) = observations.borrow().get(self.semantic_cursor).cloned() else {
                return Ok(());
            };
            let work = observation.work();
            let fuel_limit = work
                .fuel_limit
                .ok_or_else(|| diagnostic("semantic_work.receipt"))?;
            let ordinal = u64::try_from(
                self.journal
                    .events()
                    .filter(|event| matches!(event, JournalEvent::SemanticWork { .. }))
                    .count(),
            )
            .map_err(|_| diagnostic("semantic_work.receipt"))?;
            let current_finalizer_events = work.finalizer_events.as_ref().map(|events| {
                events
                    .iter()
                    .map(|event| (event.function.as_str().to_owned(), event.liveness_flag))
                    .collect::<Vec<_>>()
            });
            let event = JournalEvent::SemanticWork {
                ordinal,
                function: observation.function_id().to_owned(),
                fuel_used: work.fuel_used,
                fuel_limit,
                exhausted: work.exhausted,
                finalizer_events: current_finalizer_events.clone(),
            };
            if let Some(expected) = self.semantic_replay.get(self.semantic_cursor) {
                if !matches!(expected, JournalEvent::SemanticWork { function, fuel_used, fuel_limit: expected_limit, exhausted, finalizer_events: expected_finalizer_events, .. }
                    if function == observation.function_id()
                        && *fuel_used == work.fuel_used
                        && *expected_limit == fuel_limit
                        && *exhausted == work.exhausted
                        && *expected_finalizer_events == current_finalizer_events)
                {
                    return Err(diagnostic("semantic_work.replay"));
                }
            }
            // Replay validates the retained receipt above, then records the
            // newly charged replayed stage under its new reservation.
            self.persist(event, self.journal.usage())?;
            self.semantic_cursor += 1;
        }
    }
    fn persist(
        &mut self,
        event: JournalEvent,
        usage: CheckpointUsage,
    ) -> Result<(), Vec<Diagnostic>> {
        self.journal
            .persist(event, usage, self.store)
            .map_err(|e| vec![e])
    }
    fn dispatch(
        &mut self,
        request: &TypedEffectRequest<'_>,
    ) -> Result<Option<Vec<(String, RetainedValue)>>, Vec<Diagnostic>> {
        let pending = self
            .pending
            .as_ref()
            .ok_or_else(|| diagnostic("effect.context"))?;
        if request.authorization().binding() != pending.authorization {
            return Err(diagnostic("authorization.substitution"));
        }
        let context = EffectContext {
            turn: pending.turn as u64,
            operation: request.operation_id().to_owned(),
            effect: request.effect_id().to_owned(),
            authorization_binding: pending.authorization.clone(),
            state: pending.state.clone(),
            proposal: pending.proposal.clone(),
            arguments: request.arguments().to_vec(),
        };
        self.context = Some(context.clone());
        if self.cursor < self.replay.len() {
            if !matches!(&self.replay[self.cursor],JournalEvent::Intent(before) if before==&context)
            {
                return Err(diagnostic("replay.intent"));
            }
            self.cursor += 1;
            let Some(JournalEvent::Observed {
                context: before,
                result,
                failure,
            }) = self.replay.get(self.cursor)
            else {
                return Err(diagnostic("replay.uncertain_intent"));
            };
            if before != &context {
                return Err(diagnostic("replay.observed"));
            }
            let result = result.clone();
            let failure = failure.clone();
            self.cursor += 1;
            if let Some(reason) = failure {
                self.failure =
                    Some(failure_reason(&reason).ok_or_else(|| diagnostic("replay.failure"))?);
                return Ok(None);
            }
            return Ok(Some(result));
        }
        let mut usage = self.journal.usage();
        let argument_bytes = encode_fields(request.arguments()).len();
        if usage.calls >= self.budget.max_calls as u64
            || argument_bytes > self.budget.max_argument_bytes
        {
            self.failure = Some("call_budget");
            return Ok(None);
        }
        usage.calls = usage
            .calls
            .checked_add(1)
            .ok_or_else(|| diagnostic("usage.calls"))?;
        usage.argument_bytes = checked_sum(usage.argument_bytes, argument_bytes)?;
        if usage
            .argument_bytes
            .checked_add(usage.result_bytes)
            .is_none_or(|v| v > self.budget.max_total_bytes as u64)
        {
            self.failure = Some("argument_budget");
            return Ok(None);
        }
        self.persist(JournalEvent::Intent(context.clone()), usage)?;
        self.physical_calls += 1;
        let result = self.handler.execute(request);
        let attempted = result
            .as_ref()
            .map(|r| measured_fields(r, MAX_READ_BYTES))
            .unwrap_or(0);
        usage.result_bytes = checked_sum(usage.result_bytes, attempted)?;
        let operation = request.operation;
        let index = self
            .compiled
            .operations
            .iter()
            .position(|op| op.operation_id == operation.operation_id)
            .ok_or_else(|| diagnostic("operation.identity"))?;
        let reason = match &result {
            None => Some("handler_failed"),
            Some(_)
                if attempted > self.budget.max_result_bytes
                    || usage
                        .argument_bytes
                        .checked_add(usage.result_bytes)
                        .is_none_or(|v| v > self.budget.max_total_bytes as u64) =>
            {
                Some("result_budget")
            }
            Some(values) if values.len() != operation.results.len() => Some("result_fields"),
            Some(values)
                if values
                    .iter()
                    .zip(&operation.results)
                    .any(|((id, value), expected)| {
                        id != &expected.result_id || !expected.kind.accepts(value)
                    }) =>
            {
                Some("result_type")
            }
            Some(values) if values.iter().any(|(_, v)| scalar_bytes(v).is_none()) => {
                Some("result_scalar")
            }
            Some(values)
                if values
                    .iter()
                    .zip(&self.compiled.field_limits[index].1)
                    .any(|((_, v), limit)| scalar_bytes(v).is_none_or(|n| n > *limit)) =>
            {
                Some("result_field_budget")
            }
            _ => None,
        };
        self.persist(
            JournalEvent::Observed {
                context,
                result: if reason.is_none() {
                    result.clone().unwrap_or_default()
                } else {
                    Vec::new()
                },
                failure: reason.map(str::to_owned),
            },
            usage,
        )?;
        if let Some(reason) = reason {
            self.failure = Some(reason);
            Ok(None)
        } else {
            Ok(result)
        }
    }
}
fn failure_reason(reason: &str) -> Option<&'static str> {
    [
        "handler_failed",
        "result_budget",
        "result_fields",
        "result_type",
        "result_scalar",
        "result_field_budget",
        "result_measurement",
    ]
    .into_iter()
    .find(|item| *item == reason)
}
struct RetainedHandler<'a, 'b>(&'a mut DurableDriver<'b>);
impl TypedEffectHandler for RetainedHandler<'_, '_> {
    fn execute(
        &mut self,
        request: &TypedEffectRequest<'_>,
    ) -> Option<Vec<(String, RetainedValue)>> {
        match self.0.dispatch(request) {
            Ok(result) => result,
            Err(errors) => {
                self.0.persistence_error = Some(errors);
                None
            }
        }
    }
}
impl IterativeDriver for DurableDriver<'_> {
    fn before_stage(
        &mut self,
        role: &'static str,
        turn: usize,
        max_steps: usize,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut usage = self.journal.usage();
        usage.reserved_fuel = checked_sum(usage.reserved_fuel, max_steps)?;
        self.persist(
            JournalEvent::StageReservation {
                turn: turn as u64,
                stage: role.to_owned(),
                fuel: max_steps as u64,
            },
            usage,
        )
    }
    fn before_effect(&mut self, context: LiveContext<'_>) -> Result<(), Vec<Diagnostic>> {
        let ordinary_policy = digest(
            b"semaprax.agent-iteration-policy.v2\0",
            format!("{}\0{}", self.compiled.lifecycle.digest(), context.turn).as_bytes(),
        );
        let expected_policy = self
            .seed_binding
            .as_ref()
            .map_or(ordinary_policy.clone(), |seed| {
                digest(
                    b"semaprax.agent-migrated-iteration-policy.v1\0",
                    format!("{}\0{}", ordinary_policy, seed).as_bytes(),
                )
            });
        if context.policy != expected_policy {
            return Err(diagnostic("policy.substitution"));
        }
        // Stage receipts are committed before the effect can enter its
        // intent/host boundary. A crash after a completed stage but before
        // this acknowledgement leaves an unpaired reservation, which the
        // metered recovery route refuses before any handler work.
        self.persist_semantic_work()?;
        self.pending = Some(Pending {
            turn: context.turn,
            state: context.state.clone(),
            proposal: context.proposal_canonical.to_owned(),
            authorization: context.authorization.binding().to_owned(),
        });
        Ok(())
    }
    fn read(
        &mut self,
        authorization: &AuthorizedRequest,
    ) -> Result<Option<Vec<u8>>, Vec<Diagnostic>> {
        let turn = self
            .pending
            .as_ref()
            .ok_or_else(|| diagnostic("effect.context"))?
            .turn;
        let compiled = self.compiled;
        let proposals = self.proposals;
        let budget = self.budget;
        let (result, ordinary_failure) = {
            let mut handler = RetainedHandler(self);
            let mut dispatch = Dispatch {
                compiled,
                proposals,
                handler: &mut handler,
                budget,
                dispatched: turn,
                arguments: 0,
                results: 0,
                failure: None,
            };
            let result = dispatch.invoke(authorization);
            let failure = result.as_ref().err().copied();
            (result.ok(), failure)
        };
        if let Some(errors) = self.persistence_error.take() {
            return Err(errors);
        }
        if self.failure.is_none() {
            self.failure = ordinary_failure;
        }
        Ok(result)
    }
    fn after_transition(
        &mut self,
        _turn: usize,
        kind: &str,
        value: &RetainedValue,
    ) -> Result<(), Vec<Diagnostic>> {
        self.persist_semantic_work()?;
        let context = self
            .context
            .clone()
            .ok_or_else(|| diagnostic("transition.context"))?;
        if self.cursor < self.replay.len() {
            if !matches!(&self.replay[self.cursor],JournalEvent::Transition { context:before,transition,value:before_value } if before==&context && transition==kind && before_value==value)
            {
                return Err(diagnostic("replay.transition"));
            }
            self.cursor += 1;
            return Ok(());
        }
        self.persist(
            JournalEvent::Transition {
                context,
                transition: kind.to_owned(),
                value: value.clone(),
            },
            self.journal.usage(),
        )
    }
}

impl CompiledTypedEffects {
    /// Run a durable checked lifecycle on an explicitly held target. The
    /// selected target does not enter checkpoint identity; replay and store
    /// ownership remain the same as for the interpreter entry.
    #[allow(clippy::too_many_arguments)]
    pub fn run_durable_with_backend(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        selected: super::TargetStageBackend<'_>,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        let backend = self.durable_backend(selected, retained_checkpoint)?;
        self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            None,
            Some(backend),
        )
    }

    pub(super) fn durable_backend<'a>(
        &'a self,
        selected: super::TargetStageBackend<'a>,
        retained_checkpoint: Option<&str>,
    ) -> Result<crate::agent_lifecycle::authorization::StageBackend<'a>, DurableTypedFailure> {
        use crate::agent_lifecycle::authorization::StageBackend;
        let backend = match selected {
            super::TargetStageBackend::Interpreter => StageBackend::Interpreter,
            super::TargetStageBackend::Native(host) => StageBackend::Native { host: &host.host },
            #[cfg(test)]
            super::TargetStageBackend::CoreWasm => {
                let source = self
                    .target_source
                    .as_deref()
                    .ok_or_else(|| DurableTypedFailure {
                        diagnostics: diagnostic("backend.wasm_source"),
                        terminal: None,
                        checkpoint: retained_checkpoint.unwrap_or("").to_owned(),
                    })?;
                StageBackend::Wasm { source }
            }
            super::TargetStageBackend::CoreWasmHeld(host) => {
                let source = self
                    .target_source
                    .as_deref()
                    .ok_or_else(|| DurableTypedFailure {
                        diagnostics: diagnostic("backend.wasm_source"),
                        terminal: None,
                        checkpoint: retained_checkpoint.unwrap_or("").to_owned(),
                    })?;
                StageBackend::WasmHeld {
                    host: &host.host,
                    source,
                }
            }
        };
        Ok(backend)
    }

    /// Validate a selected destination before a migration handoff can stage
    /// its first checkpoint. This is read-only and grants no dispatch.
    pub(crate) fn validate_durable_backend(
        &self,
        selected: super::TargetStageBackend<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        self.durable_backend(selected, None)
            .map(|_| ())
            .map_err(|failure| failure.diagnostics)
    }

    /// `retained_checkpoint` must be read from the caller-authorized trusted
    /// store under exclusive writer authority. Hashes cannot authenticate
    /// caller-fabricated host observations. Root strings are binding inputs;
    /// the joined Runtime v2 wrapper supplies and authenticates its private roots.
    #[allow(clippy::too_many_arguments)]
    pub fn run_durable(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            None,
            None,
        )
    }

    /// Local backend-parity entry for the durable, frozen proposal route.
    ///
    /// Checkpoint identity does not depend on which of Interpreter, Native or
    /// Core Wasm produced or resumes it: the same canonical checkpoint bytes
    /// from one backend decode and continue correctly under any other, since
    /// every backend is checked to compute the identical semantic result and
    /// journal event sequence. This is deliberately test-only: it proves the
    /// sealed stage executors can recover the same checked grant/context/
    /// result without selecting a deployable target or adding a host
    /// capability.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::agent_lifecycle) fn run_durable_on(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            None,
            Some(backend),
        )
    }

    /// Resume a migration-owned State through the same persisted effect journal.
    /// The seed is opaque and can only be produced by the checked migration path.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_durable_from_seed(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        seed: &MigrationSeed,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            Some(seed),
            None,
        )
    }

    /// Local backend-parity entry for the migration-seeded durable route.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(in crate::agent_lifecycle) fn run_durable_from_seed_on(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        seed: &MigrationSeed,
        backend: crate::agent_lifecycle::authorization::StageBackend<'_>,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            Some(seed),
            Some(backend),
        )
    }

    /// Resume a checked migration seed on an explicitly held target.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_durable_from_seed_with_backend(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        seed: &MigrationSeed,
        selected: super::TargetStageBackend<'_>,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        let backend = self.durable_backend(selected, retained_checkpoint)?;
        self.run_durable_inner(
            task,
            proposals,
            handler,
            stages,
            effects,
            cancellation,
            execution_revision_digest,
            program_root_digest,
            retained_checkpoint,
            store,
            max_reserved_fuel,
            Some(seed),
            Some(backend),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn run_durable_inner<'a>(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        handler: &mut dyn TypedEffectHandler,
        stages: IterativeBudget,
        effects: EffectBudget,
        cancellation: &AgentCancellation,
        execution_revision_digest: &str,
        program_root_digest: &str,
        retained_checkpoint: Option<&str>,
        store: &mut dyn CheckpointStore,
        max_reserved_fuel: u64,
        seed: Option<&MigrationSeed>,
        backend: Option<crate::agent_lifecycle::authorization::StageBackend<'a>>,
    ) -> Result<DurableTypedRun, DurableTypedFailure> {
        let fail = |diagnostics: Vec<Diagnostic>| DurableTypedFailure {
            diagnostics,
            terminal: None,
            checkpoint: retained_checkpoint.unwrap_or("").to_owned(),
        };
        // Checked before any identity, decode, store write or handler
        // dispatch: a selected Wasm executor that is not handed this exact
        // registry's own retained source is refused identically on the
        // fresh and the resumed leg, whichever backend produced the
        // checkpoint being resumed.
        if wasm_source_mismatch(self, backend) {
            return Err(fail(diagnostic("backend.wasm_source")));
        }
        let (semantic_fuel_limit, metered_observations, metered_target_binding) =
            match backend.as_ref() {
                Some(crate::agent_lifecycle::authorization::StageBackend::Metered {
                    backend,
                    fuel_limit,
                    observations,
                }) => (
                    Some(*fuel_limit),
                    Some(*observations),
                    Some(self.target_execution_binding(**backend)),
                ),
                _ => (None, None, None),
            };
        let requested = super::super::invocation_digest(task, proposals, stages);
        // Ordinary checkpoint identity remains target-neutral. The metered
        // profile below separately binds its selected target before recovery
        // can reuse receipts or reserve fresh stage fuel.
        let invocation = match seed {
            None => digest(
                b"semaprax.agent-durable-typed-invocation.v2\0",
                format!(
                    "{}\0{},{},{},{}\0{}",
                    requested,
                    effects.max_calls,
                    effects.max_argument_bytes,
                    effects.max_result_bytes,
                    effects.max_total_bytes,
                    max_reserved_fuel
                )
                .as_bytes(),
            ),
            Some(seed) => digest(
                b"semaprax.agent-migrated-durable-typed-invocation.v1\0",
                format!(
                    "{}\0{}\0{}\0{},{},{},{}\0{}",
                    requested,
                    seed.binding_digest(),
                    crate::agent_lifecycle::encode_value(seed.value()),
                    effects.max_calls,
                    effects.max_argument_bytes,
                    effects.max_result_bytes,
                    effects.max_total_bytes,
                    max_reserved_fuel
                )
                .as_bytes(),
            ),
        };
        let identity = CheckpointIdentity {
            execution_revision: execution_revision_digest.to_owned(),
            program_root: program_root_digest.to_owned(),
            registry: self.digest().to_owned(),
            invocation,
        };
        let full_budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let prior = seed.map(MigrationSeed::usage).unwrap_or_default();
        if seed.is_some_and(|seed| seed.max_reserved_fuel() != max_reserved_fuel) {
            return Err(fail(diagnostic("seed.fuel_binding")));
        }
        let remaining = |total: u64, used: u64, field: &str| {
            total
                .checked_sub(used)
                .ok_or_else(|| fail(diagnostic(field)))
        };
        let remaining_calls = remaining(full_budget.max_calls as u64, prior.calls, "seed.calls")?;
        let remaining_total = remaining(
            full_budget.max_total_bytes as u64,
            prior
                .argument_bytes
                .checked_add(prior.result_bytes)
                .ok_or_else(|| fail(diagnostic("seed.bytes")))?,
            "seed.bytes",
        )?;
        let remaining_fuel = remaining(max_reserved_fuel, prior.reserved_fuel, "seed.fuel")?;
        let budget = EffectBudget {
            max_calls: usize::try_from(remaining_calls)
                .map_err(|_| fail(diagnostic("seed.calls")))?,
            max_argument_bytes: full_budget.max_argument_bytes,
            max_result_bytes: full_budget.max_result_bytes,
            max_total_bytes: usize::try_from(remaining_total)
                .map_err(|_| fail(diagnostic("seed.bytes")))?,
        };
        let limits = CheckpointLimits {
            calls: remaining_calls,
            argument_bytes: remaining_total,
            result_bytes: remaining_total,
            total_bytes: remaining_total,
            reserved_fuel: remaining_fuel,
        };
        let journal = match (retained_checkpoint, semantic_fuel_limit, metered_target_binding) {
            (Some(document), Some(fuel_limit), Some(target_binding)) => OperationCheckpoint::decode_metered_with_limits(
                document, &identity, limits, fuel_limit, &target_binding,
            )
            .map_err(|e| fail(vec![e]))?,
            (Some(document), None, None) => {
                OperationCheckpoint::decode_with_limits(document, &identity, limits)
                    .map_err(|e| fail(vec![e]))?
            }
            (None, Some(fuel_limit), Some(target_binding)) => {
                OperationCheckpoint::new_metered(identity, limits, fuel_limit, target_binding)
                    .map_err(|e| fail(vec![e]))?
            }
            (None, None, None) => {
                OperationCheckpoint::new(identity, limits).map_err(|e| fail(vec![e]))?
            }
            _ => return Err(fail(diagnostic("semantic_work.profile"))),
        };
        if journal.limits() != limits {
            return Err(fail(diagnostic("limits.substitution")));
        }
        if semantic_fuel_limit.is_none() && journal.semantic_fuel_limit().is_some() {
            return Err(fail(diagnostic("semantic_work.profile")));
        }
        if journal.recovery_disposition() == RecoveryDisposition::UncertainIntent {
            return Err(fail(diagnostic("uncertain_intent")));
        }
        let semantic_receipts = journal
            .events()
            .filter(|event| matches!(event, JournalEvent::SemanticWork { .. }))
            .cloned()
            .collect::<Vec<_>>();
        if semantic_fuel_limit.is_some()
            && retained_checkpoint.is_some()
            && journal
                .events()
                .filter(|event| matches!(event, JournalEvent::StageReservation { .. }))
                .count()
                != semantic_receipts.len()
        {
            return Err(fail(diagnostic("semantic_work.unacknowledged")));
        }
        let journal_stages = journal
            .events()
            .filter(|event| matches!(event, JournalEvent::StageReservation { .. }))
            .count();
        let effective_stages = match seed {
            None => IterativeBudget {
                max_iterations: stages.max_iterations.min(self.max_iterations),
                ..stages
            },
            Some(seed) => {
                let max_iterations = stages.max_iterations.min(self.max_iterations);
                let consumed_stages = seed
                    .prior_stages()
                    .checked_add(journal_stages)
                    .ok_or_else(|| fail(diagnostic("seed.stages")))?;
                if seed.prior_iterations() >= max_iterations || consumed_stages >= stages.max_stages
                {
                    return Err(fail(diagnostic("seed.destination_exhausted")));
                }
                IterativeBudget {
                    max_iterations: max_iterations - seed.prior_iterations(),
                    max_stages: stages.max_stages - consumed_stages,
                    max_steps_per_stage: stages.max_steps_per_stage,
                }
            }
        };
        let replay = journal
            .events()
            .filter(|event| {
                !matches!(
                    event,
                    JournalEvent::StageReservation { .. } | JournalEvent::SemanticWork { .. }
                )
            })
            .cloned()
            .collect();
        let mut driver = DurableDriver {
            compiled: self,
            proposals,
            handler,
            store,
            journal,
            replay,
            cursor: 0,
            pending: None,
            context: None,
            budget,
            persistence_error: None,
            failure: None,
            physical_calls: 0,
            seed_binding: seed.map(|seed| seed.binding_digest().to_owned()),
            metered_observations,
            semantic_replay: semantic_receipts,
            semantic_cursor: 0,
        };
        let outcome = match (seed, backend) {
            (Some(seed), None) => self.lifecycle.run_with_driver_seed(
                task,
                proposals,
                &mut driver,
                effective_stages,
                cancellation,
                seed,
            ),
            (None, None) => self.lifecycle.run_with_driver(
                task,
                proposals,
                &mut driver,
                effective_stages,
                cancellation,
            ),
            (None, Some(backend)) => self.lifecycle.run_with_driver_on(
                task,
                proposals,
                &mut driver,
                effective_stages,
                cancellation,
                backend,
            ),
            (Some(seed), Some(backend)) => self.lifecycle.run_with_driver_seed_on(
                task,
                proposals,
                &mut driver,
                effective_stages,
                cancellation,
                seed,
                backend,
            ),
        };
        // A terminal/budget exit can occur after a stage returned but before
        // another effect or transition boundary. Commit that final receipt
        // before exposing the checkpoint; otherwise the next metered resume
        // correctly treats it as an unacknowledged crash window.
        if let Err(persist_diagnostics) = driver.persist_semantic_work() {
            let checkpoint = driver.journal.canonical_json();
            return Err(match outcome {
                Err(DriverFailure::Diagnostics(diagnostics)) => DurableTypedFailure {
                    diagnostics,
                    terminal: None,
                    checkpoint,
                },
                Err(DriverFailure::Persistence {
                    terminal,
                    diagnostics,
                }) => DurableTypedFailure {
                    diagnostics,
                    terminal: Some(*terminal),
                    checkpoint,
                },
                Ok(_) => DurableTypedFailure {
                    diagnostics: persist_diagnostics,
                    terminal: None,
                    checkpoint,
                },
            });
        }
        let checkpoint = driver.journal.canonical_json();
        let checkpoint_digest = driver.journal.digest();
        let local_usage = driver.journal.usage();
        let usage = CheckpointUsage {
            calls: prior
                .calls
                .checked_add(local_usage.calls)
                .ok_or_else(|| fail(diagnostic("usage.calls")))?,
            argument_bytes: prior
                .argument_bytes
                .checked_add(local_usage.argument_bytes)
                .ok_or_else(|| fail(diagnostic("usage.arguments")))?,
            result_bytes: prior
                .result_bytes
                .checked_add(local_usage.result_bytes)
                .ok_or_else(|| fail(diagnostic("usage.results")))?,
            reserved_fuel: prior
                .reserved_fuel
                .checked_add(local_usage.reserved_fuel)
                .ok_or_else(|| fail(diagnostic("usage.fuel")))?,
        };
        let lifecycle = match outcome {
            Ok(run) => run,
            Err(DriverFailure::Diagnostics(diagnostics)) => {
                return Err(DurableTypedFailure {
                    diagnostics,
                    terminal: None,
                    checkpoint,
                })
            }
            Err(DriverFailure::Persistence {
                terminal,
                diagnostics,
            }) => {
                return Err(DurableTypedFailure {
                    diagnostics,
                    terminal: Some(*terminal),
                    checkpoint,
                })
            }
        };
        if semantic_fuel_limit.is_some()
            && driver
                .journal
                .events()
                .filter(|event| matches!(event, JournalEvent::StageReservation { .. }))
                .count()
                != driver
                    .journal
                    .events()
                    .filter(|event| matches!(event, JournalEvent::SemanticWork { .. }))
                    .count()
        {
            return Err(DurableTypedFailure {
                diagnostics: diagnostic("semantic_work.unacknowledged"),
                terminal: Some(lifecycle),
                checkpoint,
            });
        }
        if driver.cursor != driver.replay.len() && lifecycle.status() != IterativeStatus::Cancelled
        {
            return Err(DurableTypedFailure {
                diagnostics: diagnostic("replay.incomplete"),
                terminal: None,
                checkpoint,
            });
        }
        if driver.semantic_cursor < driver.semantic_replay.len() && retained_checkpoint.is_some() {
            return Err(DurableTypedFailure {
                diagnostics: diagnostic("semantic_work.replay_incomplete"),
                terminal: Some(lifecycle),
                checkpoint,
            });
        }
        let prior_iterations = seed
            .map(MigrationSeed::prior_iterations)
            .unwrap_or_default();
        let iterations = prior_iterations
            .checked_add(lifecycle.iterations())
            .ok_or_else(|| fail(diagnostic("usage.iterations")))?;
        let prior_stages = seed.map(MigrationSeed::prior_stages).unwrap_or_default();
        let local_stages = driver
            .journal
            .events()
            .filter(|event| matches!(event, JournalEvent::StageReservation { .. }))
            .count();
        let durable_stages = prior_stages
            .checked_add(local_stages)
            .ok_or_else(|| fail(diagnostic("usage.stages")))?;
        let (evidence, evidence_domain) = match seed {
            None => (
                format!("{{\"schema\":\"semaprax.agent-durable-typed-evidence.v2\",\"registry\":{},\"lifecycle_evidence\":{},\"checkpoint\":{},\"physical_calls\":{},\"calls\":{},\"argument_bytes\":{},\"result_bytes\":{},\"reserved_fuel\":{},\"failure\":{}}}\n",quote_json(self.digest()),quote_json(lifecycle.evidence_digest()),quote_json(&checkpoint_digest),driver.physical_calls,usage.calls,usage.argument_bytes,usage.result_bytes,usage.reserved_fuel,driver.failure.map(quote_json).unwrap_or_else(||"null".into())),
                b"semaprax.agent-durable-typed-evidence.v2\0".as_slice(),
            ),
            Some(seed) => (
                format!("{{\"schema\":\"semaprax.agent-migrated-durable-typed-evidence.v3\",\"registry\":{},\"lifecycle_evidence\":{},\"checkpoint\":{},\"seed\":{},\"physical_calls\":{},\"prior_calls\":{},\"prior_argument_bytes\":{},\"prior_result_bytes\":{},\"prior_reserved_fuel\":{},\"local_calls\":{},\"local_argument_bytes\":{},\"local_result_bytes\":{},\"local_reserved_fuel\":{},\"calls\":{},\"argument_bytes\":{},\"result_bytes\":{},\"reserved_fuel\":{},\"prior_iterations\":{},\"local_iterations\":{},\"iterations\":{},\"prior_stages\":{},\"local_stages\":{},\"stages\":{},\"failure\":{}}}\n",quote_json(self.digest()),quote_json(lifecycle.evidence_digest()),quote_json(&checkpoint_digest),quote_json(seed.binding_digest()),driver.physical_calls,prior.calls,prior.argument_bytes,prior.result_bytes,prior.reserved_fuel,local_usage.calls,local_usage.argument_bytes,local_usage.result_bytes,local_usage.reserved_fuel,usage.calls,usage.argument_bytes,usage.result_bytes,usage.reserved_fuel,prior_iterations,lifecycle.iterations(),iterations,prior_stages,local_stages,durable_stages,driver.failure.map(quote_json).unwrap_or_else(||"null".into())),
                b"semaprax.agent-migrated-durable-typed-evidence.v3\0".as_slice(),
            ),
        };
        let run = TypedEffectRun {
            lifecycle,
            dispatched: driver.physical_calls,
            argument_bytes: usize::try_from(usage.argument_bytes)
                .map_err(|_| fail(diagnostic("usage.overflow")))?,
            result_bytes: usize::try_from(usage.result_bytes)
                .map_err(|_| fail(diagnostic("usage.overflow")))?,
            failure: driver.failure,
            digest: digest(evidence_domain, evidence.as_bytes()),
            evidence,
        };
        Ok(DurableTypedRun {
            run,
            checkpoint,
            digest: checkpoint_digest,
            usage,
            iterations,
            stages: durable_stages,
        })
    }
}

#[cfg(test)]
mod tests;
