//! One selected-law diagnostic operation shared by the CLI and agent surfaces.
//! Only a host-held installed tool can produce proof evidence or diagnostic
//! models; the output is a bounded, read-only projection of exact strict replay.
use super::{native_proof, strict, workflow, LawSet, Result};
use crate::assurance_manifest::{smt_discharge as smt, VerifiedProjectProof};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use crate::proof_export::{
    installed::{InstalledProofTool, SmtDiagnosticResult, ToolKind},
    installed_project::prove_postcondition,
};
use serde_json::{json, Value};

pub const SCHEMA: &str = "semaprax.project-law-workflow-cli.v2";

#[derive(Clone, Copy)]
pub struct SourceGoal<'a> {
    pub path: &'a str,
    pub declaration: &'a str,
    pub ensures_index: usize,
}

#[derive(Clone, Copy)]
pub enum View {
    Summary { offset: usize, limit: usize },
    Detail,
}

pub struct Request<'a> {
    pub law_id: &'a str,
    pub source_goal: Option<SourceGoal<'a>>,
    pub view: View,
    pub max_bytes: usize,
    pub show_witness_values: bool,
    pub expected_candidate_revision: Option<&'a str>,
}

pub struct Outcome {
    pub document: String,
    pub accepted: bool,
}

/// Full strict rederivation is independent of the selected page. A stale
/// candidate refuses before any proof invocation; a law failure returns a
/// typed nonproof result with the whole-inventory verdict intact.
pub fn check(
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &strict::StrictLawPolicy,
    tool: &InstalledProofTool,
    request: &Request<'_>,
) -> Result<Outcome> {
    if !(2048..=65_536).contains(&request.max_bytes)
        || request.show_witness_values && !matches!(request.view, View::Detail)
    {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "invalid selected law workflow view or byte budget",
        )]);
    }
    let stale = request
        .expected_candidate_revision
        .is_some_and(|expected| expected != revision.project_revision());
    let law_id = request.law_id;
    if laws.semantic_digest(law_id).is_none() {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "selected law is absent from authenticated inventory",
        )]);
    }
    let mut proofs: Vec<VerifiedProjectProof> = Vec::new();
    let mut native_proofs = Vec::new();
    let work_before = tool.work_snapshot();
    let attempt = if stale {
        json!({"outcome":"stale","expected_candidate_revision":request.expected_candidate_revision,
            "current_candidate_revision":revision.project_revision(),"diagnostics":[]})
    } else if let Some(source) = request.source_goal {
        match prove_postcondition(
            revision,
            source.path,
            source.declaration,
            source.ensures_index,
            tool,
        ) {
            Ok(proof) => {
                proofs.push(proof);
                json!({"outcome":"proved","diagnostics":[]})
            }
            Err(errors) => {
                source_failure(revision, source, tool, errors, request.show_witness_values)
            }
        }
    } else {
        match native_proof::prove_scalar_law(revision, laws, law_id, tool) {
            Ok(proof) => {
                native_proofs.push(proof);
                json!({"outcome":"proved","diagnostics":[]})
            }
            Err(errors) => json!({"outcome":"incomplete","diagnostics":diagnostic_rows(&errors)}),
        }
    };
    let work = tool.work_snapshot().since(work_before);
    let report =
        strict::derive_with_native_proofs(revision, laws, policy, &proofs, &native_proofs)?;
    let view = match request.view {
        View::Detail => workflow::strict_detail(
            &report,
            revision,
            laws,
            policy,
            &proofs,
            &native_proofs,
            law_id,
            request.max_bytes,
        )?,
        View::Summary { offset, limit } => workflow::strict_summary(
            &report,
            revision,
            laws,
            policy,
            &proofs,
            &native_proofs,
            offset,
            limit,
            request.max_bytes,
        )?,
    };
    let view: Value = serde_json::from_str(&view).expect("checked workflow view is JSON");
    let accepted = view["accepted"] == true;
    let complete: Value = serde_json::from_str(&report).expect("checked strict report is JSON");
    let validity = json!({
        "schema":"semaprax.selected-law-validity.v1",
        "accepted":accepted,"proof_attempt":attempt["outcome"],
        "counts":complete["counts"],"candidate_revision":revision.project_revision(),
        "source":"replayed_strict_report","delivery_independent":true,
    });
    let failed_obligation_ids = complete["laws"]
        .as_array()
        .expect("checked strict rows")
        .iter()
        .filter(|row| row["satisfied"] == false)
        .map(|row| row["obligation_id"].clone())
        .collect::<Vec<_>>();
    let inventory: Value =
        serde_json::from_str(laws.to_json()).expect("checked law inventory is JSON");
    let definition = inventory["payload"]["laws"]
        .as_array()
        .expect("checked inventory rows")
        .iter()
        .find(|row| row["definition"]["law_id"] == law_id)
        .expect("selected law remains in inventory");
    let envelope = json!({
        "schema":SCHEMA,"candidate_revision":revision.project_revision(),
        "law_digest":laws.digest(),"policy_digest":policy.digest(),
        "selected_law_id":law_id,"semantic_digest":laws.semantic_digest(law_id),
        "semantic_subject":definition["definition"]["selector"],
        "dependencies":{"requires_laws":definition["definition"]["requires_laws"],
            "assumption_ids":definition["definition"]["assumption_ids"]},
        "failed_obligation_ids":failed_obligation_ids,
        "source_location":attempt["source_location"],
        "proof_attempt":attempt,"view":view,
        "validity":validity,
        "work":{"schema":"semaprax.installed-law-work.v1",
            "reserved_process_invocations":work.reserved_process_invocations,
            "reserved_solver_queries":work.reserved_solver_queries,
            "reserved_io_bytes":work.reserved_io_bytes,
            "model_tokens":null,"provider_cost_micros":null,
            "cost_status":"unavailable","kind":"held_process_reservation"},
        "toolchain":tool.expected_version(),"evidence_profile":kind_label(tool.kind()),
        "repair_actions":[{"kind":"repair_implementation","target":"selected_subject"},
            {"kind":"supply_checked_proof","target":"selected_obligation"},
            {"kind":"propose_specification_change","route":"protected_law_review","does_not_count_as_repair":true}],
        "source_authority":false,"publication_authority":false,
        "nonclaims":["diagnostic_view_is_not_proof_or_publication_authority","tool_execution_is_trusted_local"]
    });
    let document = serde_json::to_string(&envelope).expect("bounded JSON envelope");
    if document.len() > request.max_bytes {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "workflow envelope exceeds selected byte budget",
        )]);
    }
    Ok(Outcome { document, accepted })
}

