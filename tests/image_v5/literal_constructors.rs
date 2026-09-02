//! Authored, unrun literal grammar and source-replayed transport evidence.
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
            "spx-literal-v5-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            r#"schema = "semaprax.project.v8"
name = "literal-transport"
version = "1.0.0"
profile = "owned-data-api.v1"
entry = "literal.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["literal.public"]
tests = ["literal.tests"]
"#,
        )
        .unwrap();
        for (path, source) in [
            ("src/app.spx", "module literal.app;\n@id(\"literal.main\") fn main()->i64 {0}\n"),
            ("src/core.spx", "module literal.core;\n@id(\"literal.work\") fn work()->i64 {0}\n@id(\"literal.char\") fn char_value()->char {'a'}\n@id(\"literal.f32\") fn f32_value()->f32 {0.0f32}\n@id(\"literal.f64\") fn f64_value()->f64 {0.0}\n@id(\"literal.public\") fn public_value(value:i64)->i64 {value}\n"),
            ("src/tests.spx", "module literal.tests;\n@id(\"literal.test\") fn main()->i64 {0}\n"),
        ] {
            let parsed = semaprax::parse(source, path).unwrap();
            std::fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self, candidate: bool) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: candidate,
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
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    response["result"]["payload"].clone()
}
fn body(text: &str, bytes: Value) -> Value {
    json!({"kind":"let","name":"text","value":{"kind":"string","value":text},"body":{
        "kind":"let","name":"bytes","value":{"kind":"array_u8","values":bytes},"body":{"kind":"i64","value":7}
    }})
}

#[test]
fn literal_source_replay_matches_library_and_hole_constructor_discovery() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let base = fixture.candidate();
    let mut session = fixture.session(true);
    let opened = payload(call(&mut session, "candidate/open", json!({})));
    assert_eq!(opened["candidate_revision"], base.candidate_digest());
    let catalog = payload(call(
        &mut session,
        "change/catalog",
        json!({"candidate_revision":base.candidate_digest(),"target":"literal.work"}),
    ));
    let constructors = &catalog["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "replace_function_body")
        .unwrap()["constructors"];
    for kind in ["string", "array_u8", "char", "f32", "f64"] {
        assert!(constructors.as_array().unwrap().contains(&json!(kind)));
    }
    let signature = catalog["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "change_function_signature")
        .unwrap();
    for form in signature["exactly_one_form"].as_array().unwrap() {
        assert_eq!(
            form["new_parameter_types"],
            json!(["i64", "i32", "u8", "usize", "bool", "char", "f32", "f64"])
        );
    }
    let draft = payload(call(
        &mut session,
        "hole/open",
        json!({"candidate_revision":base.candidate_digest(),"target":"literal.work","hole_id":"body"}),
    ));
    let context = payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft["draft_revision"],"hole_id":"body"}),
    ));
    for kind in ["string", "array_u8", "char", "f32", "f64"] {
        assert!(context["constructor_kinds"]
            .as_array()
            .unwrap()
            .contains(&json!(kind)));
    }
    // The body is source-checked outside the selected executable closure; no
    // native/Wasm execution or internal String profile widening is claimed.
    for expression in [
        body("", json!([])),
        body("quote\" slash\\ newline\n nul\0 café", json!([0, 127, 255])),
    ] {
        let intent =
            json!({"kind":"replace_function_body","target":"literal.work","body":expression});
        let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
        let expected = base.apply(base.candidate_digest(), &change).unwrap();
        let actual = payload(call(
            &mut session,
            "candidate/apply-intent",
            json!({"candidate_revision":base.candidate_digest(),"intent":intent}),
        ));
        assert_eq!(actual["candidate_revision"], expected.candidate_digest());
    }
    for (target, expression) in [
        ("literal.char", json!({"kind":"char","scalar":"0001f600"})),
        ("literal.f32", json!({"kind":"f32","bits":"80000000"})),
        (
            "literal.f64",
            json!({"kind":"f64","bits":"0000000000000001"}),
        ),
    ] {
        let intent = json!({"kind":"replace_function_body","target":target,"body":expression});
        let change = SemanticChange::new(base.revision().project_revision(), &intent).unwrap();
        let expected = base.apply(base.candidate_digest(), &change).unwrap();
        let actual = payload(call(
            &mut session,
            "candidate/apply-intent",
            json!({"candidate_revision":base.candidate_digest(),"intent":intent}),
        ));
        assert_eq!(actual["candidate_revision"], expected.candidate_digest());
    }
    assert_eq!(fixture.bytes(), disk);
}

