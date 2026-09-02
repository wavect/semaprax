//! Image protocol regressions. These do not grant image bytes source authority.
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::image_transport::{
    self, ImageHostCapability, ImageSession, MAX_REQUEST_BYTES, PROTOCOL_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-image-transport-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(path), root.join(path)).unwrap();
        }
        Self(root)
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self) -> ImageSession {
        ImageSession::open(&self.manifest(), ImageHostCapability::ReadOnly).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn request(session: &mut ImageSession, method: &str, params: Value) -> Value {
    let request = json!({"jsonrpc":"2.0", "id":7, "method":method, "params":params}).to_string();
    let bytes = session.handle_frame(request.as_bytes()).unwrap();
    assert!(!bytes.contains(&b'\n'));
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn protocol_catalog_schemas_and_clients_advertise_only_read_only_methods() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let capabilities = request(&mut session, "protocol/capabilities", json!({}));
    let payload = &capabilities["result"]["payload"];
    assert_eq!(payload["protocol"], PROTOCOL_SCHEMA);
    assert_eq!(payload["capabilities"], json!(["semantic_read"]));
    assert_eq!(payload["source_authority"], false);
    let methods = payload["methods"].as_array().unwrap().clone();
    let schemas = request(&mut session, "protocol/schemas", json!({}));
    let schemas = schemas["result"]["payload"]["methods"].as_array().unwrap();
    assert_eq!(methods.len(), schemas.len());
    for (name, descriptor) in methods.iter().zip(schemas) {
        assert_eq!(name, &descriptor["method"]);
        assert_eq!(descriptor["request_schema"]["additionalProperties"], false);
        assert_eq!(
            descriptor["request_schema"]["properties"]["params"]["additionalProperties"],
            false
        );
        assert_eq!(
            descriptor["success_response_schema"]["properties"]["result"]["additionalProperties"],
            false
        );
        assert_eq!(
            descriptor["error_response_schema"]["additionalProperties"],
            false
        );
        assert!(!name.as_str().unwrap().contains("change"));
        assert!(!name.as_str().unwrap().contains("build"));
    }
    for language in ["typescript", "python", "rust"] {
        let client = request(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        );
        let source = client["result"]["payload"]["source"].as_str().unwrap();
        assert!(source.contains(PROTOCOL_SCHEMA));
        for method in &methods {
            assert!(source.contains(method.as_str().unwrap()));
        }
    }
    let queries = request(&mut session, "query/catalog", json!({}));
    for query in queries["result"]["payload"]["queries"].as_array().unwrap() {
        assert!(query["method"].as_str().unwrap().starts_with("image/"));
    }
    let instructions = request(&mut session, "protocol/instructions", json!({}));
    assert_eq!(
        instructions["result"]["payload"]["protocol"],
        PROTOCOL_SCHEMA
    );
}

#[test]
fn workspace_open_returns_compact_revision_handles_and_queries_reuse_them() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let open = request(&mut session, "workspace/open", json!({}));
    assert!(open.to_string().len() < 1024);
    assert_eq!(open["result"]["image_revision"], session.image_revision());
    assert_eq!(open, request(&mut session, "workspace/open", json!({})));
    let revision = session.image_revision().to_owned();
    let symbol = request(
        &mut session,
        "image/symbol",
        json!({"image_revision":revision, "stable_id":"calculator.add"}),
    );
    assert_eq!(
        symbol["result"]["payload"]["symbol"]["id"],
        "calculator.add"
    );
    for method in ["image/context", "image/impact"] {
        let response = request(
            &mut session,
            method,
            json!({"image_revision":revision,"target_kind":"declaration","target":"calculator.add","depth":1,"max_bytes":65536,"max_nodes":32}),
        );
        assert!(response.get("result").is_some(), "{response}");
        assert_eq!(response["result"]["image_revision"], revision);
    }
    session.finish().unwrap();
    let mut files = std::fs::read_dir(&fixture.0)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(files, ["semaprax.toml", "src"]);
}

#[test]
fn function_summary_handles_select_bounded_facets() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let revision = session.image_revision().to_owned();
    let summary = request(
        &mut session,
        "image/function-summary",
        json!({"image_revision":revision,"target":"calculator.add"}),
    );
    let facets = summary["result"]["payload"]["facets"].as_array().unwrap();
    for facet in facets {
        let response = request(
            &mut session,
            "image/facet",
            json!({"image_revision":revision,"target":"calculator.add","facet":facet["facet"],"handle":facet["handle"],"page_size":1,"max_bytes":65536}),
        );
        assert!(response.get("result").is_some(), "{response}");
        assert!(
            response["result"]["payload"]["items"]
                .as_array()
                .unwrap()
                .len()
                <= 1
        );
    }
}

