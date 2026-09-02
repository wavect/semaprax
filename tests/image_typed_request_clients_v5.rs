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
