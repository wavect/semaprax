//! Fixed, credential-free private-host demonstration of checked offline repair.
//!
//! The command deliberately has no user-controlled source, target, provider,
//! or publication operand. It runs the checked two-turn source/effect loop over
//! the bundled Project and discards the candidate after rendering its evidence.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use semaprax::agent_deployment::migrate_agent_definition_v1;
use semaprax::agent_lifecycle::iterative::compile_source_agent_lifecycle_v2;
use semaprax::agent_lifecycle::iterative::effects::{
    EffectArgument, EffectBudget, EffectOperation, EffectResult, EffectScalar,
};
use semaprax::agent_lifecycle::iterative::source_live::{SourceLivePolicy, SourceProposalPolicy};
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError, LifecycleTask};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::agent_runtime_v2::{
    bind_agent_runtime_v2_live, OfflineRepairEnvelope, OfflineRepairHandler,
    SourceModelAdapterIdentity,
};
use semaprax::execution_revision::ProgramRootRef;
use semaprax::live_invocation::{InvocationClock, SourceInvocationClock};
use semaprax::project::{with_authenticated_project, ProjectRevision};
use semaprax::provider_adapter_sdk::fixture_adapters::{usage, ScriptedStreamingAdapter};
use semaprax::provider_adapter_sdk::{
    AdapterInvocationCapability, AdapterPoll, AdapterRefusal, AdapterRequest, ProviderAdapter,
    StreamingSourceProposalAdapter,
};
use serde_json::{json, Value};

use super::CliError;
mod model_wait;
pub(super) use model_wait::run as run_model_wait;

const DEMO_SCHEMA: &str = "semaprax.private-offline-repair-demo.v1";
const DEMO_MANIFEST: &str = "../../examples/offline-repair-project/semaprax.toml";
const DEMO_TARGET: &str = "fixture.repair.value";
const CLOCK_DOMAIN: &str = "private-offline-repair-demo.v1";

struct FixedClock;
impl InvocationClock for FixedClock {
    fn now_millis(&self) -> i64 {
        1
    }
}
impl SourceInvocationClock for FixedClock {
    fn clock_domain(&self) -> &str {
        CLOCK_DOMAIN
    }
}

#[derive(Default)]
struct JournalStore {
    documents: Vec<String>,
}
impl CheckpointStore for JournalStore {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.documents.push(document.to_owned());
        Ok(())
    }
}

struct FeedbackGuardedAdapter {
    inner: ScriptedStreamingAdapter,
    starts: Rc<Cell<usize>>,
    required_feedback: Option<String>,
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
                "offline repair demo exhausted its fixed proposal script".to_owned(),
            ));
        }
        self.starts.set(self.starts.get() + 1);
        if self.required_feedback.as_ref().is_some_and(|expected| {
            serde_json::from_slice::<Value>(&request.request_bytes)
                .ok()
                .and_then(|prompt| prompt["previous_effect_hex"].as_str().map(str::to_owned))
                .as_deref()
                != Some(expected)
        }) {
            return Err(AdapterRefusal(
                "offline repair correction omitted checked diagnostic feedback".to_owned(),
            ));
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

fn operations() -> Vec<EffectOperation> {
    ["fixture.read", "fixture.read.second"]
        .into_iter()
        .map(|operation_id| EffectOperation {
            operation_id: operation_id.to_owned(),
            effect_id: "read".to_owned(),
            arguments: vec![EffectArgument {
                argument_id: "query".to_owned(),
                proposal_field_id: "fixture.agent.type.proposal.budget".to_owned(),
                kind: EffectScalar::I64,
            }],
            results: vec![EffectResult {
                result_id: "value".to_owned(),
                kind: EffectScalar::I64,
            }],
        })
        .collect()
}

fn identity() -> SourceModelAdapterIdentity {
    SourceModelAdapterIdentity {
        provider_id: "fake.local".to_owned(),
        model_id: "fake-basic".to_owned(),
        adapter_identity: "scripted-streaming-adapter".to_owned(),
        adapter_version: "1.0.0".to_owned(),
        provider_profile: "fixture".to_owned(),
    }
}

fn policy(binding: &semaprax::agent_runtime_v2::SourceModelBinding) -> SourceLivePolicy {
    SourceLivePolicy {
        deployment_binding: binding.digest().to_owned(),
        response_limit: binding.max_response_bytes(),
        ceiling: 2,
        reservation_units: 1,
        unit: "private_offline_repair_unit_v1".to_owned(),
        clock_domain: CLOCK_DOMAIN.to_owned(),
        initial_millis: 0,
        deadline_millis: 1_000,
        max_total_steps: 2_000_000,
        program_root: None,
    }
}

fn checkpoint_policy<'a>(
    binding: &'a semaprax::agent_runtime_v2::SourceModelBinding,
) -> SourceProposalPolicy<'a> {
    SourceProposalPolicy {
        deployment_binding: binding.digest(),
        response_limit: binding.max_response_bytes(),
        reservation_units: 1,
    }
}

