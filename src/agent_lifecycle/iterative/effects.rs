//! Additive typed host boundary bound to exact deployed tool contracts.
//! Reducers retain Outcome{Bytes,i64}; Bytes contains the canonical typed result.
use super::*;
pub mod durable;
pub use durable::{DurableTypedFailure, DurableTypedRun};
use crate::agent_deployment::BoundAgentDeployment;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectScalar {
    Bool,
    I32,
    I64,
    U8,
    Usize,
}
impl EffectScalar {
    fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::U8 => "u8",
            Self::Usize => "usize",
        }
    }
    fn schema_name(self) -> &'static str {
        if self == Self::Bool {
            "boolean"
        } else {
            "integer"
        }
    }
    fn accepts(self, value: &RetainedValue) -> bool {
        matches!(
            (self, value),
            (Self::Bool, RetainedValue::Bool(_))
                | (Self::I32, RetainedValue::I32(_))
                | (Self::I64, RetainedValue::I64(_))
                | (Self::U8, RetainedValue::U8(_))
                | (Self::Usize, RetainedValue::Usize(_))
        )
    }
}

/// Exact source-owned tool schema key bound to a checked Proposal field identity.
pub struct EffectArgument {
    pub argument_id: String,
    pub proposal_field_id: String,
    pub kind: EffectScalar,
}
pub struct EffectResult {
    pub result_id: String,
    pub kind: EffectScalar,
}
pub struct EffectOperation {
    pub operation_id: String,
    pub effect_id: String,
    pub arguments: Vec<EffectArgument>,
    pub results: Vec<EffectResult>,
}

/// Immutable request reachable only after the current checked authorize transition.
pub struct TypedEffectRequest<'a> {
    authorization: &'a AuthorizedRequest,
    operation: &'a EffectOperation,
    arguments: Vec<(String, RetainedValue)>,
}
impl TypedEffectRequest<'_> {
    pub fn authorization(&self) -> &AuthorizedRequest {
        self.authorization
    }
    pub fn operation_id(&self) -> &str {
        &self.operation.operation_id
    }
    pub fn effect_id(&self) -> &str {
        &self.operation.effect_id
    }
    pub fn arguments(&self) -> &[(String, RetainedValue)] {
        &self.arguments
    }
}

