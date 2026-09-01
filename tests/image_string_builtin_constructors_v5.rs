//! Authored, unrun StringOp discovery and source-replayed transport evidence.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const OPERATIONS: [(&str, &str, usize, &str, &str, &str); 7] = [
    (
        "core.string.len",
        "string_len",
        1,
        "string",
        "borrow",
        "i64",
    ),
    (
        "core.string.concat",
        "string_concat",
        2,
        "string",
        "own",
        "string",
    ),
    (
        "core.string.is_empty",
        "string_is_empty",
        1,
        "string",
        "borrow",
        "bool",
    ),
    (
        "core.string.starts_with",
        "string_starts_with",
        2,
        "string",
        "borrow",
        "bool",
    ),
    (
        "core.string.contains",
        "string_contains",
        2,
        "string",
        "borrow",
        "bool",
    ),
    (
        "core.string.len_chars",
        "string_len_chars",
        1,
        "string",
        "borrow",
        "i64",
    ),
    (
        "core.string.from_char",
        "string_from_char",
        1,
        "char",
        "value",
        "string",
    ),
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-string-builtin-v5-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "string-builtin-transport"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "string_calls.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["string_calls.public"]
tests = ["string_calls.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            (
                "src/app.spx",
                "module string_calls.app;\n@id(\"string_calls.main\") fn main()->i64 {0}\n",
            ),
            (
                "src/core.spx",
                r#"module string_calls.core;
@id("string_calls.work") fn work()->i64 {0}
@id("string_calls.truth") fn truth()->bool {false}
@id("string_calls.text") fn text()->string {"old"}
@id("string_calls.scalar") fn from_scalar(value:char)->string {"old"}
@id("string_calls.public") fn public_value()->i64 {0}
"#,
            ),
            (
                "src/tests.spx",
                "module string_calls.tests;\n@id(\"string_calls.test\") fn main()->i64 {0}\n",
            ),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self, diagnostics: bool) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                diagnostics,
                ..Default::default()
            },
        )
        .unwrap()
    }
    fn candidate(&self) -> ProjectCandidate {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
        })
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
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if !method.starts_with("protocol/") {
        params["image_revision"] = json!(session.image_revision());
    }
    let request = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(request.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn metadata(catalog: &Value) {
    let rows = catalog["builtin_calls"].as_array().unwrap();
    assert_eq!(rows.len(), 14);
    assert!(rows[..7]
        .iter()
        .all(|row| row["evidence_owner"] == "compiler_byte_operations"));
    assert!(rows[7..]
        .iter()
        .all(|row| row["evidence_owner"] == "compiler_string_operations"));
    for (index, (id, name, arity, ty, ownership, result)) in OPERATIONS.into_iter().enumerate() {
        let row = &rows[index + 7];
        assert_eq!(row.as_object().unwrap().len(), 9);
        assert_eq!(row["target"], id);
        assert_eq!(row["name"], name);
        assert_eq!(row["arity"], arity);
        assert_eq!(row["return_type_id"], result);
        assert_eq!(row["effects"], json!([]));
        assert_eq!(row["requires_full_candidate_validation"], true);
        let parameters = row["parameters"].as_array().unwrap();
        assert_eq!(parameters.len(), arity);
        for (index, parameter) in parameters.iter().enumerate() {
            assert_eq!(parameter.as_object().unwrap().len(), 5);
            assert_eq!(parameter["index"], index);
            assert_eq!(parameter["type_id"], ty);
            assert_eq!(parameter["ownership"], ownership);
            assert_eq!(parameter["type_family"], Value::Null);
        }
    }
}

#[test]
fn catalogue_and_hole_context_append_owner_derived_string_metadata_after_byte_rows() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(false);
    let base = payload(call(&mut session, "candidate/open", json!({})));
    metadata(&payload(call(
        &mut session,
        "change/catalog",
        json!({"candidate_revision":base["candidate_revision"],"target":"string_calls.work"}),
    )));
    let draft = payload(call(
        &mut session,
        "hole/open",
        json!({"candidate_revision":base["candidate_revision"],"target":"string_calls.work","hole_id":"body"}),
    ));
    metadata(&payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft["draft_revision"],"hole_id":"body"}),
    )));
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn every_string_builtin_body_is_replayed_with_exact_selected_identity_and_no_char_literal() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut session = fixture.session(false);
    let opened = payload(call(&mut session, "candidate/open", json!({})));
    for (id, _, arity, ty, _, result) in OPERATIONS {
        let (target, argument) = if ty == "char" {
            (
                "string_calls.scalar",
                json!({"kind":"place","name":"value"}),
            )
        } else {
            (
                match result {
                    "string" => "string_calls.text",
                    "bool" => "string_calls.truth",
                    _ => "string_calls.work",
                },
                json!({"kind":"string","value":"é😀"}),
            )
        };
        let intent = json!({"kind":"replace_function_body","target":target,"body":{"kind":"builtin_call","target":id,"arguments":vec![argument;arity]}});
        let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
        let expected = base.apply(base.candidate_digest(), &change).unwrap();
        let actual = payload(call(
            &mut session,
            "candidate/apply-intent",
            json!({"candidate_revision":opened["candidate_revision"],"intent":intent}),
        ));
        assert_eq!(actual["candidate_revision"], expected.candidate_digest());
        // These helpers are checked source outside the selected executable
        // closure, not evidence of new target or public String/char support.
        assert!(!expected
            .revision()
            .entry_program()
            .functions
            .iter()
            .any(|function| function.id.as_str() == target));
    }
    let response = call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":opened["candidate_revision"],"intent":{"kind":"replace_function_body","target":"string_calls.text","body":{"kind":"builtin_call","target":"core.string.from_char","arguments":[{"kind":"i64","value":65}]}}}),
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .unwrap()
            .contains("SPX-T205"),
        "{response}"
    );
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn schemas_publish_fourteen_exact_arity_branches_and_both_client_type_graphs() {
    let fixture = Fixture::new();
    for diagnostics in [false, true] {
        let mut session = fixture.session(diagnostics);
        let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
        let docs = bundle["documents"].as_array().unwrap();
        let expression = docs
            .iter()
            .find(|doc| doc["$id"] == "urn:semaprax.typed-expression.v1")
            .unwrap();
        let forms = expression["$defs"]["expression"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(
            forms
                .iter()
                .filter(|row| row["properties"]["kind"]["const"] == "builtin_call")
                .count(),
            14
        );
        assert!(forms
            .iter()
            .any(|row| row["properties"]["kind"]["const"] == "char"));
        for (id, _, arity, _, _, _) in OPERATIONS {
            let shape = forms
                .iter()
                .find(|row| row["properties"]["target"]["const"] == id)
                .unwrap();
            assert_eq!(shape["additionalProperties"], false);
            assert_eq!(shape["required"], json!(["kind", "target", "arguments"]));
            assert_eq!(shape["properties"]["arguments"]["minItems"], arity);
            assert_eq!(shape["properties"]["arguments"]["maxItems"], arity);
            assert_eq!(
                shape["properties"]["arguments"]["items"]["$ref"],
                "#/$defs/expression"
            );
        }
        let catalogue = docs
            .iter()
            .find(|doc| doc["$id"] == "urn:semaprax.project-change-catalog.v1")
            .unwrap();
        assert_eq!(catalogue["properties"]["builtin_calls"]["maxItems"], 14);
        let kinds = catalogue["properties"]["builtin_calls"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(kinds.len(), 2);
        assert_eq!(
            kinds[0]["properties"]["evidence_owner"]["const"],
            "compiler_byte_operations"
        );
        assert_eq!(
            kinds[1]["properties"]["evidence_owner"]["const"],
            "compiler_string_operations"
        );
        for language in ["typescript", "python", "rust"] {
            let client = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = client["source"].as_str().unwrap();
            assert!(source.contains("core.string.from_char"));
            assert!(source.contains("request_candidate_apply_intent_typed"));
            assert_eq!(source.contains("AttemptRepairCatalogPayload"), diagnostics);
            assert_eq!(client["io"], false);
        }
    }
}
