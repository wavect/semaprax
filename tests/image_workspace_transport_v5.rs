//! V5 lifecycle evidence authored, intentionally unrun locally.
use semaprax::image_transport::{
    ImageHostCapability, ImageSession, VNextPolicy, VNextSession, VNEXT_PROTOCOL_SCHEMA,
    VNEXT_RESULT_SCHEMA,
};
use semaprax::project::with_authenticated_project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-v5-workspace-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self, candidates: bool, diagnostics: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: candidates,
                diagnostics,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn revision(&self) -> String {
        with_authenticated_project(&self.manifest(), |snapshot| {
            Ok(snapshot.project_revision().to_owned())
        })
        .unwrap()
    }
    fn edit_app(&self) {
        let path = self.0.join("src/app.spx");
        let source = std::fs::read_to_string(&path)
            .unwrap()
            .replace("multiply(6, 7)", "multiply(6, 8)");
        let ast = semaprax::parse(&source, "src/app.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&ast)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request = json!({"jsonrpc":"2.0","id":"test","method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["schema"], VNEXT_RESULT_SCHEMA);
    assert_eq!(response["result"]["protocol"], VNEXT_PROTOCOL_SCHEMA);
    response["result"]["payload"].clone()
}
fn open(session: &mut VNextSession) -> String {
    payload(bound(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn explicit_refresh_recovers_absorbing_drift_and_retains_candidates_for_rebase() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true, true);
    let old_image = session.image_revision().to_owned();
    let root = open(&mut session);
    let changed = payload(bound(&mut session,"candidate/apply-intent",json!({"candidate_revision":root,"intent":{"kind":"rename_declaration","target":"calculator.add","name":"addition"}})))["candidate_revision"].as_str().unwrap().to_owned();
    let draft = payload(bound(
        &mut session,
        "hole/open",
        json!({"candidate_revision":root,"target":"calculator.add","hole_id":"body"}),
    ))["draft_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let rejected = payload(bound(
        &mut session,
        "candidate/attempt",
        json!({"candidate_revision":root,"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"i32","value":1}}}),
    ));
    let attempt = rejected["attempt"]["attempt_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    fixture.edit_app();
    assert!(call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_some());
    let preview = payload(bound(&mut session, "workspace/refresh-preview", json!({})));
    let expected = preview["observed_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(expected, fixture.revision());
    assert_eq!(preview["current_state_replaced"], false);
    assert_eq!(session.image_revision(), old_image);
    assert!(
        call(&mut session, "workspace/status", json!({}))
            .get("error")
            .is_some(),
        "preview must not revive the absorbing old snapshot"
    );
    // The old snapshot is now absorbing-invalid; refresh must independently load.
    let refreshed = payload(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":expected}),
    ));
    assert_eq!(refreshed["old_image_revision"], old_image);
    assert_eq!(refreshed["cleared_drafts"], 1);
    assert_eq!(refreshed["cleared_attempts"], 1);
    assert!(refreshed["retained_candidates"]
        .as_array()
        .unwrap()
        .contains(&json!(changed)));
    assert_ne!(session.image_revision(), old_image);
    assert!(bound(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":"body"})
    )
    .get("error")
    .is_some());
    assert!(bound(
        &mut session,
        "attempt/summary",
        json!({"attempt_revision":attempt})
    )
    .get("error")
    .is_some());
    payload(bound(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":changed}),
    ));
    let new_base = open(&mut session);
    payload(bound(
        &mut session,
        "candidate/rebase",
        json!({"candidate_revision":changed,"new_base_candidate_revision":new_base}),
    ));
    let old = call(
        &mut session,
        "image/symbol",
        json!({"image_revision":old_image,"stable_id":"calculator.add"}),
    );
    assert!(old["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    session.finish().unwrap();
}

#[test]
fn wrong_expected_refresh_and_malformed_notifications_leave_old_state_intact() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true, false);
    let old_image = session.image_revision().to_owned();
    let old_revision = fixture.revision();
    let root = open(&mut session);
    payload(bound(
        &mut session,
        "hole/open",
        json!({"candidate_revision":root,"target":"calculator.add","hole_id":"body"}),
    ));
    fixture.edit_app();
    let expected = fixture.revision();
    let notification = json!({"jsonrpc":"2.0","method":"workspace/refresh","params":{"image_revision":old_image,"expected_new_project_revision":expected}}).to_string();
    assert!(session.handle_frame(notification.as_bytes()).is_none());
    assert_eq!(session.image_revision(), old_image);
    assert!(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":expected,"path":"elsewhere"})
    )
    .get("error")
    .is_some());
    let failed = bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":old_revision}),
    );
    assert!(failed["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G282"));
    assert_eq!(session.image_revision(), old_image);
    let refreshed = payload(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":expected}),
    ));
    assert_eq!(
        refreshed["cleared_drafts"], 1,
        "failed refresh did not discard pending state"
    );
    assert!(refreshed["retained_candidates"]
        .as_array()
        .unwrap()
        .contains(&json!(root)));
}

