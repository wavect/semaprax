//! Actual held-Project subjects and independent published-artifact observations.
//! Native manifest checks below are test-specific, not a public package verifier.
use semaprax::project::{replay_public_api_descriptor, ProjectRevision, PublicApiDescriptor};
use semaprax::project::{ProjectNpmBuild, PublicApiSubject};
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[path = "owned_frame_artifacts/native_inventory.rs"]
mod native_inventory;
use native_inventory::{read_inventory, verify_native_inventory};

pub(crate) struct BoundProduct {
    revision: Arc<ProjectRevision>,
    descriptor: PublicApiDescriptor,
}

fn subject(revision: &ProjectRevision) -> PublicApiSubject<'_> {
    PublicApiSubject {
        project_schema: revision.manifest().schema(),
        project_revision: revision.project_revision(),
        workspace_revision: revision.workspace_revision(),
        project_graph_digest: revision.semantic_graph_digest(),
    }
}

pub(crate) fn retain(project: &Path) -> BoundProduct {
    let revision =
        semaprax::project::with_authenticated_project(&project.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    let descriptor = revision.public_api_descriptor().unwrap();
    BoundProduct {
        revision,
        descriptor,
    }
}

pub(crate) fn native_provider(
    product: &BoundProduct,
) -> semaprax::codegen::NativeOwnedDataProviderArtifact {
    let revision = &product.revision;
    let descriptor = &product.descriptor;
    let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
        revision.public_api_program(),
        revision.manifest().web_exports(),
        subject(revision),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    assert_eq!(provider.descriptor(), descriptor.canonical_bytes());
    assert_eq!(provider.descriptor_digest(), descriptor.digest());
    provider
}

pub(crate) fn verify_artifacts(project: &Path, npm: &Path, rust: &Path) -> BoundProduct {
    let product = retain(project);
    let revision = &product.revision;
    let descriptor = &product.descriptor;
    let selected = revision.manifest().web_exports();
    let expected_npm = revision.build_npm_inline(40 * 1024 * 1024).unwrap();
    expected_npm.verify().unwrap();
    ProjectNpmBuild::inspect_envelope(expected_npm.envelope(), expected_npm.max_bytes()).unwrap();
    let expected_artifacts = artifacts(&expected_npm);
    let names = expected_artifacts
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<Vec<_>>();
    let actual_npm = read_inventory(npm, &names);
    for (name, bytes) in expected_artifacts {
        assert_eq!(actual_npm[&name], bytes, "reopened npm {name}");
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&actual_npm["semaprax.api.json"]).unwrap();
    let npm_descriptor = metadata["descriptor"].as_str().unwrap().as_bytes();
    let replayed = replay_public_api_descriptor(
        revision.public_api_program(),
        selected,
        subject(revision),
        npm_descriptor,
        metadata["descriptor_digest"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(&replayed, descriptor);
    let provider = native_provider(&product);
    assert_eq!(provider.descriptor(), npm_descriptor);
    assert_eq!(provider.descriptor_digest(), descriptor.digest());
    let rust_descriptor = verify_native_inventory(rust, descriptor, provider.source().as_bytes());
    assert_eq!(rust_descriptor, npm_descriptor);
    assert_eq!(
        &replay_public_api_descriptor(
            revision.public_api_program(),
            selected,
            subject(revision),
            &rust_descriptor,
            &descriptor.digest(),
        )
        .unwrap(),
        descriptor
    );

    product
}

pub(crate) fn verify_display_rename(before: &BoundProduct, after: &BoundProduct) {
    assert_eq!(
        before.revision.manifest().to_canonical_toml(),
        after.revision.manifest().to_canonical_toml()
    );
    assert_eq!(
        before.revision.sources().len(),
        after.revision.sources().len()
    );
    let mut renamed = 0;
    for (old, new) in before
        .revision
        .sources()
        .iter()
        .zip(after.revision.sources())
    {
        assert_eq!(old.path(), new.path());
        if old.path() == "src/frame.spx" {
            assert_eq!(old.source().matches("fn payload_result(").count(), 1);
            assert_eq!(
                new.source(),
                old.source()
                    .replace("fn payload_result(", "fn decoded_payload_result(")
            );
            renamed += 1;
        } else {
            assert_eq!(old.source(), new.source());
            assert_eq!(old.source_digest(), new.source_digest());
        }
    }
    assert_eq!(renamed, 1);
    assert_eq!(before.descriptor.exports(), after.descriptor.exports());
    assert_ne!(
        before.descriptor.project_revision(),
        after.descriptor.project_revision()
    );
    assert_ne!(
        before.descriptor.workspace_revision(),
        after.descriptor.workspace_revision()
    );
    assert_ne!(
        before.descriptor.project_graph_digest(),
        after.descriptor.project_graph_digest()
    );
    let mut old: serde_json::Value =
        serde_json::from_slice(&before.descriptor.canonical_bytes()).unwrap();
    let mut new: serde_json::Value =
        serde_json::from_slice(&after.descriptor.canonical_bytes()).unwrap();
    for key in [
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ] {
        assert!(old.as_object_mut().unwrap().remove(key).is_some());
        assert!(new.as_object_mut().unwrap().remove(key).is_some());
    }
    assert_eq!(old, new, "only the three revision bindings may change");
    for (authority, foreign) in [(before, after), (after, before)] {
        let error = replay_public_api_descriptor(
            authority.revision.public_api_program(),
            authority.revision.manifest().web_exports(),
            subject(&authority.revision),
            &foreign.descriptor.canonical_bytes(),
            &foreign.descriptor.digest(),
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-J113");
        assert!(error.message.contains("retained subject"));
    }
}

impl BoundProduct {
    pub(crate) fn revision(&self) -> &ProjectRevision {
        &self.revision
    }
    pub(crate) fn descriptor(&self) -> &PublicApiDescriptor {
        &self.descriptor
    }
}

fn artifacts(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    envelope["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}