pub trait TypedEffectHandler {
    fn execute(&mut self, request: &TypedEffectRequest<'_>)
        -> Option<Vec<(String, RetainedValue)>>;
}

#[derive(Clone, Copy)]
pub struct EffectBudget {
    pub max_calls: usize,
    pub max_argument_bytes: usize,
    pub max_result_bytes: usize,
    pub max_total_bytes: usize,
}

pub struct TypedEffectRun {
    lifecycle: IterativeRun,
    dispatched: usize,
    argument_bytes: usize,
    result_bytes: usize,
    failure: Option<&'static str>,
    evidence: String,
    digest: String,
}
impl TypedEffectRun {
    pub fn lifecycle(&self) -> &IterativeRun {
        &self.lifecycle
    }
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }
    pub fn argument_bytes(&self) -> usize {
        self.argument_bytes
    }
    pub fn result_bytes(&self) -> usize {
        self.result_bytes
    }
    pub fn failure(&self) -> Option<&str> {
        self.failure
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
    pub fn evidence_digest(&self) -> &str {
        &self.digest
    }
}

pub struct CompiledTypedEffects {
    lifecycle: CompiledIterativeLifecycle,
    operations: Vec<EffectOperation>,
    selector: String,
    limits: EffectBudget,
    max_iterations: usize,
    field_limits: Vec<(Vec<usize>, Vec<usize>)>,
}

fn error(field: &str) -> Vec<Diagnostic> {
    vec![bad(&format!("typed_effects.{field}"))]
}

/// Bound deployment supplies every operation/effect/schema key and ceiling.
/// The registry only narrows those contracts to exact retained scalar types.
pub fn compile_typed_effects(
    module_source: &str,
    module_path: impl AsRef<Path>,
    deployment: &BoundAgentDeployment,
    step_type_id: &str,
    selector_field_id: &str,
    operations: Vec<EffectOperation>,
) -> Result<CompiledTypedEffects, Vec<Diagnostic>> {
    let mut lifecycle = compile_agent_lifecycle_v2(
        module_source,
        module_path,
        deployment.runtime_v1_definition(),
        step_type_id,
    )?;
    let definition: Value = serde_json::from_str(deployment.semantic_definition().canonical_json())
        .map_err(|_| error("definition"))?;
    let bound: Value =
        serde_json::from_str(deployment.canonical_json()).map_err(|_| error("deployment"))?;
    if deployment.semantic_definition().agent_id() != lifecycle.inner.agent_id {
        return Err(error("agent.identity"));
    }
    if operations.is_empty() || operations.len() > 64 {
        return Err(error("operations.capacity"));
    }
    if !lifecycle
        .inner
        .binding
        .proposal
        .iter()
        .any(|p| p.field.as_str() == selector_field_id && p.kind == ScalarKind::Usize)
    {
        return Err(error("selector.type"));
    }
    let allowed = bound["effective"]["allowed_tool_ids"]
        .as_array()
        .ok_or_else(|| error("allowed_operations"))?;
    let tools = definition["tools"]
        .as_array()
        .ok_or_else(|| error("tools"))?;
    let mut seen = BTreeSet::new();
    let mut rows = Vec::new();
    let mut field_limits = Vec::new();
    for operation in &operations {
        if !seen.insert(&operation.operation_id)
            || !allowed
                .iter()
                .any(|v| v.as_str() == Some(&operation.operation_id))
        {
            return Err(error("operation.identity"));
        }
        let tool = tools
            .iter()
            .find(|v| v["tool_id"].as_str() == Some(&operation.operation_id))
            .ok_or_else(|| error("operation.contract"))?;
        let effects = tool["effects"]
            .as_array()
            .ok_or_else(|| error("operation.effects"))?;
        // No undeclared additional effect is hidden behind a selected identity.
        if effects.len() != 1 || effects[0].as_str() != Some(&operation.effect_id) {
            return Err(error("effect.identity"));
        }
        let argument_fields = tool["arguments_schema"]["fields"]
            .as_array()
            .ok_or_else(|| error("argument.schema"))?;
        let result_fields = tool["result_schema"]["fields"]
            .as_array()
            .ok_or_else(|| error("result.schema"))?;
        if argument_fields.len() != operation.arguments.len()
            || result_fields.len() != operation.results.len()
            || argument_fields.len() > 8
            || result_fields.is_empty()
            || result_fields.len() > 8
        {
            return Err(error("fields.capacity"));
        }
        let read_limits = |fields: &[Value]| -> Result<Vec<usize>, Vec<Diagnostic>> {
            fields
                .iter()
                .map(|f| {
                    f["max_bytes"]
                        .as_u64()
                        .and_then(|n| usize::try_from(n).ok())
                        .ok_or_else(|| error("field.byte_limit"))
                })
                .collect()
        };
        field_limits.push((read_limits(argument_fields)?, read_limits(result_fields)?));
        let mut arguments = Vec::new();
        for (field, argument) in argument_fields.iter().zip(&operation.arguments) {
            if field["name"].as_str() != Some(&argument.argument_id)
                || field["type"].as_str() != Some(argument.kind.schema_name())
                || field["required"] != true
            {
                return Err(error("argument.identity"));
            }
            if !lifecycle.inner.binding.proposal.iter().any(|p| {
                p.field.as_str() == argument.proposal_field_id
                    && p.kind.name() == argument.kind.name()
            }) {
                return Err(error("argument.projection"));
            }
            arguments.push(format!(
                "[{}, {},{}]",
                quote_json(&argument.argument_id),
                quote_json(&argument.proposal_field_id),
                quote_json(argument.kind.name())
            ));
        }
        let mut results = Vec::new();
        for (field, result) in result_fields.iter().zip(&operation.results) {
            if field["name"].as_str() != Some(&result.result_id)
                || field["type"].as_str() != Some(result.kind.schema_name())
                || field["required"] != true
            {
                return Err(error("result.identity"));
            }
            results.push(format!(
                "[{},{}]",
                quote_json(&result.result_id),
                quote_json(result.kind.name())
            ));
        }
        rows.push(format!(
            "{{\"operation\":{},\"effect\":{},\"arguments\":[{}],\"results\":[{}]}}",
            quote_json(&operation.operation_id),
            quote_json(&operation.effect_id),
            arguments.join(","),
            results.join(",")
        ));
    }
    let limits = &bound["effective"]["limits"];
    let limit = |key: &str| {
        limits[key]
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| error("limits"))
    };
    let max_iterations = limit("max_turns")?.min(4096);
    let limits = EffectBudget {
        max_calls: limit("max_tool_calls")?,
        max_argument_bytes: limit("max_tool_arguments_bytes")?,
        max_result_bytes: limit("max_tool_result_bytes")?.min(MAX_READ_BYTES),
        max_total_bytes: limit("max_total_tool_bytes")?,
    };
    let source = format!("{{\"schema\":\"semaprax.agent-typed-effects.v3\",\"deployment_digest\":{},\"selector\":{},\"operations\":[{}],\"result_transport\":\"canonical_typed_fields_in_outcome_bytes\",\"lifecycle\":{}}}\n", quote_json(deployment.digest()), quote_json(selector_field_id), rows.join(","), lifecycle.canonical_json().trim_end());
    if source.len() > MAX_LIFECYCLE_BYTES {
        return Err(error("document.capacity"));
    }
    lifecycle.inner.digest = digest(b"semaprax.agent-typed-effects.v3\0", source.as_bytes());
    lifecycle.inner.source = source;
    Ok(CompiledTypedEffects {
        lifecycle,
        operations,
        selector: selector_field_id.to_owned(),
        limits,
        max_iterations,
        field_limits,
    })
}

