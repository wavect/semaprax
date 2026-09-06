//! Agent Lifecycle v1: one acyclic lifecycle whose deterministic stage
//! identities resolve to actual verified `.spx` functions, executed through
//! the retained interpreter seam with a scripted model proposal.
//!
//! The compiler takes one checked module and one canonical AgentDefinition,
//! resolves the definition's four deterministic operation identities —
//! `initialize`, `observe`, `authorize`, `reduce` — to real functions in the
//! same HIR ordinary execution uses, and validates their signatures, ownership
//! modes, declared effects, role types and derived stage graph. Every one of
//! those checks runs before a lifecycle exists, so an unresolved identity, an
//! incompatible signature, an incorrect ownership mode, a declared effect on a
//! deterministic stage, or a cyclic stage graph is rejected before any host
//! work is reachable.
//!
//! Execution runs the four deterministic stages through
//! [`crate::interpreter::retained_call`]: one retained product per stage,
//! prepared once at compile time, dispatched by stable identity with no
//! re-verification and no source path. `propose` is a scripted, offline model
//! document decoded through Agent Proposal Schema v1. `execute` is one
//! explicitly injected read operation and nothing else — this module opens no
//! file, spawns no process, and contacts no network.
//!
//! # Authority
//!
//! The authorizing transition produces [`Authorized`], an opaque value bound
//! to the exact policy, state and proposal it was granted against.
//! [`authorization`] owns its only constructor and states why no other stage,
//! and no model output, can reach it.
//!
//! # Admitted vocabulary
//!
//! Stage arguments and results live inside the retained seam's closed
//! vocabulary: `bool`, `i32`, `i64`, `u8`, `usize`, owned `Bytes`, and bounded
//! records and owned-byte variants over those leaves. `string` is not
//! admitted, because admitting it would introduce a new owned cleanup leaf
//! kind ahead of the shared cleanup machinery and the backends.

use std::path::Path;

use sha2::{Digest, Sha256};

use crate::agent_definition::compile_agent_definition;
use crate::agent_proposal::{
    compile_agent_proposal_schema, CompiledAgentProposalSchema, DecodedProposal, ProposalValue,
};
use crate::agent_runtime::AgentCancellation;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir;
use crate::interpreter::retained_call::{
    evaluate_retained_call, RetainedCallEvaluation, RetainedCallOutcome, RetainedField,
    RetainedRecord, RetainedValue,
};

pub mod authorization;
pub mod durable;
mod source;
mod stages;

#[cfg(test)]
mod tests;

pub use authorization::{Authorized, AuthorizedRequest};
pub use durable::{
    bind_durable_agent, AgentCheckpoint, CheckpointBinding, CheckpointStore, CheckpointStoreError,
    CrashPoint, DurableAgent, DurableBudget, DurableRun, DurableStatus, ProgramCounter,
    Reconciliation, Retention, CHECKPOINT_SCHEMA, DURABLE_EVIDENCE_SCHEMA,
};
pub use source::{compile_source_agent_lifecycle, verify_source_agent_lifecycle_bundle};

use stages::{BoundStage, PayloadShape, ScalarKind, StageBinding};

/// Schema identity of the canonical compiled lifecycle document.
pub const LIFECYCLE_SCHEMA: &str = "semaprax.agent-lifecycle.v1";
/// Schema identity of one lifecycle run's evidence document.
pub const EVIDENCE_SCHEMA: &str = "semaprax.agent-lifecycle-evidence.v1";

const LIFECYCLE_DOMAIN: &[u8] = b"semaprax.agent-lifecycle.digest.v1\0";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.agent-lifecycle-evidence.digest.v1\0";

const MAX_LIFECYCLE_BYTES: usize = 262_144;
const MAX_READ_BYTES: usize = 65_536;

/// The default per-stage interpreter fuel one invocation may spend.
pub const DEFAULT_STAGE_STEPS: usize = 100_000;

