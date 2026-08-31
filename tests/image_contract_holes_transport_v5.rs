//! Contract expression transport coverage. Authored and intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const TARGET: &str = "calculator.divide";
const CATALOG: &str = "candidate/contract-expression-catalog";
const OPEN: &str = "hole/open-contract-expression";
const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-contract-hole-transport-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in FILES {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        let path = root.join("src/core.spx");
        let source = std::fs::read_to_string(&path).unwrap();
        assert!(source.contains("requires right != 0\n"));
        std::fs::write(
            path,
            source.replace(
                "requires right != 0\n",
                "requires right != 0\n    ensures result == result\n",
            ),
        )
        .unwrap();
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
        FILES
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
fn frame(method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":"contract-holes","method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    serde_json::from_slice(&session.handle_frame(&frame(method, params)).unwrap()).unwrap()
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    params["image_revision"] = json!(session.image_revision());
    call(session, method, params)
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn id(value: Value, key: &str) -> String {
    value[key].as_str().unwrap().to_owned()
}
fn open(session: &mut VNextSession) -> String {
    id(
        payload(bound(session, "candidate/open", json!({}))),
        "candidate_revision",
    )
}
fn catalog(session: &mut VNextSession, candidate: &str) -> Value {
    payload(bound(
        session,
        CATALOG,
        json!({"candidate_revision":candidate,"target":TARGET}),
    ))
}
fn selected(catalog: &Value, phase: &str) -> String {
    let row = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| {
            row["phase"] == phase && row["replaceable"] == true && row["expected_type"] == "bool"
        })
        .max_by_key(|row| {
            row["source_span"]["end"].as_u64().unwrap()
                - row["source_span"]["start"].as_u64().unwrap()
        })
        .unwrap();
    row["expression_id"].as_str().unwrap().to_owned()
}
fn query(session: &mut VNextSession, draft: &str, hole: &str) -> Value {
    payload(bound(
        session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":hole}),
    ))
}

#[test]
fn phase_scope_failed_fills_and_completion_are_bound_to_immutable_drafts() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(true);
    let candidate = open(&mut session);
    let catalog = catalog(&mut session, &candidate);
    assert_eq!(
        catalog["schema"],
        "semaprax.project-contract-expression-catalog.v1"
    );
    assert!(catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .all(|row| row["phase"] == "requires" || row["phase"] == "ensures"));
    let requires = selected(&catalog, "requires");
    let ensures = selected(&catalog, "ensures");
    // The pre-existing selector remains body-only.
    assert!(bound(&mut session, "hole/open-expression", json!({"candidate_revision":candidate,"target":TARGET,"expression_id":requires,"hole_id":"old"})).get("error").is_some());
    let draft = id(
        payload(bound(
            &mut session,
            OPEN,
            json!({"candidate_revision":candidate,"target":TARGET,"expression_id":requires,"hole_id":"requires"}),
        )),
        "draft_revision",
    );
    let draft = id(
        payload(bound(
            &mut session,
            OPEN,
            json!({"candidate_revision":candidate,"draft_revision":draft,"target":TARGET,"expression_id":ensures,"hole_id":"ensures"}),
        )),
        "draft_revision",
    );
    let required = query(&mut session, &draft, "requires");
    let ensured = query(&mut session, &draft, "ensures");
    for (context, phase) in [(&required, "requires"), (&ensured, "ensures")] {
        assert_eq!(
            context["schema"],
            "semaprax.project-candidate-contract-expression-hole-context.v1"
        );
        assert_eq!(context["selected_expression"]["phase"], phase);
        assert_eq!(context["materializable"], false);
        assert_eq!(context["source_authority"], false);
        assert_eq!(
            context["scope"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["name"] == "result"),
            phase == "ensures"
        );
    }
    assert!(bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":draft})
    )
    .get("error")
    .is_some());
    for expression in [
        json!({"kind":"place","name":"result"}),
        json!({"kind":"i64","value":1}),
    ] {
        assert!(bound(
            &mut session,
            "hole/fill",
            json!({"draft_revision":draft,"hole_id":"requires","expression":expression})
        )
        .get("error")
        .is_some());
        assert_eq!(query(&mut session, &draft, "requires"), required);
        assert_eq!(query(&mut session, &draft, "ensures"), ensured);
    }
    let filled = id(
        payload(bound(
            &mut session,
            "hole/fill",
            json!({"draft_revision":draft,"hole_id":"requires","expression":{"kind":"bool","value":true}}),
        )),
        "draft_revision",
    );
    assert_eq!(
        query(&mut session, &filled, "ensures")["selected_expression"]["phase"],
        "ensures"
    );
    let filled = id(
        payload(bound(
            &mut session,
            "hole/fill",
            json!({"draft_revision":filled,"hole_id":"ensures","expression":{"kind":"bool","value":true}}),
        )),
        "draft_revision",
    );
    let completed = payload(bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":filled}),
    ));
    assert_ne!(completed["candidate_revision"], candidate);
    assert!(bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":draft})
    )
    .get("error")
    .is_some());
    session.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn body_and_contract_holes_recover_together_without_materializing_a_candidate() {
    let fixture = Fixture::new();
    let before = fixture.bytes();
    let mut session = fixture.session(true);
    let candidate = open(&mut session);
    let expression = selected(&catalog(&mut session, &candidate), "requires");
    let draft = id(
        payload(bound(
            &mut session,
            "hole/open",
            json!({"candidate_revision":candidate,"target":TARGET,"hole_id":"body"}),
        )),
        "draft_revision",
    );
    let draft = id(
        payload(bound(
            &mut session,
            OPEN,
            json!({"candidate_revision":candidate,"draft_revision":draft,"target":TARGET,"expression_id":expression,"hole_id":"contract"}),
        )),
        "draft_revision",
    );
    let expected = query(&mut session, &draft, "contract");
    let mut bytes = String::new();
    for _ in 0..1025 {
        let part = payload(bound(
            &mut session,
            "hole/recovery-export",
            json!({"draft_revision":draft,"offset":bytes.len(),"chunk_bytes":65536}),
        ));
        bytes.push_str(part["chunk"].as_str().unwrap());
        if part["next_offset"].is_null() {
            break;
        }
    }
    let capsule: Value = serde_json::from_str(&bytes).unwrap();
    assert!(capsule["holes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|hole| hole["kind"] == "contract_expression"));
    let mut restored = fixture.session(true);
    let handle = payload(bound(
        &mut restored,
        "hole/recovery-restore",
        json!({"capsule":capsule}),
    ));
    assert_eq!(handle["draft_revision"], draft);
    assert_eq!(query(&mut restored, &draft, "contract"), expected);
    assert_eq!(
        query(&mut restored, &draft, "body")["materializable"],
        false
    );
    assert!(bound(
        &mut restored,
        "hole/complete",
        json!({"draft_revision":draft})
    )
    .get("error")
    .is_some());
    session.finish().unwrap();
    restored.finish().unwrap();
    assert_eq!(fixture.bytes(), before);
}

#[test]
fn discovery_selects_only_v5_candidate_authority_and_keeps_request_selectors_closed() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(false);
    for method in [CATALOG, OPEN] {
        assert_eq!(
            call(&mut readonly, method, json!({}))["error"]["code"],
            -32601
        );
    }
    let mut session = fixture.session(true);
    let schemas = payload(call(&mut session, "protocol/schemas", json!({})));
    for method in [CATALOG, OPEN] {
        let row = schemas["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == method)
            .unwrap();
        assert_eq!(row["capability"], "candidate_prepare");
        assert_eq!(row["query"], method == CATALOG);
        let params = &row["request_schema"]["properties"]["params"];
        assert_eq!(params["additionalProperties"], false);
        for name in ["phase", "path", "ordinal", "source", "approve"] {
            assert!(params["properties"].get(name).is_none());
        }
        if method == OPEN {
            assert_eq!(params["properties"]["expression_id"]["maxLength"], 4096);
        }
        assert_eq!(
            session.parallel_read_methods().contains(&method),
            method == CATALOG
        );
    }
    let candidate = open(&mut session);
    let request = frame(
        CATALOG,
        json!({"image_revision":session.image_revision(),"candidate_revision":candidate,"target":TARGET}),
    );
    let sequential = session.handle_frame(&request);
    let read: Value = serde_json::from_slice(sequential.as_ref().unwrap()).unwrap();
    assert!(read.get("error").is_none(), "{read}");
    for workers in [1, 2, 4] {
        assert_eq!(
            session
                .handle_read_batch(&[request.as_slice()], workers)
                .unwrap(),
            vec![sequential.clone()]
        );
    }
    for schema in [
        "urn:semaprax.project-contract-expression-catalog.v1",
        "urn:semaprax.project-candidate-contract-expression-hole-context.v1",
    ] {
        assert!(schemas["unbundled_payload_schemas"]
            .as_array()
            .unwrap()
            .contains(&json!(schema)));
    }
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        for name in [
            "request_candidate_contract_expression_catalog",
            "request_hole_open_contract_expression",
        ] {
            assert!(client["source"].as_str().unwrap().contains(name));
        }
    }
    for profile in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
        ImageHostCapability::TestEnabled,
        ImageHostCapability::CandidateDiagnostics,
        ImageHostCapability::DiagnosticTests,
    ] {
        let mut legacy = ImageSession::open(&fixture.manifest(), profile).unwrap();
        for method in [CATALOG, OPEN] {
            let response: Value =
                serde_json::from_slice(&legacy.handle_frame(&frame(method, json!({}))).unwrap())
                    .unwrap();
            assert_eq!(response["error"]["code"], -32601);
        }
    }
}
