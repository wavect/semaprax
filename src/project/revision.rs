//! Immutable, authority-neutral state for one completely linked Project revision.
//!
//! A revision owns canonical manifest/source facts, linked HIR, and the retained
//! semantic index. It has no root path, held filesystem object, invalidation
//! state, or publication method, so retaining it cannot extend live input or
//! mutation authority.

use crate::diagnostic::Diagnostic;

use super::{
    admission, build::BuiltProject, execution, npm, semantic, ProjectManifest, ProjectNpmBuild,
};
use super::{
    FlatOwnedRecordEvaluation, OwnedUtf8ApiEvaluation, PublicApiArgument, PublicApiEvaluation,
    PublicApiParameterType,
};
use super::{ProjectExecution, ProjectExecutionOptions, ProjectExecutionRole, ProjectProfile};
use super::{ProjectSource, ProjectWebBuild};

/// One immutable, fully admitted Project revision without ambient authority.
pub struct ProjectRevision {
    pub(super) manifest: ProjectManifest,
    pub(super) sources: Vec<ProjectSource>,
    pub(super) workspace_manifest: String,
    pub(super) workspace_revision: String,
    pub(super) project_revision: String,
    pub(super) entry_program: crate::hir::ResolvedProgram,
    pub(super) test_program: crate::hir::ResolvedProgram,
    pub(super) semantic: semantic::ProjectSemanticState,
    pub(super) profile_admission: admission::PreparedProjectAdmission,
}

impl ProjectRevision {
    pub(super) fn from_built(manifest: ProjectManifest, built: BuiltProject) -> Self {
        Self {
            manifest,
            sources: built.sources,
            workspace_manifest: built.workspace_manifest,
            workspace_revision: built.workspace_revision,
            project_revision: built.project_revision,
            entry_program: built.entry_program,
            test_program: built.test_program,
            semantic: built.semantic,
            profile_admission: built.profile_admission,
        }
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn sources(&self) -> &[ProjectSource] {
        &self.sources
    }

    pub fn workspace_manifest(&self) -> &str {
        &self.workspace_manifest
    }

    pub fn workspace_revision(&self) -> &str {
        &self.workspace_revision
    }

    pub fn project_revision(&self) -> &str {
        &self.project_revision
    }

    pub fn entry_program(&self) -> &crate::hir::ResolvedProgram {
        &self.entry_program
    }

    pub fn test_program(&self) -> &crate::hir::ResolvedProgram {
        &self.test_program
    }

    /// Report successful admission. Complete profile validation occurred before
    /// this revision became observable.
    pub fn check(&self) -> Result<(), Vec<Diagnostic>> {
        debug_assert_eq!(
            self.profile_admission.profile(),
            self.manifest.project_profile()
        );
        Ok(())
    }

    /// Return the complete declared-project graph retained with this revision.
    pub fn semantic_graph(&self) -> &str {
        self.semantic.graph()
    }

    /// Return the canonical digest bound into this revision's semantic graph.
    pub fn semantic_graph_digest(&self) -> &str {
        self.semantic.graph_digest()
    }

    /// Render bounded Project-specific Context from the retained typed index.
    pub fn semantic_context(
        &self,
        target_kind: crate::workspace_analysis::WorkspaceAnalysisTargetKind,
        target: &str,
        options: crate::workspace_analysis::WorkspaceContextOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.semantic.context(
            self.manifest.name(),
            &self.project_revision,
            self.manifest.test_module(),
            target_kind,
            target,
            options,
        )
    }

    /// Render bounded Project-specific reverse Impact from the retained typed index.
    pub fn semantic_impact(
        &self,
        target_kind: crate::workspace_analysis::WorkspaceAnalysisTargetKind,
        target: &str,
        options: crate::workspace_analysis::WorkspaceImpactOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.semantic.impact(
            self.manifest.name(),
            &self.project_revision,
            self.manifest.test_module(),
            target_kind,
            target,
            options,
        )
    }

    /// Evaluate the exact retained entry closure in memory.
    pub fn execute_entry(
        &self,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        execution::execute(self, ProjectExecutionRole::Entry, options)
    }

    /// Evaluate the exact retained test closure in memory.
    pub fn execute_test(
        &self,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        execution::execute(self, ProjectExecutionRole::Test, options)
    }

    /// Evaluate one exact retained closure selected by its closed role.
    pub fn execute(
        &self,
        role: ProjectExecutionRole,
        options: &ProjectExecutionOptions,
    ) -> Result<ProjectExecution, Vec<Diagnostic>> {
        execution::execute(self, role, options)
    }

    /// Build Project v1 as one deterministic pathless scalar-Web carrier.
    pub fn build_web_inline(&self, max_bytes: usize) -> Result<ProjectWebBuild, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::ScalarV1 {
            let version = match self.manifest.project_profile() {
                ProjectProfile::ScalarV1 => unreachable!("scalar profile returned above"),
                ProjectProfile::UsefulTextConsumerV1 => "v2",
                ProjectProfile::UsefulDataV1 => "v3",
                ProjectProfile::UsefulDataCommandV1 => "v4",
                ProjectProfile::UsefulDataCommandV2 => "v5",
                ProjectProfile::LanguageCommandIoV1 => "v6",
                ProjectProfile::LineCommandIoV1 => "v7",
                ProjectProfile::OwnedDataApiV1 => "v8",
                ProjectProfile::FlatOwnedRecordApiV1 => "v9",
                ProjectProfile::OwnedUtf8ApiV1 => "v10",
            };
            return Err(vec![Diagnostic::io(
                "SPX-W120",
                format!("Project {version} pathless Web builds use build_npm_inline"),
            )]);
        }
        crate::wasm::prepare_project_web_with_scalar_exports(
            &self.entry_program,
            self.manifest.name(),
            &self.project_revision,
            &self.workspace_revision,
            self.semantic.graph_digest(),
            self.manifest.entry(),
            self.manifest.web_exports(),
        )
        .and_then(|prepared| {
            prepared.into_inline(
                self.manifest.name(),
                &self.project_revision,
                &self.workspace_revision,
                self.semantic.graph_digest(),
                self.manifest.entry(),
                max_bytes,
            )
        })
        .map_err(|error| vec![error])
    }

