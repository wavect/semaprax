//! Authored same-source interpreter/native/npm Result extrema equivalence.
use semaprax::interpreter::{
    evaluate_resolved_owned_data, OwnedDataCleanupEvent, OwnedDataEvaluationOutcome,
    OwnedDataValue, DEFAULT_MAX_STEPS,
};
use semaprax::project::{
    with_authenticated_project, PublicApiSubject, MAX_PROJECT_NPM_BUILD_BYTES,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::owned_npm_publication;
use crate::owned_result_product as subject;

fn native(root: &Path, provider: &str) {
    let source = format!(
        "{}\n{}\n{}\n{}",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_tuple_admission_v1/allocations.c"),
        provider,
        include_str!("../project_owned_result_extrema_v1/native.c")
    );
    let path = root.join("native.c");
    fs::write(&path, &source).unwrap();
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "native{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let output = Command::new(&compiler)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang required for Result extrema evidence");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(executable).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    assert_eq!(fs::read(path).unwrap(), source.as_bytes());
}

#[test]
fn same_source_result_extrema_match_interpreter_native_and_npm() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-result-extrema-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained Result extrema fixture: {}", root.display());
    let manifest = subject::write_project(&root);
    let sources = ["semaprax.toml", "src/app.spx", "src/tests.spx"]
        .map(|name| (name, fs::read(root.join(name)).unwrap()));
    let provider = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let program = revision.public_api_program();
        let input = [0, 255, 128, 65, 0];
        for length in [0, 1, 2, 3, 4, 5, 2, 1, 0, 3] {
            let evaluation = evaluate_resolved_owned_data(
                program,
                "result.value",
                &input[..length],
                DEFAULT_MAX_STEPS,
            )?;
            if length == 4 {
                let OwnedDataEvaluationOutcome::LanguageFailure(failure) = evaluation.outcome
                else {
                    panic!("expected checked division failure")
                };
                assert_eq!(failure.domain_id(), "semaprax.arithmetic.v1");
                assert_eq!(failure.code(), 4);
                assert!(evaluation.cleanup_events.is_empty());
            } else {
                let expected = match length {
                    0 => Err(0),
                    1 => Err(i64::MIN),
                    2 => Err(i64::MAX),
                    _ => Ok(input[..length].to_vec()),
                };
                assert_eq!(
                    evaluation.outcome,
                    OwnedDataEvaluationOutcome::Returned(OwnedDataValue::ResultBytesI64(expected))
                );
                assert_eq!(
                    evaluation.cleanup_events,
                    if length < 3 {
                        vec![]
                    } else {
                        vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
                    }
                );
            }
        }
        let descriptor = revision.public_api_descriptor()?;
        let bytes = descriptor.canonical_bytes();
        let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
            program,
            revision.manifest().web_exports(),
            PublicApiSubject {
                project_schema: revision.manifest().schema(),
                project_revision: revision.project_revision(),
                workspace_revision: revision.workspace_revision(),
                project_graph_digest: revision.semantic_graph_digest(),
            },
            &bytes,
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])?;
        assert_eq!(provider.descriptor(), bytes);
        assert_eq!(provider.descriptor_digest(), descriptor.digest());
        let inline = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        inline.verify().unwrap();
        owned_npm_publication::publish(snapshot, &manifest, &root.join("package"), false)?;
        let envelope: serde_json::Value = serde_json::from_str(inline.envelope()).unwrap();
        let files = [
            "app.wasm",
            "semaprax.js",
            "semaprax.bindings.js",
            "semaprax.bindings.d.ts",
            "semaprax.api.json",
            "package.json",
        ];
        let mut actual = fs::read_dir(root.join("package"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        actual.sort();
        let mut expected = files;
        expected.sort();
        assert_eq!(actual, expected);
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), 6);
        for (row, name) in rows.iter().zip(files) {
            assert_eq!(row["path"], name);
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(fs::read(root.join("package").join(name)).unwrap(), bytes);
        }
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join("package/semaprax.api.json")).unwrap())
                .unwrap();
        assert_eq!(
            metadata["descriptor"].as_str().unwrap().as_bytes(),
            provider.descriptor()
        );
        Ok(provider)
    })
    .unwrap();
    native(&root, provider.source());
    fs::write(
        root.join("consumer.mjs"),
        include_bytes!("../project_owned_result_extrema_v1/consumer.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node required for Result extrema evidence");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"result-extrema-ok\n");
    assert!(output.stderr.is_empty());
    for (name, bytes) in sources {
        assert_eq!(fs::read(root.join(name)).unwrap(), bytes);
    }
    // Preserve the bounded fixture and compiler artifacts; no recursive cleanup.
}
