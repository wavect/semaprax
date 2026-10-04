//! Host-selected, durable, config-driven checked candidate-repair preview.
//!
//! This generalizes the fixed `offline-repair` demonstration's
//! candidate-preview/source-diff/semantic-impact evidence
//! (`OfflineRepairEnvelope`/`OfflineRepairHandler`) to an arbitrary
//! host-selected Project, target declaration and effect contract, driven
//! through the durable, checkpoint-capable `AgentRuntimeV2` route
//! (`run_live_bound_model_durable`) so the run is resumable like the general
//! `source-live run|resume` verbs. V1 retains a bounded, credential-free
//! scripted fixture for tests; V2 binds the same explicit OpenCode process
//! provider boundary as `source-live run`; V3 adds the native Claude print host.
//! These modes produce reviewable
//! evidence for an *ephemeral* candidate only. They perform no publication or
//! source mutation; a separately authorized, exact-digest-bound session
//! (`project-candidate-git-publish`) is the only route that can commit.
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[path = "repair/barrier.rs"]
mod barrier;
#[path = "repair/claude.rs"]
mod claude;
#[path = "repair/config.rs"]
mod config;
#[path = "repair/receipt.rs"]
mod receipt_impl;
use receipt_impl::receipt;
const CONFIG_SCHEMA_V3: &str = "semaprax.source-live-cli.repair-config.v3";
const RECEIPT_SCHEMA_V3: &str = "semaprax.source-live-cli.repair-receipt.v3";

use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::canonical_retained_value_json;
use semaprax::agent_lifecycle::iterative::compile_source_agent_lifecycle_v2;
use semaprax::agent_lifecycle::iterative::effects::{
    EffectArgument, EffectBudget, EffectOperation, EffectResult, EffectScalar, TypedEffectHandler,
    TypedEffectRequest,
};
use semaprax::agent_lifecycle::iterative::source_live::{SourceLivePolicy, SourceProposalPolicy};
use semaprax::agent_lifecycle::iterative::IterativeBudget;
use semaprax::agent_lifecycle::LifecycleTask;
use semaprax::agent_runtime::AgentCancellation;
use semaprax::agent_runtime_v2::{
    bind_agent_runtime_v2_live, OfflineRepairEnvelope, OfflineRepairHandler,
    SourceModelAdapterIdentity,
};
use semaprax::execution_revision::ProgramRootRef;
use semaprax::interpreter::retained_call::RetainedValue;
use semaprax::live_invocation::source_journal::SourceJournalError;
use semaprax::live_invocation::{InvocationClock, SourceInvocationClock};
use semaprax::project::{with_authenticated_project, ProjectRevision};
use semaprax::provider_adapter_sdk::fixture_adapters::{usage, ScriptedStreamingAdapter};
use semaprax::provider_adapter_sdk::{
    AdapterInvocationCapability, AdapterPoll, AdapterRefusal, AdapterRequest, ProviderAdapter,
    StreamingSourceProposalAdapter,
};
use serde_json::{json, Map, Value};

use crate::opencode_host::repair_adapter::{
    source_model_identity, source_model_identity_for_config, OpenCodeRepairAdapter,
};
use crate::opencode_host::{
    OpenCodeGrammar, OpenCodeHostConfig, OpenCodeRunner, ProcessOpenCodeRunner,
};

use super::checkpoint::{bounded_read, CheckpointDir};
use super::CliError;
use barrier::PostSettledBarrierStore;

const FIXTURE_CLOCK_DOMAIN: &str = "semaprax.source-live-cli.repair.v1";
const UNIX_CLOCK_DOMAIN: &str = "unix_epoch_millis.v1";
const MAX_CONFIG_BYTES: usize = 16384;
const MAX_TOKEN_BYTES: usize = 240;
const MAX_TASK_BYTES: usize = 4096;
const MAX_PROPOSAL_BYTES: usize = 8192;
const RECEIPT_SCHEMA_V1: &str = "semaprax.source-live-cli.repair-receipt.v1";
const RECEIPT_SCHEMA_V2: &str = "semaprax.source-live-cli.repair-receipt.v2";
const CONFIG_SCHEMA_V1: &str = "semaprax.source-live-cli.repair-config.v1";
const CONFIG_SCHEMA_V2: &str = "semaprax.source-live-cli.repair-config.v2";
const MAX_ONE_PROVIDER_CALL_MS: i64 = 30_000;
const TERMINAL_PATCH_RECEIPT_SCHEMA: &str =
    "semaprax.source-live-cli.repair-terminal-patch-receipt.v1";

struct TerminalPatchReceipt {
    document: String,
    receipt: String,
}

impl TerminalPatchReceipt {
    fn derive(
        preview: &semaprax::agent_runtime_v2::OfflineRepairPreview,
        checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    ) -> Result<Self, CliError> {
        let candidate = preview.candidate();
        let candidate_digest = candidate.candidate_digest();
        let receipt = candidate
            .patch_receipt(candidate_digest)
            .map_err(|_| CliError::refused("repair patch receipt derivation refused"))?;
        let receipt_value: Value = serde_json::from_str(&receipt)
            .map_err(|_| CliError::refused("repair patch receipt is not compiler JSON"))?;
        let receipt_digest = receipt_value["receipt_digest"]
            .as_str()
            .ok_or(CliError::refused("repair patch receipt digest is absent"))?;
        let document = serde_json::to_string(&json!({
            "schema": TERMINAL_PATCH_RECEIPT_SCHEMA,
            "journal_binding": {
                "invocation": checkpoint.invocation(),
                "chain": checkpoint.chain(),
                "generation": checkpoint.generation(),
            },
            "candidate_digest": candidate_digest,
            "receipt_digest": receipt_digest,
            "receipt": receipt,
        }))
        .map(|document| format!("{document}\n"))
        .map_err(|_| CliError::refused("repair terminal patch receipt cannot be rendered"))?;
        Ok(Self { document, receipt })
    }