fn proposal(schema_digest: &str, budget: &str, sequence: &str) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{schema_digest}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.proposal.budget\":\"{budget}\",",
            "\"fixture.agent.type.proposal.urgent\":false,",
            "\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        schema_digest = schema_digest,
        budget = budget,
        sequence = sequence,
    )
}

fn feedback_hex(code: i64) -> String {
    format!(
        "{{\"schema\":\"semaprax.agent-effect-fields.v1\",\"fields\":[[\"value\",\"{code}\"]]}}\n"
    )
    .bytes()
    .map(|byte| format!("{byte:02x}"))
    .collect()
}

fn feedback_code(diagnostics: &[semaprax::diagnostic::Diagnostic]) -> i64 {
    diagnostics
        .iter()
        .find_map(|diagnostic| {
            diagnostic
                .code
                .strip_prefix("SPX-G")
                .and_then(|code| code.parse::<i64>().ok())
        })
        .unwrap_or(583)
}

fn demo_manifest() -> Result<PathBuf, CliError> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(DEMO_MANIFEST)
        .canonicalize()
        .map_err(|_| CliError::refused("offline repair demo Project is unavailable"))
}

fn checked_value(document: &str, field: &'static str) -> Result<Value, CliError> {
    serde_json::from_str(document).map_err(|_| CliError::refused(field))
}

fn diagnostic_error(context: &str, diagnostics: Vec<semaprax::diagnostic::Diagnostic>) -> CliError {
    CliError::detail(format!("{context}: {diagnostics:?}"))
}

fn execute(project: Arc<ProjectRevision>, source_before: Vec<u8>) -> Result<String, CliError> {
    execute_profile(project, source_before, false)
}