const NONCLAIMS: [&str; 9] = [
    "no_authorization_value_from_a_proposal_an_observation_or_a_reduction",
    "no_reusable_authorization_value_one_grant_admits_one_effect",
    "no_effect_beyond_the_single_explicitly_injected_read_operation",
    "no_ambient_filesystem_process_network_home_or_secret_authority",
    "no_checkpoint_resume_reconciliation_or_durable_state",
    "no_iterative_agent_step_continue_suspend_or_fail_execution",
    "no_string_char_or_floating_point_stage_value_transport",
    "no_agent_definition_graph_proposal_schema_or_runtime_v1_byte_modification",
    "no_cli_surface_in_this_slice",
];

fn invariant(field: &str) -> Diagnostic {
    stages::invariant(field)
}

fn refused(detail: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G571",
        format!("AgentLifecycle refused an invocation before host work: {detail}"),
    )
}

fn bundle_mismatch() -> Diagnostic {
    Diagnostic::io(
        "SPX-G572",
        "AgentLifecycle bytes are not the exact replay of their verified source and AgentDefinition",
    )
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// The task one lifecycle starts from.
///
/// The Task role type is an admitted `{ Bytes, i64 }` record, so a task is
/// exactly one owned payload and one exact integer. The compiler resolves the
/// two field identities; the caller never names them.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleTask {
    pub objective: Vec<u8>,
    pub budget: i64,
}

/// The per-invocation execution budget.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleBudget {
    pub max_steps_per_stage: usize,
}

impl Default for LifecycleBudget {
    fn default() -> Self {
        Self {
            max_steps_per_stage: DEFAULT_STAGE_STEPS,
        }
    }
}

/// The single effect this lifecycle can perform.
///
/// It is explicitly injected by the caller. The compiler grants it nothing:
/// the implementation is the host's, and the lifecycle calls it at most once
/// per run, only after the authorizing transition granted an [`Authorized`]
/// and only with the value that authorization was spent into.
pub trait AgentReadOperation {
    /// Performs one bounded read. `None` is a failed effect.
    fn read(&mut self, request: &AuthorizedRequest) -> Option<Vec<u8>>;
}

/// An effect-free fixture read operation that returns fixed bytes.
pub struct FixtureRead {
    value: Vec<u8>,
    calls: usize,
}

impl FixtureRead {
    #[must_use]
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Self {
            value: value.into(),
            calls: 0,
        }
    }

    /// How many times the lifecycle invoked this operation.
    #[must_use]
    pub const fn calls(&self) -> usize {
        self.calls
    }
}

impl AgentReadOperation for FixtureRead {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        Some(self.value.clone())
    }
}

/// The six terminal conditions of one lifecycle invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleStatus {
    /// Every stage ran and `reduce` published a Result.
    Completed,
    /// The authorizing transition refused, or a deterministic stage did not
    /// decide. No effect was performed.
    Rejected,
    /// The scripted model produced no proposal this grammar admits.
    ModelFailed,
    /// The injected read operation failed.
    EffectFailed,
    /// Cancellation was observed at a stage boundary.
    Cancelled,
    /// A stage exhausted its interpreter fuel or its call depth.
    BudgetExhausted,
}

impl LifecycleStatus {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::ModelFailed => "model_failed",
            Self::EffectFailed => "effect_failed",
            Self::Cancelled => "cancelled",
            Self::BudgetExhausted => "budget_exhausted",
        }
    }
}

/// One executed stage's deterministic facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageRecord {
    role: &'static str,
    operation_id: String,
    function_id: String,
    outcome: &'static str,
    steps_used: usize,
    cleanup_events: usize,
}

impl StageRecord {
    pub(in crate::agent_lifecycle) fn of(
        stage: &BoundStage,
        evaluation: &RetainedCallEvaluation,
    ) -> Self {
        Self {
            role: stage.role(),
            operation_id: stage.operation_id().to_owned(),
            function_id: stage.function_id().to_owned(),
            outcome: match evaluation.outcome {
                RetainedCallOutcome::Returned(_) => "returned",
                RetainedCallOutcome::LanguageFailure(_) => "language_failure",
                RetainedCallOutcome::FuelExhausted => "fuel_exhausted",
                RetainedCallOutcome::CallDepthExceeded => "call_depth_exceeded",
                RetainedCallOutcome::GuardError(_) => "guard_error",
            },
            steps_used: evaluation.steps_used,
            cleanup_events: evaluation.cleanup_events.len(),
        }
    }