    fn recover(
        document: String,
        checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    ) -> Result<Self, CliError> {
        let value: Value = serde_json::from_str(&document)
            .map_err(|_| CliError::refused("terminal patch receipt is malformed"))?;
        let object = value
            .as_object()
            .ok_or(CliError::refused("terminal patch receipt is malformed"))?;
        let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
        if keys.as_slice()
            != [
                "candidate_digest",
                "journal_binding",
                "receipt",
                "receipt_digest",
                "schema",
            ]
            || value["schema"] != TERMINAL_PATCH_RECEIPT_SCHEMA
            || value["journal_binding"]["invocation"] != checkpoint.invocation()
            || value["journal_binding"]["chain"] != checkpoint.chain()
            || value["journal_binding"]["generation"] != checkpoint.generation()
        {
            return Err(CliError::refused(
                "terminal patch receipt binding is stale or mismatched",
            ));
        }
        let receipt = value["receipt"]
            .as_str()
            .ok_or(CliError::refused("terminal patch receipt is malformed"))?
            .to_owned();
        let receipt_value: Value = serde_json::from_str(&receipt)
            .map_err(|_| CliError::refused("terminal patch receipt is malformed"))?;
        if receipt_value["receipt_digest"] != value["receipt_digest"] {
            return Err(CliError::refused("terminal patch receipt digest is stale"));
        }
        Ok(Self { document, receipt })
    }

    fn value(&self) -> Result<Value, CliError> {
        serde_json::from_str(&self.receipt)
            .map_err(|_| CliError::refused("terminal patch receipt is malformed"))
    }
}

#[cfg(test)]
std::thread_local! {
    static TEST_EFFECT_HANDLER_CALLS: Cell<usize> = const { Cell::new(0) };
    static TEST_SOURCE_SNAPSHOT_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

#[cfg(test)]
fn reset_test_effect_handler_calls() {
    TEST_EFFECT_HANDLER_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
fn test_effect_handler_calls() -> usize {
    TEST_EFFECT_HANDLER_CALLS.with(|calls| calls.get())
}

#[cfg(test)]
fn set_test_source_snapshot_hook(hook: impl FnOnce() + 'static) {
    TEST_SOURCE_SNAPSHOT_HOOK.with(|slot| {
        assert!(
            slot.borrow().is_none(),
            "source snapshot hook is already set"
        );
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
fn run_test_source_snapshot_hook() {
    TEST_SOURCE_SNAPSHOT_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook();
        }
    });
}

#[cfg(not(test))]
fn run_test_source_snapshot_hook() {}

use super::candidate_test::{
    candidate_test_bound_identity, candidate_test_evidence, candidate_test_subject,
    replayed_candidate_test_evidence,
};
pub(super) use super::candidate_test::{
    CandidateTestCapability, CandidateTestEvidence, CandidateTestHost, CandidateTestObservation,
    CandidateTestObservationError, CandidateTestObserver, CandidateTestStatus,
    CandidateTestSubject, ReplayedCandidateTestEvidence, CANDIDATE_TEST_SCHEMA,
    MAX_CANDIDATE_TEST_OBSERVATION_BYTES,
};

fn is_absolute_like(path: &Path) -> bool {
    path.is_absolute() || path.to_string_lossy().starts_with('/')
}

struct FixedClock;
impl InvocationClock for FixedClock {
    fn now_millis(&self) -> i64 {
        0
    }
}
impl SourceInvocationClock for FixedClock {
    fn clock_domain(&self) -> &str {
        FIXTURE_CLOCK_DOMAIN
    }
}

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
        UNIX_CLOCK_DOMAIN
    }
}

/// One scripted provider turn: the exact canonical proposal document the
/// fixture provider returns and whether it must observe the preceding checked
/// effect result in its prompt. The fixture never precomputes that result:
/// terminal recovery must be able to replay before any candidate preview is
/// derived from fixture-only inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
struct RepairTurn {
    document: String,
    requires_prior_feedback: bool,
}

/// V1 is a deliberate local test seam. V2 is selected only by the explicit
/// OpenCode executable and empty scratch-directory operands; source/config
/// text cannot select a provider, endpoint, credential or publication route.
enum RepairProvider {
    Scripted([RepairTurn; 2]),
    OpenCode,
    Claude,
}

/// Host-selected, config-driven repair session. Every identity below is a
/// host operand; the model supplies only the scalar argument value routed
/// through `argument_id`/`proposal_field_id`. No path, target or provider is
/// chosen by proposal text.
struct RepairConfig {
    manifest: PathBuf,
    source_path: String,
    agent_id: String,
    step_id: String,
    selector_field_id: String,
    deployment_migration_id: String,
    target: String,
    malformed_operation_id: String,
    corrected_operation_id: String,
    effect_id: String,
    argument_id: String,
    proposal_field_id: String,
    result_id: String,
    task_path: PathBuf,
    task_budget: i64,
    deadline_millis: i64,
    ceiling: i64,
    reservation_units: i64,
    max_total_steps: usize,
    max_calls: usize,
    max_argument_bytes: usize,
    max_result_bytes: usize,
    max_total_bytes: usize,
    provider: RepairProvider,
}

/// Review-facing identities already checked before provider construction.
/// These strings describe the selected profile and its compiler-owned inputs;
/// they carry no provider, candidate-test, publication, or filesystem authority.
struct RepairReceiptContext {
    provider_id: String,
    model_id: String,
    adapter_identity: String,
    adapter_version: String,
    provider_profile: String,
    program_root: String,
    source_revision: String,
    proposal_schema_digest: String,
    deployment_binding: String,
}

fn text<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, CliError> {
    map.get(key)
        .and_then(Value::as_str)
        .ok_or(CliError::refused("configuration field has the wrong type"))
}

