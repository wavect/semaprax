//! Direct field-place discovery and admission evidence, authored and unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-field-place-v5-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "field-place"
version = "0.1.0"
profile = "owned-data-api.v1"
entry = "field_place.app"
sources = ["src/app.spx", "src/frame.spx", "src/tests.spx"]
web_exports = ["field-place.public"]
tests = ["field_place.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/app.spx",
                "module field_place.app; @id(\"field-place.main\") fn main()->i64 {0}",
            ),
            (
                "src/tests.spx",
                "module field_place.tests; @id(\"field-place.test\") fn main()->i64 {0}",
            ),
        ] {
            let program = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&program)).unwrap();
        }
        let path = root.join("src/frame.spx");
        // Keep this loan-bearing fixture in the admitted flat-record profile;
        // the frame product's owned variants intentionally cannot be masked by v23.
        let source = r#"module field_place.frame;
@id("field-place.public") fn public_value(value:i64)->i64 {value}
@id("field-place.packet") record Packet {
    @id("field-place.packet.payload") payload: Bytes,
    @id("field-place.packet.sibling") sibling: Bytes,
}
@id("field-place.other") record Other {
    @id("field-place.other.payload") payload: Bytes,
    @id("field-place.other.sibling") sibling: Bytes,
}
@id("field-place.inspect") fn inspect(packet: own Packet) -> usize { 0usize }
"#;
        let parsed = semaprax::parse(source, "src/frame.spx").unwrap();
        std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.manifest(),
            VNextPolicy {
                candidate_prepare: true,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
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
        .map(|path| std::fs::read(self.0.join(path)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
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
fn opened(session: &mut VNextSession) -> (String, String) {
    let candidate = payload(call(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let draft = payload(call(
        session,
        "hole/open",
        json!({"candidate_revision":candidate,"target":"field-place.inspect","hole_id":"inspect"}),
    ))["draft_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    (candidate, draft)
}
fn borrowed(field: &str, root: Value) -> Value {
    json!({"kind":"let","name":"view","value":{"kind":"builtin_call","target":"core.bytes.as-slice","arguments":[{"kind":"field_place","target":field,"root":root}]},"body":{"kind":"builtin_call","target":"core.bytes.len","arguments":[{"kind":"place","name":"view"}]}})
}

#[test]
fn closed_constructor_and_typed_metadata_keep_place_distinct_from_value_projection() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let schemas = payload(call(
        &mut session,
        "protocol/constructor-schemas",
        json!({}),
    ));
    let document = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.typed-expression.v1")
        .unwrap();
    let branches = document["$defs"]["expression"]["oneOf"].as_array().unwrap();
    let place = branches
        .iter()
        .find(|row| row["properties"]["kind"]["const"] == "field_place")
        .unwrap();
    assert_eq!(place["additionalProperties"], false);
    assert_eq!(place["required"], json!(["kind", "target", "root"]));
    assert_eq!(place["properties"].as_object().unwrap().len(), 3);
    assert_eq!(place["properties"]["root"]["type"], "string");
    assert!(place["properties"].get("base").is_none());
    let project = branches
        .iter()
        .find(|row| row["properties"]["kind"]["const"] == "project")
        .unwrap();
    assert_eq!(project["properties"]["base"]["$ref"], "#/$defs/expression");
    assert_eq!(project["x-implicit-project-nodes"], 3);
    let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
    let catalog = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["$id"] == "urn:semaprax.project-change-catalog.v1")
        .unwrap();
    assert!(!catalog["required"]
        .as_array()
        .unwrap()
        .contains(&json!("field_places")));
    for branch in catalog["properties"]["field_places"]["items"]["oneOf"]
        .as_array()
        .unwrap()
    {
        assert_eq!(branch["additionalProperties"], false);
        assert_eq!(branch["properties"]["kind"]["const"], "field_place");
        assert_eq!(
            branch["properties"]["base_evaluation"]["const"],
            "direct_named_place_no_staging"
        );
        assert_eq!(
            branch["properties"]["root_requirement"]["const"],
            "authenticated_lexical_nominal_binding"
        );
    }
    for language in ["typescript", "python", "rust"] {
        let generated = payload(call(
            &mut session,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = generated["source"].as_str().unwrap();
        assert!(source.contains("field_place"));
        assert!(source.contains("field_places"));
        assert!(source.contains("request_candidate_apply_intent_typed"));
    }
    session.finish().unwrap();
}

#[test]
fn field_membership_is_discoverable_in_full_and_compact_hole_contexts() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session();
    let (candidate, draft) = opened(&mut session);
    let catalog = payload(call(
        &mut session,
        "change/catalog",
        json!({"candidate_revision":candidate,"target":"field-place.inspect"}),
    ));
    let context = payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":"inspect"}),
    ));
    assert_eq!(catalog["field_places"], context["field_places"]);
    let selected = context["field_places"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["target"] == "field-place.packet.payload")
        .unwrap();
    assert_eq!(selected["kind"], "field_place");
    assert_eq!(selected["owner"], "field-place.packet");
    assert_eq!(selected["name"], "payload");
    assert_eq!(selected["requires_full_candidate_validation"], true);
    assert_eq!(selected["base_evaluation"], "direct_named_place_no_staging");
    assert!(context["constructor_kinds"]
        .as_array()
        .unwrap()
        .contains(&json!("field_place")));
    let summary = payload(call(
        &mut session,
        "hole/summary",
        json!({"draft_revision":draft,"hole_id":"inspect"}),
    ));
    let reference = summary["facets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["facet"] == "constructors")
        .unwrap()["reference"]
        .clone();
    let page = payload(call(
        &mut session,
        "hole/page",
        json!({"draft_revision":draft,"hole_id":"inspect","reference":reference,"limit":64}),
    ));
    assert!(page["items"]
        .as_array()
        .unwrap()
        .contains(&json!("field_place")));
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}

#[test]
fn fill_uses_original_field_storage_and_wrong_owner_or_expression_root_preserves_draft() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session();
    let (_, draft) = opened(&mut session);
    let before = call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":"inspect"}),
    );
    for expression in [
        borrowed("field-place.other.payload", json!("packet")),
        borrowed("field-place.packet.payload", json!("missing")),
        borrowed(
            "field-place.packet.payload",
            json!({"kind":"place","name":"packet"}),
        ),
    ] {
        let rejected = call(
            &mut session,
            "hole/fill",
            json!({"draft_revision":draft,"hole_id":"inspect","expression":expression}),
        );
        assert!(rejected.get("error").is_some(), "{rejected}");
        assert_eq!(
            call(
                &mut session,
                "hole/query",
                json!({"draft_revision":draft,"hole_id":"inspect"})
            ),
            before
        );
    }
    let expression = borrowed("field-place.packet.payload", json!("packet"));
    let base = fixture.candidate();
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"replace_function_body","target":"field-place.inspect","body":expression}),
    )
    .unwrap();
    let expected = base.apply(base.candidate_digest(), &change).unwrap();
    let source = expected
        .revision()
        .sources()
        .iter()
        .find(|source| source.path() == "src/frame.spx")
        .unwrap()
        .source();
    assert!(source.contains("bytes_as_slice(packet.payload)"));
    assert!(!source.contains("spx_project_"));
    let filled = payload(call(
        &mut session,
        "hole/fill",
        json!({"draft_revision":draft,"hole_id":"inspect","expression":expression}),
    ));
    let completed = payload(call(
        &mut session,
        "hole/complete",
        json!({"draft_revision":filled["draft_revision"]}),
    ));
    assert_eq!(completed["candidate_revision"], expected.candidate_digest());
    assert_eq!(fixture.bytes(), disk);
    session.finish().unwrap();
}