    #[must_use]
    pub const fn role(&self) -> &'static str {
        self.role
    }

    #[must_use]
    pub fn function_id(&self) -> &str {
        &self.function_id
    }

    #[must_use]
    pub const fn outcome(&self) -> &'static str {
        self.outcome
    }

    #[must_use]
    pub const fn cleanup_events(&self) -> usize {
        self.cleanup_events
    }
}

/// The complete deterministic record of one lifecycle invocation.
pub struct LifecycleRun {
    status: LifecycleStatus,
    reason: &'static str,
    stages: Vec<StageRecord>,
    result: Option<RetainedValue>,
    binding: Option<String>,
    spent: bool,
    refusal_code: Option<i64>,
    evidence: String,
    evidence_digest: String,
}

impl LifecycleRun {
    #[must_use]
    pub const fn status(&self) -> LifecycleStatus {
        self.status
    }

    /// The closed reason this invocation reached its terminal condition.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }

    /// Every stage that actually ran, in execution order.
    #[must_use]
    pub fn stages(&self) -> &[StageRecord] {
        &self.stages
    }

    /// The published Result carrier, present only when the run completed.
    #[must_use]
    pub const fn result(&self) -> Option<&RetainedValue> {
        self.result.as_ref()
    }

    /// The authorization binding, present only when one was minted.
    #[must_use]
    pub fn authorization_binding(&self) -> Option<&str> {
        self.binding.as_deref()
    }

    /// Whether the minted authorization was spent into the effect boundary.
    #[must_use]
    pub const fn authorization_spent(&self) -> bool {
        self.spent
    }

    /// The exact refusal code the authorize stage returned, when it refused.
    #[must_use]
    pub const fn refusal_code(&self) -> Option<i64> {
        self.refusal_code
    }

    /// The canonical evidence document, including its terminal LF. It carries
    /// identities, outcomes and counts only, never stage payload bytes.
    #[must_use]
    pub fn evidence(&self) -> &str {
        &self.evidence
    }

    #[must_use]
    pub fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }
}

/// One compiled lifecycle: an immutable, authority-free product.
pub struct CompiledAgentLifecycle {
    agent_id: String,
    definition_digest: String,
    source_revision: String,
    program: hir::ResolvedProgram,
    proposal: CompiledAgentProposalSchema,
    binding: StageBinding,
    source: String,
    digest: String,
}

impl CompiledAgentLifecycle {
    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    #[must_use]
    pub fn definition_digest(&self) -> &str {
        &self.definition_digest
    }

    /// The semantic revision of the exact checked module whose stage bodies
    /// this lifecycle retains. It is an in-memory replay precondition and does
    /// not change the frozen lifecycle document.
    #[must_use]
    pub fn source_revision(&self) -> &str {
        &self.source_revision
    }

    /// The canonical compiled lifecycle document, including its terminal LF.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.source
    }

    /// The domain-separated lifecycle digest. This is the policy identity an
    /// authorization binds to: any change to a stage identity, signature,
    /// ownership mode, role type, decision shape or stage graph changes it and
    /// stales every authorization derived from it.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// The derived Agent Proposal Schema v1 grammar this lifecycle decodes
    /// model output against.
    #[must_use]
    pub const fn proposal_schema(&self) -> &CompiledAgentProposalSchema {
        &self.proposal
    }

    /// The deterministic execution order of the stage graph.
    #[must_use]
    pub fn stage_order(&self) -> &[&'static str] {
        &self.binding.order
    }

    /// The stable function identity bound to one deterministic stage role.
    #[must_use]
    pub fn stage_function_id(&self, role: &str) -> Option<&str> {
        self.stage(role).map(BoundStage::function_id)
    }

    fn stage(&self, role: &str) -> Option<&BoundStage> {
        match role {
            "initialize" => Some(&self.binding.initialize),
            "observe" => Some(&self.binding.observe),
            "authorize" => Some(self.binding.authorize.stage()),
            "reduce" => Some(&self.binding.reduce),
            _ => None,
        }
    }
}

