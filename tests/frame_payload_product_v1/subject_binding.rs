//! Actual held-Project subjects and independent published-artifact observations.
//! Native manifest checks below are test-specific, not a public package verifier.
use super::*;
use semaprax::project::{replay_public_api_descriptor, ProjectRevision, PublicApiDescriptor};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::Arc;

pub(super) struct BoundProduct {
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

fn read_inventory(directory: &Path, names: &[&str]) -> BTreeMap<String, Vec<u8>> {
    let mut actual = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = names.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{}", directory.display());
    names
        .iter()
        .map(|name| {
            let path = directory.join(name);
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(metadata.is_file() && !metadata.file_type().is_symlink());
            assert!(metadata.len() <= 16 * 1024 * 1024);
            ((*name).to_owned(), fs::read(path).unwrap())
        })
        .collect()
}

fn hash(bytes: &[u8]) -> String {
    let hex = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn quote(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

fn target() -> (&'static str, &'static str) {
    let triple = match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        other => panic!("unsupported physical SDK fixture target: {other:?}"),
    };
    let archive = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    (triple, archive)
}

fn verify_native_inventory(
    root: &Path,
    descriptor: &PublicApiDescriptor,
    provider: &[u8],
) -> Vec<u8> {
    let (triple, archive) = target();
    let manifest_name = "semaprax.native-rust-owned-data-sdk.json";
    let files = read_inventory(
        root,
        &[
            "Cargo.toml",
            "build.rs",
            "lib.rs",
            "owned_data_ffi.rs",
            "descriptor.json",
            archive,
            manifest_name,
        ],
    );
    let descriptor_bytes = descriptor.canonical_bytes();
    assert_eq!(files["descriptor.json"], descriptor_bytes);
    let rows = files
        .iter()
        .filter(|(path, _)| path.as_str() != manifest_name)
        .map(|(path, bytes)| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{}}}",
                quote(path),
                bytes.len(),
                quote(&hash(bytes)),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    // Derive every expected field from the real subject, actual compiler
    // provider, fixed package contract and reopened bytes—not manifest claims.
    // Exact raw equality rejects unknown/duplicate fields and noncanonical JSON.
    let expected = format!(
        "{{\"schema\":\"semaprax.native-rust-owned-data-sdk.v1\",\"crate\":{{\"name\":\"semaprax-generated-native-rust-owned-data-sdk\",\"version\":\"0.1.0\"}},\"target\":{},\"descriptor\":{{\"schema\":\"semaprax.public-owned-data-api.v1\",\"bytes\":{},\"digest\":{}}},\"provider\":{{\"abi\":\"opaque-handle.v1\",\"archive\":{},\"descriptor_digest\":{},\"source_sha256\":{},\"operations\":[\"len\",\"copy\",\"drop\"]}},\"files\":[{rows}],\"limits\":{{\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_handles\":4096,\"exact_package_files\":7}},\"nonclaims\":[\"no_raw_handle_or_context_public_api\",\"no_allocator_transfer\",\"no_allocator_oom_abort_or_panic_recovery_proof\",\"no_send_sync\"]}}\n",
        quote(triple), descriptor_bytes.len(), quote(&descriptor.digest()),
        quote(archive), quote(&descriptor.digest()), quote(&hash(provider)),
    );
    assert_eq!(files[manifest_name], expected.as_bytes());
    files["descriptor.json"].clone()
}

pub(super) fn verify_product(project: &Path, npm: &Path, rust: &Path) -> BoundProduct {
    let revision =
        semaprax::project::with_authenticated_project(&project.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();
    let descriptor = revision.public_api_descriptor().unwrap();
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
        revision.entry_program(),
        selected,
        subject(&revision),
        npm_descriptor,
        metadata["descriptor_digest"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(replayed, descriptor);
    let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
        revision.entry_program(),
        selected,
        subject(&revision),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    assert_eq!(provider.descriptor(), npm_descriptor);
    assert_eq!(provider.descriptor_digest(), descriptor.digest());
    let rust_descriptor = verify_native_inventory(rust, &descriptor, provider.source().as_bytes());
    assert_eq!(rust_descriptor, npm_descriptor);
    assert_eq!(
        replay_public_api_descriptor(
            revision.entry_program(),
            selected,
            subject(&revision),
            &rust_descriptor,
            &descriptor.digest(),
        )
        .unwrap(),
        descriptor
    );

    // These lanes now execute the actual linked Project subject, including its
    // source origins, not just an isolated module with invented revision facts.
    assert_interpreter_corpus(revision.entry_program());
    assert_native_corpus(provider.source(), "bound-project-native");
    raw_wasm::run(npm, &descriptor);
    BoundProduct {
        revision,
        descriptor,
    }
}

pub(super) fn verify_display_rename(before: &BoundProduct, after: &BoundProduct) {
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
            authority.revision.entry_program(),
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
