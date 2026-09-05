//! Exact project-local SEMAPRAX dependency closure.
//!
//! Each manifest row names one local Subject-v3 envelope. We independently
//! replay every embedded Report-v2 source, resolve the complete finite set for
//! every declared target, require every supplied coordinate to be selected,
//! and then lend only the authenticated source bytes to the ordinary workspace
//! linker. No registry, ambient cache, acquisition, or process authority is
//! available here.

use crate::diagnostic::Diagnostic;
use crate::package_lock_v3::{self, VerifiedDependencySubject};
use crate::package_resolver_v2::{self, Requirement, ResolutionInput, ResolutionOptions};
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::{ProjectManifest, PACKAGE_TARGET_NATIVE64, PACKAGE_TARGET_WASM32};

pub(super) struct HeldDependencySubject {
    pub(super) declared_name: String,
    pub(super) bytes: String,
}

pub(super) fn resolve(
    manifest: &ProjectManifest,
    inputs: Vec<HeldDependencySubject>,
) -> Result<Vec<SemanticWorkspaceSource>, Vec<Diagnostic>> {
    if manifest.dependency_sources().is_empty() {
        return Ok(Vec::new());
    }
    if manifest.project_profile() != super::ProjectProfile::ScalarV1 {
        return Err(failure(
            "ordinary local SEMAPRAX dependency subjects currently require the scalar Project profile",
        ));
    }
    if inputs.len() != manifest.dependency_sources().len() {
        return Err(failure(
            "the held dependency-subject inventory differs from the manifest",
        ));
    }

    let mut verified = Vec::with_capacity(inputs.len());
    let mut subject_bytes = Vec::with_capacity(inputs.len());
    for input in inputs {
        let subject = package_lock_v3::verify_dependency_subject(&input.bytes)
            .map_err(|_| failure("a local SEMAPRAX dependency subject failed exact replay"))?;
        if subject.coordinate.package != input.declared_name {
            return Err(failure(format!(
                "dependency source `{}` contains package `{}`",
                input.declared_name, subject.coordinate.package
            )));
        }
        if super::standard_dependencies::is_bundled(&subject.coordinate.package) {
            return Err(failure(format!(
                "dependency source `{}` attempts to replace a compiler-bundled standard-library package",
                subject.coordinate.package
            )));
        }
        verified.push(subject);
        subject_bytes.push(input.bytes);
    }

    let roots = manifest
        .dependencies()
        .iter()
        .filter(|dependency| !super::standard_dependencies::is_bundled(dependency.name()))
        .map(|dependency| Requirement {
            package: dependency.name().to_owned(),
            range: dependency.range().to_owned(),
        })
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Err(failure(
            "`[dependency-sources]` requires at least one ordinary `[dependencies]` root",
        ));
    }
    for root in &roots {
        if !verified
            .iter()
            .any(|subject| subject.coordinate.package == root.package)
        {
            return Err(failure(format!(
                "ordinary dependency `{}` has no exact `[dependency-sources]` subject",
                root.package
            )));
        }
    }

    let expected = verified
        .iter()
        .map(|subject| subject.coordinate.clone())
        .collect::<Vec<_>>();
    let targets = manifest.target_matrix().map_or_else(
        || {
            vec![
                PACKAGE_TARGET_NATIVE64.to_owned(),
                PACKAGE_TARGET_WASM32.to_owned(),
            ]
        },
        <[String]>::to_vec,
    );
    for target in targets {
        let input = ResolutionInput {
            requirements: roots.clone(),
            subjects: subject_bytes.clone(),
            target: target.clone(),
            allowed_capabilities: manifest.capabilities().to_vec(),
        };
        let options = ResolutionOptions::default();
        let evidence = package_resolver_v2::generate(&input, &options).map_err(|_| {
            failure(format!(
                "SEMAPRAX dependencies do not resolve for `{target}`"
            ))
        })?;
        let mut selected = package_resolver_v2::verify(&evidence, &input, &options)
            .map_err(|_| failure(format!("SEMAPRAX dependency replay failed for `{target}`")))?
            .packages;
        let mut expected = expected.clone();
        selected.sort();
        expected.sort();
        if selected != expected {
            return Err(failure(format!(
                "`[dependency-sources]` must contain exactly the selected closure for `{target}`"
            )));
        }
    }

    verified.sort_by(|left, right| left.coordinate.cmp(&right.coordinate));
    Ok(verified.into_iter().map(source).collect())
}

fn source(subject: VerifiedDependencySubject) -> SemanticWorkspaceSource {
    SemanticWorkspaceSource {
        path: dependency_path(&subject.subject_digest),
        source: subject.canonical_source,
    }
}

fn dependency_path(subject_digest: &str) -> String {
    let digest = subject_digest
        .strip_prefix("sha256:")
        .unwrap_or(subject_digest);
    format!("dependencies/{}/{}.spx", &digest[..32], &digest[32..])
}

fn failure(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J123", message.into()).with_help(
        "regenerate exact Subject-v3 files with `semaprax package-report-v2` and the package lock APIs, list the complete selected closure under `[dependency-sources]`, and keep every required target available",
    )]
}
