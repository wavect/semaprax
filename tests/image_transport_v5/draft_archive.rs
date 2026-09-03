//! Draft archive transport evidence, authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-archive-v5-{}-{}",
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
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
    fn change_source(&self) {
        let path = self.0.join("src/core.spx");
        let original = std::fs::read_to_string(&path).unwrap();
        let changed = original.replace("left + right", "left + right + 0");
        assert_ne!(original, changed);
        let parsed = semaprax::parse(&changed, "src/core.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
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
fn export(session: &mut VNextSession, draft: &Value) -> Value {
    let mut bytes = String::new();
    let mut offset = 0;
    let mut digest = Value::Null;
    loop {
        let chunk = payload(bound(
            session,
            "hole/archive-export",
            json!({"draft_revision":draft["draft_revision"],"offset":offset,"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["schema"], "semaprax.image-draft-archive-chunk.v1");
        assert_eq!(
            chunk["archive_schema"],
            "semaprax.project-candidate-draft-archive.v1"
        );
        assert_eq!(chunk["draft_revision"], draft["draft_revision"]);
        assert_eq!(chunk["image_revision"], session.image_revision());
        for field in [
            "source_authority",
            "approval_authority",
            "trusted_hir",
            "materializable",
        ] {
            assert_eq!(chunk[field], false);
        }
        if digest.is_null() {
            digest = chunk["archive_revision"].clone();
        } else {
            assert_eq!(digest, chunk["archive_revision"]);
        }
        assert_eq!(chunk["offset"], offset);
        bytes.push_str(chunk["chunk"].as_str().unwrap());
        match chunk["next_offset"].as_u64() {
            Some(next) => {
                assert!(next > offset);
                offset = next;
            }
            None => {
                assert_eq!(chunk["total_bytes"], bytes.len());
                break;
            }
        }
    }
    let archive: Value = serde_json::from_str(&bytes).unwrap();
    assert_eq!(archive["archive_digest"], digest);
    assert_eq!(archive["draft_digest"], draft["draft_revision"]);
    json!({"archive":archive,"archive_revision":digest,"draft_revision":draft["draft_revision"]})
}
fn diagnostic(response: &Value, code: &str) {
    assert!(response.get("error").is_some(), "{response}");
    assert!(response.to_string().contains(code), "{response}");
}

#[test]
fn chunks_restart_restore_only_draft_and_completion_requires_all_pending_fills() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut first = fixture.session(true);
    let opened = draft(&mut first);
    let partial = payload(bound(
        &mut first,
        "hole/fill",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add","expression":{"kind":"i64","value":17}}),
    ));
    let context = payload(bound(
        &mut first,
        "hole/query",
        json!({"draft_revision":partial["draft_revision"],"hole_id":"subtract"}),
    ));
    let saved = export(&mut first, &partial);
    assert_eq!(
        payload(bound(&mut first, "hole/archive-restore", saved.clone())),
        partial
    );
    drop(first);
    let mut next = fixture.session(true);
    let image = next.image_revision().to_owned();
    let restored = payload(bound(&mut next, "hole/archive-restore", saved.clone()));
    assert_eq!(restored["draft_revision"], partial["draft_revision"]);
    assert_eq!(restored["source_authority"], false);
    assert_eq!(restored["buildable"], false);
    assert_eq!(export(&mut next, &restored), saved);
    assert_eq!(
        payload(bound(
            &mut next,
            "hole/query",
            json!({"draft_revision":restored["draft_revision"],"hole_id":"subtract"})
        )),
        context
    );
    assert!(bound(
        &mut next,
        "candidate/query",
        json!({"candidate_revision":restored["source_candidate_revision"]})
    )
    .get("error")
    .is_some());
    diagnostic(
        &bound(
            &mut next,
            "hole/complete",
            json!({"draft_revision":restored["draft_revision"]}),
        ),
        "SPX-G232",
    );
    let ready = payload(bound(
        &mut next,
        "hole/fill",
        json!({"draft_revision":restored["draft_revision"],"hole_id":"subtract","expression":{"kind":"i64","value":23}}),
    ));
    let completed = payload(bound(
        &mut next,
        "hole/complete",
        json!({"draft_revision":ready["draft_revision"]}),
    ));
    assert!(completed["candidate_revision"].is_string());
    assert_eq!(completed["source_authority"], false);
    assert_eq!(completed["tests"], "not_run");
    assert_eq!(next.image_revision(), image);
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn archive_methods_are_closed_candidate_granted_v5_only_and_generated_clients_describe_them() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(false);
    for name in ["hole/archive-export", "hole/archive-restore"] {
        assert_eq!(
            bound(&mut readonly, name, json!({}))["error"]["code"],
            -32601
        );
        for capability in [
            ImageHostCapability::ReadOnly,
            ImageHostCapability::CandidateOnly,
            ImageHostCapability::TestEnabled,
            ImageHostCapability::CandidateDiagnostics,
        ] {
            let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
            let frame = json!({"jsonrpc":"2.0","id":1,"method":name,"params":{"image_revision":old.image_revision()}}).to_string();
            let response: Value =
                serde_json::from_slice(&old.handle_frame(frame.as_bytes()).unwrap()).unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
    let mut enabled = fixture.session(true);
    let schema = payload(call(&mut enabled, "protocol/schemas", json!({})));
    for name in ["hole/archive-export", "hole/archive-restore"] {
        let method = schema["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["method"] == name)
            .unwrap();
        assert_eq!(method["capability"], "candidate_prepare");
        assert_eq!(method["query"], name.ends_with("export"));
        let params = &method["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        if name.ends_with("restore") {
            for required in [
                "archive",
                "archive_revision",
                "draft_revision",
                "image_revision",
            ] {
                assert!(params["required"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(required)));
            }
        }
    }
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut enabled,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        assert!(source.contains("hole/archive-export"));
        assert!(source.contains("hole/archive-restore"));
        assert_eq!(client["io"], false);
    }
}

#[test]
fn wrong_selectors_tampering_extra_request_fields_and_changed_current_base_install_nothing() {
    let fixture = Fixture::new();
    let mut first = fixture.session(true);
    let opened = draft(&mut first);
    let saved = export(&mut first, &opened);
    drop(first);
    let mut next = fixture.session(true);
    for field in ["archive_revision", "draft_revision"] {
        let mut wrong = saved.clone();
        wrong[field] = json!(format!("sha256:{}", "0".repeat(64)));
        diagnostic(&bound(&mut next, "hole/archive-restore", wrong), "SPX-G342");
    }
    let mut wrong = saved.clone();
    wrong["archive"]["source_authority"] = json!(true);
    diagnostic(&bound(&mut next, "hole/archive-restore", wrong), "SPX-G340");
    let mut wrong = saved.clone();
    wrong["path"] = json!("ignored.json");
    assert_eq!(
        bound(&mut next, "hole/archive-restore", wrong)["error"]["code"],
        -32602
    );
    assert!(bound(
        &mut next,
        "hole/query",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add"})
    )
    .get("error")
    .is_some());
    let mut stale = saved.clone();
    stale["image_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    assert!(call(&mut next, "hole/archive-restore", stale)
        .get("error")
        .is_some());
    drop(next);
    fixture.change_source();
    let disk = fixture.bytes();
    let mut changed = fixture.session(true);
    diagnostic(
        &bound(&mut changed, "hole/archive-restore", saved),
        "SPX-G342",
    );
    assert!(bound(
        &mut changed,
        "hole/query",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add"})
    )
    .get("error")
    .is_some());
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn host_startup_restores_historical_draft_after_original_deletion_without_replacing_current_image()
{
    let original = Fixture::new();
    let original_root = original.0.clone();
    let mut first = original.session(true);
    let opened = draft(&mut first);
    let partial = payload(bound(
        &mut first,
        "hole/fill",
        json!({"draft_revision":opened["draft_revision"],"hole_id":"add","expression":{"kind":"i64","value":17}}),
    ));
    let context = payload(bound(
        &mut first,
        "hole/query",
        json!({"draft_revision":partial["draft_revision"],"hole_id":"subtract"}),
    ));
    let image = first.image_revision().to_owned();
    let saved = first
        .export_draft_archive(&image, partial["draft_revision"].as_str().unwrap())
        .unwrap();
    drop(first);
    drop(original);
    assert!(!original_root.exists());
    let sibling = Fixture::new();
    sibling.change_source();
    let disk = sibling.bytes();
    let mut current = sibling.session(true);
    let current_image = current.image_revision().to_owned();
    assert_ne!(image, current_image);
    let restored: Value = serde_json::from_str(
        &current
            .restore_draft_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.draft_digest(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(restored["draft_revision"], saved.draft_digest());
    assert_eq!(restored["source_authority"], false);
    assert_eq!(current.image_revision(), current_image);
    assert_eq!(
        current
            .export_draft_archive(&current_image, saved.draft_digest())
            .unwrap()
            .to_json(),
        saved.to_json()
    );
    assert_eq!(
        payload(bound(
            &mut current,
            "hole/query",
            json!({"draft_revision":saved.draft_digest(),"hole_id":"subtract"})
        )),
        context
    );
    assert!(bound(
        &mut current,
        "candidate/query",
        json!({"candidate_revision":restored["source_candidate_revision"]})
    )
    .get("error")
    .is_some());
    let ready = payload(bound(
        &mut current,
        "hole/fill",
        json!({"draft_revision":saved.draft_digest(),"hole_id":"subtract","expression":{"kind":"i64","value":23}}),
    ));
    let completed = payload(bound(
        &mut current,
        "hole/complete",
        json!({"draft_revision":ready["draft_revision"]}),
    ));
    assert_eq!(completed["base_revision"], saved.base_revision());
    assert_eq!(completed["source_authority"], false);
    assert_eq!(
        bound(
            &mut current,
            "candidate/commit",
            json!({"candidate_revision":completed["candidate_revision"]})
        )["error"]["code"],
        -32601
    );
    assert_eq!(current.image_revision(), current_image);
    assert_eq!(sibling.bytes(), disk);
    assert!(!original_root.exists());
}

#[test]
fn host_startup_fences_manifest_permission_late_frames_and_bounded_rpc_without_draft_installation()
{
    let fixture = Fixture::new();
    let mut first = fixture.session(true);
    let opened = draft(&mut first);
    let image = first.image_revision().to_owned();
    let saved = first
        .export_draft_archive(&image, opened["draft_revision"].as_str().unwrap())
        .unwrap();
    let late = first
        .restore_draft_archive(
            saved.to_json().as_bytes(),
            saved.archive_digest(),
            saved.draft_digest(),
        )
        .err()
        .unwrap();
    assert_eq!(late[0].code, "SPX-G303");
    let mut readonly = fixture.session(false);
    assert_eq!(
        readonly
            .restore_draft_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.draft_digest()
            )
            .err()
            .unwrap()[0]
            .code,
        "SPX-G303"
    );
    assert_eq!(
        readonly
            .export_draft_archive(&image, saved.draft_digest())
            .err()
            .unwrap()[0]
            .code,
        "SPX-G303"
    );
    let wrong = format!("sha256:{}", "0".repeat(64));
    assert_eq!(
        first
            .export_draft_archive(&wrong, saved.draft_digest())
            .err()
            .unwrap()[0]
            .code,
        "SPX-G282"
    );
    assert!(first
        .parallel_read_methods()
        .contains(&"hole/archive-export"));
    let request = json!({"jsonrpc":"2.0","id":"archive-batch","method":"hole/archive-export","params":{"image_revision":image,"draft_revision":saved.draft_digest(),"chunk_bytes":1024}}).to_string();
    let sequential = first.handle_frame(request.as_bytes());
    let read: Value = serde_json::from_slice(sequential.as_ref().unwrap()).unwrap();
    assert!(read.get("error").is_none(), "{read}");
    assert_eq!(read["result"]["payload"]["source_authority"], false);
    assert_eq!(read["result"]["payload"]["materializable"], false);
    for workers in [1, 2, 4] {
        assert_eq!(
            first
                .handle_read_batch(&[request.as_bytes()], workers)
                .unwrap(),
            vec![sequential.clone()]
        );
    }
    assert!(!first
        .parallel_read_methods()
        .contains(&"hole/archive-restore"));
    let sibling = Fixture::new();
    let manifest = std::fs::read_to_string(sibling.manifest()).unwrap();
    // Change only the package name, keeping every source/module identity admitted.
    let changed = manifest
        .lines()
        .map(|line| {
            if line.starts_with("name = ") {
                "name = \"different-calculator\""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(sibling.manifest(), changed).unwrap();
    let mut other = sibling.session(true);
    assert_eq!(
        other
            .restore_draft_archive(
                saved.to_json().as_bytes(),
                saved.archive_digest(),
                saved.draft_digest()
            )
            .err()
            .unwrap()[0]
            .code,
        "SPX-G342"
    );
    assert!(bound(
        &mut other,
        "hole/query",
        json!({"draft_revision":saved.draft_digest(),"hole_id":"add"})
    )
    .get("error")
    .is_some());
    let mut oversized = fixture.session(true);
    let mut value: Value = serde_json::from_str(saved.to_json()).unwrap();
    value["padding"] = json!("x".repeat(65536));
    let frame = json!({"jsonrpc":"2.0","id":1,"method":"hole/archive-restore","params":{"image_revision":oversized.image_revision(),"archive":value,"archive_revision":saved.archive_digest(),"draft_revision":saved.draft_digest()}}).to_string();
    assert!(frame.len() > 65536);
    let response: Value =
        serde_json::from_slice(&oversized.handle_frame(frame.as_bytes()).unwrap()).unwrap();
    assert_eq!(response["error"]["code"], -32700);
    assert!(oversized.is_terminal());
    assert!(oversized.handle_frame(b"{}").is_none());
    assert_eq!(
        oversized
            .export_draft_archive(&image, saved.draft_digest())
            .err()
            .unwrap()[0]
            .code,
        "SPX-G303"
    );
}