fn encode_fields(fields: &[(String, RetainedValue)]) -> String {
    let rows: Vec<_> = fields
        .iter()
        .map(|(id, value)| format!("[{},{}]", quote_json(id), encode_value(value)))
        .collect();
    format!(
        "{{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[{}]}}\n",
        rows.join(",")
    )
}

struct Dispatch<'a> {
    compiled: &'a CompiledTypedEffects,
    proposals: &'a [String],
    handler: &'a mut dyn TypedEffectHandler,
    budget: EffectBudget,
    dispatched: usize,
    arguments: usize,
    results: usize,
    failure: Option<&'static str>,
}
impl Dispatch<'_> {
    fn invoke(&mut self, authorization: &AuthorizedRequest) -> Result<Vec<u8>, &'static str> {
        if self.dispatched >= self.budget.max_calls {
            return Err("call_budget");
        }
        let lifecycle = &self.compiled.lifecycle;
        let source = self
            .proposals
            .get(self.dispatched)
            .ok_or("proposal_index")?;
        let decoded = lifecycle
            .inner
            .proposal
            .decode(source)
            .map_err(|_| "proposal_decode")?;
        let Some(ProposalValue::Unsigned(selector)) = decoded.field(&self.compiled.selector) else {
            return Err("selector_type");
        };
        let index = usize::try_from(*selector).map_err(|_| "selector_range")?;
        let operation = self
            .compiled
            .operations
            .get(index)
            .ok_or("selector_range")?;
        let projected = lifecycle.inner.project(&decoded).ok_or("projection")?;
        let mut arguments = Vec::new();
        for argument in &operation.arguments {
            let index = lifecycle
                .inner
                .binding
                .proposal
                .iter()
                .position(|p| p.field.as_str() == argument.proposal_field_id)
                .ok_or("argument_identity")?;
            let value = projected.get(index).ok_or("argument_index")?;
            if !argument.kind.accepts(value) {
                return Err("argument_type");
            }
            arguments.push((argument.argument_id.clone(), value.clone()));
        }
        for ((_, value), limit) in arguments.iter().zip(&self.compiled.field_limits[index].0) {
            if scalar_bytes(value).ok_or("argument_scalar")? > *limit {
                return Err("argument_field_budget");
            }
        }
        let bytes = encode_fields(&arguments).len();
        let total = self
            .arguments
            .checked_add(self.results)
            .and_then(|v| v.checked_add(bytes))
            .ok_or("byte_overflow")?;
        if bytes > self.budget.max_argument_bytes || total > self.budget.max_total_bytes {
            return Err("argument_budget");
        }
        self.arguments += bytes;
        self.dispatched += 1;
        let request = TypedEffectRequest {
            authorization,
            operation,
            arguments,
        };
        let result = self.handler.execute(&request).ok_or("handler_failed")?;
        // Meter before shape validation. Oversized/too-deep encodings saturate
        // at the hard transport bound + 1; no giant JSON allocation occurs.
        let attempted = measured_fields(&result, MAX_READ_BYTES);
        self.results = self.results.saturating_add(attempted);
        if attempted > self.budget.max_result_bytes
            || self.arguments.saturating_add(self.results) > self.budget.max_total_bytes
        {
            return Err("result_budget");
        }
        if result.len() != operation.results.len() {
            return Err("result_fields");
        }
        for ((id, value), expected) in result.iter().zip(&operation.results) {
            if id != &expected.result_id || !expected.kind.accepts(value) {
                return Err("result_type");
            }
        }
        for ((_, value), limit) in result.iter().zip(&self.compiled.field_limits[index].1) {
            if scalar_bytes(value).ok_or("result_scalar")? > *limit {
                return Err("result_field_budget");
            }
        }
        let encoded = encode_fields(&result);
        if encoded.len() != attempted {
            return Err("result_measurement");
        }
        Ok(encoded.into_bytes())
    }
}
impl AgentReadOperation for Dispatch<'_> {
    fn read(&mut self, authorization: &AuthorizedRequest) -> Option<Vec<u8>> {
        match self.invoke(authorization) {
            Ok(value) => Some(value),
            Err(reason) => {
                self.failure = Some(reason);
                None
            }
        }
    }
}

