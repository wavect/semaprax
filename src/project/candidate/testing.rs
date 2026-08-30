//! Static test relevance and explicit, policy-bounded interpreter evidence.
//! Candidate/source replay is mandatory; no source, process, or target authority.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{ProjectExecution, ProjectExecutionOptions, ProjectRevision};

use super::{wire, ProjectCandidate};

pub const PROJECT_CANDIDATE_TEST_PLAN_SCHEMA: &str = "semaprax.project-candidate-test-plan.v1";
pub const PROJECT_CANDIDATE_TEST_REPORT_SCHEMA: &str = "semaprax.project-candidate-test-report.v1";
pub const MAX_PROJECT_CANDIDATE_TEST_PLAN_BYTES: usize = 65_536;
pub const MAX_PROJECT_CANDIDATE_TEST_REPORT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CANDIDATE_TEST_STEPS: usize = 1_000_000;
const MAX_EXECUTION_BYTES: usize = 65_536;
const MAX_CALLS: usize = 65_536;
const REPORT_DOMAIN: &[u8] = b"semaprax.candidate-test.report.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Constructed by the embedding host, never widened by an execution request.
/// This policy does not configure filesystem, process, network, or trace access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CandidateTestPolicy {
    max_steps: usize,
    max_execution_bytes: usize,
    max_report_bytes: usize,
}

impl CandidateTestPolicy {
    pub fn new(
        max_steps: usize,
        max_execution_bytes: usize,
        max_report_bytes: usize,
    ) -> Result<Self> {
        if !(1..=MAX_CANDIDATE_TEST_STEPS).contains(&max_steps)
            || !(1024..=MAX_EXECUTION_BYTES).contains(&max_execution_bytes)
            || !(16_384..=MAX_PROJECT_CANDIDATE_TEST_REPORT_BYTES).contains(&max_report_bytes)
        {
            return Err(invalid(
                "candidate test policy exceeds its fixed fuel or output bounds",
            ));
        }
        ProjectExecutionOptions::new(max_execution_bytes, max_steps)
            .map_err(|error| vec![error])?;
        Ok(Self {
            max_steps,
            max_execution_bytes,
            max_report_bytes,
        })
    }
    pub fn max_steps(&self) -> usize {
        self.max_steps
    }
    pub fn max_execution_bytes(&self) -> usize {
        self.max_execution_bytes
    }
    pub fn max_report_bytes(&self) -> usize {
        self.max_report_bytes
    }
    fn value(&self) -> Value {
        json!({"max_steps":self.max_steps,"max_execution_bytes":self.max_execution_bytes,
            "max_report_bytes":self.max_report_bytes,"call_depth_limit":crate::interpreter::MAX_CALL_DEPTH,
            "trace":{"mode":"disabled","max_events":0,"max_bytes":0}})
    }
}

/// An immutable report of one completed interpreter attempt, not a gate token.
pub struct CandidateTestReport {
    json: String,
    digest: String,
    execution: ProjectExecution,
}

impl CandidateTestReport {
    pub fn to_json(&self) -> &str {
        &self.json
    }
    /// Binds every exact canonical report byte, including its terminal LF.
    pub fn report_digest(&self) -> &str {
        &self.digest
    }
    pub fn passed(&self) -> bool {
        self.execution.command_succeeded()
    }
    pub fn execution(&self) -> &ProjectExecution {
        &self.execution
    }
}

