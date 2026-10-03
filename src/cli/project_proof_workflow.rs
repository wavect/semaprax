//! Additive, read-only strict-law failure workflow for one authenticated Project.
//! The original project-proof-check success route retains its wire contract.
use semaprax::{
    agent_runtime::AgentCancellation,
    assurance_manifest::{
        law_set::{native_proof, strict, workflow},
        smt_discharge as smt, VerifiedProjectProof,
    },
    diagnostic::Diagnostic,
    project::{with_selected_law_diagnostics, ProjectRevision},
    proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits, SmtDiagnosticResult, ToolKind},
        installed_project::prove_postcondition,
    },
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::Path};

const SCHEMA: &str = "semaprax.project-law-workflow-cli.v1";

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    let Some(manifest) = args.first() else {
        return usage("absolute manifest required");
    };
    if !Path::new(manifest).is_absolute() {
        return usage("absolute manifest required");
    }
    let mut options = BTreeMap::new();
    let mut show_values = false;
    let mut cursor = 1;
    while cursor < args.len() {
        let key = args[cursor].as_str();
        if key == "--show-witness-values" {
            if show_values {
                return usage("duplicate option");
            }
            show_values = true;
            cursor += 1;
            continue;
        }
        if !matches!(
            key,
            "--workflow"
                | "--law"
                | "--tool"
                | "--executable"
                | "--version-line"
                | "--host-profile"
                | "--source"
                | "--declaration"
                | "--ensures"
                | "--offset"
                | "--limit"
                | "--max-bytes"
        ) {
            return usage("unknown workflow option");
        }
        let Some(value) = args.get(cursor + 1) else {
            return usage("option value missing");
        };
        if options.insert(key, value.as_str()).is_some() {
            return usage("duplicate option");
        }
        cursor += 2;
    }
    if [
        "--workflow",
        "--law",
        "--tool",
        "--executable",
        "--version-line",
        "--host-profile",
    ]
    .iter()
    .any(|key| !options.contains_key(key))
    {
        return usage("missing workflow selection or proof tool");
    }
    let detail = match options["--workflow"] {
        "summary" => false,
        "detail" => true,
        _ => return usage("workflow must be summary or detail"),
    };
    if show_values && !detail {
        return usage("witness values require detail view");
    }
    let selected_source = ["--source", "--declaration", "--ensures"]
        .iter()
        .filter(|key| options.contains_key(**key))
        .count();
    if selected_source != 0 && selected_source != 3 {
        return usage("source proof requires --source, --declaration and --ensures");
    }
    let index = if selected_source == 3 {
        Some(options["--ensures"].parse::<usize>().map_err(|_| {
            eprintln!("project-proof-check: invalid postcondition index");
            2
        })?)
    } else {
        None
    };
    let kind = match options["--tool"] {
        "z3" => ToolKind::Z3,
        "lean" => ToolKind::Lean,
        _ => return usage("tool must be z3 or lean"),
    };
    let profile = match options["--host-profile"] {
        "trusted-local" => HostProfile::TrustedLocal,
        "confined" => HostProfile::Confined,
        _ => return usage("unknown host profile"),
    };
    let offset = number(&options, "--offset", 0)?;
    let limit = number(&options, "--limit", 16)?;
    let max_bytes = number(&options, "--max-bytes", 65_536)?;
    if !(2048..=65_536).contains(&max_bytes) {
        return usage("workflow byte budget must be 2048..65536");
    }
    let path = Path::new(manifest);
    let law_id = options["--law"];
    let output = with_selected_law_diagnostics(path, |revision, laws, policy| {
        if laws.semantic_digest(law_id).is_none() {
            return Err(vec![Diagnostic::io("SPX-LW130", "selected law is absent from authenticated inventory")]);
        }
        // The selected host policy and protected specification are authenticated
        // before even acquiring the installed process capability.
        let tool = InstalledProofTool::open(
            Path::new(options["--executable"]),
            path.parent().expect("absolute path has parent"),
            kind, options["--version-line"], profile, Limits::default(), AgentCancellation::new(),
        ).map_err(|error| vec![error])?;
        let mut proofs: Vec<VerifiedProjectProof> = Vec::new();
        let mut native_proofs = Vec::new();
        let attempt = if let Some(index) = index {
            let source = options["--source"];
            let declaration = options["--declaration"];
            match prove_postcondition(revision, source, declaration, index, &tool) {
                Ok(proof) => { proofs.push(proof); json!({"outcome":"proved","diagnostics":[]}) }
                Err(errors) => source_failure(revision, source, declaration, index, &tool, errors, show_values),
            }
        } else {
            match native_proof::prove_scalar_law(revision, laws, law_id, &tool) {
                Ok(proof) => { native_proofs.push(proof); json!({"outcome":"proved","diagnostics":[]}) }
                Err(errors) => json!({"outcome":"incomplete","diagnostics":diagnostic_rows(&errors)}),
            }
        };
        let report = strict::derive_with_native_proofs(revision, laws, policy, &proofs, &native_proofs)?;
        let view = if detail {
            workflow::strict_detail(&report, revision, laws, policy, &proofs, &native_proofs, law_id, max_bytes)?
        } else {
            workflow::strict_summary(&report, revision, laws, policy, &proofs, &native_proofs, offset, limit, max_bytes)?
        };
        let view: Value = serde_json::from_str(&view).expect("checked workflow view is JSON");
        let accepted = view["accepted"] == true;
        let complete: Value = serde_json::from_str(&report).expect("checked strict report is JSON");
        let failed_obligation_ids = complete["laws"].as_array().expect("checked strict rows")
            .iter().filter(|row| row["satisfied"] == false)
            .map(|row| row["obligation_id"].clone()).collect::<Vec<_>>();
        let inventory: Value = serde_json::from_str(laws.to_json()).expect("checked law inventory is JSON");
        let definition = inventory["payload"]["laws"].as_array().expect("checked inventory rows")
            .iter().find(|row| row["definition"]["law_id"] == law_id)
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
            "toolchain":tool.expected_version(),"evidence_profile":kind_label(kind),
            "repair_actions":[{"kind":"repair_implementation","target":"selected_subject"},
                {"kind":"supply_checked_proof","target":"selected_obligation"},
                {"kind":"propose_specification_change","route":"protected_law_review","does_not_count_as_repair":true}],
            "source_authority":false,"publication_authority":false,
            "nonclaims":["diagnostic_view_is_not_proof_or_publication_authority","tool_execution_is_trusted_local"]
        });
        let wire = serde_json::to_string(&envelope).expect("bounded JSON envelope");
        if wire.len() > max_bytes {
            return Err(vec![Diagnostic::io("SPX-LW130", "workflow envelope exceeds selected byte budget")]);
        }
        Ok((wire, accepted))
    }).map_err(|errors| { for error in errors { eprintln!("{}: {}", error.code, error.message); } 1 })?;
    println!("{}", output.0);
    if output.1 {
        Ok(())
    } else {
        Err(1)
    }
}