#[test]
fn configuration_change_rejects_and_unchanged_explicit_refresh_reports_transient_clear() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true, false);
    let old = session.image_revision().to_owned();
    let root = open(&mut session);
    payload(bound(
        &mut session,
        "hole/open",
        json!({"candidate_revision":root,"target":"calculator.add","hole_id":"body"}),
    ));
    let unchanged = payload(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":fixture.revision()}),
    ));
    assert_eq!(unchanged["image_arc_reused"], true);
    assert_eq!(unchanged["cleared_drafts"], 1);
    assert_eq!(session.image_revision(), old);
    let path = fixture.manifest();
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("name = \"calculator\"", "name = \"calculator-renamed\"");
    std::fs::write(path, changed).unwrap();
    let result = bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":fixture.revision()}),
    );
    assert!(result["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G283"));
    assert_eq!(session.image_revision(), old);
}

#[test]
fn v5_catalog_reflects_actual_independent_host_capabilities_without_elevation() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(false, false);
    let capabilities = payload(call(&mut readonly, "protocol/capabilities", json!({})));
    let methods = capabilities["methods"].as_array().unwrap();
    assert!(methods.contains(&json!("workspace/refresh")));
    assert!(methods.contains(&json!("workspace/refresh-preview")));
    assert!(methods.contains(&json!("protocol/conformance")));
    assert!(methods.contains(&json!("image/target-admission")));
    assert!(!methods.contains(&json!("candidate/open")));
    assert!(!methods.contains(&json!("candidate/build")));
    assert!(!methods.contains(&json!("candidate/test")));
    assert!(!methods.contains(&json!("candidate/commit")));
    assert_eq!(
        bound(&mut readonly, "candidate/open", json!({}))["error"]["code"],
        -32601
    );
    let mut candidates = fixture.session(true, false);
    let capabilities = payload(call(&mut candidates, "protocol/capabilities", json!({})));
    let methods = capabilities["methods"].as_array().unwrap();
    assert!(methods.contains(&json!("candidate/interface-catalog")));
    assert!(methods.contains(&json!("hole/open-expression")));
    assert!(!methods.contains(&json!("candidate/attempt")));
    assert!(!methods.contains(&json!("candidate/test")));
    assert!(VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            diagnostics: true,
            ..Default::default()
        }
    )
    .is_err());
    assert!(VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            build_enabled: true,
            ..Default::default()
        }
    )
    .is_err());
}