fn token(map: &Map<String, Value>, key: &str) -> Result<String, CliError> {
    let value = text(map, key)?;
    if value.is_empty()
        || value.len() > MAX_TOKEN_BYTES
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(CliError::refused("configuration identifier is invalid"));
    }
    Ok(value.to_owned())
}

fn absolute(map: &Map<String, Value>, key: &str) -> Result<PathBuf, CliError> {
    let path = PathBuf::from(text(map, key)?);
    if !is_absolute_like(&path) {
        return Err(CliError::refused("configuration path must be absolute"));
    }
    Ok(path)
}

fn positive_i64(map: &Map<String, Value>, key: &str) -> Result<i64, CliError> {
    let value = map
        .get(key)
        .and_then(Value::as_i64)
        .ok_or(CliError::refused("configuration integer is invalid"))?;
    (value > 0)
        .then_some(value)
        .ok_or(CliError::refused("configuration integer must be positive"))
}

fn nonnegative_i64(map: &Map<String, Value>, key: &str) -> Result<i64, CliError> {
    let value = map
        .get(key)
        .and_then(Value::as_i64)
        .ok_or(CliError::refused("configuration integer is invalid"))?;
    (value >= 0).then_some(value).ok_or(CliError::refused(
        "configuration integer must be nonnegative",
    ))
}

fn signed_i64(map: &Map<String, Value>, key: &str) -> Result<i64, CliError> {
    map.get(key)
        .and_then(Value::as_i64)
        .ok_or(CliError::refused("configuration integer is invalid"))
}

fn positive_usize(map: &Map<String, Value>, key: &str) -> Result<usize, CliError> {
    let value = map
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|number| usize::try_from(number).ok())
        .ok_or(CliError::refused("configuration capacity is invalid"))?;
    (value > 0)
        .then_some(value)
        .ok_or(CliError::refused("configuration capacity must be positive"))
}

pub(super) enum Command {
    Run {
        config: PathBuf,
        checkpoint: PathBuf,
        provider: Option<OpenCodeOperands>,
    },
    Resume {
        config: PathBuf,
        checkpoint: PathBuf,
        provider: Option<OpenCodeOperands>,
    },
    Receipt {
        config: PathBuf,
        checkpoint: PathBuf,
        provider: Option<OpenCodeOperands>,
    },
}

pub(super) struct OpenCodeOperands {
    executable: PathBuf,
    scratch: PathBuf,
    pause_after_settled: bool,
    claude: bool,
}

impl Command {
    pub(super) fn parse(arguments: &[String]) -> Result<Self, CliError> {
        let (verb, config, checkpoint, provider) = match arguments {
            [verb, config, checkpoint] => (verb, config, checkpoint, None),
            [verb, config, checkpoint, executable_flag, executable, scratch_flag, scratch]
                if matches!(executable_flag.as_str(), "--opencode" | "--claude")
                    && scratch_flag == "--scratch" =>
            {
                (
                    verb,
                    config,
                    checkpoint,
                    Some(OpenCodeOperands {
                        executable: absolute_operand(executable)?,
                        scratch: absolute_operand(scratch)?,
                        pause_after_settled: false,
                        claude: executable_flag == "--claude",
                    }),
                )
            }
            [verb, config, checkpoint, executable_flag, executable, scratch_flag, scratch, barrier]
                if matches!(executable_flag.as_str(), "--opencode" | "--claude")
                    && scratch_flag == "--scratch"
                    && barrier == "--pause-after-settled" =>
            {
                (
                    verb,
                    config,
                    checkpoint,
                    Some(OpenCodeOperands {
                        executable: absolute_operand(executable)?,
                        scratch: absolute_operand(scratch)?,
                        pause_after_settled: true,
                        claude: executable_flag == "--claude",
                    }),
                )
            }
            _ => {
                return Err(CliError::usage(
                    "repair requires run|resume|receipt <config.json> <checkpoint-dir> [--opencode ABS --scratch EMPTY_ABS [--pause-after-settled]]",
                ));
            }
        };
        let config = absolute_operand(config)?;
        let checkpoint = absolute_operand(checkpoint)?;
        match verb.as_str() {
            "run" => Ok(Self::Run {
                config,
                checkpoint,
                provider,
            }),
            "resume" => Ok(Self::Resume {
                config,
                checkpoint,
                provider,
            }),
            "receipt" => Ok(Self::Receipt {
                config,
                checkpoint,
                provider,
            }),
            _ => Err(CliError::usage("repair expected run, resume, or receipt")),
        }
    }
}

fn absolute_operand(value: &str) -> Result<PathBuf, CliError> {
    let path = PathBuf::from(value);
    if !is_absolute_like(&path) {
        return Err(CliError::usage("repair operands must be absolute paths"));
    }
    Ok(path)
}

fn diagnostic_error(context: &str, diagnostics: Vec<semaprax::diagnostic::Diagnostic>) -> CliError {
    CliError::detail(format!("{context}: {diagnostics:?}"))
}

/// Confirms that the host-visible file is exactly the source snapshot the
/// authenticated Project compiled. The candidate route must not accidentally
/// bind its durable journal and review artifacts to a source file that changed
/// after Project authentication. Reading to the checked snapshot's exact
/// length also keeps a concurrently grown source from becoming an unbounded
/// host read.
fn verify_checked_source_snapshot(path: &Path, checked_source: &[u8]) -> Result<(), CliError> {
    let disk_source = bounded_read(path, checked_source.len()).map_err(|_| {
        CliError::refused("repair source cannot be read within checked snapshot bounds")
    })?;
    if disk_source != checked_source {
        return Err(CliError::refused(
            "repair source differs from checked Project snapshot",
        ));
    }
    Ok(())
}

