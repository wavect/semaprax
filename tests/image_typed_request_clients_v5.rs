//! Recursive request-client generation and actual Python admission evidence.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::CandidateTestPolicy;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-typed-request-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self, policy: VNextPolicy) -> VNextSession {
        VNextSession::open(&self.0.join("semaprax.toml"), policy).unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request = json!({"jsonrpc":"2.0","id":"typed-request","method":method,"params":params});
    let response: Value = serde_json::from_slice(
        &session
            .handle_frame(request.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn client(session: &mut VNextSession, language: &str) -> Value {
    let result = call(session, "protocol/client", json!({"language":language}));
    assert_eq!(result["io"], false);
    assert!(serde_json::to_vec(&result).unwrap().len() <= 900 * 1024);
    result
}

fn selected_command(variable: &str, fallback: &str) -> PathBuf {
    std::env::var_os(variable).map_or_else(|| PathBuf::from(fallback), PathBuf::from)
}
fn test_python() -> PathBuf {
    std::env::var_os("SEMAPRAX_TEST_PYTHON").map_or_else(
        || PathBuf::from("python3"),
        |value| {
            let path = PathBuf::from(value);
            assert!(path.is_absolute(), "SEMAPRAX_TEST_PYTHON must be absolute");
            path
        },
    )
}

#[test]
fn selected_recursive_request_types_are_deterministic_and_preserve_legacy_helpers() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    for policy in [
        VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        },
        VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            build_enabled: true,
            test_policy: Some(CandidateTestPolicy::new(100, 4096, 16384).unwrap()),
        },
    ] {
        let mut session = fixture.session(policy);
        for language in ["typescript", "python", "rust"] {
            let generated = client(&mut session, language);
            assert_eq!(client(&mut session, language), generated);
            let mut cold = fixture.session(policy);
            assert_eq!(client(&mut cold, language), generated);
            cold.finish().unwrap();
            let source = generated["source"].as_str().unwrap();
            for method in [
                "candidate_apply_intent",
                "hole_fill",
                "hole_recovery_restore",
                "hole_archive_restore",
            ] {
                assert!(
                    source.contains(&format!("request_{method}_typed(")),
                    "{language}: {method}"
                );
                assert!(
                    source.contains(&format!("request_{method}(")),
                    "legacy {language}: {method}"
                );
            }
            assert!(source.contains("CandidateApplyIntentTypedParams"));
            assert!(source.contains("HoleFillTypedParams"));
            assert!(source.contains("RequestType"));
            assert!(source.contains("replace_function_body"));
            assert!(!source.contains("request_candidate_commit_typed("));
            if language == "rust" {
                assert!(source.contains("Box<RequestType"));
            }
        }
        session.finish().unwrap();
    }
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn read_only_clients_cannot_acquire_constructor_or_publication_helpers() {
    let fixture = Fixture::new();
    let mut session = fixture.session(VNextPolicy::default());
    for language in ["typescript", "python", "rust"] {
        let generated = client(&mut session, language);
        let source = generated["source"].as_str().unwrap();
        assert!(source.contains("WorkspaceOpenTypedParams"));
        assert!(source.contains("request_workspace_open_typed("));
        for method in [
            "candidate_apply_intent",
            "hole_fill",
            "candidate_commit",
            "candidate_test",
            "candidate_build",
        ] {
            assert!(!source.contains(&format!("request_{method}_typed(")));
        }
    }
    session.finish().unwrap();
}