/// Compiles one lifecycle from one checked module and one AgentDefinition.
///
/// Compilation is pure. It reads no environment, no filesystem beyond the
/// caller-supplied module text, no process and no network, and it grants no
/// provider, tool, approval or publication authority.
pub fn compile_agent_lifecycle(
    module_source: &str,
    module_path: impl AsRef<Path>,
    definition_source: &str,
) -> Result<CompiledAgentLifecycle, Vec<Diagnostic>> {
    let module_path = module_path.as_ref();
    let compiled = compile_agent_definition(definition_source)?;
    let definition = compiled.definition();
    let mut type_ids = Vec::with_capacity(stages::TYPE_ROLES.len());
    for role in stages::TYPE_ROLES {
        let id = definition
            .type_id(role)
            .ok_or_else(|| vec![invariant(&format!("{role}_type.role"))])?;
        type_ids.push((role, id.to_owned()));
    }
    let mut operation_ids = Vec::with_capacity(stages::DETERMINISTIC_ROLES.len());
    for role in stages::DETERMINISTIC_ROLES {
        let (id, kind) = definition
            .operation(role)
            .ok_or_else(|| vec![invariant(&format!("{role}.role"))])?;
        if kind != "deterministic" {
            return Err(vec![invariant(&format!("{role}.kind"))]);
        }
        operation_ids.push((role, id.to_owned()));
    }
    let agent_id = definition.agent_id().to_owned();
    let definition_digest = definition.digest().to_owned();

    let program = crate::check(module_source, module_path)?;
    let source_revision = crate::graph::revision(&program);
    let program = hir::resolve(&program)?;
    let proposal = compile_agent_proposal_schema(module_source, module_path, definition_source)?;
    let binding = stages::bind(&program, &type_ids, &operation_ids)?;

    let source = render_lifecycle(
        &agent_id,
        &definition_digest,
        proposal.schema().digest(),
        &binding,
    );
    if source.len() > MAX_LIFECYCLE_BYTES {
        return Err(vec![invariant("lifecycle_bytes")]);
    }
    Ok(CompiledAgentLifecycle {
        agent_id,
        definition_digest,
        source_revision,
        program,
        proposal,
        binding,
        digest: digest(LIFECYCLE_DOMAIN, source.as_bytes()),
        source,
    })
}

/// Independently recompiles one lifecycle and requires the supplied document
/// to equal it byte for byte.
pub fn verify_agent_lifecycle_bundle(
    module_source: &str,
    module_path: impl AsRef<Path>,
    definition_source: &str,
    lifecycle_source: &str,
) -> Result<(), Vec<Diagnostic>> {
    if lifecycle_source.len() > MAX_LIFECYCLE_BYTES {
        return Err(vec![bundle_mismatch()]);
    }
    let compiled = compile_agent_lifecycle(module_source, module_path, definition_source)?;
    if compiled.canonical_json().as_bytes() != lifecycle_source.as_bytes() {
        return Err(vec![bundle_mismatch()]);
    }
    Ok(())
}

fn field(id: &hir::DeclarationId, value: RetainedValue) -> RetainedField {
    RetainedField {
        field: id.clone(),
        value,
    }
}

fn payload(shape: &PayloadShape, bytes: Vec<u8>, scalar: i64) -> RetainedValue {
    RetainedValue::Record(RetainedRecord {
        record: shape.record.clone(),
        fields: vec![
            field(&shape.bytes_field, RetainedValue::Bytes(bytes)),
            field(&shape.scalar_field, RetainedValue::I64(scalar)),
        ],
    })
}

