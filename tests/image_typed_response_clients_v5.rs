//! Generated response-client evidence, authored and intentionally unrun.
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
            "spx-typed-response-clients-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
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
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let bytes =
        json!({"jsonrpc":"2.0","id":"typed-evidence","method":method,"params":params}).to_string();
    let result: Value =
        serde_json::from_slice(&session.handle_frame(bytes.as_bytes()).unwrap()).unwrap();
    assert!(result.get("error").is_none(), "{method}: {result}");
    result
}
fn payload(response: &Value) -> &Value {
    &response["result"]["payload"]
}
fn client(session: &mut VNextSession, language: &str) -> Value {
    let response = call(session, "protocol/client", json!({"language":language}));
    let client = payload(&response).clone();
    assert_eq!(client["schema"], "semaprax.image-agent-client.v5");
    assert_eq!(client["language"], language);
    assert_eq!(client["io"], false);
    assert!(serde_json::to_vec(&client).unwrap().len() <= 900 * 1024);
    client
}
fn class(method: &str) -> String {
    method
        .split(['/', '-'])
        .map(|part| {
            let mut chars = part.chars();
            format!(
                "{}{}",
                chars.next().unwrap().to_ascii_uppercase(),
                chars.as_str()
            )
        })
        .collect()
}

#[test]
fn all_languages_emit_deterministic_typed_responses_for_only_selected_methods() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    for policy in [
        VNextPolicy::default(),
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
        let schemas = call(&mut session, "protocol/schemas", json!({}));
        let methods = payload(&schemas)["methods"].as_array().unwrap();
        for language in ["typescript", "python", "rust"] {
            let generated = client(&mut session, language);
            assert_eq!(client(&mut session, language), generated);
            let mut independent = fixture.session(policy);
            assert_eq!(client(&mut independent, language), generated);
            independent.finish().unwrap();
            let source = generated["source"].as_str().unwrap();
            assert!(source.contains("TypedResultEnvelope"));
            if policy.candidate_prepare && language == "rust" {
                let next = source
                    .lines()
                    .filter(|line| line.contains("pub r#next_offset:"));
                assert!(next.clone().count() > 0);
                assert!(next
                    .clone()
                    .all(|line| !line.contains("Presence<") && !line.contains("Option<")));
                assert!(source
                    .lines()
                    .any(|line| line.contains("pub r#frontend_work: Presence<")));
            }
            if policy.candidate_prepare && language == "typescript" {
                assert!(source.contains("\"next_offset\":"));
                assert!(!source.contains("\"next_offset\"?:"));
                assert!(source.contains("\"frontend_work\"?:"));
            }
            for method in methods {
                let method = method["method"].as_str().unwrap();
                let name = class(method);
                let function = method.replace(['/', '-'], "_");
                for expected in [
                    format!("{name}Payload"),
                    format!("{name}Result"),
                    format!("decode_request_{function}_typed"),
                    format!("decode_request_{function}("),
                    format!("request_{function}("),
                ] {
                    assert!(
                        source.contains(&expected),
                        "{language} {method}: missing {expected}"
                    );
                }
            }
            for (name, selected) in [
                ("CandidateOpen", policy.candidate_prepare),
                ("CandidateBuild", policy.build_enabled),
                ("CandidateTest", policy.test_policy.is_some()),
                ("CandidateCommit", false),
            ] {
                assert_eq!(
                    source.contains(&format!("{name}Payload")),
                    selected,
                    "{language} {name}"
                );
            }
        }
        session.finish().unwrap();
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn concrete_chunk_schemas_keep_required_null_distinct_from_omission_and_opaque_contexts() {
    let fixture = Fixture::new();
    let mut session = fixture.session(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    });
    let schemas = call(&mut session, "protocol/schemas", json!({}));
    let documents = payload(&schemas)["documents"].as_array().unwrap();
    let chunk = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.image-candidate-report-chunk.v1")
        .unwrap();
    assert!(chunk["required"]
        .as_array()
        .unwrap()
        .contains(&json!("next_offset")));
    assert!(chunk["properties"]["next_offset"]
        .to_string()
        .contains("null"));
    let preview = documents
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.image-workspace-refresh-preview.v1")
        .unwrap();
    assert!(!preview["required"]
        .as_array()
        .unwrap()
        .contains(&json!("frontend_work")));
    assert!(preview["properties"]["frontend_work"]
        .get("oneOf")
        .is_some());
    assert!(payload(&schemas)["unbundled_payload_schemas"]
        .as_array()
        .unwrap()
        .contains(&json!("urn:semaprax.project-candidate-hole-context.v1")));
    let image = session.image_revision().to_owned();
    let candidate = call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image}),
    );
    let revision = payload(&candidate)["candidate_revision"].as_str().unwrap();
    let mut offset = 0;
    let mut text = String::new();
    loop {
        let chunk = call(
            &mut session,
            "candidate/query",
            json!({"image_revision":image,"candidate_revision":revision,"offset":offset,"chunk_bytes":1024}),
        );
        let chunk = payload(&chunk);
        assert_eq!(chunk["offset"], offset);
        assert!(chunk.get("next_offset").is_some());
        assert_eq!(chunk["source_authority"], false);
        text.push_str(chunk["chunk"].as_str().unwrap());
        let Some(next) = chunk["next_offset"].as_u64() else {
            assert!(chunk["next_offset"].is_null());
            break;
        };
        assert!(next > offset);
        offset = next;
    }
    let report: Value = serde_json::from_str(&text).unwrap();
    assert!(report["schema"].is_string());
    session.finish().unwrap();
}