#[test]
fn literal_schemas_are_closed_bounded_and_selected_clients_keep_existing_authority() {
    let fixture = Fixture::new();
    for selected in [false, true] {
        let mut session = fixture.session(selected);
        let bundle = payload(call(&mut session, "protocol/schemas", json!({})));
        if selected {
            let owner = bundle["documents"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["$id"] == "urn:semaprax.typed-expression.v1")
                .unwrap();
            let forms = owner["$defs"]["expression"]["oneOf"].as_array().unwrap();
            let string = forms
                .iter()
                .find(|row| row["properties"]["kind"]["const"] == "string")
                .unwrap();
            assert_eq!(string["required"], json!(["kind", "value"]));
            assert_eq!(string["additionalProperties"], false);
            assert_eq!(string["properties"]["value"]["x-max-utf8-bytes"], 16384);
            assert!(string["properties"]["value"].get("minLength").is_none());
            let array = forms
                .iter()
                .find(|row| row["properties"]["kind"]["const"] == "array_u8")
                .unwrap();
            assert_eq!(array["required"], json!(["kind", "values"]));
            assert_eq!(array["additionalProperties"], false);
            assert_eq!(array["properties"]["values"]["maxItems"], 4095);
            assert_eq!(
                array["properties"]["values"]["items"],
                json!({"type":"integer","minimum":0,"maximum":255})
            );
            assert!(!forms
                .iter()
                .any(|row| row["properties"]["kind"]["const"] == "repeat_array_u8"));
            for (kind, field, pattern, length) in [
                ("char", "scalar", "^[0-9a-f]{8}$", 8),
                ("f32", "bits", "^[0-9a-f]{8}$", 8),
                ("f64", "bits", "^[0-9a-f]{16}$", 16),
            ] {
                let form = forms
                    .iter()
                    .find(|row| row["properties"]["kind"]["const"] == kind)
                    .unwrap();
                assert_eq!(form["required"], json!(["kind", field]));
                assert_eq!(form["additionalProperties"], false);
                assert_eq!(form["properties"].as_object().unwrap().len(), 2);
                assert_eq!(form["properties"][field]["type"], "string");
                assert_eq!(form["properties"][field]["minLength"], length);
                assert_eq!(form["properties"][field]["maxLength"], length);
                assert_eq!(form["properties"][field]["pattern"], pattern);
            }
        }
        for language in ["typescript", "python", "rust"] {
            let generated = payload(call(
                &mut session,
                "protocol/client",
                json!({"language":language}),
            ));
            let source = generated["source"].as_str().unwrap();
            assert_eq!(
                source.contains("request_candidate_apply_intent_typed"),
                selected
            );
            if selected {
                assert!(source.contains("array_u8"));
                for token in ["char", "f32", "f64"] {
                    assert!(source.contains(token));
                }
            }
            assert!(!source.contains("request_candidate_commit("));
            assert_eq!(generated["io"], false);
        }
    }
}

#[test]
fn malformed_literal_payloads_are_rejected_without_changing_retained_candidate_or_source() {
    let fixture = Fixture::new();
    let disk = fixture.bytes();
    let mut session = fixture.session(true);
    let opened = payload(call(&mut session, "candidate/open", json!({})));
    for expression in [
        json!({"kind":"string","value":"","source":"injected"}),
        json!({"kind":"string","value":false}),
        json!({"kind":"string","value":"é".repeat(8193)}),
        json!({"kind":"array_u8","values":[256]}),
        json!({"kind":"array_u8","values":[true]}),
        json!({"kind":"array_u8","values":[{"kind":"u8","value":1}]}),
        json!({"kind":"array_u8","values":vec![0;4096]}),
        json!({"kind":"repeat_array_u8","value":0,"count":1}),
        json!({"kind":"char","scalar":"0000D800"}),
        json!({"kind":"char","scalar":"0000d800"}),
        json!({"kind":"char","scalar":"00110000"}),
        json!({"kind":"f32","bits":"7f800000"}),
        json!({"kind":"f32","bits":"7fc00000"}),
        json!({"kind":"f64","bits":"7ff0000000000000"}),
        json!({"kind":"f64","bits":"7ff8000000000000"}),
    ] {
        let response = call(
            &mut session,
            "candidate/apply-intent",
            json!({"candidate_revision":opened["candidate_revision"],"intent":{"kind":"replace_function_body","target":"literal.work","body":expression}}),
        );
        assert!(response.get("error").is_some(), "{response}");
    }
    let summary = payload(call(
        &mut session,
        "candidate/query",
        json!({"candidate_revision":opened["candidate_revision"]}),
    ));
    assert_eq!(summary["candidate_revision"], opened["candidate_revision"]);
    assert_eq!(fixture.bytes(), disk);
}