/// The canonical encoding one authorization binds a state carrier through.
///
/// Identities only: every record, variant, case and field is named by its
/// persistent declaration identity, so a display rename does not change a
/// binding while an actual identity or value change does.
pub(crate) fn encode_value(value: &RetainedValue) -> String {
    match value {
        RetainedValue::Bool(value) => (*value).to_string(),
        RetainedValue::I32(value) => quote_json(&value.to_string()),
        RetainedValue::I64(value) => quote_json(&value.to_string()),
        RetainedValue::U8(value) => quote_json(&value.to_string()),
        RetainedValue::Usize(value) => quote_json(&value.to_string()),
        RetainedValue::Bytes(bytes) => {
            let mut hex = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                hex.push_str(&format!("{byte:02x}"));
            }
            format!("{{\"bytes\":{}}}", quote_json(&hex))
        }
        RetainedValue::Record(record) => format!(
            "{{\"record\":{},\"fields\":{}}}",
            quote_json(record.record.as_str()),
            encode_fields(&record.fields)
        ),
        RetainedValue::Variant(variant) => format!(
            "{{\"variant\":{},\"case\":{},\"fields\":{}}}",
            quote_json(variant.variant.as_str()),
            quote_json(variant.case.as_str()),
            encode_fields(&variant.fields)
        ),
    }
}

fn encode_fields(fields: &[RetainedField]) -> String {
    let mut output = String::from("[");
    for (index, item) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"field\":{},\"value\":{}}}",
            quote_json(item.field.as_str()),
            encode_value(&item.value)
        ));
    }
    output.push(']');
    output
}

fn render_lifecycle(
    agent_id: &str,
    definition_digest: &str,
    proposal_schema_digest: &str,
    binding: &StageBinding,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"definition_digest\":{},\"proposal_schema_digest\":{},\"types\":[",
        quote_json(LIFECYCLE_SCHEMA),
        quote_json(agent_id),
        quote_json(definition_digest),
        quote_json(proposal_schema_digest)
    );
    for (index, (role, id)) in binding.types.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(role),
            quote_json(id.as_str())
        ));
    }
    output.push_str("],\"proposal_projection\":[");
    for (index, parameter) in binding.proposal.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"stable_id\":{},\"representation\":{}}}",
            quote_json(parameter.field.as_str()),
            quote_json(parameter.kind.name())
        ));
    }
    output.push_str("],\"stages\":[");
    for (index, stage) in [
        &binding.initialize,
        &binding.observe,
        binding.authorize.stage(),
        &binding.reduce,
    ]
    .iter()
    .enumerate()
    {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"kind\":\"deterministic\",\"operation_id\":{},\"function_id\":{},\"parameters\":[",
            quote_json(stage.role()),
            quote_json(stage.operation_id()),
            quote_json(stage.function_id())
        ));
        for (position, (ownership, ty)) in stage.parameters().iter().enumerate() {
            if position > 0 {
                output.push(',');
            }
            output.push_str(&format!(
                "{{\"ownership\":{},\"type\":{}}}",
                quote_json(ownership),
                quote_json(ty)
            ));
        }
        output.push_str(&format!("],\"result\":{}}}", quote_json(stage.result())));
    }
    output.push_str(&format!(
        "],\"decision\":{{\"type\":{},\"grant_case\":{},\"grant_seal_field\":{},\"grant_budget_field\":{},\"refuse_case\":{},\"refuse_code_field\":{}}},\"stage_graph\":{{\"order\":[",
        quote_json(binding.authorize.decision_type().as_str()),
        quote_json(binding.authorize.grant_case().as_str()),
        quote_json(binding.authorize.grant_seal_field().as_str()),
        quote_json(binding.authorize.grant_budget_field().as_str()),
        quote_json(binding.authorize.refuse_case().as_str()),
        quote_json(binding.authorize.refuse_code_field().as_str())
    ));
    for (index, role) in binding.order.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(role));
    }
    output.push_str("],\"edges\":[");
    for (index, edge) in stages::stage_edges().iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"from\":{},\"to\":{}}}",
            quote_json(edge.from),
            quote_json(edge.to)
        ));
    }
    output.push_str("]},\"nonclaims\":[");
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}