fn source_failure(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
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
        .find(|source| source.path() == source_path)
    else {
        return result;
    };
    let Ok(program) = semaprax::parse(source.source(), source_path) else {
        return result;
    };
    let Some(function) = program
        .functions
        .iter()
        .find(|function| function.stable_id == declaration)
    else {
        return result;
    };
    let Ok(encoding) = smt::translate_function(function) else {
        result["outcome"] = json!("unsupported");
        return result;
    };
    if index >= encoding.ensures.len() {
        return result;
    }
    let script = smt::render_postcondition_script(&encoding, index, tool.proof_timeout_ms());
    result["obligation_id"] = json!(smt::postcondition_obligation_id(declaration, index));
    result["source_path"] = json!(source_path);
    result["source_digest"] = json!(source.source_digest());
    result["source_location"] = json!({"path":source_path,"line":function.ensures[index].span.line,
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

fn number(options: &BTreeMap<&str, &str>, key: &str, default: usize) -> Result<usize, u8> {
    options.get(key).map_or(Ok(default), |raw| {
        raw.parse().map_err(|_| {
            eprintln!("project-proof-check: invalid {key}");
            2
        })
    })
}

fn kind_label(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Z3 => "installed_z3",
        ToolKind::Lean => "installed_lean",
    }
}

fn usage(message: &str) -> Result<(), u8> {
    eprintln!("project-proof-check: {message}");
    Err(2)
}