fn source_failure(
    revision: &ProjectRevision,
    source_goal: SourceGoal<'_>,
    tool: &InstalledProofTool,
    errors: Vec<Diagnostic>,
    show_values: bool,
) -> Value {
    let mut result = json!({"outcome":"incomplete","diagnostics":diagnostic_rows(&errors)});
    if tool.kind() != ToolKind::Z3 {
        return result;
    }
    let Some(source) = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_goal.path)
    else {
        return result;
    };
    let Ok(program) = crate::parse(source.source(), source_goal.path) else {
        return result;
    };
    let Some(function) = program
        .functions
        .iter()
        .find(|function| function.stable_id == source_goal.declaration)
    else {
        return result;
    };
    let Ok(encoding) = smt::translate_function(function) else {
        result["outcome"] = json!("unsupported");
        return result;
    };
    if source_goal.ensures_index >= encoding.ensures.len() {
        return result;
    }
    let index = source_goal.ensures_index;
    let script = smt::render_postcondition_script(&encoding, index, tool.proof_timeout_ms());
    result["obligation_id"] = json!(smt::postcondition_obligation_id(
        source_goal.declaration,
        index
    ));
    result["source_path"] = json!(source_goal.path);
    result["source_digest"] = json!(source.source_digest());
    result["source_location"] = json!({"path":source_goal.path,"line":function.ensures[index].span.line,
        "column":function.ensures[index].span.column});
    match tool.smt_diagnostic_query(&script) {
        Ok(SmtDiagnosticResult::Sat(model)) => match smt::replay_function(function, &model) {
            Ok(smt::ReplayOutcome::EnsuresViolated { ensures_index }) if ensures_index == index => {
                result["outcome"] = json!("disproved_concrete");
                result["counterexample"] = witness(&model, show_values, "ensures_violated");
            }
            Ok(smt::ReplayOutcome::Trapped { detail }) => {
                result["outcome"] = json!("disproved_concrete");
                result["counterexample"] = witness(&model, show_values, "checked_trap");
                result["trace"] = json!({"kind":"checked_trap","detail":detail});
            }
            _ => result["outcome"] = json!("solver_error"),
        },
        Ok(SmtDiagnosticResult::Unknown) => result["outcome"] = json!("unknown"),
        Ok(SmtDiagnosticResult::TimedOut) => result["outcome"] = json!("timeout"),
        Ok(SmtDiagnosticResult::Unsat) => result["outcome"] = json!("incomplete"),
        Err(error) => {
            result["outcome"] = json!("solver_error");
            result["diagnostic_query"] = json!({"code":error.code,"message":error.message});
        }
    }
    result
}

fn witness(model: &smt::Model, show_values: bool, failure: &str) -> Value {
    let values = model
        .iter()
        .map(|(name, value)| {
            let value = match value {
                smt::ModelValue::Int(n) => json!({"type":"int","value":n.to_string()}),
                smt::ModelValue::Bool(b) => json!({"type":"bool","value":b}),
            };
            (name.clone(), value)
        })
        .collect::<serde_json::Map<_, _>>();
    json!({"replay":"checked_source","failure":failure,"validated":true,
        "redacted":!show_values,"values":if show_values { Value::Object(values) } else { Value::Null }})
}

fn diagnostic_rows(errors: &[Diagnostic]) -> Vec<Value> {
    errors
        .iter()
        .map(|error| json!({"code":error.code,"message":error.message}))
        .collect()
}

fn kind_label(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Z3 => "installed_z3",
        ToolKind::Lean => "installed_lean",
    }
}
