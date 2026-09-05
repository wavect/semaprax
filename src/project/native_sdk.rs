//! Authenticated Project subject for the external Native Rust SDK builder.
//!
//! The root compiler cannot depend on the SDK builder. Instead, this module
//! lends one already authenticated linked-HIR subject to an effectful callback
//! while retaining all Project handles through the final publication check.

use crate::diagnostic::Diagnostic;
use crate::hir::{IdentityOrigin, ResolvedProgram};
use std::path::Path;

use super::{ProjectSnapshot, ProjectSource, AUTHENTICATED_PROJECT_SUBJECT_OPERATION};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
const RUST_OWNED_DATA_PUBLICATION_SUBJECT: &str = "Project v8 Native Rust owned-data package";
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
const RUST_FLAT_OWNED_RECORD_PUBLICATION_SUBJECT: &str =
    "Project v9 Native Rust flat owned-record package";
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
const RUST_OWNED_UTF8_PUBLICATION_SUBJECT: &str = "Project v10 Native Rust owned-UTF8 package";
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
const RUST_NESTED_OWNED_RECORD_PUBLICATION_SUBJECT: &str =
    "Project v11 Native Rust nested owned-record package";

/// Invocation-borrowed, target-neutral subject for the standalone owned-data
/// Rust SDK builder and the activated Project-v8 route. Construction always
/// derives and replays the canonical public descriptor before lending any
/// effectful build operation.
pub struct ProjectOwnedDataNativeSdkSubject<'a> {
    program: &'a ResolvedProgram,
    selected: &'a [String],
    subject: super::PublicApiSubject<'a>,
    descriptor: super::PublicApiDescriptor,
}

impl<'a> ProjectOwnedDataNativeSdkSubject<'a> {
    pub fn program(&self) -> &ResolvedProgram {
        self.program
    }

    pub fn selected(&self) -> &[String] {
        self.selected
    }

    pub const fn subject(&self) -> super::PublicApiSubject<'a> {
        self.subject
    }

    pub fn descriptor(&self) -> &super::PublicApiDescriptor {
        &self.descriptor
    }
}

