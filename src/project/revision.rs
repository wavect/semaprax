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
use super::{ProjectSource, ProjectWebBuild, ScalarWitInterfaceArtifactV1};

/// One immutable, fully admitted Project revision without ambient authority.
pub struct ProjectRevision {
    pub(super) manifest: ProjectManifest,
    pub(super) sources: Vec<ProjectSource>,
    pub(super) workspace_manifest: String,
    pub(super) workspace_revision: String,
    pub(super) project_revision: String,
    pub(super) entry_program: crate::hir::ResolvedProgram,
    pub(super) public_api_program: crate::hir::ResolvedProgram,
    pub(super) test_program: crate::hir::ResolvedProgram,
    pub(super) semantic: semantic::ProjectSemanticState,
    pub(super) profile_admission: admission::PreparedProjectAdmission,
    pub(super) source_agents: Vec<super::ResolvedSourceAgent>,
    pub(super) agent_definitions: Vec<crate::agent_definition::CompiledAgentDefinition>,
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
            public_api_program: built.public_api_program,
            test_program: built.test_program,
            semantic: built.semantic,
            profile_admission: built.profile_admission,
            source_agents: built.source_agents,
            agent_definitions: built.agent_definitions,
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

    /// Compiler-admitted HIR-equivalent source Agent nodes in stable-ID order.
    pub fn source_agents(&self) -> &[super::ResolvedSourceAgent] {
        &self.source_agents
    }

    /// Frozen AgentDefinition v1 compatibility products for source Agents.
    pub fn agent_definitions(&self) -> &[crate::agent_definition::CompiledAgentDefinition] {
        &self.agent_definitions
    }

    /// Derive the canonical, authority-free semantic workspace identity over
    /// this already admitted immutable Project revision.
    pub fn canonical_workspace_revision(
        &self,
    ) -> Result<super::SemanticWorkspaceRevision, Vec<Diagnostic>> {
        if self.agent_definitions.is_empty() {
            super::SemanticWorkspaceRevision::derive(self)
        } else {
            let definitions = self.agent_definitions.iter().collect::<Vec<_>>();
            super::SemanticWorkspaceRevision::derive_with_agent_definitions(
                self,
                self.project_revision(),
                &definitions,
            )
        }
    }

    pub fn entry_program(&self) -> &crate::hir::ResolvedProgram {
        &self.entry_program
    }

    /// Return the admitted entry-plus-selected-export closure used by public
    /// API targets. This remains distinct from [`Self::entry_program`], which
    /// owns executable entry semantics and cleanup ordering.
    pub fn public_api_program(&self) -> &crate::hir::ResolvedProgram {
        &self.public_api_program
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

    /// Rebuild and independently replay the retained Project-v1 scalar WIT
    /// interface. The returned artifact contains no executable bytes or host
    /// authority.
    pub fn scalar_wit_interface_v1(&self) -> Result<ScalarWitInterfaceArtifactV1, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::ScalarV1 {
            return Err(vec![Diagnostic::io(
                "SPX-J105",
                "public scalar WIT interface requires Project v1",
            )]);
        }
        let retained = self
            .profile_admission
            .scalar_wit_descriptor()
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-J105",
                    "retained Project v1 admission has no scalar WIT descriptor",
                )]
            })?;
        self.replay_scalar_wit_interface_v1(&retained.canonical_bytes(), &retained.digest())
    }

    /// Verify caller-supplied descriptor bytes against this exact immutable
    /// Project-v1 subject and return the independently reconstructed artifact.
    pub fn replay_scalar_wit_interface_v1(
        &self,
        descriptor_bytes: &[u8],
        digest: &str,
    ) -> Result<ScalarWitInterfaceArtifactV1, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::ScalarV1 {
            return Err(vec![Diagnostic::io(
                "SPX-J105",
                "public scalar WIT interface replay requires Project v1",
            )]);
        }
        super::scalar_wit::replay_scalar_wit_interface_v1(
            &self.entry_program,
            self.manifest.web_exports(),
            super::scalar_wit::ScalarWitSubject {
                project_name: self.manifest.name(),
                project_revision: &self.project_revision,
                workspace_revision: &self.workspace_revision,
                project_graph_digest: self.semantic.graph_digest(),
            },
            descriptor_bytes,
            digest,
        )
        .map_err(|error| vec![error])
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

    pub(super) fn execute_test_cancellable(
        &self,
        options: &ProjectExecutionOptions,
        cancellation: &super::ProjectExecutionCancellation,
    ) -> Result<super::execution::CancellableProjectExecution, Vec<Diagnostic>> {
        execution::execute_cancellable(self, ProjectExecutionRole::Test, options, cancellation)
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
                ProjectProfile::NestedOwnedRecordApiV1 => "v11",
                ProjectProfile::NetworkCommandIoV1 => "v12",
                ProjectProfile::HttpsCommandIoV1 => "v13",
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
            &self.public_api_program,
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
            &self.public_api_program,
            self.manifest.web_exports(),
            subject,
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])
    }

    /// Evaluate one manifest-selected Project-v2 borrowed-text export from
    /// this immutable revision. Selection and argument-shape checks remain
    /// bound to the already-admitted manifest and linked HIR; the evaluator
    /// receives no filesystem, process, or publication authority.
    pub fn evaluate_text_api_v1(
        &self,
        entry_id: &str,
        arguments: &[PublicApiArgument<'_>],
        max_steps: usize,
    ) -> Result<PublicApiEvaluation, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::UsefulTextConsumerV1 {
            return Err(vec![Diagnostic::io(
                "SPX-F102",
                "borrowed-text API evaluation requires the useful-text-consumer.v1 Project profile",
            )]);
        }
        if !self
            .manifest
            .web_exports()
            .iter()
            .any(|selected| selected == entry_id)
        {
            return Err(vec![Diagnostic::io(
                "SPX-F102",
                format!("borrowed-text API export `{entry_id}` is not selected by the manifest"),
            )]);
        }
        crate::interpreter::evaluate_resolved_public_api(
            &self.public_api_program,
            entry_id,
            arguments,
            max_steps,
        )
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
            &self.public_api_program,
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
            &self.public_api_program,
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
            &self.public_api_program,
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
            &self.public_api_program,
            self.manifest.web_exports(),
            subject,
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])
    }

    /// Independently replay the retained canonical Project-v11 nested
    /// owned-record descriptor. This grants no target or publication authority.
    pub fn nested_owned_record_api_descriptor(
        &self,
    ) -> Result<super::NestedOwnedRecordApiDescriptor, Vec<Diagnostic>> {
        if self.manifest.project_profile() != ProjectProfile::NestedOwnedRecordApiV1 {
            return Err(vec![Diagnostic::io(
                "SPX-J118",
                "public nested owned-record API description requires Project v11 nested-owned-record-api.v1",
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
            .nested_record_descriptor()
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-J118",
                    "retained Project v11 admission has no nested owned-record descriptor",
                )]
            })?;
        super::replay_nested_owned_record_api_descriptor(
            &self.public_api_program,
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
            &self.public_api_program,
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
