//! Closed host-policy v4 regressions authored, deliberately unrun locally.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::with_authenticated_project;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-v5-semantic-cache-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn run(&self, policy: &Value, input: &str) -> Output {
        let path = self.0.join("host.json");
        std::fs::write(&path, policy.to_string()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve-workspace")
            .arg(self.0.join("semaprax.toml"))
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
    fn refresh_input(&self) -> String {
        let manifest = self.0.join("semaprax.toml");
        let cold = VNextSession::open(&manifest, VNextPolicy::default()).unwrap();
        let revision = with_authenticated_project(&manifest, |snapshot| {
            Ok(snapshot.project_revision().to_owned())
        })
        .unwrap();
        [
            json!({"jsonrpc":"2.0","id":1,"method":"protocol/capabilities","params":{}}),
            json!({"jsonrpc":"2.0","id":2,"method":"workspace/open","params":{}}),
            json!({"jsonrpc":"2.0","id":3,"method":"workspace/refresh","params":{"image_revision":cold.image_revision(),"expected_new_project_revision":revision}}),
            json!({"jsonrpc":"2.0","id":4,"method":"workspace/open","params":{"semantic_cache":true}}),
        ].into_iter().map(|frame|format!("{frame}\n")).collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn policy(version: u8) -> Value {
    let mut value = json!({"schema":format!("semaprax.workspace-host-policy.v{version}"),"candidate_prepare":false,"diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null});
    if version >= 2 {
        value["frontend_cache"] = json!(false);
    }
    if version >= 3 {
        value["candidate_archives"] = json!([]);
    }
    if version >= 4 {
        value["semantic_cache"] = json!(false);
    }
    value
}
fn rows(output: Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

#[test]
fn explicit_semantic_cache_reuses_checked_modules_without_changing_identity_or_authority() {
    let fixture = Fixture::new();
    let input = fixture.refresh_input();
    let cold = rows(fixture.run(&policy(1), &input));
    assert_eq!(cold.len(), 4);
    assert!(cold[2]["result"]["payload"].get("frontend_work").is_none());
    for (frontend, semantic) in [(false, false), (true, false), (true, true)] {
        let mut selected = policy(4);
        selected["frontend_cache"] = json!(frontend);
        selected["semantic_cache"] = json!(semantic);
        let observed = rows(fixture.run(&selected, &input));
        assert_eq!(observed.len(), 4);
        assert_eq!(observed[0], cold[0]);
        assert_eq!(observed[1], cold[1]);
        assert_eq!(
            observed[2]["result"]["image_revision"],
            cold[2]["result"]["image_revision"]
        );
        assert_eq!(observed[3]["error"]["code"], -32602); // Requests never select the cache.
        let report = &observed[2]["result"]["payload"];
        assert_eq!(report["image_arc_reused"], true);
        if !frontend {
            assert!(report.get("frontend_work").is_none());
            continue;
        }
        let work = &report["frontend_work"];
        assert_eq!(
            work["schema"],
            if semantic {
                "semaprax.project-semantic-cache-work.v1"
            } else {
                "semaprax.project-frontend-cache-work.v1"
            }
        );
        assert_eq!(work["work"]["modules_parsed"], 0);
        assert_eq!(work["work"]["modules_reused"], 3);
        assert_eq!(
            work["work"]["modules_resolved"],
            if semantic { 0 } else { 3 }
        );
        assert_eq!(
            work["work"]["checked_HIR_reused"],
            if semantic { 3 } else { 0 }
        );
        assert_eq!(work["work"]["full_cross_file_checks"], true);
        assert_eq!(work["work"]["full_link_and_profile_admission"], true);
    }
}

#[test]
fn older_host_policies_reject_semantic_cache_even_when_false() {
    let fixture = Fixture::new();
    for version in 1..=3 {
        for enabled in [false, true] {
            let mut selected = policy(version);
            selected["semantic_cache"] = json!(enabled);
            let output = fixture.run(&selected, "");
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G280"));
        }
    }
}

#[test]
fn v4_selection_is_required_boolean_closed_and_requires_frontend_cache() {
    let fixture = Fixture::new();
    let mut missing = policy(4);
    missing.as_object_mut().unwrap().remove("semantic_cache");
    let mut invalid = vec![missing];
    for wrong in [Value::Null, json!(1), json!("true"), json!({})] {
        let mut selected = policy(4);
        selected["frontend_cache"] = json!(true);
        selected["semantic_cache"] = wrong;
        invalid.push(selected);
    }
    let mut without_frontend = policy(4);
    without_frontend["semantic_cache"] = json!(true);
    invalid.push(without_frontend);
    let mut unknown = policy(4);
    unknown["semantic_cache_root"] = json!("/tmp/request-selected");
    invalid.push(unknown);
    for selected in invalid {
        let output = fixture.run(&selected, "");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("SPX-G280"));
    }
}

fn session_call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if method.starts_with("workspace/refresh") {
        params["image_revision"] = json!(session.image_revision());
    }
    let request = json!({"jsonrpc":"2.0","id":"semantic-refresh","method":method,"params":params})
        .to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn session_payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn checked_work(report: &Value, resolved: usize, reused: usize) {
    let work = &report["frontend_work"];
    assert_eq!(work["schema"], "semaprax.project-semantic-cache-work.v1");
    assert_eq!(work["work"]["modules_parsed"], resolved);
    assert_eq!(work["work"]["modules_resolved"], resolved);
    assert_eq!(work["work"]["checked_HIR_reused"], reused);
    assert_eq!(work["work"]["full_cross_file_checks"], true);
    assert_eq!(work["work"]["full_link_and_profile_admission"], true);
}

#[test]
fn semantic_refresh_preview_and_wrong_expectation_do_not_adopt_or_revive_stale_state() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("semaprax.toml");
    let mut session =
        VNextSession::open_with_semantic_cache(&manifest, VNextPolicy::default()).unwrap();
    let original_image = session.image_revision().to_owned();
    let original_revision = with_authenticated_project(&manifest, |snapshot| {
        Ok(snapshot.project_revision().to_owned())
    })
    .unwrap();
    let app = fixture.0.join("src/app.spx");
    let source = std::fs::read_to_string(&app).unwrap();
    assert!(source.contains("multiply(6, 7)"));
    let changed = source.replace("multiply(6, 7)", "multiply(6, 8)");
    let ast = semaprax::parse(&changed, "src/app.spx").unwrap();
    std::fs::write(&app, semaprax::format::canonical(&ast)).unwrap();
    assert!(session_call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_some());
    let preview = session_payload(session_call(
        &mut session,
        "workspace/refresh-preview",
        json!({}),
    ));
    checked_work(&preview, 1, 2);
    assert_eq!(preview["current_state_replaced"], false);
    assert_eq!(session.image_revision(), original_image);
    assert!(session_call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_some());
    assert!(session_call(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":original_revision})
    )
    .get("error")
    .is_some());
    assert_eq!(session.image_revision(), original_image);
    let current_revision = preview["observed_project_revision"].as_str().unwrap();
    let refreshed = session_payload(session_call(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":current_revision}),
    ));
    checked_work(&refreshed, 1, 2); // Neither preview nor failed refresh primed the live cache.
    let cold = VNextSession::open(&manifest, VNextPolicy::default()).unwrap();
    assert_eq!(session.image_revision(), cold.image_revision());
    let warm = session_payload(session_call(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":current_revision}),
    ));
    checked_work(&warm, 0, 3);
    assert_eq!(warm["image_arc_reused"], true);
    assert!(session_call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_none());
    session.finish().unwrap();
}
