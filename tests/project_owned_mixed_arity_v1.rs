//! Authored same-Project mixed-signature evidence, not executed support claims.
#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

use semaprax::project::{
    with_authenticated_project, ProjectNpmBuild, PublicApiSubject, MAX_PROJECT_NPM_BUILD_BYTES,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "support/owned_npm_publication.rs"]
mod owned_npm_publication;
#[path = "support/owned_mixed_arity_product.rs"]
mod subject;

const FILES: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];

fn native(root: &Path, provider: &str) {
    let source = format!(
        "{}\n{}\n{}\n{}",
        include_str!("support/native_fixture_stdio.c"),
        include_str!("native_owned_tuple_admission_v1/allocations.c"),
        provider,
        include_str!("project_owned_mixed_arity_v1/native.c"),
    );
    let path = root.join("native.c");
    fs::write(&path, &source).unwrap();
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "native{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(&compiler)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for mixed-arity native evidence");
        assert!(
            compiled.status.success(),
            "{optimization}: {}\n{}",
            String::from_utf8_lossy(&compiled.stdout),
            String::from_utf8_lossy(&compiled.stderr)
        );
        let observed = Command::new(&executable).output().unwrap();
        assert!(
            observed.status.success(),
            "{optimization}: {}\n{}",
            String::from_utf8_lossy(&observed.stdout),
            String::from_utf8_lossy(&observed.stderr)
        );
        assert_eq!(observed.stdout, b"mixed-arity-native-ok\n");
        assert!(observed.stderr.is_empty());
    }
    assert_eq!(fs::read(path).unwrap(), source.as_bytes());
}

fn plain_file(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;
        assert_eq!(metadata.file_attributes() & 0x400, 0);
    }
}

#[test]
fn real_project_zero_through_eight_mixed_arguments_match_native_and_npm() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-mixed-arity-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained mixed-arity fixture: {}", root.display());
    let manifest = subject::write_project(&root, 8);
    let sources = ["semaprax.toml", "src/app.spx", "src/tests.spx"]
        .map(|name| (name, fs::read(root.join(name)).unwrap()));
    let (provider, artifacts) = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let descriptor = revision.public_api_descriptor()?;
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        assert_eq!(descriptor.exports().len(), 9);
        let expected_types = [
            "i64",
            "bool",
            "borrow-str",
            "borrow-slice-u8",
            "i64",
            "bool",
            "borrow-str",
            "borrow-slice-u8",
        ];
        for (arity, export) in descriptor.exports().iter().enumerate() {
            assert_eq!(export.stable_id().as_str(), format!("mixed.arity{arity}"));
            assert_eq!(export.result().wire_name(), "owned-bytes");
            assert_eq!(
                export
                    .parameters()
                    .iter()
                    .map(|parameter| parameter.ty().wire_name())
                    .collect::<Vec<_>>(),
                expected_types[..arity]
            );
        }
        let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
            revision.entry_program(),
            revision.manifest().web_exports(),
            PublicApiSubject {
                project_schema: revision.manifest().schema(),
                project_revision: revision.project_revision(),
                workspace_revision: revision.workspace_revision(),
                project_graph_digest: revision.semantic_graph_digest(),
            },
            &bytes,
            &digest,
        )
        .map_err(|error| vec![error])?;
        assert_eq!(provider.descriptor(), bytes);
        assert_eq!(provider.descriptor_digest(), digest);
        let inline = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        inline.verify().unwrap();
        ProjectNpmBuild::inspect_envelope(inline.envelope(), MAX_PROJECT_NPM_BUILD_BYTES).unwrap();
        owned_npm_publication::publish(snapshot, &manifest, &root.join("package"), false)?;
        let envelope: serde_json::Value = serde_json::from_str(inline.envelope()).unwrap();
        assert_eq!(envelope["schema"], "semaprax.project-npm-build.v7");
        let mut actual = fs::read_dir(root.join("package"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = FILES;
        expected.sort();
        assert_eq!(actual, expected);
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), FILES.len());
        let mut retained = Vec::new();
        for (row, name) in rows.iter().zip(FILES) {
            assert_eq!(row["path"], name);
            let hex = row["hex"].as_str().unwrap();
            assert_eq!(hex.len() % 2, 0);
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect::<Vec<_>>();
            let path = root.join("package").join(name);
            plain_file(&path);
            assert_eq!(fs::read(&path).unwrap(), bytes);
            retained.push((path, bytes));
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("package/semaprax.api.json")).unwrap())
                .unwrap();
        assert_eq!(metadata["descriptor"].as_str().unwrap().as_bytes(), bytes);
        assert_eq!(metadata["descriptor_digest"], digest);
        Ok((provider, retained))
    })
    .unwrap();
    fs::write(root.join("provider.c"), provider.source()).unwrap();
    fs::write(root.join("descriptor.json"), provider.descriptor()).unwrap();
    native(&root, provider.source());
    let consumer = include_bytes!("project_owned_mixed_arity_v1/consumer.mjs");
    fs::write(root.join("consumer.mjs"), consumer).unwrap();
    let node = std::env::var_os("NODE").map_or_else(|| PathBuf::from("node"), PathBuf::from);
    let output = Command::new(node)
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node is required for mixed-arity npm evidence");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"mixed-arity-npm-ok\n");
    assert!(output.stderr.is_empty());
    for (path, bytes) in artifacts {
        plain_file(&path);
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
    for (name, bytes) in sources {
        assert_eq!(fs::read(root.join(name)).unwrap(), bytes);
    }
    assert_eq!(
        fs::read(root.join("provider.c")).unwrap(),
        provider.source().as_bytes()
    );
    assert_eq!(
        fs::read(root.join("descriptor.json")).unwrap(),
        provider.descriptor()
    );
    assert_eq!(fs::read(root.join("consumer.mjs")).unwrap(), consumer);
    // Retain the bounded evidence tree; no recursive cleanup or runtime proof claim.
}
