//! Typed-hole restart recovery evidence, authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-recovery-v5-{}-{}",
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
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self, prepare: bool) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: prepare,
                ..Default::default()
            },
        )
        .unwrap()
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
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn draft(session: &mut VNextSession) -> Value {
    let candidate = payload(bound(session, "candidate/open", json!({})));
    let first = payload(bound(
        session,
        "hole/open",
        json!({"candidate_revision":candidate["candidate_revision"],"target":"calculator.add","hole_id":"add"}),
    ));
    payload(bound(
        session,
        "hole/open",
        json!({"candidate_revision":candidate["candidate_revision"],"draft_revision":first["draft_revision"],"target":"calculator.subtract","hole_id":"subtract"}),
    ))
}
fn export(session: &mut VNextSession, draft: &Value) -> String {
    let mut offset = 0usize;
    let mut bytes = String::new();
    let mut capsule_schema = None;
    loop {
        let chunk = payload(bound(
            session,
            "hole/recovery-export",
            json!({"draft_revision":draft["draft_revision"],"offset":offset,"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["schema"], "semaprax.image-draft-recovery-chunk.v1");
        let selected = chunk["capsule_schema"].as_str().unwrap();
        assert!(matches!(
            selected,
            "semaprax.project-candidate-draft-recovery.v1"
                | "semaprax.project-candidate-draft-recovery.v2"
        ));
        assert!(capsule_schema
            .as_deref()
            .is_none_or(|expected| expected == selected));
        capsule_schema = Some(selected.to_owned());
        assert_eq!(chunk["draft_revision"], draft["draft_revision"]);
        assert_eq!(chunk["materializable"], false);
        assert_eq!(chunk["source_authority"], false);
        assert_eq!(chunk["offset"], offset);
        bytes.push_str(chunk["chunk"].as_str().unwrap());
        match chunk["next_offset"].as_u64() {
            Some(next) => {
                assert!(next as usize > offset);
                offset = next as usize;
            }
            None => {
                assert_eq!(chunk["total_bytes"], bytes.len());
                break;
            }
        }
    }
    let capsule: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(capsule["schema"], capsule_schema.unwrap());
    bytes
}

#[test]
fn restart_restores_only_the_draft_and_requires_all_remaining_fills() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut original = fixture.session(true);
    let opened = draft(&mut original);
    let partial = payload(bound(
        &mut original,
        "hole/fill",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add","expression":{"kind":"i64","value":17}}),
    ));
    let context = payload(bound(
        &mut original,
        "hole/query",
        json!({"draft_revision":partial["draft_revision"],"hole_id":"subtract"}),
    ));
    let saved = export(&mut original, &partial);
    let same_session = payload(bound(
        &mut original,
        "hole/recovery-restore",
        json!({"capsule":serde_json::from_str::<Value>(&saved).unwrap()}),
    ));
    assert_eq!(same_session, partial);
    drop(original);
    let mut restarted = fixture.session(true);
    let restored = payload(bound(
        &mut restarted,
        "hole/recovery-restore",
        json!({"capsule":serde_json::from_str::<Value>(&saved).unwrap()}),
    ));
    assert_eq!(restored["draft_revision"], partial["draft_revision"]);
    assert_eq!(restored["buildable"], false);
    assert_eq!(restored["source_authority"], false);
    assert_eq!(export(&mut restarted, &restored), saved);
    assert_eq!(
        payload(bound(
            &mut restarted,
            "hole/query",
            json!({"draft_revision":restored["draft_revision"],"hole_id":"subtract"})
        )),
        context
    );
    assert!(bound(
        &mut restarted,
        "candidate/query",
        json!({"candidate_revision":restored["source_candidate_revision"]})
    )
    .get("error")
    .is_some());
    assert!(bound(
        &mut restarted,
        "hole/complete",
        json!({"draft_revision":restored["draft_revision"]})
    )
    .get("error")
    .is_some());
    let ready = payload(bound(
        &mut restarted,
        "hole/fill",
        json!({"draft_revision":restored["draft_revision"],"hole_id":"subtract","expression":{"kind":"i64","value":23}}),
    ));
    let complete = payload(bound(
        &mut restarted,
        "hole/complete",
        json!({"draft_revision":ready["draft_revision"]}),
    ));
    assert_eq!(complete["source_authority"], false);
    assert_eq!(complete["tests"], "not_run");
    assert!(complete["candidate_revision"].is_string());
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn recovery_methods_require_v5_candidate_permission_and_are_absent_from_old_profiles() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(false);
    for method in ["hole/recovery-export", "hole/recovery-restore"] {
        assert_eq!(
            bound(&mut readonly, method, json!({}))["error"]["code"],
            -32601
        );
        for capability in [
            ImageHostCapability::ReadOnly,
            ImageHostCapability::CandidateOnly,
            ImageHostCapability::TestEnabled,
            ImageHostCapability::CandidateDiagnostics,
        ] {
            let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
            let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":{"image_revision":old.image_revision()}}).to_string();
            let response: Value =
                serde_json::from_slice(&old.handle_frame(frame.as_bytes()).unwrap()).unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
    let mut enabled = fixture.session(true);
    let schema = payload(call(&mut enabled, "protocol/schemas", json!({})));
    for name in ["hole/recovery-export", "hole/recovery-restore"] {
        let method = schema["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == name)
            .unwrap();
        assert_eq!(method["capability"], "candidate_prepare");
        assert_eq!(method["query"], name.ends_with("export"));
        assert_eq!(
            method["request_schema"]["properties"]["params"]["additionalProperties"],
            false
        );
    }
    for language in ["typescript", "python", "rust"] {
        let generated = payload(call(
            &mut enabled,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = generated["source"].as_str().unwrap();
        assert!(source.contains("hole/recovery-export"));
        assert!(source.contains("hole/recovery-restore"));
        assert_eq!(generated["io"], false);
    }
}

#[test]
fn malformed_and_stale_recovery_requests_do_not_install_drafts() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut first = fixture.session(true);
    let opened = draft(&mut first);
    let saved: Value = serde_json::from_str(&export(&mut first, &opened)).unwrap();
    let mut restarted = fixture.session(true);
    let mut invalid = saved.clone();
    invalid["approval"] = json!(true);
    assert!(bound(
        &mut restarted,
        "hole/recovery-restore",
        json!({"capsule":invalid})
    )
    .get("error")
    .is_some());
    assert!(call(
        &mut restarted,
        "hole/recovery-restore",
        json!({"image_revision":format!("sha256:{}", "0".repeat(64)),"capsule":saved})
    )
    .get("error")
    .is_some());
    assert!(bound(
        &mut restarted,
        "hole/query",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add"})
    )
    .get("error")
    .is_some());
    let restored = payload(bound(
        &mut restarted,
        "hole/recovery-restore",
        json!({"capsule":saved}),
    ));
    assert_eq!(restored["draft_revision"], opened["draft_revision"]);
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn explicit_refresh_clears_drafts_and_recovery_never_remaps_a_changed_source_base() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let opened = draft(&mut session);
    let saved: Value = serde_json::from_str(&export(&mut session, &opened)).unwrap();
    let preview = payload(bound(&mut session, "workspace/refresh-preview", json!({})));
    payload(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":preview["observed_project_revision"]}),
    ));
    assert!(bound(
        &mut session,
        "hole/query",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add"})
    )
    .get("error")
    .is_some());
    payload(bound(
        &mut session,
        "hole/recovery-restore",
        json!({"capsule":saved}),
    ));
    let path = fixture.0.join("src/app.spx");
    let text = std::fs::read_to_string(&path)
        .unwrap()
        .replace("multiply(6, 7)", "multiply(6, 8)");
    std::fs::write(
        path,
        semaprax::format::canonical(&semaprax::parse(&text, "src/app.spx").unwrap()),
    )
    .unwrap();
    let changed = fixture.bytes();
    let preview = payload(bound(&mut session, "workspace/refresh-preview", json!({})));
    payload(bound(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":preview["observed_project_revision"]}),
    ));
    assert!(bound(
        &mut session,
        "hole/recovery-restore",
        json!({"capsule":saved})
    )
    .get("error")
    .is_some());
    assert!(bound(
        &mut session,
        "hole/query",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add"})
    )
    .get("error")
    .is_some());
    assert_eq!(fixture.bytes(), changed);
}
