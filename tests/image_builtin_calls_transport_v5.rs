//! Builtin constructor discovery and typed-hole transport, authored and unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const OPERATIONS: [(&str, &str, usize); 7] = [
    ("core.bytes.len", "byte_len", 1),
    ("core.bytes.get", "byte_get", 2),
    ("core.bytes.range", "byte_range", 3),
    ("core.bytes.copy", "bytes_copy", 1),
    ("core.bytes.as-slice", "bytes_as_slice", 1),
    ("core.array-u8.as-slice", "array_as_slice", 1),
    ("core.str.as-bytes", "str_as_bytes", 1),
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-builtin-transport-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/frame.spx",
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
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |s| {
            ProjectCandidate::open(s.retain_revision(), s.project_revision())
        })
        .unwrap()
    }
    fn bytes(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/frame.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|p| std::fs::read(self.0.join(p)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn bound(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if !matches!(method, "protocol/schemas" | "protocol/client") {
        params["image_revision"] = json!(session.image_revision());
    }
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn open(session: &mut VNextSession) -> String {
    payload(bound(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn builtin(target: &str, arguments: Vec<Value>) -> Value {
    json!({"kind":"builtin_call","target":target,"arguments":arguments})
}
fn place(name: &str) -> Value {
    json!({"kind":"place","name":name})
}
fn assert_metadata(value: &Value) {
    let rows = value["builtin_calls"].as_array().unwrap();
    assert_eq!(rows.len(), 14);
    let rows = rows
        .iter()
        .filter(|row| row["evidence_owner"] == "compiler_byte_operations")
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), OPERATIONS.len());
    for (id, name, arity) in OPERATIONS {
        let row = rows.iter().find(|row| row["target"] == id).unwrap();
        assert_eq!(row["kind"], "builtin_call");
        assert_eq!(row["name"], name);
        assert_eq!(row["arity"], arity);
        assert_eq!(row["parameters"].as_array().unwrap().len(), arity);
        assert_eq!(row["effects"], json!([]));
        assert_eq!(row["evidence_owner"], "compiler_byte_operations");
        assert_eq!(row["requires_full_candidate_validation"], true);
        for (index, param) in row["parameters"].as_array().unwrap().iter().enumerate() {
            assert_eq!(param["index"], index);
            assert_eq!(
                param["ownership"],
                if index == 0 { "borrow" } else { "value" }
            );
            if id == "core.array-u8.as-slice" && index == 0 {
                assert!(param["type_id"].is_null());
                assert_eq!(param["type_family"], "array_u8_any_length");
            } else {
                assert!(param["type_id"].is_string());
                assert!(param["type_family"].is_null());
            }
        }
    }
}

#[test]
fn constructor_schemas_preserve_seven_byte_alternatives_with_string_operations_added() {
    let fixture = Fixture::new();
    let mut session = fixture.session(true);
    let bundle = payload(bound(
        &mut session,
        "protocol/constructor-schemas",
        json!({}),
    ));
    let expression = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|doc| doc["$id"] == "urn:semaprax.typed-expression.v1")
        .unwrap();
    let alternatives = expression["$defs"]["expression"]["oneOf"]
        .as_array()
        .unwrap();
    let builtins = alternatives
        .iter()
        .filter(|row| row["properties"]["kind"]["const"] == "builtin_call")
        .collect::<Vec<_>>();
    assert_eq!(builtins.len(), 14);
    assert_eq!(
        builtins
            .iter()
            .filter(|row| OPERATIONS
                .iter()
                .any(|(id, _, _)| row["properties"]["target"]["const"] == *id))
            .count(),
        OPERATIONS.len()
    );
    for (id, _, arity) in OPERATIONS {
        let row = builtins
            .iter()
            .find(|row| row["properties"]["target"]["const"] == id)
            .unwrap();
        assert_eq!(row["additionalProperties"], false);
        assert_eq!(row["required"], json!(["kind", "target", "arguments"]));
        let args = &row["properties"]["arguments"];
        assert_eq!(args["minItems"], arity);
        assert_eq!(args["maxItems"], arity);
        assert_eq!(args["items"]["$ref"], "#/$defs/expression");
    }
    assert_eq!(
        alternatives
            .iter()
            .filter(|row| row["properties"]["kind"]["const"] == "call")
            .count(),
        1
    );
    let candidate = open(&mut session);
    let catalog = payload(bound(
        &mut session,
        "change/catalog",
        json!({"candidate_revision":candidate,"target":"frame.payload"}),
    ));
    assert_metadata(&catalog);
    let draft = payload(bound(
        &mut session,
        "hole/open",
        json!({"candidate_revision":candidate,"target":"frame.payload","hole_id":"payload"}),
    ));
    let context = payload(bound(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft["draft_revision"],"hole_id":"payload"}),
    ));
    assert_metadata(&context);
    assert!(context["constructor_kinds"]
        .as_array()
        .unwrap()
        .contains(&json!("builtin_call")));
    assert!(!context["accessible_calls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|call| OPERATIONS.iter().any(|(id, _, _)| call["id"] == *id)));
    assert_eq!(context["source_authority"], false);
    assert_eq!(context["materializable"], false);
    session.finish().unwrap();
}

#[test]
fn builtin_hole_fill_and_apply_intent_have_exact_library_candidate_identity_without_source_writes()
{
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let expression = builtin("core.bytes.copy", vec![place("frame")]);
    let intent = json!({"kind":"replace_function_body","target":"frame.payload","body":expression});
    let expected = base
        .apply(
            base.candidate_digest(),
            &SemanticChange::new(base.revision().project_revision(), &intent).unwrap(),
        )
        .unwrap();
    let mut session = fixture.session(true);
    let root = open(&mut session);
    let before = payload(bound(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":root}),
    ));
    let draft = payload(bound(
        &mut session,
        "hole/open",
        json!({"candidate_revision":root,"target":"frame.payload","hole_id":"payload"}),
    ));
    let params = json!({"draft_revision":draft["draft_revision"],"hole_id":"payload"});
    let context = payload(bound(&mut session, "hole/query", params.clone()));
    for invalid in [
        builtin("core.bytes.copy", vec![]),
        builtin("core.unknown", vec![place("frame")]),
        json!({"kind":"call","target":"core.bytes.copy","arguments":[place("frame")]}),
    ] {
        let response = bound(
            &mut session,
            "hole/fill",
            json!({"draft_revision":draft["draft_revision"],"hole_id":"payload","expression":invalid}),
        );
        assert!(
            response["error"]["message"]
                .as_str()
                .unwrap()
                .contains("SPX-G225"),
            "{response}"
        );
        assert_eq!(
            payload(bound(&mut session, "hole/query", params.clone())),
            context
        );
    }
    let filled = payload(bound(
        &mut session,
        "hole/fill",
        json!({"draft_revision":draft["draft_revision"],"hole_id":"payload","expression":expression}),
    ));
    let completed = payload(bound(
        &mut session,
        "hole/complete",
        json!({"draft_revision":filled["draft_revision"]}),
    ));
    assert_eq!(completed["candidate_revision"], expected.candidate_digest());
    let applied = payload(bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":intent}),
    ));
    assert_eq!(applied["candidate_revision"], expected.candidate_digest());
    assert_eq!(
        payload(bound(
            &mut session,
            "candidate/query",
            json!({"candidate_revision":root})
        )),
        before
    );
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn builtin_constructor_does_not_grant_candidate_preparation_to_a_readonly_host() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(false);
    let bundle = payload(bound(&mut session, "protocol/schemas", json!({})));
    assert!(!bundle["methods"]
        .as_array()
        .unwrap()
        .iter()
        .any(|method| method["method"] == "candidate/apply-intent"));
    let rejected = bound(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":"sha256:0000000000000000000000000000000000000000000000000000000000000000","intent":{"kind":"replace_function_body","target":"frame.payload","body":builtin("core.bytes.copy",vec![place("frame")])}}),
    );
    assert_eq!(rejected["error"]["code"], -32601);
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}
