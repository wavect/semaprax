//! Retained, read-only semantic views over one authenticated Project v1 load.
//!
//! The graph and typed analysis index originate in the same Phase-A build as
//! the linked entry/test closures. Their wire schemas deliberately describe
//! declared Project inputs, not a managed Semantic Workspace generation.

use std::collections::BTreeMap;

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{
    ProjectAnalysisSubject, WorkspaceAnalysis, WorkspaceAnalysisTargetKind,
    WorkspaceContextOptions, WorkspaceImpactOptions,
};
use crate::workspace_graph::{self, WorkspaceGraphProjection};

pub const PROJECT_SEMANTIC_GRAPH_SCHEMA: &str = "semaprax.project-semantic-graph.v1";
pub const PROJECT_SEMANTIC_CONTEXT_SCHEMA: &str = "semaprax.project-semantic-context.v1";
pub const PROJECT_SEMANTIC_IMPACT_SCHEMA: &str = crate::workspace_analysis::PROJECT_IMPACT_SCHEMA;

pub(super) struct ProjectSemanticState {
    project_schema: &'static str,
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
        project_schema: &'static str,
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
            project_schema,
            project_name,
            project_revision,
            test_module,
        )?;
        let graph_json = graph.json().to_owned();
        let graph_digest = graph.digest().to_owned();
        let analysis = WorkspaceAnalysis::build_project(projection, &graph_digest)?;
        Ok(Self {
            project_schema,
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

    pub(super) fn image_indexes(&self) -> serde_json::Value {
        self.analysis.image_indexes()
    }

    pub(super) fn image_modules(&self) -> &[workspace_graph::WorkspaceGraphProjectionModule] {
        self.analysis.modules()
    }

    pub(super) fn image_edges(&self) -> &[workspace_graph::WorkspaceEdge] {
        self.analysis.edges()
    }

    pub(super) fn image_symbol(&self, id: &str) -> Option<serde_json::Value> {
        let mut symbol = self.analysis.image_symbol(id)?;
        if let Some(function) = self.rename_functions.get(id) {
            symbol["name"] = serde_json::Value::String(function.name.clone());
        }
        Some(symbol)
    }

    pub(super) fn rename_function(&self, stable_id: &str) -> Option<&ProjectRenameFunction> {
        self.rename_functions.get(stable_id)
    }

    pub(super) fn display_rename_equivalent(&self, candidate: &Self) -> bool {
        self.analysis
            .project_display_rename_equivalent(&candidate.analysis)
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
                project_schema: self.project_schema,
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

    pub(super) fn impact(
        &self,
        project_name: &str,
        project_revision: &str,
        test_module: &str,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceImpactOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.analysis.render_project_impact(
            ProjectAnalysisSubject {
                project_schema: self.project_schema,
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
#[path = "semantic/tests.rs"]
mod tests;
