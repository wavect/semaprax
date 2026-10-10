//! The committed v30 example bootstraps through the public CLI and ordinary files.
use super::*;
use std::io::Write as _;
use std::process::Stdio;

const EXPECTED: &[u8] = br#"{"locations":["NORTH","SOUTH"],"stock":[{"sku":"CRATE-1","quantity":7,"available":false},{"sku":"CRATE-2","quantity":4,"available":true}]}"#;

fn copy_example(source: &std::path::Path, destination: &std::path::Path) {
    std::fs::create_dir_all(destination.join("src")).unwrap();
    std::fs::create_dir_all(destination.join("fixtures")).unwrap();
    for path in [
        "semaprax.toml",
        "src/schema.spx",
        "src/app.spx",
        "src/app.command.spx",
        "src/tests.spx",
        "fixtures/request.json",
        "fixtures/malformed.json",
    ] {
        std::fs::copy(source.join(path), destination.join(path)).unwrap();
    }
}

fn compile_native(
    source: &str,
    directory: &std::path::Path,
    optimization: &str,
) -> std::path::PathBuf {
    let suffix = optimization.trim_start_matches("-O");
    let source_path = directory.join(format!("owned-json-{suffix}.c"));
    let binary = directory.join(format!("owned-json-{suffix}"));
    std::fs::write(&source_path, source).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
        .arg(&source_path)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

fn execute(binary: &std::path::Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input).unwrap());
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    output
}

#[test]
fn public_owned_json_command_bootstraps_and_runs_valid_and_malformed_stdin() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/owned-json-command-project");
    let root = temporary("owned-json-public-example");
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
            "warehouse.request",
            "--output",
            "src/schema.generated.spx",
            "--profile",
            "stream-owned-request.v1",
        ])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        derive.status.success(),
        "{}",
        String::from_utf8_lossy(&derive.stderr)
    );
    assert!(generated.is_file());
    let generated_source = std::fs::read_to_string(&generated).unwrap();
    assert_eq!(canonical(&generated_source), generated_source);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::verify_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "warehouse.request",
            &generated_source,
            project::JsonCodecProfile::StreamOwnedRequest,
        )
    })
    .unwrap();
    std::fs::copy(&generated, root.join("src/schema.spx")).unwrap();
    std::fs::copy(root.join("src/app.command.spx"), root.join("src/app.spx")).unwrap();

    let c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        assert!(revision
            .semantic_graph()
            .contains("warehouse.request.json.owned.decode"));
        assert!(revision.semantic_graph().contains("core.vec.sort-owned"));
        codegen::emit_hir_c_with_stdin_stream_owned_data(
            snapshot.public_api_program(),
            "warehouse.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    let valid = std::fs::read(root.join("fixtures/request.json")).unwrap();
    let malformed = std::fs::read(root.join("fixtures/malformed.json")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = compile_native(&c, &root, optimization);
        let output = execute(&binary, &valid);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, EXPECTED);
        assert!(output.stderr.is_empty());

        let output = execute(&binary, &malformed);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}