impl ProjectCandidate {
    /// Select the one declared test root by static transitive HIR dependencies.
    /// A nonselected root is not a safety, coverage, or compatibility proof.
    pub fn test_plan(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let before = reachability(&self.base)?;
        let after = reachability(&self.revision)?;
        let mut targets = BTreeSet::new();
        let mut fallback = BTreeSet::new();
        for change in &self.changes {
            let intent = &change.intent;
            let kind = intent["kind"]
                .as_str()
                .ok_or_else(|| invalid("retained test intention has no kind"))?;
            if kind == "add_declaration" {
                targets.insert(required_id(&intent["declaration"], "id")?.to_owned());
            } else {
                targets.insert(required_id(intent, "target")?.to_owned());
                if kind == "extract_function" {
                    targets.insert(required_id(intent, "new_id")?.to_owned());
                }
            }
            match kind {
                "rename_declaration"
                | "replace_function_body"
                | "replace_expression"
                | "replace_contract_expression"
                | "change_function_signature"
                | "add_contract"
                | "add_declaration"
                | "extract_function" => {}
                "move_declaration" => {
                    fallback.insert("module_binding_and_origin_change");
                }
                "add_record_field" => {
                    fallback.insert("non_callable_record_shape_change");
                }
                _ => {
                    fallback.insert("unclassified_semantic_change");
                }
            }
        }
        if !targets.is_empty() && (before.opaque || after.opaque) {
            fallback.insert("opaque_static_call_dependency");
        }
        let before_hits = targets
            .intersection(&before.ids)
            .cloned()
            .collect::<Vec<_>>();
        let after_hits = targets
            .intersection(&after.ids)
            .cloned()
            .collect::<Vec<_>>();
        let selected = !before_hits.is_empty() || !after_hits.is_empty() || !fallback.is_empty();
        let origin = test_origin(&self.revision)?;
        render(
            json!({
                "schema":PROJECT_CANDIDATE_TEST_PLAN_SCHEMA,
                "candidate_digest":self.candidate_digest(),"base_project_revision":self.base.project_revision(),
                "project_revision":self.revision.project_revision(),"declared_test_count":1,
                "selection_basis":"static_transitive_HIR_calls_not_runtime_coverage",
                "changed_targets":targets,"base_reachable_changed_targets":before_hits,
                "candidate_reachable_changed_targets":after_hits,"conservative_reasons":fallback,
                "base_reachable_callable_count":before.ids.len(),"candidate_reachable_callable_count":after.ids.len(),
                "selected":selected,"test_origin":origin,
                "selected_tests":if selected { vec![origin.clone()] } else { Vec::<Value>::new() },
                "execution":"not_run","explicit_execution_scope":"complete_manifest_declared_test_closure",
                "nonclaims":["not_dynamic_coverage","not_proof_unselected_code_is_safe","no_external_tests_or_callers","no_execution_or_source_authority"]
            }),
            MAX_PROJECT_CANDIDATE_TEST_PLAN_BYTES,
        )
    }

    /// Execute only after exact independent candidate replay. Even an empty
    /// relevance selection executes the complete manifest test closure when
    /// explicitly requested. The caller supplies immutable host policy.
    pub fn execute_tests(
        &self,
        expected_candidate: &str,
        policy: &CandidateTestPolicy,
    ) -> Result<CandidateTestReport> {
        self.require_candidate(expected_candidate)?;
        CandidateTestPolicy::new(
            policy.max_steps,
            policy.max_execution_bytes,
            policy.max_report_bytes,
        )?;
        let replay = ProjectCandidate::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        let plan = replay.test_plan(replay.candidate_digest())?;
        let plan_value: Value =
            serde_json::from_str(&plan).map_err(|_| invalid("compiler test plan is invalid"))?;
        let options = policy.value();
        let options_bytes = render(options.clone(), policy.max_report_bytes)?;
        let origins = json!({"base":source_inventory(&replay.base),"candidate":source_inventory(&replay.revision)});
        let origin_bytes = render(origins.clone(), policy.max_report_bytes)?;
        let diffs = Value::Array(diff_inventory(&replay)?);
        let diff_bytes = render(diffs.clone(), policy.max_report_bytes)?;
        let mut report = json!({
            "schema":PROJECT_CANDIDATE_TEST_REPORT_SCHEMA,
            "compiler":{"package_version":env!("CARGO_PKG_VERSION"),"compatibility":"semaprax.candidate-tests.interpreter.v1","binary_identity_claimed":false},
            "candidate_digest":replay.candidate_digest(),"base_project_revision":replay.base.project_revision(),
            "project_revision":replay.revision.project_revision(),"workspace_revision":replay.revision.workspace_revision(),
            "candidate_replay":"exact_source_and_evidence_replay_before_execution",
            "source_inventory":origins,"source_inventory_digest":wire::digest(b"semaprax.candidate-test.sources.v1\0",origin_bytes.as_bytes()),
            "source_diffs":diffs,"source_diff_inventory_digest":wire::digest(b"semaprax.candidate-test.diffs.v1\0",diff_bytes.as_bytes()),
            "test_origin":test_origin(&replay.revision)?,"test_plan_digest":wire::digest(b"semaprax.candidate-test.plan.v1\0",plan.as_bytes()),
            "statically_selected":plan_value["selected"],"execution_scope":"complete_manifest_declared_test_closure",
            "options":options,"options_digest":wire::digest(b"semaprax.candidate-test.options.v1\0",options_bytes.as_bytes()),
            "execution":null,"passed":false,
            "nonclaims":["reference_interpreter_only","no_native_or_wasm_execution","no_full_quality_gate_success","no_dynamic_coverage","no_source_publication_authority","no_trace_produced"]
        });
        // Fail oversized provenance before spending interpreter fuel. The final
        // envelope can still exceed the host report limit: such failure yields
        // no report, never a truncated success receipt.
        let _ = render(report.clone(), policy.max_report_bytes)?;
        let execution = replay.revision.execute_test(
            &ProjectExecutionOptions::new(policy.max_execution_bytes, policy.max_steps)
                .map_err(|error| vec![error])?,
        )?;
        if execution.stable_id() != replay.revision.test_program().entrypoint.as_str()
            || execution.module() != replay.revision.manifest().test_module()
        {
            return Err(invalid(
                "candidate test execution origin differs from its admitted test root",
            ));
        }
        let envelope: Value = serde_json::from_str(execution.envelope())
            .map_err(|_| invalid("compiler execution envelope is invalid"))?;
        let outcome = render(envelope["outcome"].clone(), policy.max_report_bytes)?;
        report["execution"] = json!({"envelope":execution.envelope(),
            "envelope_digest":wire::digest(b"semaprax.candidate-test.execution-envelope.v1\0",execution.envelope().as_bytes()),
            "outcome_digest":wire::digest(b"semaprax.candidate-test.outcome.v1\0",outcome.as_bytes()),
            "steps_used":execution.steps_used(),"max_steps":execution.max_steps()});
        report["passed"] = json!(execution.command_succeeded());
        let json = render(report, policy.max_report_bytes)?;
        let digest = wire::digest(REPORT_DOMAIN, json.as_bytes());
        Ok(CandidateTestReport {
            json,
            digest,
            execution,
        })
    }
}

