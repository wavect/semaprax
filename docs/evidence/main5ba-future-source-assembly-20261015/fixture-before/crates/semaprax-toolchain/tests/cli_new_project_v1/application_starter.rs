//! Full fresh-tree route; retained acceptance bodies stay independent and unchanged.
use super::*;
use std::io::Write;
use std::process::Stdio;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repository fixture root")
}
fn install(
    root: &Path,
    original: &Path,
    plan: &Path,
    profile: &str,
    activation: Option<&Path>,
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax-full"));
    command
        .arg("new")
        .arg(root)
        .arg("--from")
        .arg(original)
        .arg("--codec-plan")
        .arg(plan)
        .arg("--command-profile")
        .arg(profile);
    if let Some(activation) = activation {
        command
            .arg("--command-path")
            .arg("src/app.spx")
            .arg("--command-source")
            .arg(activation);
    }
    command.output().unwrap()
}

fn assert_legacy_native_json_refusal(root: &Path) {
    let output = cli(root, &["test", ".", "--target", "native", "--json"]);
    assert!(!output.status.success());
    assert!(output.stderr.is_empty(), "{:?}", output);
    let diagnostics = serde_json::Deserializer::from_slice(&output.stdout)
        .into_iter::<serde_json::Value>()
        .collect::<Result<Vec<_>, _>>()
        .expect("native refusal emits complete JSON diagnostics on stdout");
    assert_eq!(
        diagnostics.first().map(|value| &value["code"]),
        Some(&serde_json::json!("SPX-B104"))
    );
}
fn assert_roundtrip(root: &Path) {
    semaprax::project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(
            snapshot.manifest().to_canonical_toml(),
            std::fs::read_to_string(root.join("semaprax.toml")).unwrap()
        );
        for source in snapshot
            .sources()
            .iter()
            .filter(|s| snapshot.manifest().sources().iter().any(|p| p == s.path()))
        {
            let parsed = semaprax::parse(source.source(), source.path()).unwrap();
            assert_eq!(semaprax::format::canonical(&parsed), source.source());
        }
        let text = std::fs::read_to_string(root.join("README.md")).unwrap();
        let receipt: serde_json::Value = serde_json::from_str(
            text.split("```json\n")
                .nth(1)
                .unwrap()
                .split("\n```")
                .next()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["project_revision"], snapshot.project_revision());
        assert_eq!(receipt["workspace_revision"], snapshot.workspace_revision());
        assert_eq!(
            receipt["semantic_graph_digest"],
            snapshot.retain_revision().semantic_graph_digest()
        );
        assert_eq!(
            receipt["manifest_sha256"],
            sha(&std::fs::read(root.join("semaprax.toml")).unwrap())
        );
        assert!(
            receipt["codec_bundle"]["authored_sources"]
                .as_array()
                .unwrap()
                .len()
                >= snapshot.manifest().sources().len()
        );
        assert_eq!(
            receipt["sources"].as_array().unwrap().len(),
            snapshot.manifest().sources().len()
        );
        for source in receipt["sources"].as_array().unwrap() {
            assert_eq!(
                source["sha256"],
                sha(&std::fs::read(root.join(source["path"].as_str().unwrap())).unwrap())
            );
        }
        snapshot.check()
    })
    .unwrap();
}
fn sha(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(sha2::Sha256::digest(bytes))
    )
}
fn execute(binary: &Path, input: &[u8]) -> Output {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

// Preserve the original wire spelling control (including escaped keys/identifiers)
// instead of replacing all cases with serde's sorted object-key rendering.
fn wire(value: &serde_json::Value, field: Option<&str>, escaped: bool) -> String {
    match value {
        serde_json::Value::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(key, value)| format!(
                    "{}:{}",
                    if escaped {
                        escape(key)
                    } else {
                        serde_json::to_string(key).unwrap()
                    },
                    wire(value, Some(key), escaped)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        serde_json::Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| wire(item, field, escaped))
                .collect::<Vec<_>>()
                .join(",")
        ),
        serde_json::Value::String(text) if escaped && matches!(field, Some("id" | "servers")) => {
            escape(text)
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}
fn escape(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .bytes()
            .map(|byte| format!("\\u{byte:04x}"))
            .collect::<String>()
    )
}

fn expected_wire(value: &serde_json::Value) -> String {
    let assignments = value["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            format!(
                "{{\"id\":{},\"server\":{},\"arrival\":{},\"start\":{},\"finish\":{},\"wait\":{},\"late\":{}}}",
                serde_json::to_string(item["id"].as_str().unwrap()).unwrap(),
                serde_json::to_string(item["server"].as_str().unwrap()).unwrap(),
                item["arrival"], item["start"], item["finish"], item["wait"], item["late"]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let metrics = &value["metrics"];
    format!(
        "{{\"assignments\":[{assignments}],\"metrics\":{{\"patients\":{},\"total_wait\":{},\"max_wait\":{},\"late\":{},\"busy_time\":{},\"makespan\":{},\"peak_queue\":{},\"utilization_ppm\":{}}}}}\n",
        metrics["patients"],
        metrics["total_wait"],
        metrics["max_wait"],
        metrics["late"],
        metrics["busy_time"],
        metrics["makespan"],
        metrics["peak_queue"],
        metrics["utilization_ppm"]
    )
}

#[test]
fn application_starter_typed_shiftsim_fresh_check_native_tests_and_original_fifteen() {
    let fixture = Fixture::new("application-typed");
    let source = repo().join("examples/shiftsim-typed-record-successor");
    let raw_before = read_tree(&source);
    let raw_output = fixture.root.join("raw-application");
    let noncanonical = install(
        &raw_output,
        &source,
        &source.join("codec-plan.json"),
        "language-command-io.collection-record.v1",
        Some(&source.join("src/app.command.spx")),
    );
    assert!(!noncanonical.status.success());
    assert!(
        String::from_utf8_lossy(&noncanonical.stderr).contains("SPX-G170"),
        "{:?}",
        noncanonical
    );
    assert!(!raw_output.exists());
    assert_eq!(read_tree(&source), raw_before);
    // Preserve every original fixture byte, then use the compiler's actual
    // parser/formatter for the successful authored inventory (including command).
    let authored = fixture.root.join("authored");
    for (relative, bytes) in &raw_before {
        let path = authored.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes).unwrap();
        if relative.ends_with(".spx") {
            let text = std::str::from_utf8(bytes).unwrap();
            let parsed = semaprax::parse(text, relative).unwrap();
            std::fs::write(&path, semaprax::format::canonical(&parsed)).unwrap();
        }
    }
    assert_eq!(read_tree(&source), raw_before);
    let source = authored;
    let before = read_tree(&source);
    let output = fixture.root.join("application");
    assert_success(&install(
        &output,
        &source,
        &source.join("codec-plan.json"),
        "language-command-io.collection-record.v1",
        Some(&source.join("src/app.command.spx")),
    ));
    assert_eq!(
        read_tree(&source),
        before,
        "installer must not modify authored inputs"
    );
    assert_roundtrip(&output);
    let repeated = fixture.root.join("repeated");
    assert_success(&install(
        &repeated,
        &source,
        &source.join("codec-plan.json"),
        "language-command-io.collection-record.v1",
        Some(&source.join("src/app.command.spx")),
    ));
    assert_eq!(
        read_tree(&output),
        read_tree(&repeated),
        "fresh destination spelling must not alter complete bytes or receipts"
    );
    assert_success(&cli(&output, &["check", "."]));
    // Frozen route remains refused; the new typed pure route is explicit.
    let legacy = cli(&output, &["test", ".", "--target", "native"]);
    assert!(!legacy.status.success());
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("SPX-B104"));
    assert_legacy_native_json_refusal(&output);
    let tests = cli(
        &output,
        &[
            "test",
            ".",
            "--target",
            "native-collection-record-pure.v1",
            "--json",
        ],
    );
    assert_success(&tests);
    let result: serde_json::Value = serde_json::from_slice(&tests.stdout).unwrap();
    assert_eq!(
        result["schema"],
        "semaprax.native-test.collection-record-pure.v1"
    );
    assert_eq!(result["passed"], true);
    assert_eq!(result["cases"].as_array().unwrap().len(), 3);
    assert_success(&cli(
        &output,
        &["build", ".", "--target", "native", "-o", "app"],
    ));
    let corpus: serde_json::Value = serde_json::from_slice(
        &std::fs::read(repo().join("benchmarks/event-sim-tokens-v1/acceptance/corpus.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        corpus["valid"].as_array().unwrap().len() + corpus["invalid"].as_array().unwrap().len(),
        15
    );
    for case in corpus["valid"].as_array().unwrap() {
        let mut input = vec![b' '; case["leading_whitespace_bytes"].as_u64().unwrap_or(0) as usize];
        input.extend_from_slice(
            wire(
                &case["input"],
                None,
                case["request_encoding"] == "unicode-escaped-keys-and-identifiers-v1",
            )
            .as_bytes(),
        );
        let result = execute(&output.join("app"), &input);
        assert_success(&result);
        assert!(result.stderr.is_empty());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
            case["expected"],
            "{}",
            case["name"]
        );
        assert_eq!(
            result.stdout,
            expected_wire(&case["expected"]).as_bytes(),
            "{}",
            case["name"]
        );
    }
    for case in corpus["invalid"].as_array().unwrap() {
        let input = case
            .get("raw_input")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .unwrap_or_else(|| wire(&case["input"], None, false));
        let result = execute(&output.join("app"), input.as_bytes());
        assert_eq!(result.status.code(), Some(2), "{}", case["name"]);
        assert!(result.stdout.is_empty());
        assert_eq!(String::from_utf8_lossy(&result.stderr).lines().count(), 1);
    }
    assert!(!install(
        &output,
        &source,
        &source.join("codec-plan.json"),
        "language-command-io.collection-record.v1",
        Some(&source.join("src/app.command.spx"))
    )
    .status
    .success());
    assert_eq!(read_tree(&source), before);
}

#[cfg(unix)]
#[test]
fn application_starter_selected_file_declared_native_tests_and_unchanged_cli49() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new("application-selected-file");
    let authored = fixture.root.join("authored");
    std::fs::create_dir_all(authored.join("src")).unwrap();
    let reference = repo().join("experiments/selected-file-input/loglens");
    let manifest = std::fs::read_to_string(reference.join("semaprax.toml")).unwrap()
        .replace("\"loglens.spx\", \"unit.spx\"", "\"src/app.spx\", \"src/schema.spx\", \"src/tests.spx\"")
        .replace("[targets]", "[dependencies]\nstd.data.json.digits = \"=0.1.0\"\nstd.data.json.query = \"=0.1.0\"\nstd.data.json.scan = \"=0.1.0\"\nstd.data.json.token = \"=0.1.0\"\nstd.data.json.write = \"=0.1.0\"\n\n[targets]");
    std::fs::write(authored.join("semaprax.toml"), manifest).unwrap();
    std::fs::copy(reference.join("loglens.spx"), authored.join("src/app.spx")).unwrap();
    std::fs::copy(reference.join("unit.spx"), authored.join("src/tests.spx")).unwrap();
    std::fs::write(authored.join("src/schema.spx"), "module loglens.schema;\n@id(\"starter.row\") record Row { @id(\"starter.row.value\") value:i64, }\n@id(\"starter.anchor\") fn anchor()->i64{0}\n").unwrap();
    let plan = fixture.root.join("plan.json");
    std::fs::write(&plan, r#"{"schema":"semaprax.json-codec-plan.v1","entries":[{"source":"src/schema.spx","record":"starter.row","profile":"flat-scalars.v1"}]}"#).unwrap();
    let output = fixture.root.join("application");
    // Retain the original noncanonical input as an explicit negative case,
    // then provide canonical authored source to the successful installer.
    let before = read_tree(&authored);
    let noncanonical = install(
        &output,
        &authored,
        &plan,
        "source-command.selected-file.resource-output.v1",
        None,
    );
    assert!(!noncanonical.status.success());
    assert!(
        String::from_utf8_lossy(&noncanonical.stderr).contains("SPX-G170"),
        "{:?}",
        noncanonical
    );
    assert!(!output.exists());
    assert_eq!(read_tree(&authored), before);
    let schema_path = authored.join("src/schema.spx");
    let schema = std::fs::read_to_string(&schema_path).unwrap();
    let parsed = semaprax::parse(&schema, "src/schema.spx").unwrap();
    std::fs::write(&schema_path, semaprax::format::canonical(&parsed)).unwrap();
    assert_success(&install(
        &output,
        &authored,
        &plan,
        "source-command.selected-file.resource-output.v1",
        None,
    ));
    assert_roundtrip(&output);
    assert_success(&cli(&output, &["check", "."]));
    assert_success(&cli(&output, &["test", ".", "--target", "native"]));
    assert_success(&cli(
        &output,
        &["build", ".", "--target", "native", "-o", "loglens"],
    ));
    // Copy the independent receiver unchanged; the installer owns source/tree
    // assembly, while this receiver retains every original assertion body.
    std::fs::copy(reference.join("run.sh"), output.join("run.sh")).unwrap();
    std::fs::set_permissions(
        output.join("run.sh"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::fs::copy(reference.join("tests.mjs"), output.join("tests.mjs")).unwrap();
    let result = Command::new("node")
        .arg("tests.mjs")
        .current_dir(&output)
        .env(
            "LOGLENS_SAMPLE",
            repo().join("benchmarks/cli-tokens-v1/sample.log"),
        )
        .output()
        .unwrap();
    assert_success(&result);
    assert!(String::from_utf8_lossy(&result.stdout).contains("49"));
    assert_eq!(
        std::fs::read(output.join("tests.mjs")).unwrap(),
        std::fs::read(reference.join("tests.mjs")).unwrap()
    );
}

#[test]
fn tiny_collection_record_selector_executes_typed_native_roots_and_keeps_legacy_refusal() {
    let fixture = Fixture::new("tiny-pure-native-selector");
    assert_success(&cli(
        &fixture.root,
        &[
            "new",
            "typed",
            "--name",
            "tiny",
            "--template",
            "stdin-stream-collection-record",
        ],
    ));
    let root = fixture.root.join("typed");
    semaprax::project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(
            snapshot.manifest().schema(),
            semaprax::project::PROJECT_SCHEMA_V31
        );
        assert_eq!(
            snapshot.native_test_roots().unwrap_err()[0].code,
            "SPX-B104"
        );
        let profile = semaprax::project::ProjectNativeTestProfile::CollectionRecordPureV1;
        let roots = snapshot.native_test_roots_with_profile(profile)?;
        assert_eq!(roots.len(), 2);
        assert!(roots.iter().all(|root| !root.result_is_exit_status()));
        let undeclared = root.join("undeclared-native-root");
        assert_eq!(
            snapshot
                .build_native_test_with_profile("tiny.not-a-declared-test", &undeclared, profile)
                .unwrap_err()[0]
                .code,
            "SPX-G172"
        );
        assert!(!undeclared.exists());
        Ok(())
    })
    .unwrap();
    let legacy = cli(&root, &["test", ".", "--target", "native"]);
    assert!(!legacy.status.success());
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("SPX-B104"));
    assert_legacy_native_json_refusal(&root);
    let explicit = cli(
        &root,
        &[
            "test",
            ".",
            "--target",
            "native-collection-record-pure.v1",
            "--json",
        ],
    );
    assert_success(&explicit);
    let result: serde_json::Value = serde_json::from_slice(&explicit.stdout).unwrap();
    assert_eq!(
        result["schema"],
        "semaprax.native-test.collection-record-pure.v1"
    );
    assert_eq!(result["passed"], true);
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    assert!(cases
        .iter()
        .all(|case| case["result"] == 0 && case["stdout"] == "0\n" && case["stderr"] == ""));
}