/// Classify a retained V2 journal with the closed recovery error, rather than
/// exposing compiler-internal diagnostic formatting to an operator. This is
/// deliberately a refusal only: the journal is still authenticated by the
/// checked runtime, and this host-side label grants no replay or provider
/// authority.
fn v2_replay_refusal(error: SourceJournalError) -> CliError {
    let reason = match error {
        SourceJournalError::Malformed
        | SourceJournalError::Chain
        | SourceJournalError::Generation
        | SourceJournalError::Order => "repair V2 retained checkpoint is malformed",
        SourceJournalError::Binding => "repair V2 checkpoint binding is stale or mismatched",
        SourceJournalError::Time => "repair V2 checkpoint clock is stale",
        SourceJournalError::Capacity => "repair V2 checkpoint exceeds replay capacity",
        SourceJournalError::Uncertain => "repair V2 checkpoint has an uncertain provider delivery",
        SourceJournalError::Store(_) | SourceJournalError::Poisoned => {
            "repair V2 checkpoint replay is unavailable"
        }
    };
    CliError::refused(reason)
}

fn checked_value(document: &str, field: &'static str) -> Result<Value, CliError> {
    serde_json::from_str(document).map_err(|_| CliError::refused(field))
}

fn scripted_identity() -> SourceModelAdapterIdentity {
    SourceModelAdapterIdentity {
        provider_id: "fake.local".to_owned(),
        model_id: "fake-basic".to_owned(),
        adapter_identity: "scripted-streaming-adapter".to_owned(),
        adapter_version: "1.0.0".to_owned(),
        provider_profile: "fixture".to_owned(),
    }
}

/// Source Agent model rows carry the provider and model as separate fields.
/// The fixed OpenCode command model is provider-qualified, so retain that
/// qualification for the host adapter while binding only its exact model
/// component to the checked source deployment.
fn source_deployment_identity(
    mut identity: SourceModelAdapterIdentity,
) -> Result<SourceModelAdapterIdentity, CliError> {
    identity.model_id = identity
        .model_id
        .strip_prefix("opencode/")
        .filter(|model| !model.is_empty())
        .map(str::to_owned)
        .ok_or(CliError::refused(
            "repair OpenCode model identity is not provider-qualified",
        ))?;
    Ok(identity)
}

fn operations(config: &RepairConfig) -> Vec<EffectOperation> {
    [
        &config.malformed_operation_id,
        &config.corrected_operation_id,
    ]
    .into_iter()
    .map(|operation_id| EffectOperation {
        operation_id: operation_id.clone(),
        effect_id: config.effect_id.clone(),
        arguments: vec![EffectArgument {
            argument_id: config.argument_id.clone(),
            proposal_field_id: config.proposal_field_id.clone(),
            kind: EffectScalar::I64,
        }],
        results: vec![EffectResult {
            result_id: config.result_id.clone(),
            kind: EffectScalar::I64,
        }],
    })
    .collect()
}

fn policy(
    config: &RepairConfig,
    deployment_binding: &str,
    response_limit: usize,
    clock_domain: &str,
) -> SourceLivePolicy {
    SourceLivePolicy {
        deployment_binding: deployment_binding.to_owned(),
        response_limit,
        ceiling: config.ceiling,
        reservation_units: config.reservation_units,
        unit: "semaprax.source-live-cli.repair-unit.v1".to_owned(),
        clock_domain: clock_domain.to_owned(),
        initial_millis: 0,
        deadline_millis: config.deadline_millis,
        max_total_steps: config.max_total_steps,
        program_root: None,
    }
}

fn checkpoint_policy<'a>(
    deployment_binding: &'a str,
    response_limit: usize,
    reservation_units: i64,
) -> SourceProposalPolicy<'a> {
    SourceProposalPolicy {
        deployment_binding,
        response_limit,
        reservation_units,
    }
}

struct FeedbackGuardedAdapter {
    inner: ScriptedStreamingAdapter,
    starts: Rc<Cell<usize>>,
    requires_prior_feedback: bool,
    expected_feedback: Rc<RefCell<Option<String>>>,
    refuse_start: bool,
}
impl ProviderAdapter for FeedbackGuardedAdapter {
    fn capabilities(&self) -> &semaprax::provider_adapter_sdk::AdapterCapabilities {
        self.inner.capabilities()
    }

    fn start(
        &mut self,
        capability: &AdapterInvocationCapability,
        request: &AdapterRequest,
    ) -> Result<(), AdapterRefusal> {
        if self.refuse_start {
            return Err(AdapterRefusal(
                "repair preview exhausted its scripted proposal turns".to_owned(),
            ));
        }
        self.starts.set(self.starts.get() + 1);
        if self.requires_prior_feedback {
            let observed = serde_json::from_slice::<Value>(&request.request_bytes)
                .ok()
                .and_then(|prompt| {
                    prompt["previous_effect_hex"]
                        .as_str()
                        .map(ToOwned::to_owned)
                });
            let expected = self.expected_feedback.borrow();
            if expected.is_none() || observed.as_deref() != expected.as_deref() {
                return Err(AdapterRefusal(
                    "repair correction omitted checked diagnostic feedback".to_owned(),
                ));
            }
        }
        self.inner.start(capability, request)
    }

    fn poll(&mut self) -> AdapterPoll {
        self.inner.poll()
    }

    fn cancel(&mut self, reason: &str) {
        self.inner.cancel(reason);
    }
}

/// Captures the canonical bytes returned by the actual effect handler so the
/// scripted corrective turn can verify the runtime's preceding observation.
/// This is populated only after recovery has admitted and dispatched an
/// effect; it never previews a fixture candidate to manufacture feedback.
struct FeedbackRecordingHandler<'host, 'observer> {
    inner: OfflineRepairHandler,
    preceding_effect_hex: Rc<RefCell<Option<String>>>,
    source_revision: String,
    candidate_test: Option<&'host mut CandidateTestHost<'observer>>,
    candidate_test_evidence: Option<CandidateTestEvidence>,
    candidate_test_refused: bool,
}

