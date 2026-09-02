use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::project::with_authenticated_project;
use crate::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceContextOptions, WorkspaceImpactOptions,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture() -> Fixture {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-semantic-unit-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(source.join(path), root.join(path)).unwrap();
    }
    Fixture(root.canonicalize().unwrap())
}

#[test]
fn retained_project_graph_and_context_are_project_bound_and_deterministic() {
    let fixture = fixture();
    let manifest = fixture.0.join("semaprax.toml");
    let first = with_authenticated_project(&manifest, |snapshot| {
        let graph = snapshot.semantic_graph().to_owned();
        let context = snapshot.semantic_context(
            WorkspaceAnalysisTargetKind::Declaration,
            "calculator.add",
            WorkspaceContextOptions::new(WorkspaceAnalysisDirection::Both, 4, 1024 * 1024, 1024)
                .unwrap(),
        )?;
        Ok((graph, context))
    })
    .unwrap();
    let second = with_authenticated_project(&manifest, |snapshot| {
        Ok((
            snapshot.semantic_graph().to_owned(),
            snapshot.semantic_context(
                WorkspaceAnalysisTargetKind::Declaration,
                "calculator.add",
                WorkspaceContextOptions::default(),
            )?,
        ))
    })
    .unwrap();
    assert_eq!(first, second);
    let graph: serde_json::Value = serde_json::from_str(&first.0).unwrap();
    assert_eq!(graph["schema"], PROJECT_SEMANTIC_GRAPH_SCHEMA);
    assert_eq!(graph["project_schema"], "semaprax.project.v1");
    assert!(graph.get("workspace_manifest_schema").is_none());
    assert_eq!(graph["modules"].as_array().unwrap().len(), 3);
    let context: serde_json::Value = serde_json::from_str(&first.1).unwrap();
    assert_eq!(context["schema"], PROJECT_SEMANTIC_CONTEXT_SCHEMA);
    assert_eq!(context["project_revision"], graph["project_revision"]);
    assert_eq!(context["project_graph_digest"], graph["graph_digest"]);

    let truncated = with_authenticated_project(&manifest, |snapshot| {
        snapshot.semantic_context(
            WorkspaceAnalysisTargetKind::Declaration,
            "calculator.tests.main",
            WorkspaceContextOptions::new(WorkspaceAnalysisDirection::Both, 4, 1024 * 1024, 1)
                .unwrap(),
        )
    })
    .unwrap();
    let truncated: serde_json::Value = serde_json::from_str(&truncated).unwrap();
    assert_eq!(truncated["truncation"]["truncated"], true);
    assert!(!truncated["frontier"].as_array().unwrap().is_empty());
}

#[test]
fn request_boundary_absorbs_final_recheck_invalidation_before_later_action() {
    let fixture = fixture();
    let manifest = fixture.0.join("semaprax.toml");
    let mut snapshot = crate::project::load_snapshot(&manifest).unwrap();
    let app = fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&app).unwrap();
    let first = snapshot.with_authenticated_request(|snapshot| {
        let rendered = snapshot.semantic_graph().to_owned();
        std::fs::write(&app, format!("{source}\n")).unwrap();
        Ok(rendered)
    });
    assert!(first.is_err());

    let acted = std::cell::Cell::new(false);
    let second = snapshot.with_authenticated_request(|_| {
        acted.set(true);
        Ok(())
    });
    assert!(second.is_err());
    assert!(!acted.get());
}

#[test]
fn retained_project_impact_is_project_bound_reverse_only_and_deterministic() {
    let fixture = fixture();
    let manifest_path = fixture.0.join("semaprax.toml");
    let render = || {
        let snapshot = crate::project::load_snapshot(&manifest_path).unwrap();
        snapshot
            .semantic
            .impact(
                snapshot.manifest.name(),
                snapshot.project_revision(),
                snapshot.manifest.test_module(),
                WorkspaceAnalysisTargetKind::Declaration,
                "calculator.add",
                WorkspaceImpactOptions::default(),
            )
            .unwrap()
    };
    let first = render();
    assert_eq!(first, render());
    assert!(!first.ends_with('\n'));

    let report: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(report["schema"], PROJECT_SEMANTIC_IMPACT_SCHEMA);
    assert_eq!(report["project_schema"], "semaprax.project.v1");
    assert_eq!(report["query"]["direction"], "reverse");
    assert_eq!(report["project_graph_digest"], {
        let snapshot = crate::project::load_snapshot(&manifest_path).unwrap();
        snapshot.semantic.graph_digest().to_owned()
    });
    assert!(report.get("workspace_manifest_schema").is_none());
    assert!(report["affected"].as_array().unwrap().len() > 1);
    assert!(!report["dependency_edges"].as_array().unwrap().is_empty());
    assert_eq!(
        report["budget"]["used_output_bytes"].as_u64().unwrap() as usize,
        first.len()
    );

    let snapshot = crate::project::load_snapshot(&manifest_path).unwrap();
    let truncated = snapshot
        .semantic
        .impact(
            snapshot.manifest.name(),
            snapshot.project_revision(),
            snapshot.manifest.test_module(),
            WorkspaceAnalysisTargetKind::Declaration,
            "calculator.add",
            WorkspaceImpactOptions::new(16, 4096, 1).unwrap(),
        )
        .unwrap();
    let truncated: serde_json::Value = serde_json::from_str(&truncated).unwrap();
    assert_eq!(truncated["truncation"]["truncated"], true);
    assert!(!truncated["frontier"].as_array().unwrap().is_empty());
}