/// The authorization facts of one terminal condition.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Authorization {
    binding: Option<String>,
    spent: bool,
    refusal_code: Option<i64>,
}

impl Authorization {
    /// No authorization was minted.
    const fn none() -> Self {
        Self {
            binding: None,
            spent: false,
            refusal_code: None,
        }
    }

    /// The authorize stage refused with an exact code.
    const fn refused(code: i64) -> Self {
        Self {
            binding: None,
            spent: false,
            refusal_code: Some(code),
        }
    }

    /// One authorization was minted and either dropped unspent or consumed.
    const fn minted(binding: String, spent: bool) -> Self {
        Self {
            binding: Some(binding),
            spent,
            refusal_code: None,
        }
    }
}

fn render_evidence(
    agent_id: &str,
    lifecycle_digest: &str,
    status: LifecycleStatus,
    reason: &str,
    stages: &[StageRecord],
    authorization: &Authorization,
) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"lifecycle_digest\":{},\"status\":{},\"reason\":{},\"stages\":[",
        quote_json(EVIDENCE_SCHEMA),
        quote_json(agent_id),
        quote_json(lifecycle_digest),
        quote_json(status.name()),
        quote_json(reason)
    );
    for (index, stage) in stages.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"operation_id\":{},\"function_id\":{},\"outcome\":{},\"steps\":{},\"cleanup_events\":{}}}",
            quote_json(stage.role),
            quote_json(&stage.operation_id),
            quote_json(&stage.function_id),
            quote_json(stage.outcome),
            stage.steps_used,
            stage.cleanup_events
        ));
    }
    output.push_str(&format!(
        "],\"authorization\":{{\"minted\":{},\"binding\":{},\"spent\":{},\"refusal_code\":{}}},\"nonclaims\":[",
        authorization.binding.is_some(),
        authorization
            .binding
            .as_deref()
            .map_or_else(|| "null".to_owned(), quote_json),
        authorization.spent,
        authorization
            .refusal_code
            .map_or_else(|| "null".to_owned(), |code| quote_json(&code.to_string()))
    ));
    for (index, nonclaim) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(nonclaim));
    }
    output.push_str("]}\n");
    output
}

impl CompiledAgentLifecycle {
    fn finish(
        &self,
        status: LifecycleStatus,
        reason: &'static str,
        stages: Vec<StageRecord>,
        result: Option<RetainedValue>,
        authorization: Authorization,
    ) -> LifecycleRun {
        let evidence = render_evidence(
            &self.agent_id,
            &self.digest,
            status,
            reason,
            &stages,
            &authorization,
        );
        LifecycleRun {
            status,
            reason,
            stages,
            result,
            binding: authorization.binding,
            spent: authorization.spent,
            refusal_code: authorization.refusal_code,
            evidence_digest: digest(EVIDENCE_DOMAIN, evidence.as_bytes()),
            evidence,
        }
    }

    fn evaluate(
        &self,
        stage: &BoundStage,
        arguments: &[RetainedValue],
        max_steps: usize,
    ) -> Result<RetainedCallEvaluation, Vec<Diagnostic>> {
        evaluate_retained_call(&self.program, stage.prepared(), arguments, max_steps)
    }

    /// Projects one decoded proposal into the ordered scalar arguments the
    /// compiler admitted the authorize and reduce stages against.
    pub(crate) fn project(&self, decoded: &DecodedProposal) -> Option<Vec<RetainedValue>> {
        if decoded.case().is_some() {
            return None;
        }
        let mut projected = Vec::with_capacity(self.binding.proposal.len());
        for parameter in &self.binding.proposal {
            let value = decoded.field(parameter.field.as_str())?;
            projected.push(match (parameter.kind, value) {
                (ScalarKind::Bool, ProposalValue::Bool(value)) => RetainedValue::Bool(*value),
                (ScalarKind::I32, ProposalValue::Signed(value)) => {
                    RetainedValue::I32(i32::try_from(*value).ok()?)
                }
                (ScalarKind::I64, ProposalValue::Signed(value)) => RetainedValue::I64(*value),
                (ScalarKind::U8, ProposalValue::Unsigned(value)) => {
                    RetainedValue::U8(u8::try_from(*value).ok()?)
                }
                (ScalarKind::Usize, ProposalValue::Unsigned(value)) => RetainedValue::Usize(*value),
                _ => return None,
            });
        }
        Some(projected)
    }