#[test]
fn legacy_sessions_keep_their_protocol_and_exclude_explicit_refresh() {
    let fixture = Fixture::new();
    for (profile, schema) in [
        (
            ImageHostCapability::ReadOnly,
            "semaprax.image-agent-protocol.v1",
        ),
        (
            ImageHostCapability::CandidateOnly,
            "semaprax.image-agent-protocol.v2",
        ),
        (
            ImageHostCapability::CandidateDiagnostics,
            "semaprax.image-agent-protocol.v4",
        ),
    ] {
        let mut session = ImageSession::open(&fixture.manifest(), profile).unwrap();
        let open = br#"{"jsonrpc":"2.0","id":1,"method":"workspace/open","params":{}}"#;
        let response: Value = serde_json::from_slice(&session.handle_frame(open).unwrap()).unwrap();
        assert_eq!(response["result"]["protocol"], schema);
        let refresh = json!({"jsonrpc":"2.0","id":2,"method":"workspace/refresh","params":{"image_revision":session.image_revision(),"expected_new_project_revision":fixture.revision()}}).to_string();
        let response: Value =
            serde_json::from_slice(&session.handle_frame(refresh.as_bytes()).unwrap()).unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}

#[test]
fn interface_delta_chunks_reconstruct_the_exact_library_report() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.0.join("src/core.spx")).unwrap();
    let expected = with_authenticated_project(&fixture.manifest(), |snapshot| {
        let candidate = semaprax::project::ProjectCandidate::open(
            snapshot.retain_revision(),
            snapshot.project_revision(),
        )?;
        candidate.interface_delta(candidate.candidate_digest())
    })
    .unwrap();
    let mut session = fixture.session(true, false);
    let candidate = open(&mut session);
    let mut offset = 0;
    let mut report = String::new();
    loop {
        let chunk = payload(bound(
            &mut session,
            "candidate/interface-delta",
            json!({"candidate_revision":candidate,"offset":offset,"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["schema"], "semaprax.image-interface-delta-chunk.v1");
        assert_eq!(
            chunk["report_schema"],
            "semaprax.project-candidate-interface-delta.v1"
        );
        assert_eq!(chunk["candidate_revision"], candidate);
        assert_eq!(chunk["source_authority"], false);
        report.push_str(chunk["chunk"].as_str().unwrap());
        match chunk["next_offset"].as_u64() {
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
            None => break,
        }
    }
    assert_eq!(report, expected);
    let bad = bound(
        &mut session,
        "candidate/interface-delta",
        json!({"candidate_revision":candidate,"offset":report.len()+1}),
    );
    assert!(bad["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G310"));
    let unknown = bound(
        &mut session,
        "candidate/interface-delta",
        json!({"candidate_revision":format!("sha256:{}", "0".repeat(64))}),
    );
    assert!(unknown.get("error").is_some());
    assert_eq!(
        std::fs::read(fixture.0.join("src/core.spx")).unwrap(),
        before
    );
    session.finish().unwrap();
}

#[test]
fn review_facet_discovery_is_host_selected_and_legacy_profiles_stay_closed() {
    let fixture = Fixture::new();
    for (candidates, diagnostics) in [(false, false), (true, false), (true, true)] {
        let mut session = fixture.session(candidates, diagnostics);
        let capabilities = payload(call(&mut session, "protocol/capabilities", json!({})));
        let methods = capabilities["methods"].as_array().unwrap();
        assert_eq!(
            methods.contains(&json!("candidate/interface-delta")),
            candidates
        );
        assert_eq!(
            methods.contains(&json!("candidate/symbol-diagnostics")),
            diagnostics
        );
        assert!(!session
            .parallel_read_methods()
            .contains(&"candidate/interface-delta"));
        assert!(!session
            .parallel_read_methods()
            .contains(&"candidate/symbol-diagnostics"));
        if !diagnostics {
            let rejected = bound(
                &mut session,
                "candidate/symbol-diagnostics",
                json!({"candidate_revision":format!("sha256:{}", "0".repeat(64)),"target":"calculator.add"}),
            );
            assert_eq!(rejected["error"]["code"], -32601);
        }
        session.finish().unwrap();
    }
    let mut legacy = ImageSession::open(
        &fixture.manifest(),
        ImageHostCapability::CandidateDiagnostics,
    )
    .unwrap();
    for method in ["candidate/interface-delta", "candidate/symbol-diagnostics"] {
        let request = json!({"jsonrpc":"2.0","id":1,"method":method,"params":{}}).to_string();
        let rejected: Value =
            serde_json::from_slice(&legacy.handle_frame(request.as_bytes()).unwrap()).unwrap();
        assert_eq!(rejected["error"]["code"], -32601);
    }
}