    /// Build one deterministic, pathless, context-bound npm carrier.
    pub fn build_npm_inline(&self, max_bytes: usize) -> Result<ProjectNpmBuild, Vec<Diagnostic>> {
        npm::prepare(
            &self.manifest,
            &self.entry_program,
            &self.project_revision,
            &self.workspace_revision,
            self.semantic.graph_digest(),
            max_bytes,
        )
        .map_err(|error| vec![error])
    }

    /// Derive and independently replay the canonical Project v8 public API
    /// descriptor from this immutable retained subject.
    pub fn public_api_descriptor(&self) -> Result<super::PublicApiDescriptor, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::OwnedDataApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-J105",
                "public owned-data API description requires Project v8 owned-data-api.v1",
            )]);
        }
        let subject = super::PublicApiSubject {
            project_schema: self.manifest.schema(),
            project_revision: &self.project_revision,
            workspace_revision: &self.workspace_revision,
            project_graph_digest: self.semantic.graph_digest(),
        };
        let descriptor = self.profile_admission.owned_descriptor().ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-J105",
                "retained Project v8 admission has no owned-data descriptor",
            )]
        })?;
        super::replay_public_api_descriptor(
            &self.entry_program,
            self.manifest.web_exports(),
            subject,
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])
    }

    /// Evaluate one manifest-selected Project v8 export from this immutable
    /// retained subject. Descriptor replay and exact invocation-shape checks
    /// precede the authority-free interpreter call.
    pub fn evaluate_public_api_v1(
        &self,
        entry_id: &str,
        arguments: &[PublicApiArgument<'_>],
        max_steps: usize,
    ) -> Result<PublicApiEvaluation, Vec<Diagnostic>> {
        let descriptor = self.public_api_descriptor()?;
        if !super::public_api::valid_stable_id(entry_id) {
            return Err(vec![Diagnostic::io(
                "SPX-F102",
                "interpreter admission failed (unsupported_callee): public API selector is invalid",
            )]);
        }
        let export = descriptor
            .exports()
            .iter()
            .find(|export| export.stable_id().as_str() == entry_id)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-F102",
                    format!(
                        "interpreter admission failed (unsupported_callee): retained Project v8 descriptor does not select export `{entry_id}`"
                    ),
                )]
            })?;
        if arguments.len() != export.parameters().len() {
            return Err(vec![Diagnostic::io(
                "SPX-F103",
                format!(
                    "public API export `{entry_id}` takes {} argument(s), {} were provided",
                    export.parameters().len(),
                    arguments.len()
                ),
            )]);
        }
        for (ordinal, (parameter, argument)) in
            export.parameters().iter().zip(arguments).enumerate()
        {
            if !public_argument_matches(parameter.ty(), argument) {
                return Err(vec![Diagnostic::io(
                    "SPX-F103",
                    format!(
                        "parameter `{}` at ordinal {ordinal} of public API export `{entry_id}` expects {}, but the argument is {}",
                        parameter.source_name(),
                        parameter.ty().wire_name(),
                        public_argument_name(argument),
                    ),
                )]);
            }
        }
        crate::interpreter::evaluate_resolved_public_api(
            &self.entry_program,
            entry_id,
            arguments,
            max_steps,
        )
    }

    /// Independently replay the retained canonical Project v9 flat
    /// owned-record descriptor from this immutable retained subject.
    pub fn flat_owned_record_api_descriptor(
        &self,
    ) -> Result<super::FlatOwnedRecordApiDescriptor, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::FlatOwnedRecordApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-J113",
                "public flat owned-record API description requires Project v9 flat-owned-record-api.v1",
            )]);
        }
        let subject = super::PublicApiSubject {
            project_schema: self.manifest.schema(),
            project_revision: &self.project_revision,
            workspace_revision: &self.workspace_revision,
            project_graph_digest: self.semantic.graph_digest(),
        };
        let descriptor = self
            .profile_admission
            .flat_record_descriptor()
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-J113",
                    "retained Project v9 admission has no flat owned-record descriptor",
                )]
            })?;
        super::replay_flat_owned_record_api_descriptor(
            &self.entry_program,
            self.manifest.web_exports(),
            subject,
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])
    }

    /// Evaluate one manifest-selected Project v9 flat owned-record export
    /// from this immutable retained subject. Descriptor replay and exact
    /// invocation-shape checks precede the authority-free interpreter call.
    pub fn evaluate_flat_owned_record_api_v1(
        &self,
        entry_id: &str,
        arguments: &[PublicApiArgument<'_>],
        max_steps: usize,
    ) -> Result<FlatOwnedRecordEvaluation, Vec<Diagnostic>> {
        let descriptor = self.flat_owned_record_api_descriptor()?;
        if !super::public_api::valid_stable_id(entry_id) {
            return Err(vec![Diagnostic::io(
                "SPX-F102",
                "interpreter admission failed (unsupported_callee): flat owned-record selector is invalid",
            )]);
        }
        let export = descriptor
            .exports()
            .iter()
            .find(|export| export.stable_id().as_str() == entry_id)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-F102",
                    format!(
                        "interpreter admission failed (unsupported_callee): retained Project v9 descriptor does not select export `{entry_id}`"
                    ),
                )]
            })?;
        if arguments.len() != export.parameters().len() {
            return Err(vec![Diagnostic::io(
                "SPX-F103",
                format!(
                    "flat owned-record export `{entry_id}` takes {} argument(s), {} were provided",
                    export.parameters().len(),
                    arguments.len()
                ),
            )]);
        }
        for (ordinal, ((_, source_name, expected), argument)) in
            export.parameters().iter().zip(arguments).enumerate()
        {
            if !public_argument_matches(*expected, argument) {
                return Err(vec![Diagnostic::io(
                    "SPX-F103",
                    format!(
                        "parameter `{source_name}` at ordinal {ordinal} of flat owned-record export `{entry_id}` expects {}, but the argument is {}",
                        expected.wire_name(),
                        public_argument_name(argument),
                    ),
                )]);
            }
        }
        crate::interpreter::evaluate_resolved_flat_owned_record_api(
            &self.entry_program,
            export,
            arguments,
            max_steps,
        )
    }

    /// Independently replay the retained canonical Project v10 owned UTF-8
    /// descriptor from this immutable retained subject.
    pub fn owned_utf8_api_descriptor(&self) -> Result<super::PublicApiDescriptor, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::OwnedUtf8ApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-J105",
                "public owned UTF-8 API description requires Project v10 owned-utf8-api.v1",
            )]);
        }
        let subject = super::PublicApiSubject {
            project_schema: self.manifest.schema(),
            project_revision: &self.project_revision,
            workspace_revision: &self.workspace_revision,
            project_graph_digest: self.semantic.graph_digest(),
        };
        let descriptor = self.profile_admission.owned_descriptor().ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-J105",
                "retained Project v10 admission has no owned UTF-8 descriptor",
            )]
        })?;
        super::replay_public_api_descriptor(
            &self.entry_program,
            self.manifest.web_exports(),
            subject,
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])
    }

    /// Evaluate one manifest-selected Project v10 export from this immutable
    /// retained subject. Exact descriptor replay and invocation-shape checks
    /// precede the authority-free, cumulatively bounded UTF-8 interpreter.
    pub fn evaluate_owned_utf8_api_v1(
        &self,
        entry_id: &str,
        arguments: &[PublicApiArgument<'_>],
        max_steps: usize,
    ) -> Result<OwnedUtf8ApiEvaluation, Vec<Diagnostic>> {
        let descriptor = self.owned_utf8_api_descriptor()?;
        if !super::public_api::valid_stable_id(entry_id) {
            return Err(vec![Diagnostic::io(
                "SPX-F102",
                "interpreter admission failed (unsupported_callee): owned UTF-8 API selector is invalid",
            )]);
        }
        let export = descriptor
            .exports()
            .iter()
            .find(|export| export.stable_id().as_str() == entry_id)
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-F102",
                    format!(
                        "interpreter admission failed (unsupported_callee): retained Project v10 descriptor does not select export `{entry_id}`"
                    ),
                )]
            })?;
        if arguments.len() != export.parameters().len() {
            return Err(vec![Diagnostic::io(
                "SPX-F103",
                format!(
                    "owned UTF-8 API export `{entry_id}` takes {} argument(s), {} were provided",
                    export.parameters().len(),
                    arguments.len()
                ),
            )]);
        }
        for (ordinal, (parameter, argument)) in
            export.parameters().iter().zip(arguments).enumerate()
        {
            if !public_argument_matches(parameter.ty(), argument) {
                return Err(vec![Diagnostic::io(
                    "SPX-F103",
                    format!(
                        "parameter `{}` at ordinal {ordinal} of owned UTF-8 API export `{entry_id}` expects {}, but the argument is {}",
                        parameter.source_name(),
                        parameter.ty().wire_name(),
                        public_argument_name(argument),
                    ),
                )]);
            }
        }
        crate::interpreter::evaluate_resolved_owned_utf8_api(
            &self.entry_program,
            export,
            arguments,
            max_steps,
        )
    }

    /// Emit the sole retained test-module closure as legacy core Wasm.
    pub fn test_wasm_module(&self) -> Result<Vec<u8>, Vec<Diagnostic>> {
        crate::wasm::emit_resolved_module(&self.test_program).map_err(|error| vec![error])
    }
}