fn execute_profile(
    project: Arc<ProjectRevision>,
    source_before: Vec<u8>,
    wait_profile: bool,
) -> Result<String, CliError> {
    let root = project
        .program_root()
        .map_err(|_| CliError::refused("offline repair demo ProgramRoot is unavailable"))?;
    let source = project
        .sources()
        .iter()
        .find(|source| source.path() == "src/app.spx")
        .ok_or(CliError::refused(
            "offline repair demo source is unavailable",
        ))?;
    let (_, deployment) = migrate_agent_definition_v1(
        project.agent_definitions()[0]
            .definition()
            .canonical_source(),
        "private.offline.repair.demo",
    )
    .map_err(|diagnostics| {
        diagnostic_error("offline repair deployment migration refused", diagnostics)
    })?;
    let compiled = compile_source_agent_lifecycle_v2(
        source.source(),
        source.path(),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .map_err(|diagnostics| {
        diagnostic_error("offline repair lifecycle compilation refused", diagnostics)
    })?;
    let schema = compiled.proposal_schema();
    let runtime = bind_agent_runtime_v2_live(
        Arc::clone(&project),
        ProgramRootRef::V1(&root),
        root.program_root_digest(),
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
        "fixture.agent.type.proposal.sequence",
        operations(),
        &deployment,
        LifecycleTask {
            objective: b"repair the fixed private demo candidate".to_vec(),
            budget: 12,
        },
        semaprax::agent_lifecycle::iterative::IterativeBudget::default(),
        EffectBudget {
            max_calls: 2,
            max_argument_bytes: 4096,
            max_result_bytes: 4096,
            max_total_bytes: 8192,
        },
    )
    .map_err(|diagnostics| {
        diagnostic_error("offline repair runtime binding refused", diagnostics)
    })?;

    let envelope =
        OfflineRepairEnvelope::new(Arc::clone(&project), DEMO_TARGET).map_err(|diagnostics| {
            diagnostic_error("offline repair target envelope refused", diagnostics)
        })?;
    let malformed = envelope.preview(0, true).err().ok_or(CliError::refused(
        "offline repair malformed candidate unexpectedly admitted",
    ))?;
    let expected_feedback = feedback_hex(feedback_code(&malformed));
    let mut handler = OfflineRepairHandler::new(
        envelope,
        "fixture.read",
        "fixture.read.second",
        "read",
        "query",
        "value",
    )
    .map_err(|diagnostics| {
        diagnostic_error("offline repair effect contract refused", diagnostics)
    })?;

    let scripts = RefCell::new(VecDeque::from([
        (proposal(schema.schema().digest(), "0", "0"), None),
        (
            proposal(schema.schema().digest(), "7", "1"),
            Some(expected_feedback.clone()),
        ),
    ]));
    let starts = Rc::new(Cell::new(0));
    let factory_starts = Rc::clone(&starts);
    let mut factory = move || -> Box<dyn ProviderAdapter> {
        let next = scripts.borrow_mut().pop_front();
        let refuse_start = next.is_none();
        let (document, required_feedback) = next.unwrap_or_else(|| (String::new(), None));
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
            starts: Rc::clone(&factory_starts),
            required_feedback,
            refuse_start,
        })
    };
    let wait = if wait_profile {
        Some(
            runtime
                .source_model_wait_binding(model_wait::WRAPPER_ID, model_wait::EVALUATION_FUEL)
                .map_err(|diagnostics| {
                    diagnostic_error("offline repair wait binding refused", diagnostics)
                })?,
        )
    } else {
        None
    };
    let binding = runtime
        .source_model_binding(identity())
        .map_err(|diagnostics| {
            diagnostic_error("offline repair source model binding refused", diagnostics)
        })?;
    let cancellation = AgentCancellation::new();
    let mut source = StreamingSourceProposalAdapter::new_bound_checkpointed(
        &mut factory,
        AdapterInvocationCapability::grant("private fixed offline repair demo"),
        schema,
        binding.clone(),
        binding.invocation_capability(),
        checkpoint_policy(&binding),
    )
    .map_err(|diagnostics| {
        diagnostic_error("offline repair source adapter refused", diagnostics)
    })?;
    let mut store = JournalStore::default();
    let key = semaprax::resumable_effects::source_checkpoint::SourceCheckpointKey::new([68; 32]);
    let (ordinary, waited) = if let Some(wait) = &wait {
        (
            None,
            Some(
                runtime
                    .run_live_bound_model_durable_with_wait(
                        wait,
                        &key,
                        &mut source,
                        &mut handler,
                        policy(&binding),
                        &FixedClock,
                        &cancellation,
                        None,
                        &mut store,
                    )
                    .map_err(|failure| {
                        diagnostic_error(
                            "offline repair checked model wait execution refused",
                            failure.failure().diagnostics.to_vec(),
                        )
                    })?,
            ),
        )
    } else {
        (
            Some(
                runtime
                    .run_live_bound_model_durable(
                        &mut source,
                        &mut handler,
                        policy(&binding),
                        &FixedClock,
                        &cancellation,
                        None,
                        &mut store,
                    )
                    .map_err(|failure| {
                        diagnostic_error(
                            "offline repair checked source execution refused",
                            failure.failure().diagnostics.to_vec(),
                        )
                    })?,
            ),
            None,
        )
    };
    let complete = ordinary
        .as_ref()
        .or_else(|| waited.as_ref().map(|e| e.model()))
        .expect("selected execution profile returns one evidence value");
    drop(source);
    if starts.get() != 2 || handler.rejection_count() != 1 || store.documents.is_empty() {
        return Err(CliError::refused(
            "offline repair demo did not reach its checked two-call terminal",
        ));
    }
    let preview = handler.latest_preview().ok_or(CliError::refused(
        "offline repair corrected candidate was not retained",
    ))?;
    if preview.bool_literal() || preview.replacement() != 7 {
        return Err(CliError::refused(
            "offline repair candidate evidence disagrees with the fixed correction",
        ));
    }
    let manifest = if wait_profile {
        model_wait::manifest()?
    } else {
        demo_manifest()?
    };
    let source_path = manifest
        .parent()
        .expect("canonical manifest has a parent")
        .join("src/app.spx");
    if std::fs::read(source_path)
        .map_err(|_| CliError::refused("offline repair demo source cannot be reread"))?
        != source_before
    {
        return Err(CliError::refused("offline repair demo source changed"));
    }
    let journal = checked_value(
        store.documents.last().expect("nonempty checked journal"),
        "offline repair journal rendering refused",
    )?;
    let mut report = json!({
        "schema": DEMO_SCHEMA,
        "target": DEMO_TARGET,
        "candidate_digest": preview.candidate().candidate_digest(),
        "source_review": checked_value(preview.source_review(), "offline repair source review refused")?,
        "semantic_delta": checked_value(preview.semantic_delta(), "offline repair semantic delta refused")?,
        "impact_summary": checked_value(preview.impact_summary(), "offline repair impact summary refused")?,
        "model_dispatches": complete.run().model_dispatches,
        "effect_dispatches": complete.run().effect_dispatches,
        "provider_starts": starts.get(),
        "rejected_candidates": handler.rejection_count(),
        "feedback_guarded": true,
        "journal_generations": store.documents.len(),
        "journal": journal,
        "source_mutation": false,
        "publication_authority": false,
    });
    if let Some(waited) = &waited {
        model_wait::add_report(&mut report, waited)?;
    }
    serde_json::to_string(&report)
        .map(|report| format!("{report}\n"))
        .map_err(|_| CliError::refused("offline repair report cannot be rendered"))
}

pub(super) fn run(arguments: &[String]) -> Result<String, CliError> {
    if !arguments.is_empty() {
        return Err(CliError::usage(
            "offline-repair takes no operands; it always runs the fixed demo Project",
        ));
    }
    let manifest = demo_manifest()?;
    let source_path = manifest
        .parent()
        .expect("canonical manifest has a parent")
        .join("src/app.spx");
    let source_before = std::fs::read(&source_path)
        .map_err(|_| CliError::refused("offline repair demo source is unavailable"))?;
    let project = with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
        .map_err(|diagnostics| {
            diagnostic_error(
                "offline repair demo Project authentication refused",
                diagnostics,
            )
        })?;
    execute(project, source_before)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_demo_runs_two_checked_calls_without_source_mutation() {
        let rendered = run(&[]).unwrap();
        let report: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(report["schema"], DEMO_SCHEMA);
        assert_eq!(report["model_dispatches"], 2);
        assert_eq!(report["effect_dispatches"], 2);
        assert_eq!(report["provider_starts"], 2);
        assert_eq!(report["rejected_candidates"], 1);
        assert_eq!(report["source_mutation"], false);
        assert_eq!(report["publication_authority"], false);
        assert!(report["journal"]["entries"].is_array());
    }
}