/// Validate one HIR/subject pair and lend its exact canonical descriptor to an
/// owned-data SDK operation. This does not parse or activate Project v8.
pub fn with_native_owned_data_sdk_subject<T>(
    program: &ResolvedProgram,
    selected: &[String],
    subject: super::PublicApiSubject<'_>,
    operation: impl FnOnce(ProjectOwnedDataNativeSdkSubject<'_>) -> Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    let descriptor = super::derive_public_api_descriptor(program, selected, subject)
        .map_err(|error| vec![error])?;
    super::replay_public_api_descriptor(
        program,
        selected,
        subject,
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .map_err(|error| vec![error])?;
    operation(ProjectOwnedDataNativeSdkSubject {
        program,
        selected,
        subject,
        descriptor,
    })
}

/// One manifest-selected stable-ID export and its authenticated declaration
/// origin inside the complete Project source set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectNativeSdkExport {
    stable_id: String,
    module: String,
    path: String,
}

impl ProjectNativeSdkExport {
    pub fn stable_id(&self) -> &str {
        &self.stable_id
    }

    pub fn module(&self) -> &str {
        &self.module
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

/// Borrowed, non-authoritative facts for one authenticated Project SDK build.
///
/// This value cannot be constructed by external callers. It deliberately has
/// no publication method; the SDK builder receives it only inside
/// [`ProjectSnapshot::with_authenticated_native_rust_sdk_subject`].
pub struct ProjectNativeSdkSubject<'a> {
    canonical_manifest: String,
    project_name: &'a str,
    project_revision: &'a str,
    workspace_revision: &'a str,
    project_graph_digest: &'a str,
    entry_module: &'a str,
    sources: &'a [ProjectSource],
    exports: Vec<ProjectNativeSdkExport>,
    program: &'a ResolvedProgram,
}

impl<'a> ProjectNativeSdkSubject<'a> {
    fn new(snapshot: &'a ProjectSnapshot) -> Result<Self, Vec<Diagnostic>> {
        let mut exports = Vec::with_capacity(snapshot.manifest.web_exports().len());
        for stable_id in snapshot.manifest.web_exports() {
            let function = snapshot
                .semantic
                .rename_function(stable_id)
                .ok_or_else(|| subject_error("manifest export is absent from Project semantics"))?;
            if function.origin != IdentityOrigin::Explicit {
                return Err(subject_error(
                    "Project Native Rust SDK exports require explicit identities",
                ));
            }
            exports.push(ProjectNativeSdkExport {
                stable_id: stable_id.clone(),
                module: function.module.clone(),
                path: function.path.clone(),
            });
        }
        Ok(Self {
            canonical_manifest: snapshot.manifest.to_canonical_toml(),
            project_name: snapshot.manifest.name(),
            project_revision: &snapshot.project_revision,
            workspace_revision: &snapshot.workspace_revision,
            project_graph_digest: snapshot.semantic.graph_digest(),
            entry_module: snapshot.manifest.entry(),
            sources: &snapshot.sources,
            exports,
            program: &snapshot.public_api_program,
        })
    }

    pub fn canonical_manifest(&self) -> &str {
        &self.canonical_manifest
    }

    pub fn project_name(&self) -> &str {
        self.project_name
    }

    pub fn project_revision(&self) -> &str {
        self.project_revision
    }

    pub fn workspace_revision(&self) -> &str {
        self.workspace_revision
    }

    pub fn project_graph_digest(&self) -> &str {
        self.project_graph_digest
    }

    pub fn entry_module(&self) -> &str {
        self.entry_module
    }

    pub fn sources(&self) -> &[ProjectSource] {
        self.sources
    }

    pub fn exports(&self) -> &[ProjectNativeSdkExport] {
        &self.exports
    }

    pub fn program(&self) -> &ResolvedProgram {
        self.program
    }
}

/// Compiler-replayed data lent to an explicitly injected private publisher.
/// This object owns no tool, filesystem handle, or publication authority.
pub struct ProjectNativeRustPackage {
    descriptor: Vec<u8>,
    descriptor_digest: String,
    selected: Vec<String>,
    provider: Vec<u8>,
    mode: ProjectNativeRustPackageMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectNativeRustPackageMode {
    OwnedData,
    FlatOwnedRecord,
    OwnedUtf8,
    NestedOwnedRecord,
}

impl ProjectNativeRustPackage {
    pub fn descriptor(&self) -> &[u8] {
        &self.descriptor
    }
    pub fn descriptor_digest(&self) -> &str {
        &self.descriptor_digest
    }
    pub fn selected(&self) -> &[String] {
        &self.selected
    }
    pub fn provider(&self) -> &[u8] {
        &self.provider
    }
    pub fn mode(&self) -> ProjectNativeRustPackageMode {
        self.mode
    }
}

impl ProjectSnapshot {
    /// Standalone compiler builds have no private package publication host.
    pub fn build_rust(&mut self, output: &Path) -> Result<(), Vec<Diagnostic>> {
        self.build_rust_with(output, |_, _| {
            Err(vec![Diagnostic::io(
                "SPX-J114",
                "the rust target requires the unpublished semaprax-toolchain host",
            )])
        })
    }

    /// Build the exact profile-selected Project-v8/v9/v10/v11 closure as a safe
    /// Rust package. Descriptor and semantic-recipe replay are completed
    /// before the lower held-tool/publication layer receives any bytes.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    pub fn build_rust_with(
        &mut self,
        output: &Path,
        publish: impl FnOnce(ProjectNativeRustPackage, &Path) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.manifest().is_v9() {
            return self.build_flat_owned_record_rust(output, publish);
        }
        if self.manifest().is_v11() {
            return self.build_nested_owned_record_rust(output, publish);
        }
        if !self.manifest().is_v8() && !self.manifest().is_v10() {
            return Err(vec![Diagnostic::io(
                "SPX-J114",
                "the rust target requires the exact Project v8 owned-data-api.v1, Project v9 flat-owned-record-api.v1, or Project v10 owned-utf8-api.v1 profile",
            )]);
        }
        let project_v10 = self.manifest().is_v10();
        let version = if project_v10 { "v10" } else { "v8" };
        let selected = self.manifest().web_exports().to_vec();
        let subject = super::PublicApiSubject {
            project_schema: self.manifest().schema(),
            project_revision: self.project_revision(),
            workspace_revision: self.workspace_revision(),
            project_graph_digest: self.semantic.graph_digest(),
        };
        let descriptor =
            super::derive_public_api_descriptor(self.public_api_program(), &selected, subject)
                .map_err(|error| vec![error])?;
        let descriptor_bytes = descriptor.canonical_bytes();
        let descriptor_digest = descriptor.digest();
        let replayed = super::replay_public_api_descriptor(
            self.public_api_program(),
            &selected,
            subject,
            &descriptor_bytes,
            &descriptor_digest,
        )
        .map_err(|error| vec![error])?;
        if replayed != descriptor {
            return Err(vec![rust_build_error(format!(
                "Project {version} descriptor derivation and replay disagree"
            ))]);
        }

        let recipe = super::npm::render_owned_data_semantic_recipe(self.public_api_program())
            .map_err(|error| vec![error])?;
        let replayed_program =
            super::npm::replay_owned_data_semantic_recipe(self.public_api_program(), &recipe)
                .map_err(|error| vec![error])?;
        let replayed_descriptor = super::replay_public_api_descriptor(
            &replayed_program,
            &selected,
            subject,
            &descriptor_bytes,
            &descriptor_digest,
        )
        .map_err(|error| vec![error])?;
        if replayed_descriptor != descriptor {
            return Err(vec![rust_build_error(format!(
                "Project {version} descriptor disagrees with semantic-recipe replay"
            ))]);
        }

        let emit_provider = if project_v10 {
            crate::codegen::emit_project_v10_native_owned_utf8_provider
        } else {
            crate::codegen::emit_project_v8_native_owned_data_provider
        };
        let provider = emit_provider(
            self.public_api_program(),
            &selected,
            subject,
            &descriptor_bytes,
            &descriptor_digest,
        )
        .map_err(|error| vec![error])?;
        let replayed_provider = emit_provider(
            &replayed_program,
            &selected,
            subject,
            &descriptor_bytes,
            &descriptor_digest,
        )
        .map_err(|error| vec![error])?;
        if provider != replayed_provider
            || provider.descriptor() != descriptor_bytes
            || provider.descriptor_digest() != descriptor_digest
        {
            return Err(vec![rust_build_error(format!(
                "Project {version} native provider disagrees with independent replay"
            ))]);
        }

        self.recheck()?;
        let provider_bytes = provider.source().as_bytes().to_vec();
        let plan = ProjectNativeRustPackage {
            descriptor: descriptor_bytes,
            descriptor_digest,
            selected,
            provider: provider_bytes,
            mode: if project_v10 {
                ProjectNativeRustPackageMode::OwnedUtf8
            } else {
                ProjectNativeRustPackageMode::OwnedData
            },
        };
        publish(plan, output)?;
        self.published_subject = Some(if project_v10 {
            RUST_OWNED_UTF8_PUBLICATION_SUBJECT
        } else {
            RUST_OWNED_DATA_PUBLICATION_SUBJECT
        });
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn build_flat_owned_record_rust(
        &mut self,
        output: &Path,
        publish: impl FnOnce(ProjectNativeRustPackage, &Path) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        let selected = self.manifest().web_exports().to_vec();
        let subject = super::PublicApiSubject {
            project_schema: self.manifest().schema(),
            project_revision: self.project_revision(),
            workspace_revision: self.workspace_revision(),
            project_graph_digest: self.semantic.graph_digest(),
        };
        let descriptor = super::derive_flat_owned_record_api_descriptor(
            self.public_api_program(),
            &selected,
            subject,
        )
        .map_err(|error| vec![error])?;
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        let replayed = super::replay_flat_owned_record_api_descriptor(
            self.public_api_program(),
            &selected,
            subject,
            &bytes,
            &digest,
        )
        .map_err(|error| vec![error])?;
        if replayed != descriptor {
            return Err(vec![rust_build_error(
                "Project v9 descriptor derivation and replay disagree",
            )]);
        }
        let recipe = super::npm::render_owned_data_semantic_recipe(self.public_api_program())
            .map_err(|error| vec![error])?;
        let replayed_program =
            super::npm::replay_owned_data_semantic_recipe(self.public_api_program(), &recipe)
                .map_err(|error| vec![error])?;
        let replayed_descriptor = super::replay_flat_owned_record_api_descriptor(
            &replayed_program,
            &selected,
            subject,
            &bytes,
            &digest,
        )
        .map_err(|error| vec![error])?;
        if replayed_descriptor != descriptor {
            return Err(vec![rust_build_error(
                "Project v9 descriptor disagrees with semantic-recipe replay",
            )]);
        }
        let provider = crate::codegen::emit_project_v9_native_flat_owned_record_provider(
            self.public_api_program(),
            &selected,
            subject,
            &bytes,
            &digest,
        )
        .map_err(|error| vec![error])?;
        let replayed_provider = crate::codegen::emit_project_v9_native_flat_owned_record_provider(
            &replayed_program,
            &selected,
            subject,
            &bytes,
            &digest,
        )
        .map_err(|error| vec![error])?;
        if provider != replayed_provider
            || provider.descriptor() != bytes
            || provider.descriptor_digest() != digest
        {
            return Err(vec![rust_build_error(
                "Project v9 native provider disagrees with independent replay",
            )]);
        }
        self.recheck()?;
        let provider_bytes = provider.source().as_bytes().to_vec();
        let plan = ProjectNativeRustPackage {
            descriptor: bytes,
            descriptor_digest: digest,
            selected,
            provider: provider_bytes,
            mode: ProjectNativeRustPackageMode::FlatOwnedRecord,
        };
        publish(plan, output)?;
        self.published_subject = Some(RUST_FLAT_OWNED_RECORD_PUBLICATION_SUBJECT);
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn build_nested_owned_record_rust(
        &mut self,
        output: &Path,
        publish: impl FnOnce(ProjectNativeRustPackage, &Path) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        let selected = self.manifest().web_exports().to_vec();
        let subject = super::PublicApiSubject {
            project_schema: self.manifest().schema(),
            project_revision: self.project_revision(),
            workspace_revision: self.workspace_revision(),
            project_graph_digest: self.semantic.graph_digest(),
        };
        let descriptor = super::derive_nested_owned_record_api_descriptor(
            self.public_api_program(),
            &selected,
            subject,
        )
        .map_err(|error| vec![error])?;
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        let replay = |program| {
            super::replay_nested_owned_record_api_descriptor(
                program, &selected, subject, &bytes, &digest,
            )
        };
        if replay(self.public_api_program()).map_err(|error| vec![error])? != descriptor {
            return Err(vec![rust_build_error(
                "Project v11 descriptor derivation and replay disagree",
            )]);
        }
        let recipe = super::npm::render_owned_data_semantic_recipe(self.public_api_program())
            .map_err(|error| vec![error])?;
        let replayed_program =
            super::npm::replay_owned_data_semantic_recipe(self.public_api_program(), &recipe)
                .map_err(|error| vec![error])?;
        if replay(&replayed_program).map_err(|error| vec![error])? != descriptor {
            return Err(vec![rust_build_error(
                "Project v11 descriptor disagrees with semantic-recipe replay",
            )]);
        }
        let emit = |program| {
            crate::codegen::emit_project_v11_native_nested_owned_record_provider(
                program, &selected, subject, &bytes, &digest,
            )
        };
        let provider = emit(self.public_api_program()).map_err(|error| vec![error])?;
        let replayed_provider = emit(&replayed_program).map_err(|error| vec![error])?;
        if provider != replayed_provider
            || provider.descriptor() != bytes
            || provider.descriptor_digest() != digest
        {
            return Err(vec![rust_build_error(
                "Project v11 native provider disagrees with independent replay",
            )]);
        }
        self.recheck()?;
        publish(
            ProjectNativeRustPackage {
                descriptor: bytes,
                descriptor_digest: digest,
                selected,
                provider: provider.source().as_bytes().to_vec(),
                mode: ProjectNativeRustPackageMode::NestedOwnedRecord,
            },
            output,
        )?;
        self.published_subject = Some(RUST_NESTED_OWNED_RECORD_PUBLICATION_SUBJECT);
        self.recheck()
            .map_err(|drift| self.publication_uncertainty(drift))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    pub fn build_rust_with(
        &mut self,
        _output: &Path,
        _publish: impl FnOnce(ProjectNativeRustPackage, &Path) -> Result<(), Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        Err(vec![Diagnostic::io(
            "SPX-J114",
            "the rust target is available only on Linux, macOS, and Windows hosts",
        )])
    }

    /// Lend one authenticated Project subject to a potentially effectful
    /// operation while all declared Project inputs remain held.
    ///
    /// Drift before the callback prevents every callback effect. Once the
    /// callback starts, later drift is conservatively reported as `SPX-J103`
    /// even when the callback returns an error: the root compiler cannot infer
    /// whether an external operation crossed its publication boundary.
    pub fn with_authenticated_native_rust_sdk_subject<T>(
        &mut self,
        operation: impl FnOnce(ProjectNativeSdkSubject<'_>) -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        self.recheck()?;
        self.published_subject = Some(AUTHENTICATED_PROJECT_SUBJECT_OPERATION);
        let subject = ProjectNativeSdkSubject::new(self)?;
        let result = operation(subject);
        match (result, self.recheck()) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(drift)) => Err(self.publication_uncertainty(drift)),
            (Err(primary), Ok(())) => Err(primary),
            (Err(mut primary), Err(drift)) => {
                let mut uncertainty = self.publication_uncertainty(drift);
                primary.append(&mut uncertainty);
                Err(primary)
            }
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn rust_build_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B114", message)
}

fn subject_error(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J112", message)]
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    #[test]
    fn owned_profile_publication_subjects_are_exact_and_distinct() {
        assert_eq!(
            super::RUST_OWNED_DATA_PUBLICATION_SUBJECT,
            "Project v8 Native Rust owned-data package"
        );
        assert_eq!(
            super::RUST_FLAT_OWNED_RECORD_PUBLICATION_SUBJECT,
            "Project v9 Native Rust flat owned-record package"
        );
        assert_eq!(
            super::RUST_OWNED_UTF8_PUBLICATION_SUBJECT,
            "Project v10 Native Rust owned-UTF8 package"
        );
    }

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn fixture() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-native-sdk-subject-{}-{}",
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
    fn subject_binds_complete_project_and_exact_manifest_exports() {
        let fixture = fixture();
        let manifest = fixture.0.join("semaprax.toml");
        let facts = super::super::with_authenticated_project(&manifest, |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|subject| {
                assert_eq!(subject.project_name(), "calculator");
                assert_eq!(subject.entry_module(), "calculator.app");
                assert_eq!(subject.sources().len(), 3);
                assert_eq!(
                    subject
                        .exports()
                        .iter()
                        .map(|export| export.stable_id())
                        .collect::<Vec<_>>(),
                    [
                        "calculator.add",
                        "calculator.divide",
                        "calculator.is-negative",
                        "calculator.multiply",
                        "calculator.not",
                        "calculator.subtract",
                    ]
                );
                assert!(subject
                    .exports()
                    .iter()
                    .all(|export| export.module() == "calculator.core"));
                assert!(subject
                    .exports()
                    .iter()
                    .all(|export| export.path() == "src/core.spx"));
                assert_eq!(subject.program().module, "calculator.app");
                Ok((
                    subject.project_revision().to_owned(),
                    subject.workspace_revision().to_owned(),
                    subject.project_graph_digest().to_owned(),
                    subject.canonical_manifest().to_owned(),
                ))
            })
        })
        .unwrap();
        assert!(facts.0.starts_with("sha256:"));
        assert!(facts.1.starts_with("sha256:"));
        assert!(facts.2.starts_with("sha256:"));
        assert!(facts.3.contains("web_exports = [\"calculator.add\""));
    }

    #[test]
    fn publication_callback_is_prechecked_and_post_success_drift_is_uncertain() {
        let fixture = fixture();
        let manifest = fixture.0.join("semaprax.toml");
        let core = fixture.0.join("src/core.spx");
        let source = std::fs::read_to_string(&core).unwrap();
        let diagnostics = super::super::with_authenticated_project(&manifest, |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|_| {
                std::fs::write(&core, format!("{source}\n")).unwrap();
                Ok(())
            })
        })
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SPX-J103"
                && diagnostic
                    .message
                    .contains("authenticated Project subject operation")
        }));

        std::fs::write(&core, source).unwrap();
        let source = std::fs::read_to_string(&core).unwrap();
        let diagnostics = super::super::with_authenticated_project(&manifest, |snapshot| {
            snapshot.with_authenticated_native_rust_sdk_subject(|_| {
                std::fs::write(&core, format!("{source}\n")).unwrap();
                Err::<(), _>(super::subject_error(
                    "operation failed after a possible effect",
                ))
            })
        })
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SPX-J103"
                && diagnostic
                    .message
                    .contains("authenticated Project subject operation")
        }));

        std::fs::write(&core, source).unwrap();
        let mut snapshot = super::super::load_snapshot(&manifest).unwrap();
        std::fs::write(&core, "foreign").unwrap();
        let acted = std::cell::Cell::new(false);
        assert!(snapshot
            .with_authenticated_native_rust_sdk_subject(|_| {
                acted.set(true);
                Ok(())
            })
            .is_err());
        assert!(!acted.get());
    }
}