fn public_argument_matches(
    expected: PublicApiParameterType,
    argument: &PublicApiArgument<'_>,
) -> bool {
    matches!(
        (expected, argument),
        (PublicApiParameterType::I64, PublicApiArgument::I64(_))
            | (PublicApiParameterType::Bool, PublicApiArgument::Bool(_))
            | (
                PublicApiParameterType::BorrowStr,
                PublicApiArgument::BorrowStr(_)
            )
            | (
                PublicApiParameterType::BorrowSliceU8,
                PublicApiArgument::BorrowSliceU8(_)
            )
    )
}

fn public_argument_name(argument: &PublicApiArgument<'_>) -> &'static str {
    match argument {
        PublicApiArgument::I64(_) => "i64",
        PublicApiArgument::Bool(_) => "bool",
        PublicApiArgument::BorrowStr(_) => "borrow-str",
        PublicApiArgument::BorrowSliceU8(_) => "borrow-slice-u8",
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::workspace_analysis::{
        WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
    };

    use super::*;

    #[test]
    fn retained_revision_outlives_live_authority_and_keeps_every_read_only_product() {
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project/semaprax.toml");
        let snapshot = super::super::load_snapshot(&manifest).unwrap();
        let revision = snapshot.retain_revision();
        let expected_project_revision = snapshot.project_revision().to_owned();
        drop(snapshot);

        revision.check().unwrap();
        assert_eq!(revision.project_revision(), expected_project_revision);
        assert_eq!(revision.manifest().name(), "calculator");
        assert_eq!(revision.sources().len(), 3);
        assert!(revision.workspace_manifest().ends_with('\n'));
        assert_eq!(revision.entry_program().module, "calculator.app");
        assert_eq!(revision.test_program().module, "calculator.tests");
        assert!(revision
            .semantic_graph()
            .contains(expected_project_revision.as_str()));
        assert!(revision
            .semantic_context(
                WorkspaceAnalysisTargetKind::Declaration,
                "calculator.add",
                WorkspaceContextOptions::default(),
            )
            .unwrap()
            .contains(expected_project_revision.as_str()));
        assert!(revision
            .semantic_impact(
                WorkspaceAnalysisTargetKind::Declaration,
                "calculator.add",
                WorkspaceImpactOptions::default(),
            )
            .unwrap()
            .contains(expected_project_revision.as_str()));
        assert!(revision
            .execute_test(&ProjectExecutionOptions::default())
            .unwrap()
            .command_succeeded());
        revision.test_wasm_module().unwrap();
        revision
            .build_web_inline(crate::wasm::MAX_PROJECT_WEB_BUILD_BYTES)
            .unwrap()
            .verify()
            .unwrap();
    }

    #[test]
    fn project_v6_execution_envelope_replays_independently() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples/spxgrep-language-command-project/semaprax.toml");
        let snapshot = super::super::load_snapshot(&manifest).unwrap();
        let execution = snapshot
            .execute_entry(&ProjectExecutionOptions::default())
            .unwrap();
        super::super::verify_execution_envelope(execution.envelope()).unwrap();

        let tampered = execution.envelope().replace(
            "\"project_schema\":\"semaprax.project.v6\"",
            "\"project_schema\":\"semaprax.project.v7\"",
        );
        assert_ne!(tampered, execution.envelope());
        assert!(super::super::verify_execution_envelope(&tampered).is_err());
    }
}