#[test]
fn source_paths_authority_escalation_and_invalid_params_are_rejected() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    for method in [
        "build",
        "change/apply",
        "rename/apply",
        "authority/elevate",
        "workspace/refresh",
    ] {
        assert_eq!(
            request(&mut session, method, json!({}))["error"]["code"],
            -32601
        );
    }
    for (method, params) in [
        ("workspace/open", json!({"manifest":"other.toml"})),
        (
            "protocol/capabilities",
            json!({"capabilities":["source_write"]}),
        ),
        ("image/symbol", json!({"stable_id":"calculator.add"})),
        (
            "image/symbol",
            json!({"image_revision":"bad", "stable_id":"calculator.add"}),
        ),
        ("protocol/client", json!({"language":"shell"})),
    ] {
        assert_eq!(
            request(&mut session, method, params)["error"]["code"],
            -32602
        );
    }
    let stale = format!("sha256:{}", "0".repeat(64));
    let stale = request(
        &mut session,
        "image/symbol",
        json!({"image_revision":stale, "stable_id":"calculator.add"}),
    );
    assert_eq!(stale["error"]["code"], -32000);
    assert!(stale["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G221"));
}

#[test]
fn strict_codec_rejects_duplicates_and_notifications_do_no_work() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    for frame in [
        br#"{"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{"path":"a","path":"b"}}"#
            .as_slice(),
        br#"{"jsonrpc":"2.0","id":1,"id":2,"method":"workspace/open"}"#.as_slice(),
        b"[]".as_slice(),
        br#"{"jsonrpc":"2.0","id":null,"method":"workspace/open"}"#.as_slice(),
    ] {
        let response: Value =
            serde_json::from_slice(&session.handle_frame(frame).unwrap()).unwrap();
        assert!(response.get("error").is_some());
    }
    assert!(session
        .handle_frame(br#"{"jsonrpc":"2.0","method":"image/symbol","params":{}}"#)
        .is_none());
    assert!(session.handle_frame(b"").is_none());
}

#[test]
fn framed_serve_is_deterministic_and_oversized_input_is_terminal() {
    let fixture = Fixture::new();
    let input = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"workspace/open\"}\n";
    let mut first = Vec::new();
    image_transport::serve(
        Cursor::new(input),
        &mut first,
        &fixture.manifest(),
        ImageHostCapability::ReadOnly,
    )
    .unwrap();
    let mut second = Vec::new();
    image_transport::serve(
        Cursor::new(input),
        &mut second,
        &fixture.manifest(),
        ImageHostCapability::ReadOnly,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(first.iter().filter(|byte| **byte == b'\n').count(), 1);
    let mut oversized = vec![b' '; MAX_REQUEST_BYTES + 1];
    oversized.push(b'\n');
    oversized.extend_from_slice(input);
    let mut output = Vec::new();
    image_transport::serve(
        Cursor::new(oversized),
        &mut output,
        &fixture.manifest(),
        ImageHostCapability::ReadOnly,
    )
    .unwrap();
    assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
    let error: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(error["error"]["code"], -32700);
}

#[cfg(unix)]
#[test]
fn observed_source_drift_is_absorbing_even_if_original_bytes_return() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let source = fixture.0.join("src/core.spx");
    let original = std::fs::read(&source).unwrap();
    std::fs::write(&source, b"changed\n").unwrap();
    assert_eq!(
        request(&mut session, "workspace/status", json!({}))["error"]["code"],
        -32000
    );
    std::fs::write(&source, original).unwrap();
    assert_eq!(
        request(&mut session, "workspace/open", json!({}))["error"]["code"],
        -32000
    );
    assert!(session.finish().is_err());
}

#[test]
fn serve_image_cli_is_strict_and_has_no_banner() {
    let fixture = Fixture::new();
    for args in [
        vec!["serve-image"],
        vec!["serve-image", "semaprax.toml", "extra"],
        vec!["serve-image", "--write"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&fixture.0)
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&fixture.0)
        .args(["serve-image", "semaprax.toml"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"workspace/open\"}\n")
            .unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty());
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["result"]["protocol"], PROTOCOL_SCHEMA);
}
