//! Retained, read-only semantic views over one authenticated Project v1 load.
//!
//! The graph and typed analysis index originate in the same Phase-A build as
//! the linked entry/test closures. Their wire schemas deliberately describe
//! declared Project inputs, not a managed Semantic Workspace generation.

use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{
    ProjectAnalysisSubject, WorkspaceAnalysis, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
};
use crate::workspace_graph::{self, WorkspaceGraphProjection};

pub const PROJECT_SEMANTIC_GRAPH_SCHEMA: &str = "semaprax.project-semantic-graph.v1";
pub const PROJECT_SEMANTIC_CONTEXT_SCHEMA: &str = "semaprax.project-semantic-context.v1";

pub(super) struct ProjectSemanticState {
    graph_json: String,
    graph_digest: String,
    analysis: WorkspaceAnalysis,
    rename_functions: BTreeMap<String, ProjectRenameFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ProjectRenameFunction {
    pub(super) path: String,
    pub(super) module: String,
    pub(super) name: String,
    pub(super) origin: crate::hir::IdentityOrigin,
}

impl ProjectSemanticState {
    pub(super) fn new(
        projection: WorkspaceGraphProjection,
        project_name: &str,
        project_revision: &str,
        test_module: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        let function_origins = projection
            .declarations()
            .iter()
            .filter(|declaration| declaration.kind() == crate::hir::DeclarationKind::Function)
            .map(|declaration| (declaration.id().to_owned(), declaration.origin()))
            .collect::<BTreeMap<_, _>>();
        let mut rename_functions = BTreeMap::new();
        for module in projection.modules() {
            for function in module.functions() {
                if let Some(origin) = function_origins.get(function.id.as_str()) {
                    rename_functions.insert(
                        function.id.as_str().to_owned(),
                        ProjectRenameFunction {
                            path: module.path().to_owned(),
                            module: module.module().to_owned(),
                            name: function.name.clone(),
                            origin: *origin,
                        },
                    );
                }
            }
        }
        let graph = workspace_graph::render_project_semantic_graph(
            &projection,
            project_name,
            project_revision,
            test_module,
        )?;
        let graph_json = graph.json().to_owned();
        let graph_digest = graph.digest().to_owned();
        let analysis = WorkspaceAnalysis::build_project(projection, &graph_digest)?;
        Ok(Self {
            graph_json,
            graph_digest,
            analysis,
            rename_functions,
        })
    }

    pub(super) fn graph(&self) -> &str {
        &self.graph_json
    }

    pub(super) fn graph_digest(&self) -> &str {
        &self.graph_digest
    }

    pub(super) fn rename_function(&self, stable_id: &str) -> Option<&ProjectRenameFunction> {
        self.rename_functions.get(stable_id)
    }

    pub(super) fn context(
        &self,
        project_name: &str,
        project_revision: &str,
        test_module: &str,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceContextOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.analysis.render_project_context(
            ProjectAnalysisSubject {
                project_name,
                project_revision,
                test_module,
                graph_digest: &self.graph_digest,
            },
            target_kind,
            target,
            options,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::project::with_authenticated_project;
    use crate::workspace_analysis::{WorkspaceAnalysisDirection, WorkspaceContextOptions};

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
                WorkspaceContextOptions::new(
                    WorkspaceAnalysisDirection::Both,
                    4,
                    1024 * 1024,
                    1024,
                )
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
}
