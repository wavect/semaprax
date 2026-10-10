//! V32 transport evidence is native; unchanged direct decoding owns all-backend parity.
use super::*;
use std::io::Write as _;
use std::process::Stdio;

const EXAMPLE: &str = "examples/stream-nested-order-json-project";
const READ: &str = "size_t count = fread(buffer, sizeof(uint8_t), (size_t)capacity, context->stream);";

fn install() -> std::path::PathBuf {
    let root = super::super::temporary("stream-nested-orders");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
    for path in ["semaprax.toml", "src/schema.spx", "src/app.spx"] {
        std::fs::copy(source.join(path), root.join(path)).unwrap();
    }
    let original = std::fs::read(root.join("src/schema.spx")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["json-codec", ".", "--source", "src/schema.spx", "--type", "orders.request",
            "--profile", "bounded-stream-nested-request.v1", "--max-string-bytes", "16",
            "--max-array-items", "8", "--output", "src/generated.spx"])
        .current_dir(&root).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(std::fs::read(root.join("src/schema.spx")).unwrap(), original);
    let generated = std::fs::read_to_string(root.join("src/generated.spx")).unwrap();
    assert_eq!(canonical(&generated), generated);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        project::verify_json_codec_source_with_profile(&revision, "src/schema.spx", "orders.request",
            &generated, project::JsonCodecProfile::StreamNestedRequest {
                max_string_bytes: 16, max_array_items: 8,
            })?;
        assert_eq!(project::verify_json_codec_source_with_profile(&revision, "src/schema.spx",
            "orders.request", &generated, project::JsonCodecProfile::NestedRequest {
                max_string_bytes: 16, max_array_items: 8,
            }).unwrap_err()[0].code, "SPX-J180");
        Ok(())
    }).unwrap();
    std::fs::copy(root.join("src/generated.spx"), root.join("src/schema.spx")).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&std::fs::read_to_string(
        source.join("src/app.command.spx")).unwrap())).unwrap();
    root
}

fn source(root: &std::path::Path) -> String {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let graph = revision.semantic_graph();
        assert!(graph.contains("orders.request.json.nested.stream-decode"));
        assert!(graph.contains("orders.request.json.stream.normalize"));
        codegen::emit_hir_c_with_stdin_stream_nested_outcomes(snapshot.public_api_program(),
            "orders.command").map(|c| {
                let sources = revision.sources().iter().map(|s| format!("{}={:x}", s.path(),
                    semaprax::digest_hex::LowerHex(Sha256::digest(s.source().as_bytes()))))
                    .collect::<Vec<_>>().join(",");
                eprintln!("stream-nested-audit project={} hir={} sources={} c={:x}",
                    revision.project_revision(), revision.semantic_graph_digest(), sources,
                    semaprax::digest_hex::LowerHex(Sha256::digest(c.as_bytes())));
                c
            }).map_err(|error| vec![error])
    }).unwrap()
}

fn compile(c: &str, root: &std::path::Path, label: &str, optimization: &str) -> std::path::PathBuf {
    let file = root.join(format!("{label}.c"));
    let binary = root.join(label);
    std::fs::write(&file, c).unwrap();
    let output = Command::new("clang").args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
        .arg(file).arg("-o").arg(&binary).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    binary
}

fn execute(binary: &std::path::Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary).stdin(Stdio::piped()).stdout(Stdio::piped())
        .stderr(Stdio::piped()).spawn().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let input = input.to_vec();
    let writer = std::thread::spawn(move || stdin.write_all(&input).unwrap());
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    output
}

fn assert_output(output: &std::process::Output, status: i32, stdout: &[u8], stderr: &[u8]) {
    assert_eq!(output.status.code(), Some(status), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, stdout);
    assert_eq!(output.stderr, stderr);
}

fn escaped_key(value: &str) -> String {
    format!("\"{}\"", value.bytes().map(|byte| format!("\\u{byte:04x}")).collect::<String>())
}

fn maximal_spelling() -> Vec<u8> {
    let text = format!("\"{}\"", "\\u0000".repeat(16));
    let configuration = format!("{{{}:{},{}:18446744073709551615}}",
        escaped_key("label"), text, escaped_key("retry"));
    assert_eq!(configuration.len(), 187);
    let row = format!("{{{}:{},{}:255}}", escaped_key("sku"), text, escaped_key("quantity"));
    assert_eq!(row.len(), 176);
    let array = format!("[{}]", std::iter::repeat_n(row, 8).collect::<Vec<_>>().join(","));
    assert_eq!(array.len(), 1417);
    let whole = format!("{{{}:{},{}:{},{}:false}}", escaped_key("configuration"), configuration,
        escaped_key("lines"), array, escaped_key("urgent"));
    assert_eq!(whole.len(), 1766); // Independent exact bound, not generator output.
    whole.into_bytes()
}

