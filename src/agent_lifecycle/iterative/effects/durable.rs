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

pub struct DurableTypedRun {
    run: TypedEffectRun,
    checkpoint: String,
    digest: String,
    usage: CheckpointUsage,
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
}
fn diagnostic(reason: &str) -> Vec<Diagnostic> {
    error(&format!("durable.{reason}"))
}
fn checked_sum(left: u64, right: usize) -> Result<u64, Vec<Diagnostic>> {
    left.checked_add(u64::try_from(right).map_err(|_| diagnostic("usage.overflow"))?)
        .ok_or_else(|| diagnostic("usage.overflow"))
}
impl DurableDriver<'_> {
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
        let expected_policy = digest(
            b"semaprax.agent-iteration-policy.v2\0",
            format!("{}\0{}", self.compiled.lifecycle.digest(), context.turn).as_bytes(),
        );
        if context.policy != expected_policy {
            return Err(diagnostic("policy.substitution"));
        }
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
        let fail = |diagnostics: Vec<Diagnostic>| DurableTypedFailure {
            diagnostics,
            terminal: None,
            checkpoint: retained_checkpoint.unwrap_or("").to_owned(),
        };
        let requested = super::super::invocation_digest(task, proposals, stages);
        let invocation = digest(
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
        );
        let identity = CheckpointIdentity {
            execution_revision: execution_revision_digest.to_owned(),
            program_root: program_root_digest.to_owned(),
            registry: self.digest().to_owned(),
            invocation,
        };
        let budget = EffectBudget {
            max_calls: effects.max_calls.min(self.limits.max_calls),
            max_argument_bytes: effects
                .max_argument_bytes
                .min(self.limits.max_argument_bytes),
            max_result_bytes: effects.max_result_bytes.min(self.limits.max_result_bytes),
            max_total_bytes: effects.max_total_bytes.min(self.limits.max_total_bytes),
        };
        let limits = CheckpointLimits {
            calls: budget.max_calls as u64,
            argument_bytes: budget.max_total_bytes as u64,
            result_bytes: budget.max_total_bytes as u64,
            total_bytes: budget.max_total_bytes as u64,
            reserved_fuel: max_reserved_fuel,
        };
        let journal = match retained_checkpoint {
            Some(document) => OperationCheckpoint::decode_with_limits(document, &identity, limits)
                .map_err(|e| fail(vec![e]))?,
            None => OperationCheckpoint::new(identity, limits).map_err(|e| fail(vec![e]))?,
        };
        if journal.limits() != limits {
            return Err(fail(diagnostic("limits.substitution")));
        }
        if journal.recovery_disposition() == RecoveryDisposition::UncertainIntent {
            return Err(fail(diagnostic("uncertain_intent")));
        }
        let replay = journal
            .events()
            .filter(|event| !matches!(event, JournalEvent::StageReservation { .. }))
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
        };
        let stages = IterativeBudget {
            max_iterations: stages.max_iterations.min(self.max_iterations),
            ..stages
        };
        let outcome =
            self.lifecycle
                .run_with_driver(task, proposals, &mut driver, stages, cancellation);
        let checkpoint = driver.journal.canonical_json();
        let checkpoint_digest = driver.journal.digest();
        let usage = driver.journal.usage();
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
        if driver.cursor != driver.replay.len() && lifecycle.status() != IterativeStatus::Cancelled
        {
            return Err(DurableTypedFailure {
                diagnostics: diagnostic("replay.incomplete"),
                terminal: None,
                checkpoint,
            });
        }
        let evidence=format!("{{\"schema\":\"semaprax.agent-durable-typed-evidence.v2\",\"registry\":{},\"lifecycle_evidence\":{},\"checkpoint\":{},\"physical_calls\":{},\"calls\":{},\"argument_bytes\":{},\"result_bytes\":{},\"reserved_fuel\":{},\"failure\":{}}}\n",quote_json(self.digest()),quote_json(lifecycle.evidence_digest()),quote_json(&checkpoint_digest),driver.physical_calls,usage.calls,usage.argument_bytes,usage.result_bytes,usage.reserved_fuel,driver.failure.map(quote_json).unwrap_or_else(||"null".into()));
        let run = TypedEffectRun {
            lifecycle,
            dispatched: driver.physical_calls,
            argument_bytes: usize::try_from(usage.argument_bytes)
                .map_err(|_| fail(diagnostic("usage.overflow")))?,
            result_bytes: usize::try_from(usage.result_bytes)
                .map_err(|_| fail(diagnostic("usage.overflow")))?,
            failure: driver.failure,
            digest: digest(
                b"semaprax.agent-durable-typed-evidence.v2\0",
                evidence.as_bytes(),
            ),
            evidence,
        };
        Ok(DurableTypedRun {
            run,
            checkpoint,
            digest: checkpoint_digest,
            usage,
        })
    }
}

#[cfg(test)]
mod tests;
