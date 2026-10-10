//! Public bootstrap and native stdin execution for the streamed UTF-8 profile.
use super::*;
use super::public_example::{compile_native, execute};

const EXPECTED: &[u8] = r#"{"labels":["","\u0000","é","é"],"rows":[{"number":5,"text":"é\u0000😀"},{"number":4,"text":""},{"number":4,"text":""}]}"#.as_bytes();

fn copy_example(source: &std::path::Path, destination: &std::path::Path) {
    for directory in ["src", "fixtures"] {
        std::fs::create_dir_all(destination.join(directory)).unwrap();
    }
    for path in [
        "semaprax.toml",
        "src/schema.spx",
        "src/app.spx",
        "src/app.command.spx",
        "fixtures/request.json",
        "fixtures/malformed.json",
        "fixtures/wrong-type.json",
    ] {
        std::fs::copy(source.join(path), destination.join(path)).unwrap();
    }
}

#[test]
fn public_stream_utf8_owned_request_bootstraps_and_runs_native_stdin() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/stream-utf8-owned-json-project");
    let root = temporary("stream-utf8-owned-json-public-example");
    copy_example(&source, &root);
    let generated = root.join("src/schema.generated.spx");
    assert!(!generated.exists());
    let derive = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "json-codec",
            ".",
            "--source",
            "src/schema.spx",
            "--type",
            "catalog.request",
            "--output",
            "src/schema.generated.spx",
            "--profile",
            "stream-utf8-owned-request.v1",
            "--max-string-bytes",
            "64",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        derive.status.success(),
        "{}",
        String::from_utf8_lossy(&derive.stderr)
    );
    let generated_source = std::fs::read_to_string(&generated).unwrap();
    assert_eq!(canonical(&generated_source), generated_source);
    let profile = project::JsonCodecProfile::StreamUtf8OwnedRequest {
        max_string_bytes: 64,
    };
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::verify_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "catalog.request",
            &generated_source,
            profile,
        )
    })
    .unwrap();
    std::fs::copy(&generated, root.join("src/schema.spx")).unwrap();
    std::fs::copy(root.join("src/app.command.spx"), root.join("src/app.spx")).unwrap();

    let c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let graph = snapshot.retain_revision().semantic_graph();
        assert!(graph.contains("catalog.request.json.stream.normalize"));
        assert!(graph.contains("catalog.request.json.utf8.owned.decode"));
        assert!(graph.contains("catalog.request.json.utf8.owned.encode"));
        codegen::emit_hir_c_with_stdin_stream_owned_data(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();

    let valid = std::fs::read(root.join("fixtures/request.json")).unwrap();
    let malformed = std::fs::read(root.join("fixtures/malformed.json")).unwrap();
    let wrong_type = std::fs::read(root.join("fixtures/wrong-type.json")).unwrap();
    let padded = [vec![b' '; 70_000], valid.clone(), vec![b'\n'; 1_000]].concat();
    for optimization in ["-O0", "-O2"] {
        let binary = compile_native(&c, &root, optimization);
        for input in [&valid, &padded] {
            let output = execute(&binary, input);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, EXPECTED);
            assert!(output.stderr.is_empty());
        }
        for input in [&malformed, &wrong_type] {
            let output = execute(&binary, input);
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert!(output.stderr.is_empty());
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