#[test]
fn streamed_nested_orders_publish_owned_values_after_unbounded_whitespace_and_full_escape_spelling() {
    let root = install();
    let c = source(&root);
    let example = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE).join("fixtures");
    let valid = std::fs::read(example.join("request.json")).unwrap();
    let padded = [vec![b' '; 70_000], valid.clone(), vec![b'\n'; 1_000]].concat();
    let maximal = maximal_spelling();
    let padded_maximal = [vec![b'\t'; 80_000], maximal.clone(), vec![b' '; 4097]].concat();
    for optimization in ["-O0", "-O2"] {
        let binary = compile(&c, &root, optimization, optimization);
        for input in [&valid, &padded] {
            assert_output(&execute(&binary, input), 0, b"3:3:true:2:6:6", b"");
        }
        for input in [&maximal, &padded_maximal] {
            assert_output(&execute(&binary, input), 0, b"16:18446744073709551615:false:8:2040:128", b"");
        }
        for (file, error) in [("duplicate.json", b"error:2:31:2".as_slice()),
            ("string-capacity.json", b"error:6:26:2"), ("array-capacity.json", b"error:8:251:4"),
            ("malformed.json", b"error:1:93:0")] {
            let input = std::fs::read(example.join(file)).unwrap();
            assert_output(&execute(&binary, &input), 2, b"", error);
        }
        assert_output(&execute(&binary, b"  {\"unknown\":0}"), 2, b"", b"error:4:1:0");
        // The envelope covers valid schemas. Invalid over-buffer content still
        // consumes/validates the full grammar before selecting capacity refusal.
        let invalid_oversize = [b"\"".as_slice(), &vec![b'a'; 131_072], b"\""].concat();
        assert_output(&execute(&binary, &invalid_oversize), 2, b"", b"error:9:131072:0");
        let late_fault = [invalid_oversize.as_slice(), b"x"].concat();
        assert_output(&execute(&binary, &late_fault), 2, b"", b"error:1:131074:0");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn streamed_nested_orders_observe_real_provider_splits_and_refuse_unauthorized_or_oversize_derivation() {
    let root = install();
    let c = source(&root);
    assert_eq!(c.matches(READ).count(), 1, "exact authenticated process provider site");
    let observed = c.replace(READ, &format!("{READ}\n    fprintf(stderr, \"observed-chunk:%zu\\n\", count);"));
    eprintln!("stream-nested-observation original={:x} instrumented={:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(c.as_bytes())),
        semaprax::digest_hex::LowerHex(Sha256::digest(observed.as_bytes())));
    let binary = compile(&observed, &root, "observed", "-O0");
    let fixture = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE)
        .join("fixtures/request.json")).unwrap();
    let unicode = String::from_utf8(fixture).unwrap().replace("\\u00e9\\u0000", "é\\u0000\\uD83D\\uDE00").into_bytes();
    for split in 0..=unicode.len() {
        let input = [vec![b' '; 4096 - split], unicode.clone()].concat();
        let mut output = execute(&binary, &input);
        let mut chunks = Vec::new();
        let mut diagnostics = Vec::new();
        for line in output.stderr.split_inclusive(|byte| *byte == b'\n') {
            if let Some(value) = line.strip_prefix(b"observed-chunk:") {
                chunks.push(std::str::from_utf8(value).unwrap().trim().parse::<usize>().unwrap());
            } else { diagnostics.extend_from_slice(line); }
        }
        let mut expected = vec![4096; input.len() / 4096];
        if input.len() % 4096 != 0 { expected.push(input.len() % 4096); }
        expected.push(0);
        assert_eq!(chunks, expected, "actual provider inventory at split {split}");
        output.stderr = diagnostics;
        assert_output(&output, 0, b"7:3:true:2:6:6", b"");
    }
    let bootstrap = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
    std::fs::copy(bootstrap.join("src/app.spx"), root.join("src/app.spx")).unwrap();
    let schema = std::fs::read_to_string(bootstrap.join("src/schema.spx")).unwrap();
    for (label, refused) in [
        ("no-permit", schema.replace("permit { process.stdin.read }", "")),
        ("wide-envelope", schema.replace("sku: string", &format!("{}: string", "a".repeat(64)))
            .replace("quantity: u8", &format!("{}: u8", "b".repeat(64)))),
    ] {
        std::fs::write(root.join("src/schema.spx"), &refused).unwrap();
        let result = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot|
            project::derive_json_codec_source_with_profile(&snapshot.retain_revision(), "src/schema.spx",
                "orders.request", project::JsonCodecProfile::StreamNestedRequest {
                    max_string_bytes: 64, max_array_items: 256,
                }));
        assert_eq!(result.unwrap_err()[0].code, "SPX-J180", "{label}");
        assert_eq!(std::fs::read_to_string(root.join("src/schema.spx")).unwrap(), refused);
    }
    std::fs::remove_dir_all(root).unwrap();
}
