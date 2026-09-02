//! Draft-bound current expression discovery. Authored; intentionally unrun.
use semaprax::image_transport::{ImageHostCapability, ImageSession, VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectCandidateDraft,
    PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const METHOD: &str = "hole/expression-catalog";
const UNKNOWN: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-draft-expression-v5-{}-{}",
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
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
    fn draft(&self) -> ProjectCandidateDraft {
        let draft = ProjectCandidateDraft::open(Arc::new(self.candidate())).unwrap();
        let draft = draft
            .with_body_hole(draft.draft_digest(), "calculator.add", "add")
            .unwrap();
        draft
            .with_body_hole(draft.draft_digest(), "calculator.subtract", "subtract")
            .unwrap()
    }
    fn session(&self) -> (VNextSession, String, String) {
        let mut session = VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap();
        let base = self.candidate();
        let candidate = base.candidate_digest().to_owned();
        session.retain_archived_candidate(base, &candidate).unwrap();
        let draft = self.draft();
        let revision = draft.draft_digest().to_owned();
        session.retain_archived_draft(draft, &revision).unwrap();
        (session, candidate, revision)
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
fn frame(id: usize, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if !matches!(method, "protocol/schemas" | "protocol/client") {
        params["image_revision"] = json!(session.image_revision());
    }
    serde_json::from_slice(&session.handle_frame(&frame(1, method, params)).unwrap()).unwrap()
}
fn payload(value: Value) -> Value {
    assert!(value.get("error").is_none(), "{value}");
    value["result"]["payload"].clone()
}
fn rejected(value: Value, code: &str) {
    assert!(
        value["error"]["message"].as_str().unwrap().contains(code),
        "{value}"
    );
}
fn params(draft: &str, target: &str, region: &str) -> Value {
    json!({"draft_revision":draft,"target":target,"region":region})
}
fn replacement() -> Value {
    json!({"kind":"let","name":"sum","value":{"kind":"binary","op":"+","left":{"kind":"place","name":"left"},"right":{"kind":"place","name":"right"}},"body":{"kind":"place","name":"sum"}})
}

#[test]
fn both_regions_match_the_library_and_do_not_complete_or_publish_the_draft() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let draft = fixture.draft();
    let before = draft.to_json().to_owned();
    let (mut session, _, revision) = fixture.session();
    for region in ["body", "contract"] {
        let expected: Value = serde_json::from_str(
            &if region == "body" {
                draft.expression_catalog(&revision, "calculator.divide")
            } else {
                draft.contract_expression_catalog(&revision, "calculator.divide")
            }
            .unwrap(),
        )
        .unwrap();
        let actual = payload(call(
            &mut session,
            METHOD,
            params(&revision, "calculator.divide", region),
        ));
        assert_eq!(actual, expected);
        assert_eq!(actual.as_object().unwrap().len(), 16);
        assert_eq!(actual["schema"], PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA);
        assert_eq!(actual["draft_revision"], revision);
        assert_eq!(actual["materializable"], false);
        assert_eq!(actual["source_authority"], false);
        assert!(actual.get("candidate_revision").is_none());
        assert!(actual.get("candidate_digest").is_none());
        let rows = actual["expressions"].as_array().unwrap();
        assert!(!rows.is_empty());
        assert!(rows.iter().all(|row| if region == "body" {
            row["phase"] == "body"
        } else {
            row["phase"] == "requires" || row["phase"] == "ensures"
        }));
    }
    rejected(
        call(
            &mut session,
            "hole/complete",
            json!({"draft_revision":revision}),
        ),
        "SPX-G232",
    );
    assert_eq!(draft.to_json(), before);
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn post_fill_catalogue_selects_current_source_without_registering_last_valid_candidate() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let (mut session, base, revision) = fixture.session();
    let before = payload(call(
        &mut session,
        METHOD,
        params(&revision, "calculator.add", "body"),
    ));
    let filled = payload(call(
        &mut session,
        "hole/fill",
        json!({"draft_revision":revision,"hole_id":"add","expression":replacement()}),
    ));
    let current = filled["draft_revision"].as_str().unwrap().to_owned();
    let report = payload(call(
        &mut session,
        METHOD,
        params(&current, "calculator.add", "body"),
    ));
    assert_ne!(
        report["last_valid_candidate_digest"],
        before["last_valid_candidate_digest"]
    );
    assert_ne!(report["last_valid_revision"], before["last_valid_revision"]);
    assert_ne!(
        report["source"]["source_digest"],
        before["source"]["source_digest"]
    );
    rejected(
        call(
            &mut session,
            "candidate/query",
            json!({"candidate_revision":report["last_valid_candidate_digest"]}),
        ),
        "SPX-G224",
    );
    let expression = report["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["replaceable"] == true && row["kind"] == "binary")
        .unwrap()["expression_id"]
        .clone();
    let opened = payload(call(
        &mut session,
        "hole/open-expression",
        json!({"candidate_revision":base,"draft_revision":current,"target":"calculator.add","expression_id":expression,"hole_id":"new-expression"}),
    ));
    let next = opened["draft_revision"].as_str().unwrap();
    let context = payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":next,"hole_id":"new-expression"}),
    ));
    assert_eq!(context["expression_id"], expression);
    assert_eq!(
        context["last_valid_revision"],
        report["last_valid_revision"]
    );
    rejected(
        call(
            &mut session,
            "hole/complete",
            json!({"draft_revision":next}),
        ),
        "SPX-G232",
    );
    payload(call(
        &mut session,
        "hole/discard",
        json!({"draft_revision":revision}),
    ));
    rejected(
        call(
            &mut session,
            METHOD,
            params(&revision, "calculator.add", "body"),
        ),
        "SPX-G232",
    );
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn selected_closed_schema_and_typed_clients_preserve_old_profile_denials() {
    let fixture = Fixture::new();
    for prepare in [false, true] {
        let mut session = VNextSession::open(
            &fixture.manifest(),
            VNextPolicy {
                candidate_prepare: prepare,
                ..Default::default()
            },
        )
        .unwrap();
        let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
        let method = bundle["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["method"] == METHOD);
        assert_eq!(method.is_some(), prepare);
        if let Some(method) = method {
            assert_eq!(method["query"], true);
            assert_eq!(method["capability"], "candidate_prepare");
            assert_eq!(
                method["request_schema"]["properties"]["params"]["required"],
                json!(["image_revision", "draft_revision", "target", "region"])
            );
            assert_eq!(
                method["request_schema"]["properties"]["params"]["properties"]["region"]["enum"],
                json!(["body", "contract"])
            );
            let id = format!("urn:{PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA}");
            let schema = bundle["documents"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["$id"] == id)
                .unwrap();
            assert_eq!(schema["additionalProperties"], false);
            assert_eq!(schema["required"].as_array().unwrap().len(), 16);
            assert!(schema["properties"].get("candidate_revision").is_none());
            let row = &schema["properties"]["expressions"]["items"];
            assert_eq!(row["additionalProperties"], false);
            assert_eq!(row["required"].as_array().unwrap().len(), 9);
            assert_eq!(
                row["properties"]["scope"]["items"]["additionalProperties"],
                false
            );
            assert!(!bundle["unbundled_payload_schemas"]
                .as_array()
                .unwrap()
                .contains(&json!(PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA)));
        } else {
            let denied = call(
                &mut session,
                METHOD,
                params(UNKNOWN, "calculator.add", "body"),
            );
            assert_eq!(denied["error"]["code"], -32601);
        }
        for language in ["typescript", "python", "rust"] {
            let client = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            for name in [
                "HoleExpressionCatalogPayload",
                "HoleExpressionCatalogTypedParams",
                "request_hole_expression_catalog_typed",
                "decode_request_hole_expression_catalog_typed",
            ] {
                assert_eq!(
                    client["source"].as_str().unwrap().contains(name),
                    prepare,
                    "{language}: {name}"
                );
            }
        }
        session.finish().unwrap();
    }
    for capability in [
        ImageHostCapability::ReadOnly,
        ImageHostCapability::CandidateOnly,
    ] {
        let mut old = ImageSession::open(&fixture.manifest(), capability).unwrap();
        let response: Value =
            serde_json::from_slice(&old.handle_frame(&frame(1, METHOD, json!({}))).unwrap())
                .unwrap();
        assert_eq!(response["error"]["code"], -32601);
    }
}

#[test]
fn detached_queries_match_sequential_bytes_and_stale_selectors_fail_closed() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let (mut sequential, _, revision) = fixture.session();
    let image = sequential.image_revision().to_owned();
    let frames = [
        frame(
            1,
            METHOD,
            json!({"image_revision":image,"draft_revision":revision,"target":"calculator.divide","region":"body"}),
        ),
        frame(
            2,
            METHOD,
            json!({"image_revision":image,"draft_revision":revision,"target":"calculator.divide","region":"contract"}),
        ),
        frame(
            3,
            METHOD,
            json!({"image_revision":image,"draft_revision":UNKNOWN,"target":"calculator.divide","region":"body"}),
        ),
    ];
    let expected = frames
        .iter()
        .map(|frame| sequential.handle_frame(frame))
        .collect::<Vec<_>>();
    for response in expected.iter().take(2) {
        payload(serde_json::from_slice(response.as_ref().unwrap()).unwrap());
    }
    rejected(
        serde_json::from_slice(expected[2].as_ref().unwrap()).unwrap(),
        "SPX-G232",
    );
    let borrowed = frames.iter().map(Vec::as_slice).collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        let (mut parallel, _, _) = fixture.session();
        assert_eq!(
            parallel.handle_read_batch(&borrowed, workers).unwrap(),
            expected
        );
        parallel.finish().unwrap();
    }
    let stale: Value = serde_json::from_slice(&sequential.handle_frame(&frame(9, METHOD, json!({"image_revision":UNKNOWN,"draft_revision":revision,"target":"calculator.add","region":"body"}))).unwrap()).unwrap();
    rejected(stale, "SPX-G282");
    assert_eq!(fixture.bytes(), disk);
    sequential.finish().unwrap();
}

#[test]
fn source_authentication_precedes_unknown_draft_lookup_and_remains_terminal() {
    let fixture = Fixture::new();
    let (mut session, _, _) = fixture.session();
    let request = frame(
        1,
        METHOD,
        json!({"image_revision":session.image_revision(),"draft_revision":UNKNOWN,"target":"calculator.add","region":"body"}),
    );
    let path = fixture.0.join("src/app.spx");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid source drift\n").unwrap();
    let errors = session
        .handle_read_batch(&[request.as_slice()], 2)
        .unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-J102"),
        "{errors:?}"
    );
    std::fs::write(path, original).unwrap();
    assert!(session.handle_read_batch(&[request.as_slice()], 1).is_err());
    assert!(session.finish().is_err());
}