impl FeedbackRecordingHandler<'_, '_> {
    fn latest_preview(&self) -> Option<&semaprax::agent_runtime_v2::OfflineRepairPreview> {
        self.inner.latest_preview()
    }

    fn rejection_count(&self) -> u32 {
        self.inner.rejection_count()
    }

    fn candidate_test_evidence(&self) -> Option<&CandidateTestEvidence> {
        self.candidate_test_evidence.as_ref()
    }

    fn candidate_test_refused(&self) -> bool {
        self.candidate_test_refused
    }
}

impl TypedEffectHandler for FeedbackRecordingHandler<'_, '_> {
    fn execute(
        &mut self,
        request: &TypedEffectRequest<'_>,
    ) -> Option<Vec<(String, RetainedValue)>> {
        #[cfg(test)]
        TEST_EFFECT_HANDLER_CALLS.with(|calls| calls.set(calls.get() + 1));
        let mut result = self.inner.execute(request)?;
        if let (Some(candidate_test), Some(preview)) = (
            self.candidate_test.as_deref_mut(),
            self.inner.latest_preview(),
        ) {
            let subject =
                candidate_test_subject(&candidate_test.capability, preview, &self.source_revision);
            let observation = match candidate_test.observe(&subject) {
                Ok(observation) => observation,
                Err(_) => {
                    self.candidate_test_refused = true;
                    return None;
                }
            };
            let evidence = match candidate_test_evidence(observation, &subject) {
                Ok(evidence) => evidence,
                Err(()) => {
                    self.candidate_test_refused = true;
                    return None;
                }
            };
            let [(result_id, RetainedValue::I64(_))] = result.as_slice() else {
                return None;
            };
            result = vec![(
                result_id.clone(),
                RetainedValue::I64(evidence.feedback_code),
            )];
            self.candidate_test_evidence = Some(evidence);
        }
        *self.preceding_effect_hex.borrow_mut() = Some(canonical_effect_hex(&result));
        Some(result)
    }
}

fn canonical_effect_hex(fields: &[(String, RetainedValue)]) -> String {
    let rows = fields
        .iter()
        .map(|(id, value)| {
            format!(
                "[{},{}]",
                serde_json::to_string(id).expect("effect result identifiers are strings"),
                canonical_retained_value_json(value)
            )
        })
        .collect::<Vec<_>>();
    format!(
        "{{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[{}]}}\n",
        rows.join(",")
    )
    .bytes()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

/// One runner instance can serve successive fresh adapter instances while the
/// SDK preserves the one-start-per-adapter rule. The shared cell is private to
/// one CLI traversal and is never checkpointed; the journal, not this handle,
/// decides whether recovery may dispatch again.
struct SharedRunner<R>(Rc<RefCell<R>>);

impl<R: OpenCodeRunner> OpenCodeRunner for SharedRunner<R> {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, crate::opencode_host::OpenCodeRunnerFailure> {
        self.0.borrow_mut().run(config, prompt)
    }

    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        session: &str,
    ) -> Result<Vec<u8>, crate::opencode_host::OpenCodeRunnerFailure> {
        self.0.borrow_mut().export(config, session)
    }

    fn cancelled(&self, config: &OpenCodeHostConfig) -> bool {
        self.0.borrow().cancelled(config)
    }
}

pub(super) fn run(arguments: &[String]) -> Result<String, CliError> {
    execute_with_runner(Command::parse(arguments)?, ProcessOpenCodeRunner)
}

fn execute_with_runner<R: OpenCodeRunner + 'static>(
    command: Command,
    runner: R,
) -> Result<String, CliError> {
    execute_with_runner_and_candidate_test(command, runner, None)
}

/// Private embedding seam for an already-selected candidate-test capability.
pub(super) fn execute_with_runner_and_candidate_test<
    'host,
    'observer,
    R: OpenCodeRunner + 'static,