    /// Spends one authorization into the single injected read operation.
    ///
    /// The authorization is consumed by move, and its binding is independently
    /// recomputed from the policy, the state and the proposal presented here.
    /// A substituted state or a substituted proposal therefore fails closed
    /// before the read operation is called at all.
    pub(crate) fn spend(
        &self,
        authorized: Authorized,
        state: &RetainedValue,
        proposal_canonical: &str,
        read: &mut dyn AgentReadOperation,
    ) -> Result<Vec<u8>, Diagnostic> {
        let request = authorized.consume();
        let expected = authorization::binding(
            &self.digest,
            state,
            proposal_canonical,
            self.binding.authorize.grant_case(),
            request.seal(),
        );
        if expected != request.binding() {
            return Err(refused(
                "authorization is not bound to this state and proposal",
            ));
        }
        let value = read
            .read(&request)
            .ok_or_else(|| refused("the injected read operation failed"))?;
        if value.len() > MAX_READ_BYTES {
            return Err(refused(
                "the injected read operation exceeded its byte bound",
            ));
        }
        Ok(value)
    }

    /// Executes one complete lifecycle.
    ///
    /// `Err` is reserved for a fail-closed compiler or seam fault. Every
    /// terminal condition of the lifecycle itself — success, rejection, model
    /// failure, effect failure, cancellation and budget exhaustion — is an
    /// `Ok` run carrying its status and its deterministic evidence.
    pub fn run(
        &self,
        task: &LifecycleTask,
        proposal_source: &str,
        read: &mut dyn AgentReadOperation,
        budget: LifecycleBudget,
        cancellation: &AgentCancellation,
    ) -> Result<LifecycleRun, Vec<Diagnostic>> {
        let mut records = Vec::with_capacity(stages::DETERMINISTIC_ROLES.len());
        let steps = budget.max_steps_per_stage;
        if cancellation.is_cancelled() {
            return Ok(self.finish(
                LifecycleStatus::Cancelled,
                "cancelled_before_initialize",
                records,
                None,
                Authorization::none(),
            ));
        }

        let evaluation = self.evaluate(
            &self.binding.initialize,
            &[payload(
                &self.binding.task,
                task.objective.clone(),
                task.budget,
            )],
            steps,
        )?;
        records.push(StageRecord::of(&self.binding.initialize, &evaluation));
        let state = match evaluation.outcome {
            RetainedCallOutcome::Returned(value) => value,
            other => {
                return Ok(self.finish(
                    terminal(&other),
                    "initialize_did_not_return",
                    records,
                    None,
                    Authorization::none(),
                ))
            }
        };
        if !self.carries(&state, "state") {
            return Err(vec![invariant("initialize.result.identity")]);
        }

        if cancellation.is_cancelled() {
            return Ok(self.finish(
                LifecycleStatus::Cancelled,
                "cancelled_before_observe",
                records,
                None,
                Authorization::none(),
            ));
        }
        let evaluation =
            self.evaluate(&self.binding.observe, std::slice::from_ref(&state), steps)?;
        records.push(StageRecord::of(&self.binding.observe, &evaluation));
        let observation = match evaluation.outcome {
            RetainedCallOutcome::Returned(value) => value,
            other => {
                return Ok(self.finish(
                    terminal(&other),
                    "observe_did_not_return",
                    records,
                    None,
                    Authorization::none(),
                ))
            }
        };
        if !self.carries(&observation, "observation") {
            return Err(vec![invariant("observe.result.identity")]);
        }

        // The model is scripted and offline. Its output is untrusted data and
        // is admitted only through the derived Proposal Schema v1 decoder.
        let Ok(decoded) = self.proposal.decode(proposal_source) else {
            return Ok(self.finish(
                LifecycleStatus::ModelFailed,
                "proposal_rejected_by_its_grammar",
                records,
                None,
                Authorization::none(),
            ));
        };
        let Some(projected) = self.project(&decoded) else {
            return Ok(self.finish(
                LifecycleStatus::ModelFailed,
                "proposal_outside_the_stage_projection",
                records,
                None,
                Authorization::none(),
            ));
        };

        if cancellation.is_cancelled() {
            return Ok(self.finish(
                LifecycleStatus::Cancelled,
                "cancelled_before_authorize",
                records,
                None,
                Authorization::none(),
            ));
        }
        let mut arguments = Vec::with_capacity(1 + projected.len());
        arguments.push(state.clone());
        arguments.extend(projected.iter().cloned());
        let (decision, record) = authorization::run_authorize_stage(
            &self.program,
            &self.binding.authorize,
            &arguments,
            steps,
            &self.digest,
            &state,
            decoded.canonical_json(),
        )?;
        records.push(record);
        let authorized = match decision {
            authorization::AuthorizationOutcome::Granted(authorized) => authorized,
            authorization::AuthorizationOutcome::Refused(code) => {
                return Ok(self.finish(
                    LifecycleStatus::Rejected,
                    "authorize_refused",
                    records,
                    None,
                    Authorization::refused(code),
                ))
            }
            authorization::AuthorizationOutcome::Undecided(reason) => {
                let status = if reason == "fuel" || reason == "depth" {
                    LifecycleStatus::BudgetExhausted
                } else {
                    LifecycleStatus::Rejected
                };
                return Ok(self.finish(
                    status,
                    "authorize_did_not_decide",
                    records,
                    None,
                    Authorization::none(),
                ));
            }
        };
        let binding = authorized.binding().to_owned();

        if cancellation.is_cancelled() {
            // The authorization is dropped unspent: cancellation observed at
            // this boundary performs no effect.
            drop(authorized);
            return Ok(self.finish(
                LifecycleStatus::Cancelled,
                "cancelled_before_execute",
                records,
                None,
                Authorization::minted(binding, false),
            ));
        }

        let value = match self.spend(authorized, &state, decoded.canonical_json(), read) {
            Ok(value) => value,
            Err(_) => {
                return Ok(self.finish(
                    LifecycleStatus::EffectFailed,
                    "execute_failed",
                    records,
                    None,
                    Authorization::minted(binding, true),
                ))
            }
        };

        let mut arguments = Vec::with_capacity(2 + projected.len());
        arguments.push(state);
        arguments.extend(projected);
        arguments.push(payload(&self.binding.outcome, value, 0));
        let evaluation = self.evaluate(&self.binding.reduce, &arguments, steps)?;
        records.push(StageRecord::of(&self.binding.reduce, &evaluation));
        let result = match evaluation.outcome {
            RetainedCallOutcome::Returned(value) => value,
            other => {
                return Ok(self.finish(
                    terminal(&other),
                    "reduce_did_not_return",
                    records,
                    None,
                    Authorization::minted(binding, true),
                ))
            }
        };
        if !self.carries(&result, "result") {
            return Err(vec![invariant("reduce.result.identity")]);
        }
        Ok(self.finish(
            LifecycleStatus::Completed,
            "reduce_published_a_result",
            records,
            Some(result),
            Authorization::minted(binding, true),
        ))
    }

    /// Whether one harvested carrier names the declaration bound to a role.
    fn carries(&self, value: &RetainedValue, role: &str) -> bool {
        let expected = self.binding.type_id(role);
        match value {
            RetainedValue::Record(record) => record.record == *expected,
            RetainedValue::Variant(variant) => variant.variant == *expected,
            _ => false,
        }
    }
}

/// The terminal condition of a stage that did not return.
fn terminal(outcome: &RetainedCallOutcome) -> LifecycleStatus {
    match outcome {
        RetainedCallOutcome::FuelExhausted | RetainedCallOutcome::CallDepthExceeded => {
            LifecycleStatus::BudgetExhausted
        }
        _ => LifecycleStatus::Rejected,
    }
}
