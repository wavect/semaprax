use std::path::Path;
use std::process::Command;

use semaprax::{ast::Span, codegen, diagnostic::Diagnostic, graph, parse, verify};

const VALID: &str = r#"
module test.answer;

@id("math.add")
fn add(a: i64, b: i64) -> i64
    requires a >= 0
    ensures result == a + b
{
    a + b
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(20, 22)
}
"#;

#[test]
fn valid_program_has_stable_graph() {
    let first = parse(VALID, Path::new("valid.spx")).unwrap();
    let second = parse(VALID, Path::new("elsewhere.spx")).unwrap();
    assert!(verify::verify(&first).is_empty());
    assert_eq!(graph::revision(&first), graph::revision(&second));
    let json = graph::to_json(&first).unwrap();
    assert!(json.contains("\"id\":\"math.add\""));
    assert!(json.contains("\"calls\":[\"math.add\"]"));
}

#[test]
fn revision_is_a_canonical_sha256_content_address() {
    let program = parse(VALID, Path::new("revision.spx")).unwrap();
    let revision = graph::revision(&program);
    let digest = revision.strip_prefix("sha256:").unwrap();
    assert_eq!(revision.len(), 71);
    assert_eq!(digest.len(), 64);
    assert!(digest
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert!(!revision.contains("fnv1a64"));

    let changed = VALID.replace("add(20, 22)", "add(20, 23)");
    let changed = parse(&changed, Path::new("revision-changed.spx")).unwrap();
    assert_ne!(revision, graph::revision(&changed));

    let graph_json = graph::to_json(&program).unwrap();
    let context = graph::context_json(&program, "app.main", 0)
        .unwrap()
        .unwrap();
    let encoded = semaprax::diagnostic::quote_json(&revision);
    assert!(graph_json.contains(&format!("\"revision\":{encoded}")));
    assert!(context.contains(&format!("\"revision\":{encoded}")));
}

#[test]
fn malformed_graph_invocations_reject_before_loading_source() {
    let missing = format!("semaprax-graph-missing-{}.spx", std::process::id());
    assert!(!Path::new(&missing).exists());
    for arguments in [
        vec!["graph", missing.as_str(), "extra"],
        vec!["graph", missing.as_str(), "--unknown"],
        vec!["graph", "--unknown"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(&arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{arguments:?}");
        assert!(output.stdout.is_empty(), "{arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.starts_with("graph requires exactly")
                || stderr.starts_with("unknown graph option"),
            "{arguments:?}: {stderr}"
        );
        assert!(!stderr.contains("cannot read"), "{arguments:?}: {stderr}");
    }
}

#[test]
fn context_slice_follows_calls() {
    let program = parse(VALID, Path::new("valid.spx")).unwrap();
    let by_id = graph::context_json(&program, "app.main", 1)
        .unwrap()
        .unwrap();
    let by_name = graph::context_json(&program, "main", 1).unwrap().unwrap();
    assert_eq!(by_id, by_name);
    assert!(by_id.contains("\"name\":\"main\""));
    assert!(by_id.contains("\"name\":\"add\""));
    assert!(by_id.contains(
        "\"view\":{\"kind\":\"context\",\"root\":\"app.main\",\"depth\":1,\"truncated\":false,\"frontier\":[]}"
    ));

    let bounded = graph::context_json(&program, "app.main", 0)
        .unwrap()
        .unwrap();
    assert!(bounded.contains(
        "\"view\":{\"kind\":\"context\",\"root\":\"app.main\",\"depth\":0,\"truncated\":true,\"frontier\":[\"math.add\"]}"
    ));
    assert!(!bounded.contains("\"id\":\"math.add\",\"kind\":\"function\""));
}

#[test]
fn graph_v10_exposes_resolved_identity_types_ownership_and_facts() {
    let program = parse(VALID, Path::new("resolved-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(json.contains("\"entrypoint\":\"app.main\""));
    assert!(json.contains("\"identity_origin\":\"explicit\",\"persistent\":true"));
    assert!(json.contains("\"id\":\"declaration:8:math.add:value:param:1:0\",\"name\":\"a\""));
    assert!(json.contains("\"result_id\":\"declaration:8:math.add:value:result:0:\""));
    assert!(json.contains("\"callee\":\"math.add\""));
    assert!(json.contains("\"type_id\":\"i64\",\"ownership_mode\":\"value\""));
    assert!(json.contains("\"layout_key\":\"scalar:i64\""));
}

#[test]
fn graph_i64_literals_are_lossless_for_javascript_agents() {
    let source = r#"
module test.lossless_i64_graph;
@id("app.main")
fn main() -> i64 { 9223372036854775807 }
"#;
    let program = parse(source, Path::new("lossless-i64-graph.spx")).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"kind\":\"int\",\"value\":\"9223372036854775807\""));
    assert!(!json.contains("\"value\":9223372036854775807"));

    if Command::new("node").arg("--version").output().is_ok() {
        let script = r#"
const graph = JSON.parse(process.argv[1]);
let found = false;
JSON.stringify(graph, (key, value) => {
  if (key === "value" && value === "9223372036854775807") found = true;
  return value;
});
if (!found) process.exit(2);
"#;
        let output = Command::new("node")
            .arg("-e")
            .arg(script)
            .arg(&json)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Node failed to preserve graph i64: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn context_includes_referenced_nominal_types_without_unrelated_resources() {
    let source = r#"
module test.type_context;
@id("type.used")
resource Used {
    @id("type.used.drop")
    drop trivial;
}
@id("type.unused")
resource Unused {
    @id("type.unused.drop")
    drop trivial;
}
@id("used.inspect")
fn inspect(value: borrow Used) -> i64 { 1 }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let program = parse(source, Path::new("type-context.spx")).unwrap();
    let context = graph::context_json(&program, "used.inspect", 0)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"id\":\"type.used\""));
    assert!(context.contains("\"type\":{\"kind\":\"nominal\",\"declaration\":\"type.used\""));
    assert!(!context.contains("type.unused"));
    assert!(!context.contains("\"id\":\"app.main\",\"kind\":\"function\""));
}

#[test]
fn graph_boundaries_reject_invalid_ast() {
    let source = r#"
module test.invalid_graph;
@id("app.main")
fn main() -> i64 { missing }
"#;
    let program = parse(source, Path::new("invalid-graph.spx")).unwrap();
    assert_eq!(graph::to_json(&program).unwrap_err()[0].code, "SPX-T202");
    assert_eq!(
        graph::context_json(&program, "main", 0).unwrap_err()[0].code,
        "SPX-T202"
    );
}

#[test]
fn graph_uses_canonical_source_revision_and_marks_automatic_ids_unstable() {
    let automatic = r#"
module test.automatic_graph;
fn main() -> i64 { 42 }
"#;
    let automatic = parse(automatic, Path::new("automatic.spx")).unwrap();
    let json = graph::to_json(&automatic).unwrap();
    assert!(json.contains(&format!(
        "\"revision\":{}",
        semaprax::diagnostic::quote_json(&graph::revision(&automatic))
    )));
    assert!(json.contains("\"identity_origin\":\"automatic\",\"persistent\":false"));
}

#[test]
fn context_prefers_an_exact_declaration_id_over_a_colliding_display_name() {
    let source = r#"
module test.context_collision;
@id("x")
fn target() -> i64 { 1 }
@id("other.x")
fn x() -> i64 { 2 }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let program = parse(source, Path::new("context-collision.spx")).unwrap();
    let context = graph::context_json(&program, "x", 0).unwrap().unwrap();
    assert!(context.contains("\"root\":\"x\""));
    assert!(context.contains("\"id\":\"x\",\"kind\":\"function\",\"name\":\"target\""));
    assert!(!context.contains("other.x"));
}

#[test]
fn context_and_graph_include_contract_dependencies() {
    let source = r#"
module test.contract_graph;
@id("contract.pure")
fn pure(value: i64) -> i64 { value }
@id("contract.guarded")
fn guarded() -> i64 requires pure(1) == 1 ensures pure(result) == 42 { 42 }
@id("app.main")
fn main() -> i64 { guarded() }
"#;
    let program = parse(source, Path::new("contract-graph.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let context = graph::context_json(&program, "contract.guarded", 1)
        .unwrap()
        .unwrap();
    assert!(context.contains("\"id\":\"contract.pure\""));
    assert!(context.contains("\"requires_graph\":[{"));
    assert!(context.contains("\"ensures_graph\":[{"));
    assert!(context.contains("\"calls\":[\"contract.pure\"]"));
}

#[test]
fn missing_effect_is_rejected() {
    let source = r#"
module test.effects;
permit { clock.read }
@id("clock.tick")
fn tick(value: i64) -> i64 uses { clock.read } { value + 1 }
@id("app.main")
fn main() -> i64 { tick(41) }
"#;
    let program = parse(source, Path::new("effects.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-E102"));
}

#[test]
fn contracts_cannot_call_effectful_functions() {
    let source = r#"
module test.contract_effect;
permit { clock.read }
@id("clock.tick")
fn tick(value: i64) -> i64 uses { clock.read } { value + 1 }
@id("app.main")
fn main() -> i64 ensures tick(result) == 43 { 42 }
"#;
    let program = parse(source, Path::new("contract-effect.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.iter().any(|item| item.code == "SPX-C102"));
}

#[test]
fn native_backend_produces_executable() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(VALID, Path::new("valid.spx")).unwrap();
    let output = std::env::temp_dir().join(format!("semaprax-test-{}", std::process::id()));
    codegen::build(&program, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    let _ = std::fs::remove_file(&output);
    assert!(result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
}

#[test]
fn native_backend_accepts_legal_scalar_self_comparisons_under_werror() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.native_self_comparison;
@id("app.main")
fn main() -> i64 {
    let i64_value = 5;
    let i32_value = 5i32;
    let u8_value = 5u8;
    let usize_value = 5usize;
    let bool_value = true;
    let f64_value = 5.0;
    let f32_value = 5.0f32;
    let char_value = 'A';
    if i64_value == i64_value && i32_value != i32_value == false
        && u8_value <= u8_value && usize_value >= usize_value
        && bool_value == bool_value && f64_value == f64_value
        && f32_value == f32_value && char_value == char_value
    { 1 } else { 0 }
}

"#;
    let program = parse(source, Path::new("native-self-comparison.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let output =
        std::env::temp_dir().join(format!("semaprax-self-comparison-{}", std::process::id()));
    codegen::build(&program, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    let _ = std::fs::remove_file(&output);
    assert!(result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "1");
}

#[test]
fn native_recursion_depth_is_a_reported_runtime_failure_not_a_signal() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let cases = [
        (
            "mutual",
            r#"
module test.native_mutual_recursion;
@id("app.f")
fn f(value: i64) -> i64 { g(value) }
@id("app.g")
fn g(value: i64) -> i64 { f(value) }
@id("app.main")
fn main() -> i64 { f(1) }
"#,
        ),
        (
            "direct",
            r#"
module test.native_direct_recursion;
@id("app.down")
fn down(value: i64) -> i64 {
    if value == 0 { 7 } else { down(value - 1) }
}
@id("app.main")
fn main() -> i64 { down(1000000) }
"#,
        ),
    ];
    for (ordinal, (name, source)) in cases.into_iter().enumerate() {
        let program = parse(source, Path::new("native-recursion.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let output = std::env::temp_dir().join(format!(
            "semaprax-recursion-{}-{ordinal}",
            std::process::id()
        ));
        codegen::build(&program, &output).unwrap();
        let result = Command::new(&output).output().unwrap();
        let _ = std::fs::remove_file(&output);
        assert_eq!(result.status.code(), Some(73), "{name}");
        assert!(result.stdout.is_empty(), "{name}");
        assert_eq!(
            String::from_utf8_lossy(&result.stderr),
            "SEMAPRAX runtime failure: call depth exceeded (256 frames)\n",
            "{name}"
        );
    }

    let shallow = r#"
module test.native_shallow_recursion;
@id("app.down")
fn down(value: i64) -> i64 {
    if value == 0 { 7 } else { down(value - 1) }
}
@id("app.main")
fn main() -> i64 { down(100) }
"#;
    let program = parse(shallow, Path::new("native-shallow-recursion.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let output =
        std::env::temp_dir().join(format!("semaprax-shallow-recursion-{}", std::process::id()));
    codegen::build(&program, &output).unwrap();
    let result = Command::new(&output).output().unwrap();
    let _ = std::fs::remove_file(&output);
    assert!(result.status.success());
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "7");
}

#[test]
fn backends_reject_unverified_programs_without_panicking() {
    let source = r#"
module test.invalid_backend;
@id("app.main")
fn main() -> i64 { if true { missing } else { 0 } }
"#;
    let program = parse(source, Path::new("invalid-backend.spx")).unwrap();
    assert_eq!(codegen::emit_c(&program).unwrap_err().code, "SPX-T202");
    assert_eq!(
        semaprax::wasm::emit_module(&program).unwrap_err().code,
        "SPX-T202"
    );
}

#[test]
fn glued_integer_suffixes_fail_with_stable_lexer_diagnostics() {
    let cases = ["12f32", "12abc", "12_x", "12i64"];
    for literal in cases {
        let statement = format!("let glued = {literal}; 0");
        let source = format!(
            r#"
module test.int_lex;
@id("app.main")
fn main() -> i64 {{ {statement} }}
"#
        );
        let error = parse(&source, Path::new("int-lex.spx")).expect_err(&format!(
            "glued integer suffix `{literal}` must be rejected"
        ));
        assert_eq!(error.code, "SPX-P003", "{literal}: {error}");
        assert!(error.message.contains("suffix"), "{literal}: {error}");
    }
}

#[test]
fn human_diagnostics_include_terminal_safe_source_locations() {
    let span = Span {
        start: 4,
        end: 9,
        line: 2,
        column: 3,
    };
    let located = Diagnostic::error("SPX-T001", "invalid source", span)
        .at_path("src/main.spx")
        .with_help("replace it");
    assert_eq!(
        located.to_string(),
        "error[SPX-T001]: invalid source at src/main.spx:2:3\n  help: replace it"
    );
    assert_eq!(
        Diagnostic::error("SPX-T001", "invalid source", span).to_string(),
        "error[SPX-T001]: invalid source at 2:3"
    );
    assert_eq!(
        Diagnostic::io("SPX-T002", "cannot read").to_string(),
        "error[SPX-T002]: cannot read"
    );
    assert_eq!(
        Diagnostic::io("SPX-T002", "cannot read")
            .at_path("bad\n\x1b[31m.spx")
            .to_string(),
        "error[SPX-T002]: cannot read at bad\\n\\u{1b}[31m.spx"
    );
    assert_eq!(
        located.json(),
        "{\"code\":\"SPX-T001\",\"severity\":\"error\",\"message\":\"invalid source\",\"path\":\"src/main.spx\",\"location\":{\"line\":2,\"column\":3,\"start\":4,\"end\":9},\"help\":\"replace it\"}"
    );
}

#[test]
fn single_file_build_refuses_existing_and_invalid_destinations_without_clobbering() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-source-build-freshness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("input.spx");
    std::fs::write(&source, VALID).unwrap();
    let source_bytes = std::fs::read(&source).unwrap();

    let default_output = source.with_extension("out");
    let wasm_default = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("build")
        .arg(&source)
        .args(["--target", "wasm", "--json"])
        .output()
        .unwrap();
    assert!(wasm_default.status.success());
    assert!(wasm_default.stderr.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&wasm_default.stdout).unwrap();
    assert_eq!(report["status"], "built");
    assert_eq!(report["target"], "wasm");
    assert_eq!(report["product"], "web package");
    assert_eq!(report["output"], default_output.display().to_string());
    let wasm_bytes = std::fs::read(default_output.join("app.wasm")).unwrap();
    std::fs::remove_dir_all(&default_output).unwrap();
    let web_alias = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("build")
        .arg(&source)
        .args(["--target", "web"])
        .output()
        .unwrap();
    assert!(web_alias.status.success());
    assert_eq!(
        std::fs::read(default_output.join("app.wasm")).unwrap(),
        wasm_bytes
    );
    std::fs::remove_dir_all(&default_output).unwrap();

    let run = |target: &str, output: &Path| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("build")
            .arg(&source)
            .args(["--target", target, "-o"])
            .arg(output)
            .output()
            .unwrap()
    };

    let victim = root.join("victim");
    std::fs::write(&victim, b"precious\n").unwrap();
    let result = run("native", &victim);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("SPX-I307"));
    assert_eq!(std::fs::read(&victim).unwrap(), b"precious\n");

    let json_error = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("build")
        .arg(&source)
        .args(["--target", "native", "-o"])
        .arg(&victim)
        .arg("--json")
        .output()
        .unwrap();
    assert_eq!(json_error.status.code(), Some(1));
    assert!(json_error.stderr.is_empty());
    let diagnostic: serde_json::Value = serde_json::from_slice(&json_error.stdout).unwrap();
    assert_eq!(diagnostic["code"], "SPX-I307");

    let result = run("native", &source);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("SPX-I307"));
    assert_eq!(std::fs::read(&source).unwrap(), source_bytes);

    let web = root.join("web");
    std::fs::create_dir(&web).unwrap();
    std::fs::write(web.join("index.html"), b"mine\n").unwrap();
    std::fs::write(web.join("keep.txt"), b"keep\n").unwrap();
    let result = run("web", &web);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("SPX-I307"));
    assert_eq!(std::fs::read(web.join("index.html")).unwrap(), b"mine\n");
    assert_eq!(std::fs::read(web.join("keep.txt")).unwrap(), b"keep\n");

    let missing = root.join("missing").join("artifact");
    let result = run("native", &missing);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("SPX-I301"));
    assert!(!root.join("missing").exists());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let read_only = root.join("read-only");
        std::fs::create_dir(&read_only).unwrap();
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o555)).unwrap();
        let output = read_only.join("artifact");
        let result = run("native", &output);
        std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(result.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&result.stderr).contains("SPX-I301"));
        assert!(!output.exists());
        std::fs::remove_dir(read_only).unwrap();
    }

    if Command::new("clang").arg("--version").output().is_ok() {
        let fresh = root.join(format!("fresh{}", std::env::consts::EXE_SUFFIX));
        let result = run("native", &fresh);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let executed = Command::new(&fresh).output().unwrap();
        assert!(executed.status.success());
        assert_eq!(executed.stdout, b"42\n");
        std::fs::remove_file(fresh).unwrap();
    }

    let concurrent = root.join("concurrent-web");
    let command = || {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"));
        child
            .arg("build")
            .arg(&source)
            .args(["--target", "web", "-o"])
            .arg(&concurrent)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        child.spawn().unwrap()
    };
    let first = command();
    let second = command();
    let results = [
        first.wait_with_output().unwrap(),
        second.wait_with_output().unwrap(),
    ];
    assert_eq!(
        results
            .iter()
            .filter(|result| result.status.success())
            .count(),
        1
    );
    let loser = results
        .iter()
        .find(|result| !result.status.success())
        .unwrap();
    assert_eq!(loser.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&loser.stderr).contains("SPX-I307"));
    std::fs::remove_dir_all(concurrent).unwrap();

    std::fs::remove_file(web.join("index.html")).unwrap();
    std::fs::remove_file(web.join("keep.txt")).unwrap();
    std::fs::remove_dir(web).unwrap();
    std::fs::remove_file(victim).unwrap();
    std::fs::remove_file(source).unwrap();
    std::fs::remove_dir(root).unwrap();
}