>(
    command: Command,
    runner: R,
    mut candidate_test: Option<&'host mut CandidateTestHost<'observer>>,
) -> Result<String, CliError> {
    let candidate_test_selected = candidate_test.is_some();
    let (config_path, checkpoint_path, fresh, terminal_receipt_only, provider_operands) =
        match command {
            Command::Run {
                config,
                checkpoint,
                provider,
            } => (config, checkpoint, true, false, provider),
            Command::Resume {
                config,
                checkpoint,
                provider,
            } => (config, checkpoint, false, false, provider),
            Command::Receipt {
                config,
                checkpoint,
                provider,
            } => (config, checkpoint, false, true, provider),
        };
    let pause_after_settled = provider_operands
        .as_ref()
        .is_some_and(|operands| operands.pause_after_settled);
    if terminal_receipt_only && pause_after_settled {
        return Err(CliError::usage(
            "repair receipt does not accept --pause-after-settled",
        ));
    }
    let config = RepairConfig::load(&config_path)?;
    if matches!(&config.provider, RepairProvider::Claude)
        != provider_operands.as_ref().is_some_and(|value| value.claude)
    {
        return Err(CliError::usage(
            "repair Claude configuration requires --claude ABS --scratch EMPTY_ABS",
        ));
    }

    // --- Ordinary lock/authority is acquired first, before any evidence
    // replay, staging or candidate creation. ---
    let manifest = config
        .manifest
        .canonicalize()
        .map_err(|_| CliError::refused("repair Project manifest is unavailable"))?;
    let project_root = manifest
        .parent()
        .ok_or(CliError::refused("repair Project manifest has no root"))?
        .to_owned();
    let project = with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
        .map_err(|diagnostics| {
            diagnostic_error("repair Project authentication refused", diagnostics)
        })?;

    let source = project
        .sources()
        .iter()
        .find(|source| source.path() == config.source_path)
        .ok_or(CliError::refused("repair source is unavailable"))?;
    let source_disk_path = project_root.join(&config.source_path);
    let source_before = source.source().as_bytes();
    // Test-only mutation is deliberately at the authentication-to-host-read
    // boundary so the regression executes this production recheck rather than
    // merely calling its helper.
    run_test_source_snapshot_hook();
    verify_checked_source_snapshot(&source_disk_path, source_before)?;

    let root = project
        .program_root()
        .map_err(|_| CliError::refused("repair ProgramRoot is unavailable"))?;
    let (_, deployment) = migrate_agent_definition_v1(
        project.agent_definitions()[0]
            .definition()
            .canonical_source(),
        &config.deployment_migration_id,
    )
    .map_err(|diagnostics| diagnostic_error("repair deployment migration refused", diagnostics))?;
    let compiled = compile_source_agent_lifecycle_v2(
        source.source(),
        source.path(),
        &config.agent_id,
        &config.step_id,
    )
    .map_err(|diagnostics| diagnostic_error("repair lifecycle compilation refused", diagnostics))?;
    let task = LifecycleTask {
        objective: bounded_read(&config.task_path, MAX_TASK_BYTES)?,
        budget: config.task_budget,
    };
    let iterative_budget = IterativeBudget::default();
    let effect_budget = EffectBudget {
        max_calls: config.max_calls,
        max_argument_bytes: config.max_argument_bytes,
        max_result_bytes: config.max_result_bytes,
        max_total_bytes: config.max_total_bytes,
    };
    let runtime = bind_agent_runtime_v2_live(
        Arc::clone(&project),
        ProgramRootRef::V1(&root),
        root.program_root_digest(),
        &config.source_path,
        &config.agent_id,
        &config.step_id,
        &config.selector_field_id,
        operations(&config),
        &deployment,
        task.clone(),
        iterative_budget,
        effect_budget,
    )
    .map_err(|diagnostics| diagnostic_error("repair runtime binding refused", diagnostics))?;
    if candidate_test.is_some()
        && !matches!(
            &config.provider,
            RepairProvider::OpenCode | RepairProvider::Claude
        )
    {
        return Err(CliError::refused(
            "candidate-test capability requires OpenCode repair configuration",
        ));
    }
    let (adapter_identity, clock, process_adapter_identity): (
        _,
        Box<dyn SourceInvocationClock>,
        Option<String>,
    ) = match &config.provider {
        RepairProvider::Scripted(_) if provider_operands.is_none() => {
            (scripted_identity(), Box::new(FixedClock), None)
        }
        RepairProvider::Scripted(_) => {
            return Err(CliError::usage(
                "repair fixture configuration does not accept OpenCode operands",
            ));
        }
        RepairProvider::OpenCode if provider_operands.is_some() => {
            let operands = provider_operands
                .as_ref()
                .expect("provider operands were checked");
            let executable = operands
                .executable
                .canonicalize()
                .map_err(|_| CliError::refused("repair OpenCode executable is unavailable"))?;
            let scratch = operands
                .scratch
                .canonicalize()
                .map_err(|_| CliError::refused("repair OpenCode scratch is unavailable"))?;
            let process_identity = source_model_identity(&executable, &scratch);
            let process_adapter_identity = process_identity.adapter_identity.clone();
            (
                candidate_test_bound_identity(
                    source_deployment_identity(process_identity)?,
                    candidate_test.as_deref().map(|host| &host.capability),
                    &config.target,
                    source.source_revision(),
                ),
                Box::new(UnixClock),
                Some(process_adapter_identity),
            )
        }
        RepairProvider::Claude => {
            let operands = provider_operands.as_ref().expect("Claude operands checked");
            let identity = claude::identity(operands)?;
            let process_identity = identity.adapter_identity.clone();
            (
                candidate_test_bound_identity(
                    identity,
                    candidate_test.as_deref().map(|host| &host.capability),
                    &config.target,
                    source.source_revision(),
                ),
                Box::new(UnixClock),
                Some(process_identity),
            )
        }
        RepairProvider::OpenCode => {
            return Err(CliError::usage(
                "repair OpenCode configuration requires --opencode ABS --scratch EMPTY_ABS",
            ));
        }
    };
    let bound_adapter_identity = adapter_identity.adapter_identity.clone();
    let receipt_adapter_identity = adapter_identity.clone();
    let model_binding = runtime
        .source_model_binding(adapter_identity)
        .map_err(|diagnostics| {
            diagnostic_error("repair source model binding refused", diagnostics)
        })?;
    let source_policy = policy(
        &config,
        model_binding.digest(),
        model_binding.max_response_bytes(),
        clock.clock_domain(),
    );
    let receipt_context = matches!(
        &config.provider,
        RepairProvider::OpenCode | RepairProvider::Claude
    )
    .then(|| RepairReceiptContext {
        provider_id: receipt_adapter_identity.provider_id,
        model_id: receipt_adapter_identity.model_id,
        adapter_identity: receipt_adapter_identity.adapter_identity,
        adapter_version: receipt_adapter_identity.adapter_version,
        provider_profile: receipt_adapter_identity.provider_profile,
        program_root: root.program_root_digest().to_owned(),
        source_revision: source.source_revision().to_owned(),
        proposal_schema_digest: compiled.proposal_schema().schema().digest().to_owned(),
        deployment_binding: model_binding.digest().to_owned(),
    });

    let mut store = if fresh {
        CheckpointDir::fresh(&checkpoint_path, &project_root)?
    } else {
        CheckpointDir::existing(&checkpoint_path, &project_root)?
    };
    let latest = store.latest()?;
    if fresh && latest.is_some() || !fresh && latest.is_none() {
        return Err(CliError::refused(
            "checkpoint mode does not match latest journal",
        ));
    }

    // Recovery is deliberately completed before this function creates an
    // adapter, grants an adapter capability, or creates fixture-only candidate
    // and effect handlers. This applies to the inherited V1 fixture as well as
    // the V2 OpenCode route: a terminal journal is sufficient for a read-only
    // receipt, so fixture-only diagnostic derivation must not turn replay into
    // a target lookup or candidate-preview action. The exact binding is
    // derived by the existing typed runtime and the retained document is
    // admitted by the existing journal decoder; this preflight neither
    // recreates either trust calculation nor treats the evidence as authority.
    let retained_pause_marker = if !fresh {
        let recovered = runtime
            .preflight_source_live_checkpoint(
                &model_binding,
                source_policy.clone(),
                latest.as_deref().expect("resume mode has a latest journal"),
                clock.as_ref(),
            )
            .map_err(|error| match &config.provider {
                RepairProvider::OpenCode | RepairProvider::Claude => v2_replay_refusal(error),
                RepairProvider::Scripted(_) => {
                    CliError::refused("repair V1 retained checkpoint cannot be recovered")
                }
            })?;
        store.set_generation(recovered.generation());
        if recovered.terminal_snapshot().is_some() {
            let terminal_patch_receipt = matches!(
                &config.provider,
                RepairProvider::OpenCode | RepairProvider::Claude
            )
            .then(|| {
                store
                    .terminal_patch_receipt()?
                    .ok_or(CliError::refused("terminal patch receipt is unavailable"))
                    .and_then(|document| TerminalPatchReceipt::recover(document, &recovered))
            })
            .transpose()?;
            let replayed_candidate_test_evidence = replayed_candidate_test_evidence(
                &recovered,
                &config.corrected_operation_id,
                &config.result_id,
                candidate_test_selected,
            );
            return receipt_with_preview(
                &config,
                None,
                0,
                None,
                replayed_candidate_test_evidence,
                candidate_test_selected,
                receipt_context.as_ref(),
                terminal_patch_receipt.as_ref(),
                &recovered,
                0,
                0,
            );
        }
        if terminal_receipt_only {
            return Err(CliError::refused(
                "repair receipt requires a terminal checkpoint",
            ));
        }
        barrier::marker_for_recovered_checkpoint(&recovered)
    } else {
        None
    };

    let envelope = OfflineRepairEnvelope::new(Arc::clone(&project), config.target.clone())
        .map_err(|diagnostics| diagnostic_error("repair target envelope refused", diagnostics))?;
    let preceding_effect_hex = Rc::new(RefCell::new(None));
    let mut handler = FeedbackRecordingHandler {
        inner: OfflineRepairHandler::new(
            envelope,
            config.malformed_operation_id.clone(),
            config.corrected_operation_id.clone(),
            config.effect_id.clone(),
            config.argument_id.clone(),
            config.result_id.clone(),
        )
        .map_err(|diagnostics| diagnostic_error("repair effect contract refused", diagnostics))?,
        preceding_effect_hex: Rc::clone(&preceding_effect_hex),
        source_revision: source.source_revision().to_owned(),
        candidate_test: candidate_test.take(),
        candidate_test_evidence: None,
        candidate_test_refused: false,
    };

    let mut scripted_starts = None;
    let mut pause_marker_host = None;
    let mut factory: Box<dyn FnMut() -> Box<dyn ProviderAdapter>> = match &config.provider {
        RepairProvider::Scripted(turns) => {
            let scripts = RefCell::new(VecDeque::from(
                turns
                    .clone()
                    .map(|turn| (turn.document, turn.requires_prior_feedback)),
            ));
            let starts = Rc::new(Cell::new(0));
            scripted_starts = Some((Rc::clone(&starts), turns.len()));
            Box::new(move || -> Box<dyn ProviderAdapter> {
                let next = scripts.borrow_mut().pop_front();
                let refuse_start = next.is_none();
                let (document, requires_prior_feedback) =
                    next.unwrap_or_else(|| (String::new(), false));
                Box::new(FeedbackGuardedAdapter {
                    inner: ScriptedStreamingAdapter::new(
                        document
                            .as_bytes()
                            .chunks(3)
                            .map(ToOwned::to_owned)
                            .collect(),
                        document.into_bytes(),
                        usage(1, 1, 0),
                        true,
                    ),
                    starts: Rc::clone(&starts),
                    requires_prior_feedback,
                    expected_feedback: Rc::clone(&preceding_effect_hex),
                    refuse_start,
                })
            })
        }
        RepairProvider::Claude => claude::factory(
            provider_operands.expect("Claude operands checked"),
            config.deadline_millis.saturating_sub(clock.now_millis()),
            process_adapter_identity
                .as_deref()
                .expect("Claude identity checked"),
            bound_adapter_identity.clone(),
            compiled.proposal_schema(),
            retained_pause_marker.as_deref(),
            &mut pause_marker_host,
        )?,
        RepairProvider::OpenCode => {
            let operands = provider_operands.expect("OpenCode operands were checked above");
            let grammar = OpenCodeGrammar::from_proposal(compiled.proposal_schema())
                .map_err(|_| CliError::refused("repair OpenCode grammar admission refused"))?;
            let remaining = config.deadline_millis.saturating_sub(clock.now_millis());
            let call_millis = remaining.clamp(1, MAX_ONE_PROVIDER_CALL_MS) as u64;
            let host = OpenCodeHostConfig::new(
                operands.executable,
                operands.scratch,
                Duration::from_millis(call_millis),
                grammar,
            )
            .map_err(|_| CliError::refused("repair OpenCode host configuration refused"))?;
            if process_adapter_identity.as_deref()
                != Some(
                    source_model_identity_for_config(&host)
                        .adapter_identity
                        .as_str(),
                )
            {
                return Err(CliError::refused(
                    "repair OpenCode executable changed while binding the host",
                ));
            }
            host.clear_repair_post_settled_marker(retained_pause_marker.as_deref())
                .map_err(|_| {
                    CliError::refused(
                        "repair OpenCode post-settlement pause marker does not match authenticated checkpoint",
                    )
                })?;
            if pause_after_settled {
                pause_marker_host = Some(host.clone());
            }
            let runner = Rc::new(RefCell::new(runner));
            let bound_adapter_identity = bound_adapter_identity.clone();
            Box::new(move || -> Box<dyn ProviderAdapter> {
                Box::new(OpenCodeRepairAdapter::new_with_adapter_identity(
                    host.clone(),
                    SharedRunner(Rc::clone(&runner)),
                    bound_adapter_identity.clone(),
                ))
            })
        }
    };
    let cancellation = AgentCancellation::new();
    let mut source = StreamingSourceProposalAdapter::new_bound_checkpointed(
        &mut factory,
        AdapterInvocationCapability::grant("source-live repair host-selected provider"),
        compiled.proposal_schema(),
        model_binding.clone(),
        model_binding.invocation_capability(),
        checkpoint_policy(
            model_binding.digest(),
            model_binding.max_response_bytes(),
            config.reservation_units,
        ),
    )
    .map_err(|diagnostics| diagnostic_error("repair source adapter refused", diagnostics))?;
    let retained_checkpoint = latest.as_deref();
    // This host-local wrapper has no journal authority of its own. It only
    // observes a successful physical checkpoint commit and, when explicitly
    // selected by the operator, parks after one settled provider response.
    let mut barrier_store = PostSettledBarrierStore::new(&mut store, pause_marker_host);
    let complete = runtime
        .run_live_bound_model_durable(
            &mut source,
            &mut handler,
            source_policy,
            clock.as_ref(),
            &cancellation,
            retained_checkpoint,
            &mut barrier_store,
        )
        .map_err(|failure| {
            diagnostic_error(
                "repair checked source execution refused",
                failure.failure().diagnostics.to_vec(),
            )
        })?;
    drop(source);
    drop(barrier_store);

    if handler.candidate_test_refused() {
        return Err(CliError::refused(
            "repair candidate-test observation was refused",
        ));
    }

    let model_dispatches = complete.run().model_dispatches;
    let effect_dispatches = complete.run().effect_dispatches;
    // A pure terminal-checkpoint replay dispatches nothing. Fixture mode also
    // proves its complete fixed sequence was consumed; OpenCode mode instead
    // relies on the retained journal's acknowledged intent/settlement chain.
    if let Some((starts, turns)) = scripted_starts {
        if model_dispatches > 0 && starts.get() != turns {
            return Err(CliError::refused(
                "repair preview did not consume its exact scripted turn sequence",
            ));
        }
    }
    verify_checked_source_snapshot(&source_disk_path, source_before)?;

    let preview = handler.latest_preview();
    let terminal_patch_receipt = matches!(
        &config.provider,
        RepairProvider::OpenCode | RepairProvider::Claude
    )
    .then(|| {
        preview
            .ok_or(CliError::refused(
                "repair terminal candidate preview is unavailable",
            ))
            .and_then(|preview| TerminalPatchReceipt::derive(preview, &complete.run().checkpoint))
    })
    .transpose()?;
    if let Some(terminal_patch_receipt) = &terminal_patch_receipt {
        store.retain_terminal_patch_receipt(&terminal_patch_receipt.document)?;
    }
    let rejection_count = handler.rejection_count();
    let candidate_test_evidence = handler.candidate_test_evidence();
    // A resumed invocation may execute new candidate tests. Its live evidence
    // owns this receipt; deriving replay evidence from the just-written journal
    // would count that same observation twice. Replay-only runs use the journal.
    let replayed_candidate_test_evidence = if fresh || candidate_test_evidence.is_some() {
        None
    } else {
        replayed_candidate_test_evidence(
            &complete.run().checkpoint,
            &config.corrected_operation_id,
            &config.result_id,
            candidate_test_selected,
        )
    };
    receipt_with_preview(
        &config,
        preview,
        rejection_count,
        candidate_test_evidence,
        replayed_candidate_test_evidence,
        candidate_test_selected,
        receipt_context.as_ref(),
        terminal_patch_receipt.as_ref(),
        &complete.run().checkpoint,
        model_dispatches,
        effect_dispatches,
    )
}