impl CompiledTypedEffects {
    pub fn canonical_json(&self) -> &str {
        self.lifecycle.canonical_json()
    }
    pub fn digest(&self) -> &str {
        self.lifecycle.digest()
    }
    pub fn proposal_schema(&self) -> &CompiledAgentProposalSchema {
        self.lifecycle.proposal_schema()
    }
    pub fn run(
        &self,
        task: &LifecycleTask,
        proposals: &[String],
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
        let mut dispatch = Dispatch {
            compiled: self,
            proposals,
            handler,
            budget,
            dispatched: 0,
            arguments: 0,
            results: 0,
            failure: None,
        };
        let stages = IterativeBudget {
            max_iterations: stages.max_iterations.min(self.max_iterations),
            ..stages
        };
        let lifecycle = self
            .lifecycle
            .run(task, proposals, &mut dispatch, stages, cancellation)?;
        let evidence = format!("{{\"schema\":\"semaprax.agent-typed-effects-evidence.v3\",\"registry\":{},\"lifecycle_evidence\":{},\"limits\":[{},{},{},{}],\"dispatched\":{},\"argument_bytes\":{},\"result_bytes\":{},\"failure\":{}}}\n", quote_json(self.digest()), quote_json(lifecycle.evidence_digest()), budget.max_calls, budget.max_argument_bytes, budget.max_result_bytes, budget.max_total_bytes, dispatch.dispatched, dispatch.arguments, dispatch.results, dispatch.failure.map(quote_json).unwrap_or_else(|| "null".into()));
        Ok(TypedEffectRun {
            lifecycle,
            dispatched: dispatch.dispatched,
            argument_bytes: dispatch.arguments,
            result_bytes: dispatch.results,
            failure: dispatch.failure,
            digest: digest(
                b"semaprax.agent-typed-effects-evidence.v3\0",
                evidence.as_bytes(),
            ),
            evidence,
        })
    }
}

