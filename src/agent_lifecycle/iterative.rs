//! Additive bounded iterative lifecycle, reusing checked retained stage calls.
use super::*;
mod render;
mod step;
#[cfg(test)]
mod tests;

fn bad(field: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G582",
        format!("Agent iterative lifecycle invariant failed: {field}"),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IterativeBudget {
    pub max_iterations: usize,
    pub max_stages: usize,
    pub max_steps_per_stage: usize,
}
impl Default for IterativeBudget {
    fn default() -> Self {
        Self {
            max_iterations: 32,
            max_stages: 97,
            max_steps_per_stage: DEFAULT_STAGE_STEPS,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IterativeStatus {
    Complete,
    Suspend,
    Fail,
    Rejected,
    ModelFailed,
    EffectFailed,
    Cancelled,
    BudgetExhausted,
}

/// Immutable evidence and terminal carrier. A suspended carrier grants no resume authority.
pub struct IterativeRun {
    status: IterativeStatus,
    iterations: usize,
    stages: Vec<StageRecord>,
    effects: usize,
    value: Option<RetainedValue>,
    authorization_bindings: Vec<String>,
    invocation_digest: String,
    evidence: String,
    digest: String,
}
impl IterativeRun {
    pub fn invocation_digest(&self) -> &str {
        &self.invocation_digest
    }
    pub fn status(&self) -> IterativeStatus {
        self.status
    }
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    pub fn stages(&self) -> &[StageRecord] {
        &self.stages
    }
    pub fn effects(&self) -> usize {
        self.effects
    }
    pub fn value(&self) -> Option<&RetainedValue> {
        self.value.as_ref()
    }
    pub fn authorization_bindings(&self) -> &[String] {
        &self.authorization_bindings
    }

    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    pub fn evidence_digest(&self) -> &str {
        &self.digest
    }
    fn finish(
        mut self,
        status: IterativeStatus,
        value: Option<RetainedValue>,
        policy: &str,
    ) -> Self {
        self.status = status;
        self.value = value;
        let stage_rows: Vec<_> = self
            .stages
            .iter()
            .map(|s| {
                format!(
                    "[{}, {},{},{}]",
                    quote_json(s.role),
                    quote_json(&s.function_id),
                    quote_json(s.outcome),
                    s.steps_used
                )
            })
            .collect();
        let bindings: Vec<_> = self
            .authorization_bindings
            .iter()
            .map(|s| quote_json(s))
            .collect();
        let carrier = self.value.as_ref().map(|v| {
            digest(
                b"semaprax.agent-step.value.v2\0",
                encode_value(v).as_bytes(),
            )
        });
        self.evidence = format!("{{\"schema\":\"semaprax.agent-iterative-evidence.v2\",\"policy\":{},\"invocation_digest\":{},\"status\":{},\"iterations\":{},\"effects\":{},\"stages\":[{}],\"authorizations\":[{}],\"value_digest\":{}}}\n", quote_json(policy), quote_json(&self.invocation_digest), quote_json(&format!("{status:?}")), self.iterations, self.effects, stage_rows.join(","), bindings.join(","), carrier.as_ref().map(|s| quote_json(s)).unwrap_or_else(|| "null".into()));
        self.digest = digest(
            b"semaprax.agent-iterative-evidence.v2\0",
            self.evidence.as_bytes(),
        );
        self
    }
}

pub struct CompiledIterativeLifecycle {
    inner: CompiledAgentLifecycle,
    step: step::StepShape,
}
impl CompiledIterativeLifecycle {
    pub fn digest(&self) -> &str {
        &self.inner.digest
    }
    pub fn canonical_json(&self) -> &str {
        &self.inner.source
    }
    pub fn proposal_schema(&self) -> &CompiledAgentProposalSchema {
        &self.inner.proposal
    }
    pub fn source_revision(&self) -> &str {
        &self.inner.source_revision
    }

    /// Initialize once, then observe/decode/authorize/execute/reduce in order.
    /// Every iteration mints a new opaque authorization with a turn-specific policy.
    pub fn run(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
        read: &mut dyn AgentReadOperation,
        budget: IterativeBudget,
        cancellation: &AgentCancellation,
    ) -> Result<IterativeRun, Vec<Diagnostic>> {
        if budget.max_iterations > 4096 || budget.max_stages > 12289 {
            return Err(vec![bad("budget.capacity")]);
        }
        let inner = &self.inner;
        let mut run = IterativeRun {
            status: IterativeStatus::BudgetExhausted,
            iterations: 0,
            stages: Vec::new(),
            effects: 0,
            value: None,
            authorization_bindings: Vec::new(),
            invocation_digest: invocation_digest(task, proposals, budget),
            evidence: String::new(),
            digest: String::new(),
        };
        macro_rules! stop {
            ($status:expr, $value:expr) => {
                return Ok(run.finish($status, $value, self.digest()))
            };
        }
        macro_rules! boundary {
            () => {
                if cancellation.is_cancelled() {
                    stop!(IterativeStatus::Cancelled, None);
                }
                if run.stages.len() >= budget.max_stages {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
            };
        }
        macro_rules! evaluate {
            ($stage:expr, $arguments:expr) => {{
                boundary!();
                let evaluation = inner.evaluate($stage, $arguments, budget.max_steps_per_stage)?;
                run.stages.push(StageRecord::of($stage, &evaluation));
                match evaluation.outcome {
                    RetainedCallOutcome::Returned(value) => value,
                    RetainedCallOutcome::FuelExhausted | RetainedCallOutcome::CallDepthExceeded => {
                        stop!(IterativeStatus::BudgetExhausted, None);
                    }
                    _ => {
                        stop!(IterativeStatus::Rejected, None);
                    }
                }
            }};
        }
        if budget.max_iterations == 0 {
            stop!(IterativeStatus::BudgetExhausted, None);
        }
        let mut state = evaluate!(
            &inner.binding.initialize,
            &[payload(
                &inner.binding.task,
                task.objective.clone(),
                task.budget
            )]
        );
        if !inner.carries(&state, "state") {
            return Err(vec![bad("initialize.identity")]);
        }
        loop {
            if run.iterations >= budget.max_iterations {
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            let observation = evaluate!(&inner.binding.observe, std::slice::from_ref(&state));
            if !inner.carries(&observation, "observation") {
                return Err(vec![bad("observe.identity")]);
            }
            let Some(proposal_source) = proposals.get(run.iterations) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            let Ok(decoded) = inner.proposal.decode(proposal_source) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            let Some(projected) = inner.project(&decoded) else {
                stop!(IterativeStatus::ModelFailed, None);
            };
            boundary!();
            let policy = digest(
                b"semaprax.agent-iteration-policy.v2\0",
                format!("{}\0{}", self.digest(), run.iterations).as_bytes(),
            );
            let mut args = vec![state.clone()];
            args.extend(projected.iter().cloned());
            let (decision, record) = authorization::run_authorize_stage(
                &inner.program,
                &inner.binding.authorize,
                &args,
                budget.max_steps_per_stage,
                &policy,
                &state,
                decoded.canonical_json(),
            )?;
            run.stages.push(record);
            let authorized = match decision {
                authorization::AuthorizationOutcome::Granted(value) => value,
                authorization::AuthorizationOutcome::Refused(_) => {
                    stop!(IterativeStatus::Rejected, None);
                }
                authorization::AuthorizationOutcome::Undecided("fuel" | "depth") => {
                    stop!(IterativeStatus::BudgetExhausted, None);
                }
                _ => {
                    stop!(IterativeStatus::Rejected, None);
                }
            };
            if cancellation.is_cancelled() {
                stop!(IterativeStatus::Cancelled, None);
            }
            // Reserve reducer capacity before dispatch: no known-doomed effect.
            if run.stages.len() >= budget.max_stages {
                stop!(IterativeStatus::BudgetExhausted, None);
            }
            let request = authorized.consume();
            let expected = authorization::binding(
                &policy,
                &state,
                decoded.canonical_json(),
                inner.binding.authorize.grant_case(),
                request.seal(),
            );
            if expected != request.binding() {
                return Err(vec![bad("authorization.binding")]);
            }
            run.authorization_bindings
                .push(request.binding().to_owned());
            run.effects += 1;
            let Some(bytes) = read.read(&request) else {
                stop!(IterativeStatus::EffectFailed, None);
            };
            if bytes.len() > MAX_READ_BYTES {
                stop!(IterativeStatus::EffectFailed, None);
            }
            let mut args = vec![state];
            args.extend(projected);
            args.push(payload(&inner.binding.outcome, bytes, 0));
            let value = evaluate!(&inner.binding.reduce, &args);
            run.iterations += 1;
            let (transition, value) = self.step.decode(value)?;
            match transition {
                "Continue" => state = value,
                "Complete" => stop!(IterativeStatus::Complete, Some(value)),
                "Suspend" => stop!(IterativeStatus::Suspend, Some(value)),
                "Fail" => stop!(IterativeStatus::Fail, Some(value)),
                _ => return Err(vec![bad("step.transition")]),
            }
        }
    }
}

/// Compile the additive Step-returning reducer profile. Definition v1 bytes stay frozen.
pub fn compile_agent_lifecycle_v2(
    module_source: &str,
    module_path: impl AsRef<Path>,
    definition_source: &str,
    step_type_id: &str,
) -> Result<CompiledIterativeLifecycle, Vec<Diagnostic>> {
    let module_path = module_path.as_ref();
    let compiled = compile_agent_definition(definition_source)?;
    let definition = compiled.definition();
    let checked = crate::check(module_source, module_path)?;
    let source_revision = crate::graph::revision(&checked);
    let program = hir::resolve(&checked)?;
    let mut type_ids = Vec::new();
    for role in stages::TYPE_ROLES {
        type_ids.push((
            role,
            definition
                .type_id(role)
                .ok_or_else(|| vec![bad("type.role")])?
                .to_owned(),
        ));
    }
    let mut operation_ids = Vec::new();
    for role in stages::DETERMINISTIC_ROLES {
        let (id, kind) = definition
            .operation(role)
            .ok_or_else(|| vec![bad("operation.role")])?;
        if kind != "deterministic" {
            return Err(vec![bad("operation.kind")]);
        }
        operation_ids.push((role, id.to_owned()));
    }
    let step = step::StepShape::bind(&program, step_type_id, &type_ids[1].1, &type_ids[5].1)?;
    let binding =
        stages::bind_with_step_result(&program, &type_ids, &operation_ids, Some(&step.id))?;
    let proposal = compile_agent_proposal_schema(module_source, module_path, definition_source)?;
    let source = render::render(
        definition.agent_id(),
        definition.digest(),
        proposal.schema().digest(),
        &binding,
        &source_revision,
        &step,
    );
    if source.len() > MAX_LIFECYCLE_BYTES {
        return Err(vec![bad("lifecycle.bytes")]);
    }
    let inner = CompiledAgentLifecycle {
        agent_id: definition.agent_id().to_owned(),
        definition_digest: definition.digest().to_owned(),
        source_revision,
        program,
        proposal,
        binding,
        digest: digest(
            b"semaprax.agent-iterative-lifecycle.v2\0",
            source.as_bytes(),
        ),
        source,
    };
    Ok(CompiledIterativeLifecycle { inner, step })
}

/// Select and lower the exact checked source Agent before binding its iterative stages.
pub fn compile_source_agent_lifecycle_v2(
    module_source: &str,
    module_path: impl AsRef<Path>,
    agent_id: &str,
    step_type_id: &str,
) -> Result<CompiledIterativeLifecycle, Vec<Diagnostic>> {
    let module_path = module_path.as_ref();
    let checked = crate::check(module_source, module_path)?;
    let mut selected = checked
        .agents
        .iter()
        .filter(|agent| agent.stable_id == agent_id);
    let agent = selected
        .next()
        .ok_or_else(|| vec![bad("source_agent.selection")])?;
    if selected.next().is_some() {
        return Err(vec![bad("source_agent.duplicate")]);
    }
    let definition = crate::project::compile_source_agent_declaration(agent)?;
    let lifecycle = compile_agent_lifecycle_v2(
        module_source,
        module_path,
        definition.definition().canonical_source(),
        step_type_id,
    )?;
    if lifecycle.inner.agent_id != agent.stable_id
        || lifecycle.inner.definition_digest != definition.definition().digest()
    {
        return Err(vec![bad("source_agent.definition")]);
    }
    Ok(lifecycle)
}

fn invocation_digest(
    task: &LifecycleTask,
    proposals: &[String],
    budget: IterativeBudget,
) -> String {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.agent-iterative-invocation.v2\0");
    hash.update((task.objective.len() as u64).to_le_bytes());
    hash.update(&task.objective);
    hash.update(task.budget.to_le_bytes());
    for limit in [
        budget.max_iterations,
        budget.max_stages,
        budget.max_steps_per_stage,
    ] {
        hash.update((limit as u64).to_le_bytes());
    }
    hash.update((proposals.len() as u64).to_le_bytes());
    for proposal in proposals {
        hash.update((proposal.len() as u64).to_le_bytes());
        hash.update(proposal.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