#[test]
fn generated_python_resolves_recursive_types_and_submits_exact_intent_for_compiler_admission() {
    // Generation itself never executes Python; this consumer harness does.
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    });
    let generated = client(&mut session, "python");
    let image = session.image_revision().to_owned();
    let candidate = call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image}),
    );
    let params = json!({"image_revision":image,"candidate_revision":candidate["candidate_revision"],"intent":{
        "kind":"replace_function_body","target":"calculator.add",
        "body":{"kind":"let","name":"answer","value":{"kind":"i64","value":7},"body":{"kind":"place","name":"answer"}}
    }});
    std::fs::write(
        fixture.0.join("generated.py"),
        generated["source"].as_str().unwrap(),
    )
    .unwrap();
    std::fs::write(fixture.0.join("params.json"), params.to_string()).unwrap();
    std::fs::write(fixture.0.join("check.py"), PYTHON_EVIDENCE).unwrap();
    let output = Command::new(test_python())
        .arg("-I")
        .arg(fixture.0.join("check.py"))
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !output.stdout.contains(&b'\r'),
        "wire frame must not contain CR"
    );
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    assert_eq!(
        output.stdout.iter().filter(|&&byte| byte == b'\n').count(),
        1
    );
    let request: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(request["params"], params);
    let response: Value =
        serde_json::from_slice(&session.handle_frame(&output.stdout).unwrap()).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["payload"]["source_authority"], false);
    let mut rejected = request;
    rejected["params"]["intent"]["body"] = json!({"kind":"place","name":"missing_binding"});
    let error: Value = serde_json::from_slice(
        &session
            .handle_frame(rejected.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert!(
        error.get("error").is_some(),
        "structural types cannot admit an unbound place"
    );
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

const PYTHON_EVIDENCE: &str = r#"
import importlib.util
import json
from pathlib import Path
import sys
import typing
root = Path(__file__).parent
spec = importlib.util.spec_from_file_location('generated', root / 'generated.py')
client = importlib.util.module_from_spec(spec)
sys.modules['generated'] = client
spec.loader.exec_module(client)
for name, value in list(vars(client).items()):
    if name.startswith('RequestType') and typing.is_typeddict(value):
        typing.get_type_hints(value, vars(client), vars(client), include_extras=True)
params = json.loads((root / 'params.json').read_text())
typed = client.request_candidate_apply_intent_typed('typed-request', params)
assert typed == client.request_candidate_apply_intent('typed-request', params)
assert json.loads(typed)['params'] == params
assert not hasattr(client, 'request_candidate_commit_typed')
# Exercise Windows text newline translation even when this harness runs on Unix.
sys.stdout.reconfigure(newline="\r\n")
sys.stdout.buffer.write(typed.encode('utf-8'))
"#;

fn request_params(image: &str, candidate: &Value) -> Value {
    json!({"image_revision":image,"candidate_revision":candidate["candidate_revision"],"intent":{
        "kind":"replace_function_body","target":"calculator.add",
        "body":{"kind":"let","name":"answer","value":{"kind":"i64","value":7},"body":{"kind":"place","name":"answer"}}
    }})
}

fn admit_external_frame(session: &mut VNextSession, output: &[u8], params: &Value) {
    assert!(!output.contains(&b'\r'), "wire frame must not contain CR");
    assert_eq!(output.last(), Some(&b'\n'));
    assert_eq!(output.iter().filter(|&&byte| byte == b'\n').count(), 1);
    let request: Value = serde_json::from_slice(output).unwrap();
    assert_eq!(
        request,
        json!({
            "jsonrpc": "2.0",
            "id": "typed-request",
            "method": "candidate/apply-intent",
            "params": params,
        })
    );
    let response: Value = serde_json::from_slice(&session.handle_frame(output).unwrap()).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["payload"]["source_authority"], false);
    let mut rejected = request;
    rejected["id"] = json!("typed-request-hostile");
    rejected["params"]["intent"]["body"] = json!({"kind":"place","name":"missing_binding"});
    let error: Value = serde_json::from_slice(
        &session
            .handle_frame(rejected.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(error["error"]["code"], -32000);
    assert!(
        error["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SPX-G225"),
        "structural clients cannot admit an unbound place: {error}"
    );
}

#[test]
fn generated_rust_resolves_recursive_types_and_submits_exact_intent_for_compiler_admission() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    });
    let generated = client(&mut session, "rust");
    let image = session.image_revision().to_owned();
    let candidate = call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image}),
    );
    let params = request_params(&image, &candidate);
    let root = fixture.0.join("rust-request-client");
    std::fs::create_dir_all(root.join("src")).unwrap();
    let locked_version = |name: &str| {
        let selected = format!("name = \"{name}\"");
        include_str!("../Cargo.lock")
            .split("[[package]]")
            .find(|package| package.lines().any(|line| line == selected))
            .unwrap()
            .lines()
            .find_map(|line| {
                line.strip_prefix("version = \"")
                    .and_then(|value| value.strip_suffix('"'))
            })
            .unwrap()
    };
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "typed-request-rust-evidence"
version = "0.0.0"
edition = "2021"
[workspace]
[dependencies]
serde = { version = "=@SERDE_VERSION@", features = ["derive"] }
serde_json = "=@SERDE_JSON_VERSION@"
[profile.dev]
debug = 0
incremental = false
"#
        .replace("@SERDE_VERSION@", locked_version("serde"))
        .replace("@SERDE_JSON_VERSION@", locked_version("serde_json")),
    )
    .unwrap();
    std::fs::write(
        root.join("src/client.rs"),
        generated["source"].as_str().unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("params.json"), params.to_string()).unwrap();
    std::fs::write(root.join("src/main.rs"), RUST_REQUEST_EVIDENCE).unwrap();
    let cargo = selected_command("SEMAPRAX_TEST_CARGO", "cargo");
    let locked = Command::new(&cargo)
        .args(["generate-lockfile", "--offline", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        locked.status.success(),
        "{}",
        String::from_utf8_lossy(&locked.stderr)
    );
    let output = Command::new(&cargo)
        .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
        .arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", root.join("target"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    admit_external_frame(&mut session, &output.stdout, &params);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
#[ignore = "requires provisioned absolute SEMAPRAX_TEST_TSC 5.8.3 and SEMAPRAX_TEST_NODE >=22"]
fn provisioned_typescript_submits_exact_typed_request_for_compiler_admission() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let tsc = selected_command("SEMAPRAX_TEST_TSC", "");
    let node = selected_command("SEMAPRAX_TEST_NODE", "");
    assert!(
        tsc.is_absolute() && node.is_absolute(),
        "provisioned TypeScript tools must be absolute"
    );
    let tsc_version = Command::new(&tsc).arg("--version").output().unwrap();
    assert!(tsc_version.status.success());
    assert_eq!(
        String::from_utf8(tsc_version.stdout).unwrap().trim(),
        "Version 5.8.3"
    );
    let node_version = Command::new(&node).arg("--version").output().unwrap();
    assert!(node_version.status.success());
    let major = String::from_utf8(node_version.stdout)
        .unwrap()
        .trim()
        .strip_prefix('v')
        .and_then(|value| value.split('.').next())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap();
    assert!(major >= 22, "provisioned Node must be >=22");
    let mut session = fixture.session(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    });
    let generated = client(&mut session, "typescript");
    let image = session.image_revision().to_owned();
    let candidate = call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image}),
    );
    let params = request_params(&image, &candidate);
    let root = fixture.0.join("typescript-request-client");
    std::fs::create_dir_all(&root).unwrap();
    let mut source = generated["source"].as_str().unwrap().to_owned();
    source.push_str("\nconst EVIDENCE_PARAMS = ");
    source.push_str(&params.to_string());
    source.push_str(
        r#" as const satisfies CandidateApplyIntentTypedParams;
const EVIDENCE_FRAME = request_candidate_apply_intent_typed("typed-request", EVIDENCE_PARAMS);
if (!EVIDENCE_FRAME.endsWith("\n")) throw new Error("missing frame LF");
console.log(EVIDENCE_FRAME.slice(0, -1));
"#,
    );
    let input = root.join("request.ts");
    let out = root.join("out");
    std::fs::write(&input, source).unwrap();
    let compiled = Command::new(&tsc)
        .args([
            "--strict",
            "--noEmitOnError",
            "--target",
            "ES2022",
            "--module",
            "NodeNext",
            "--moduleResolution",
            "NodeNext",
            "--outDir",
        ])
        .arg(&out)
        .arg(&input)
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&compiled.stdout),
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&node)
        .arg(out.join("request.js"))
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    admit_external_frame(&mut session, &output.stdout, &params);
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

const RUST_REQUEST_EVIDENCE: &str = r#"
mod client;
fn main() {
    let params: client::CandidateApplyIntentTypedParams = serde_json::from_str(include_str!("../params.json")).unwrap();
    let frame = client::request_candidate_apply_intent_typed(client::RpcId::Text("typed-request".into()), params).unwrap();
    assert!(frame.ends_with('\n'));
    print!("{frame}");
}
"#;