fn scalar_bytes(value: &RetainedValue) -> Option<usize> {
    Some(match value {
        RetainedValue::Bool(value) => {
            if *value {
                4
            } else {
                5
            }
        }
        RetainedValue::I32(value) => value.to_string().len(),
        RetainedValue::I64(value) => value.to_string().len(),
        RetainedValue::U8(value) => value.to_string().len(),
        RetainedValue::Usize(value) => i64::try_from(*value).ok()?.to_string().len(),
        _ => return None,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::agent_lifecycle::tests::{DEFINITION, MODULE, RUNTIME_V1};
    pub(crate) fn source(terminal: &str) -> String {
        let start = MODULE.find("@id(\"fixture.agent.fn.reduce\")").unwrap();
        let end = MODULE[start..].find("@id(\"app.main\")").unwrap() + start;
        let reducer = format!(
            r#"
@id("fixture.agent.type.step")
variant Step {{
    @id("fixture.agent.step.continue") Continue {{
        @id("fixture.agent.step.continue.objective") objective: Bytes,
        @id("fixture.agent.step.continue.budget") budget: i64,
        @id("fixture.agent.step.continue.epoch") epoch: i64,
    }},
    @id("fixture.agent.step.complete") Complete {{
        @id("fixture.agent.step.complete.summary") summary: Bytes,
        @id("fixture.agent.step.complete.budget") budget: i64,
        @id("fixture.agent.step.complete.status") status: i64,
    }},
    @id("fixture.agent.step.suspend") Suspend {{
        @id("fixture.agent.step.suspend.objective") objective: Bytes,
        @id("fixture.agent.step.suspend.budget") budget: i64,
        @id("fixture.agent.step.suspend.epoch") epoch: i64,
    }},
    @id("fixture.agent.step.fail") Fail {{
        @id("fixture.agent.step.fail.code") code: i64,
    }},
}}
@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Step {{
    if state.epoch < 3 {{
        Step::Continue {{ objective: state.objective, budget: state.budget, epoch: state.epoch + 1 }}
    }} else {{ {terminal} }}
}}
"#
        );
        format!("{}{}{}", &MODULE[..start], reducer, &MODULE[end..])
    }

    pub(crate) fn deployment() -> BoundAgentDeployment {
        deployment_turns(4)
    }
    pub(crate) fn deployment_turns(turns: usize) -> BoundAgentDeployment {
        let runtime = RUNTIME_V1
            .replace(
                "\"type\":\"string\",\"required\":true,\"max_bytes\":64",
                "\"type\":\"integer\",\"required\":true,\"max_bytes\":20",
            )
            .replace("\"max_tool_calls\":1", "\"max_tool_calls\":4")
            .replace("\"max_turns\":2", &format!("\"max_turns\":{turns}"));
        let value: Value = serde_json::from_str(&runtime).unwrap();
        let mut tools = value["tools"].clone();
        let mut second = tools[0].clone();
        second["tool_id"] = Value::String("fixture.read.second".into());
        tools.as_array_mut().unwrap().push(second);
        let start = runtime.find("\"tools\":").unwrap() + "\"tools\":".len();
        let end = runtime.find(",\"policy\":").unwrap();
        let runtime = format!(
            "{}{}{}",
            &runtime[..start],
            crate::agent_definition::render_tools(&tools).unwrap(),
            &runtime[end..]
        )
        .replace(
            "\"allowed_tool_ids\":[\"fixture.read\"]",
            "\"allowed_tool_ids\":[\"fixture.read\",\"fixture.read.second\"]",
        );
        let definition = DEFINITION.replace("RUNTIME", &runtime);
        let (semantic, deployment) = crate::agent_deployment::migrate_agent_definition_v1(
            &definition,
            "fixture.typed.deployment",
        )
        .unwrap();
        crate::agent_deployment::bind_agent_deployment(&semantic, &deployment).unwrap()
    }
    pub(crate) fn operation() -> EffectOperation {
        EffectOperation {
            operation_id: "fixture.read".into(),
            effect_id: "read".into(),
            arguments: vec![EffectArgument {
                argument_id: "query".into(),
                proposal_field_id: "fixture.agent.type.proposal.budget".into(),
                kind: EffectScalar::I64,
            }],
            results: vec![EffectResult {
                result_id: "value".into(),
                kind: EffectScalar::I64,
            }],
        }
    }
    pub(crate) fn compile() -> CompiledTypedEffects {
        let source = source("Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }").replace("sequence > 0usize", "sequence <= 1usize");
        let mut second = operation();
        second.operation_id = "fixture.read.second".into();
        compile_typed_effects(
            &source,
            "typed-effects.spx",
            &deployment(),
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            vec![operation(), second],
        )
        .unwrap_or_else(|e| panic!("{e:?}"))
    }
    struct Handler {
        calls: usize,
        wrong: bool,
    }
    impl TypedEffectHandler for Handler {
        fn execute(
            &mut self,
            request: &TypedEffectRequest<'_>,
        ) -> Option<Vec<(String, RetainedValue)>> {
            self.calls += 1;
            assert!(matches!(
                request.operation_id(),
                "fixture.read" | "fixture.read.second"
            ));
            assert_eq!(request.effect_id(), "read");
            assert_eq!(
                request.arguments(),
                &[("query".into(), RetainedValue::I64(1))]
            );
            Some(vec![(
                "value".into(),
                if self.wrong {
                    RetainedValue::Bool(true)
                } else {
                    RetainedValue::I64(8)
                },
            )])
        }
    }
    fn budgets() -> EffectBudget {
        EffectBudget {
            max_calls: 4,
            max_argument_bytes: 4096,
            max_result_bytes: 4096,
            max_total_bytes: 8192,
        }
    }
    #[test]
    fn typed_registry_dispatches_fresh_authorized_checked_scalar_results() {
        let compiled = compile();
        let proposals =
            vec![crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0"); 3];
        let mut proposals = proposals;
        proposals[1] = crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "1");
        let mut handler = Handler {
            calls: 0,
            wrong: false,
        };
        let run = compiled
            .run(
                &LifecycleTask {
                    objective: vec![],
                    budget: 10,
                },
                &proposals,
                &mut handler,
                IterativeBudget::default(),
                budgets(),
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!(run.lifecycle().status(), IterativeStatus::Complete);
        assert_eq!((run.dispatched(), handler.calls), (3, 3));
        assert_ne!(
            run.lifecycle().authorization_bindings()[0],
            run.lifecycle().authorization_bindings()[1]
        );
        assert!(run.result_bytes() > 0);
    }
    #[test]
    fn typed_result_mismatch_and_call_ceiling_stop_without_another_effect() {
        let compiled = compile();
        let proposals =
            vec![crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0"); 3];
        for wrong in [true, false] {
            let mut handler = Handler { calls: 0, wrong };
            let run = compiled
                .run(
                    &LifecycleTask {
                        objective: vec![],
                        budget: 10,
                    },
                    &proposals,
                    &mut handler,
                    IterativeBudget::default(),
                    EffectBudget {
                        max_calls: 1,
                        ..budgets()
                    },
                    &AgentCancellation::new(),
                )
                .unwrap();
            assert_eq!(run.lifecycle().status(), IterativeStatus::EffectFailed);
            assert_eq!((run.dispatched(), handler.calls), (1, 1));
            assert!(run.result_bytes() > 0, "failed host work must stay charged");
            assert_eq!(
                run.failure(),
                Some(if wrong { "result_type" } else { "call_budget" })
            );
        }
    }
    #[test]
    fn deployment_effect_and_projection_substitutions_are_rejected() {
        let source = source("Step::Fail { code: 1 }");
        for wrong_effect in [true, false] {
            let mut operation = operation();
            if wrong_effect {
                operation.effect_id = "write".into();
            } else {
                operation.arguments[0].proposal_field_id =
                    "fixture.agent.type.proposal.urgent".into();
            }
            assert!(compile_typed_effects(
                &source,
                "typed-effects.spx",
                &deployment(),
                "fixture.agent.type.step",
                "fixture.agent.type.proposal.sequence",
                vec![operation]
            )
            .is_err());
        }
    }
    #[test]
    fn deployment_turn_ceiling_intersects_caller_iteration_budget() {
        let source =
            source("Step::Fail { code: 1 }").replace("sequence > 0usize", "sequence == 0usize");
        let compiled = compile_typed_effects(
            &source,
            "typed-effects.spx",
            &deployment_turns(2),
            "fixture.agent.type.step",
            "fixture.agent.type.proposal.sequence",
            vec![operation()],
        )
        .unwrap();
        let proposals =
            vec![crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0"); 4];
        let mut handler = Handler {
            calls: 0,
            wrong: false,
        };
        let run = compiled
            .run(
                &LifecycleTask {
                    objective: vec![],
                    budget: 10,
                },
                &proposals,
                &mut handler,
                IterativeBudget::default(),
                budgets(),
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!(run.lifecycle().status(), IterativeStatus::BudgetExhausted);
        assert_eq!((run.lifecycle().iterations(), handler.calls), (2, 2));
    }

    #[test]
    fn failed_and_oversized_host_encoding_is_measured_before_validation() {
        let small = vec![("wrong\nkey".into(), RetainedValue::Bytes(vec![1, 2, 3]))];
        assert_eq!(
            measured_fields(&small, MAX_READ_BYTES),
            encode_fields(&small).len()
        );
        let massive = vec![(
            "value".into(),
            RetainedValue::Bytes(vec![0; MAX_READ_BYTES]),
        )];
        assert_eq!(
            measured_fields(&massive, MAX_READ_BYTES),
            MAX_READ_BYTES + 1
        );
        struct Oversized;
        impl TypedEffectHandler for Oversized {
            fn execute(
                &mut self,
                _: &TypedEffectRequest<'_>,
            ) -> Option<Vec<(String, RetainedValue)>> {
                Some(vec![(
                    "value".into(),
                    RetainedValue::Bytes(vec![0; MAX_READ_BYTES]),
                )])
            }
        }
        let compiled = compile();
        let proposals =
            vec![crate::agent_lifecycle::tests::proposal(&compiled.lifecycle.inner, "1", "0"); 3];
        let run = compiled
            .run(
                &LifecycleTask {
                    objective: vec![],
                    budget: 10,
                },
                &proposals,
                &mut Oversized,
                IterativeBudget::default(),
                budgets(),
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!(run.failure(), Some("result_budget"));
        assert_eq!(run.result_bytes(), MAX_READ_BYTES + 1);
        assert_eq!(run.dispatched(), 1);
    }
}

/// Exact canonical encoded length up to cap; cap+1 is an explicit overflow
/// charge. Node/depth excess also charges overflow and cannot reach encoding.
fn measured_fields(fields: &[(String, RetainedValue)], cap: usize) -> usize {
    struct Meter {
        count: usize,
        cap: usize,
        nodes: usize,
    }
    impl Meter {
        fn add(&mut self, count: usize) {
            self.count = self.count.saturating_add(count).min(self.cap + 1);
        }
        fn full(&self) -> bool {
            self.count > self.cap
        }
        fn string(&mut self, value: &str) {
            self.add(2);
            if value.len() > self.cap.saturating_sub(self.count) {
                self.count = self.cap + 1;
                return;
            }
            for character in value.chars() {
                self.add(match character {
                    '"' | '\\' | '\n' | '\r' | '\t' => 2,
                    c if c.is_control() => 6,
                    c => c.len_utf8(),
                });
                if self.full() {
                    break;
                }
            }
        }
        fn record_fields(&mut self, fields: &[RetainedField], depth: usize) {
            self.add(2);
            for (index, field) in fields.iter().enumerate() {
                if self.full() {
                    break;
                }
                if index != 0 {
                    self.add(1);
                }
                self.add("{\"field\":,\"value\":}".len());
                self.string(field.field.as_str());
                self.value(&field.value, depth);
            }
        }
        fn value(&mut self, value: &RetainedValue, depth: usize) {
            if self.full() {
                return;
            }
            self.nodes += 1;
            if depth >= 32 || self.nodes > 4096 {
                self.count = self.cap + 1;
                return;
            }
            match value {
                RetainedValue::Bool(value) => self.add(if *value { 4 } else { 5 }),
                RetainedValue::I32(value) => self.add(value.to_string().len() + 2),
                RetainedValue::I64(value) => self.add(value.to_string().len() + 2),
                RetainedValue::U8(value) => self.add(value.to_string().len() + 2),
                RetainedValue::Usize(value) => self.add(value.to_string().len() + 2),
                RetainedValue::Bytes(bytes) => self.add(
                    "{\"bytes\":\"\"}"
                        .len()
                        .saturating_add(bytes.len().saturating_mul(2)),
                ),
                RetainedValue::Record(record) => {
                    self.add("{\"record\":,\"fields\":}".len());
                    self.string(record.record.as_str());
                    self.record_fields(&record.fields, depth + 1);
                }
                RetainedValue::Variant(variant) => {
                    self.add("{\"variant\":,\"case\":,\"fields\":}".len());
                    self.string(variant.variant.as_str());
                    self.string(variant.case.as_str());
                    self.record_fields(&variant.fields, depth + 1);
                }
            }
        }
    }
    let mut meter = Meter {
        count: 0,
        cap,
        nodes: 0,
    };
    meter.add("{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[]}\n".len());
    for (index, (id, value)) in fields.iter().enumerate() {
        if meter.full() {
            break;
        }
        if index != 0 {
            meter.add(1);
        }
        meter.add(3); // [key,value]
        meter.string(id);
        meter.value(value, 0);
    }
    meter.count
}
