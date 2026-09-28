//! Opt-in interpreter model wait over the bundled offline repair lifecycle.
use super::*;
use semaprax::execution_revision::typed::AgentRuntimeV2DurableModelWaitEvidence;

pub(super) const WRAPPER_ID: &str = "fixture.agent.fn.await_proposal";
pub(super) const EVALUATION_FUEL: usize = 1000;
const MANIFEST: &str = "../../examples/offline-repair-model-wait-project/semaprax.toml";
const SCHEMA: &str = "semaprax.private-offline-repair-model-wait-demo.v1";

pub(super) fn manifest() -> Result<PathBuf, CliError> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(MANIFEST)
        .canonicalize()
        .map_err(|_| CliError::refused("offline repair model wait Project is unavailable"))
}

pub(in crate::source_live_cli) fn run(arguments: &[String]) -> Result<String, CliError> {
    if !arguments.is_empty() {
        return Err(CliError::usage(
            "offline-repair-model-wait takes no operands",
        ));
    }
    let manifest = manifest()?;
    let source_path = manifest
        .parent()
        .expect("manifest parent")
        .join("src/app.spx");
    let source_before = std::fs::read(&source_path)
        .map_err(|_| CliError::refused("offline repair model wait source is unavailable"))?;
    let project = with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
        .map_err(|diagnostics| {
            diagnostic_error(
                "offline repair model wait authentication refused",
                diagnostics,
            )
        })?;
    execute_profile(project, source_before, true)
}

pub(super) fn add_report(
    report: &mut Value,
    waited: &AgentRuntimeV2DurableModelWaitEvidence,
) -> Result<(), CliError> {
    let canonical = std::str::from_utf8(waited.wait_evidence())
        .map_err(|_| CliError::refused("offline repair wait evidence is not UTF-8"))?;
    let evidence = checked_value(canonical, "offline repair wait evidence rendering refused")?;
    report["schema"] = json!(SCHEMA);
    report["model_wait_wrapper"] = json!(WRAPPER_ID);
    report["wait_evaluation_fuel"] = json!(EVALUATION_FUEL);
    report["wait_evidence"] = evidence;
    report["wait_evidence_canonical"] = json!(canonical);
    report["wait_evidence_root"] = json!(waited.evidence_root().digest());
    report["execution_engine"] = json!("standalone-source-interpreter");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_model_wait_profile_has_canonical_checked_graph() {
        let source =
            include_str!("../../../../../examples/offline-repair-model-wait-project/src/app.spx");
        let checked = semaprax::check(source, "offline-repair-model-wait.spx").unwrap();
        let canonical = semaprax::format::canonical(&checked);
        let reparsed = semaprax::parse(&canonical, "offline-repair-model-wait.spx").unwrap();
        assert_eq!(canonical, semaprax::format::canonical(&reparsed));
        let rechecked = semaprax::check(&canonical, "offline-repair-model-wait.spx").unwrap();
        assert_eq!(
            semaprax::graph::to_json(&checked).unwrap(),
            semaprax::graph::to_json(&rechecked).unwrap()
        );
        let graph: Value =
            serde_json::from_str(&semaprax::graph::to_json(&rechecked).unwrap()).unwrap();
        let wrapper = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == WRAPPER_ID)
            .unwrap();
        let ty = |id| {
            semaprax::hir::ResolvedType::Nominal {
                declaration: semaprax::hir::DeclarationId::new(id),
                arguments: vec![],
            }
            .identity_key()
        };
        assert_eq!(wrapper["persistent"], true);
        assert_eq!(
            wrapper["params"][0]["type_id"],
            ty("fixture.agent.type.observation")
        );
        assert_eq!(wrapper["params"][0]["ownership_mode"], "value");
        assert_eq!(wrapper["return_type_id"], ty("fixture.agent.type.proposal"));
        assert_eq!(wrapper["body"]["statements"].as_array().unwrap().len(), 0);
        assert_eq!(wrapper["body"]["tail"]["kind"], "yield");
        assert_eq!(
            wrapper["body"]["tail"]["request_type_id"],
            ty("fixture.agent.type.observation")
        );
        assert_eq!(
            wrapper["body"]["tail"]["type_id"],
            ty("fixture.agent.type.proposal")
        );
    }

    #[test]
    fn explicit_demo_executes_real_model_wait_and_feedback_guarded_repair() {
        let rendered = crate::source_live_cli::run(&["offline-repair-model-wait".into()]).unwrap();
        let report: Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(report["schema"], SCHEMA);
        assert_eq!(report["model_wait_wrapper"], WRAPPER_ID);
        assert_eq!(report["execution_engine"], "standalone-source-interpreter");
        assert_eq!(report["model_dispatches"], 2);
        assert_eq!(report["effect_dispatches"], 2);
        assert_eq!(report["provider_starts"], 2);
        assert_eq!(report["rejected_candidates"], 1);
        assert_eq!(report["feedback_guarded"], true);
        assert_eq!(report["source_mutation"], false);
        assert_eq!(report["publication_authority"], false);
        assert_eq!(
            report["journal"]["schema"],
            "semaprax.live-invocation.source-persisted-journal.v7"
        );
        let rows = report["journal"]["entries"].as_array().unwrap();
        assert_eq!(
            rows.iter().filter(|e| e["kind"] == "wait_prepared").count(),
            2
        );
        assert_eq!(
            rows.iter()
                .filter(|e| e["kind"] == "wait_completed")
                .count(),
            2
        );
        let reserved: u64 = rows
            .iter()
            .filter(|e| e["kind"] == "wait_evaluation_reserved")
            .map(|e| e["fuel"].as_u64().unwrap())
            .sum();
        assert_eq!(reserved, 4000);
        assert_eq!(report["wait_evidence"]["total_wait_fuel"], reserved);
        let exact: Value =
            serde_json::from_str(report["wait_evidence_canonical"].as_str().unwrap()).unwrap();
        assert_eq!(exact, report["wait_evidence"]);
        assert!(report["wait_evidence_root"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(!report["wait_evidence_canonical"]
            .as_str()
            .unwrap()
            .ends_with('\n'));
        assert!(crate::source_live_cli::run(&[
            "offline-repair-model-wait".into(),
            "unexpected".into()
        ])
        .is_err());
    }
}