#[allow(clippy::too_many_arguments)]
fn receipt_with_preview(
    config: &RepairConfig,
    preview: Option<&semaprax::agent_runtime_v2::OfflineRepairPreview>,
    rejection_count: u32,
    candidate_test_evidence: Option<&CandidateTestEvidence>,
    replayed_candidate_test_evidence: Option<ReplayedCandidateTestEvidence>,
    candidate_test_selected: bool,
    receipt_context: Option<&RepairReceiptContext>,
    terminal_patch_receipt: Option<&TerminalPatchReceipt>,
    checkpoint: &semaprax::live_invocation::source_journal::RecoveredSourceCheckpoint,
    model_dispatches: u32,
    effect_dispatches: u32,
) -> Result<String, CliError> {
    let base = receipt(
        config,
        preview,
        candidate_test_evidence,
        replayed_candidate_test_evidence,
        candidate_test_selected,
        receipt_context,
        terminal_patch_receipt,
        checkpoint,
        model_dispatches,
        effect_dispatches,
    )?;
    let mut value: Value = serde_json::from_str(&base)
        .map_err(|_| CliError::refused("repair report cannot be rendered"))?;
    value["rejected_candidates"] = json!(rejection_count);
    serde_json::to_string(&value)
        .map(|report| format!("{report}\n"))
        .map_err(|_| CliError::refused("repair report cannot be rendered"))
}

#[cfg(test)]
#[path = "repair_tests.rs"]
mod tests;