#[test]
fn generated_python_typed_decoders_preserve_runtime_validation_and_opaque_report_boundaries() {
    // This authored test runs Python only when explicitly executed later. The
    // generator itself never runs a client or acquires filesystem authority.
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(VNextPolicy {
        candidate_prepare: true,
        ..Default::default()
    });
    let generated = client(&mut session, "python");
    let image = session.image_revision().to_owned();
    let workspace = call(&mut session, "workspace/open", json!({}));
    let candidate = call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image}),
    );
    let revision = payload(&candidate)["candidate_revision"].as_str().unwrap();
    let first_chunk = call(
        &mut session,
        "candidate/query",
        json!({"image_revision":image,"candidate_revision":revision,"chunk_bytes":1024}),
    );
    let total = payload(&first_chunk)["total_bytes"].as_u64().unwrap();
    assert!(total > 1);
    let final_chunk = call(
        &mut session,
        "candidate/query",
        json!({"image_revision":image,"candidate_revision":revision,"offset":total-1,"chunk_bytes":1024}),
    );
    assert!(payload(&final_chunk)["next_offset"].is_null());
    let draft = call(
        &mut session,
        "hole/open",
        json!({"image_revision":image,"candidate_revision":revision,"target":"calculator.add","hole_id":"body"}),
    );
    let context = call(
        &mut session,
        "hole/query",
        json!({"image_revision":image,"draft_revision":payload(&draft)["draft_revision"],"hole_id":"body"}),
    );
    let preview = call(
        &mut session,
        "workspace/refresh-preview",
        json!({"image_revision":image}),
    );
    assert!(payload(&preview).get("frontend_work").is_none());
    let fixtures = json!({"workspace_open":workspace,"candidate_open":candidate,"candidate_query":final_chunk,"hole_open":draft,"hole_query":context,"workspace_refresh_preview":preview});
    std::fs::write(
        fixture.0.join("generated_client.py"),
        generated["source"].as_str().unwrap(),
    )
    .unwrap();
    std::fs::write(fixture.0.join("responses.json"), fixtures.to_string()).unwrap();
    std::fs::write(fixture.0.join("check_client.py"), PYTHON_EVIDENCE).unwrap();
    let output = Command::new("python3")
        .arg("-I")
        .arg(fixture.0.join("check_client.py"))
        .current_dir(&fixture.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"typed-response-evidence-ok\n");
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), disk);
}

const PYTHON_EVIDENCE: &str = r#"
import copy
import importlib.util
import json
from pathlib import Path
import sys
import typing

# Exercise Windows text translation; the asserted transcript uses binary stdout.
sys.stdout.reconfigure(newline="\r\n")
root = Path(__file__).parent
spec = importlib.util.spec_from_file_location('generated_client', root / 'generated_client.py')
client = importlib.util.module_from_spec(spec)
sys.modules['generated_client'] = client
spec.loader.exec_module(client)
fixtures = json.loads((root / 'responses.json').read_text())
for method, response in fixtures.items():
    raw = json.dumps(response)
    old = getattr(client, 'decode_request_' + method)
    typed = getattr(client, 'decode_request_' + method + '_typed')
    assert old(raw, 'typed-evidence') == typed(raw, 'typed-evidence') == response['result']
    for bad in [dict(response, id='wrong'), dict(response, authority=True)]:
        for decoder in (old, typed):
            try:
                decoder(json.dumps(bad), 'typed-evidence')
            except ValueError:
                pass
            else:
                raise AssertionError(('accepted hostile envelope', method))

def rejects(method, response):
    for suffix in ('', '_typed'):
        decoder = getattr(client, 'decode_request_' + method + suffix)
        try:
            decoder(json.dumps(response), 'typed-evidence')
        except ValueError:
            pass
        else:
            raise AssertionError(('accepted hostile payload', method, response))

chunk = fixtures['candidate_query']
assert chunk['result']['payload']['next_offset'] is None
bad = copy.deepcopy(chunk)
del bad['result']['payload']['next_offset']
rejects('candidate_query', bad)
bad = copy.deepcopy(chunk)
bad['result']['payload']['next_offset'] = '0'
rejects('candidate_query', bad)
bad = copy.deepcopy(chunk)
bad['result']['payload']['next_offset'] = 2**64
rejects('candidate_query', bad)
bad = copy.deepcopy(fixtures['workspace_refresh_preview'])
bad['result']['payload']['frontend_work'] = None
rejects('workspace_refresh_preview', bad)
bad = copy.deepcopy(fixtures['candidate_open'])
bad['result']['payload']['source_authority'] = True
rejects('candidate_open', bad)
bad = copy.deepcopy(fixtures['candidate_open'])
bad['result']['payload']['candidate_revision'] = 'sha256:wrong'
rejects('candidate_open', bad)

# Opaque source-HIR interiors are deliberately not promoted to fully validated
# semantic structures by the additive static types.
opaque = copy.deepcopy(fixtures['hole_query'])
opaque['result']['payload']['unbundled_interior'] = {'arbitrary': [1, False, None]}
assert client.decode_request_hole_query_typed(json.dumps(opaque), 'typed-evidence') == opaque['result']
opaque['result']['payload']['schema'] = 'foreign.schema'
rejects('hole_query', opaque)

assert 'next_offset' in client.CandidateQueryPayload.__required_keys__
assert 'frontend_work' in client.WorkspaceRefreshPreviewPayload.__optional_keys__
hints = typing.get_type_hints(client.CandidateQueryPayload, vars(client), vars(client))
assert type(None) in typing.get_args(hints['next_offset'])
assert client.HoleQueryPayload is typing.Any
# Keep this exact-byte test marker independent of Windows text-mode CRLF.
sys.stdout.buffer.write(b'typed-response-evidence-ok\n')
"#;