struct Reachability {
    ids: BTreeSet<String>,
    opaque: bool,
}

fn reachability(revision: &ProjectRevision) -> Result<Reachability> {
    let program = revision.test_program();
    if program.functions.len() > MAX_CALLS {
        return Err(capacity("test callable inventory exceeds its bound"));
    }
    let mut graph = BTreeMap::new();
    let mut calls = 0usize;
    for function in &program.functions {
        let mut outgoing = BTreeSet::new();
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(function.ensures.iter())
        {
            crate::hir::visit_resolved_calls(expression, &mut |callee, _, _| {
                calls = calls.saturating_add(1);
                if calls <= MAX_CALLS {
                    outgoing.insert(callee.as_str().to_owned());
                }
            });
        }
        if calls > MAX_CALLS {
            return Err(capacity("test call inventory exceeds its bound"));
        }
        if graph.insert(function.id.as_str(), outgoing).is_some() {
            return Err(invalid("test callable inventory has duplicate identities"));
        }
    }
    let root = program.entrypoint.as_str();
    if !graph.contains_key(root) {
        return Err(invalid("declared test entry is absent from admitted HIR"));
    }
    let mut ids = BTreeSet::new();
    let mut pending = vec![root.to_owned()];
    let mut opaque = false;
    while let Some(id) = pending.pop() {
        if !ids.insert(id.clone()) {
            continue;
        }
        if ids.len() > MAX_CALLS {
            return Err(capacity("test dependency closure exceeds its bound"));
        }
        if let Some(outgoing) = graph.get(id.as_str()) {
            pending.extend(outgoing.iter().filter(|id| !ids.contains(*id)).cloned());
        } else {
            opaque = true;
        }
    }
    Ok(Reachability { ids, opaque })
}

fn test_origin(revision: &ProjectRevision) -> Result<Value> {
    let id = revision.test_program().entrypoint.as_str();
    let module = revision
        .semantic
        .image_modules()
        .iter()
        .find(|module| {
            module.module() == revision.manifest().test_module()
                && module
                    .functions()
                    .iter()
                    .any(|function| function.id.as_str() == id)
        })
        .ok_or_else(|| invalid("test root lacks an authenticated source origin"))?;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == module.path())
        .ok_or_else(|| invalid("test root source origin is absent"))?;
    Ok(
        json!({"module":module.module(),"stable_id":id,"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()}),
    )
}

fn source_inventory(revision: &ProjectRevision) -> Vec<Value> {
    revision.sources().iter().map(|source| json!({"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})).collect()
}

fn diff_inventory(candidate: &ProjectCandidate) -> Result<Vec<Value>> {
    let mut diffs = Vec::new();
    for (before, after) in candidate
        .base
        .sources()
        .iter()
        .zip(candidate.revision.sources())
    {
        if before.source() != after.source() {
            let diff = wire::source_diff(before.path(), before.source(), after.source())?;
            diffs.push(json!({"path":before.path(),"base_source_digest":before.source_digest(),"candidate_source_digest":after.source_digest(),
                "source_diff_digest":wire::digest(b"semaprax.candidate.source-diff.v1\0",diff.as_bytes()),"source_diff_bytes":diff.len()}));
        }
    }
    Ok(diffs)
}

fn required_id<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| invalid("retained test intention lacks a required identity"))
}
fn render(value: Value, bytes: usize) -> Result<String> {
    wire::render(value, bytes)
        .map_err(|_| capacity("candidate test report exceeds its output bound"))
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G239", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G240", message)]
}
