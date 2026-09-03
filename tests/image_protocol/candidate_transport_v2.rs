//! Candidate-only protocol evidence, authored without executing local gates.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::image_transport::{
    ImageHostCapability, ImageSession, CANDIDATE_PROTOCOL_SCHEMA, PROTOCOL_SCHEMA,
};
use semaprax::project::SemanticChange;
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-image-candidates-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self, profile: ImageHostCapability) -> ImageSession {
        ImageSession::open(&self.0.join("semaprax.toml"), profile).unwrap()
    }
    fn inventory(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    visit(root, &entry.path(), result);
                } else {
                    result.insert(
                        entry.path().strip_prefix(root).unwrap().to_path_buf(),
                        std::fs::read(entry.path()).unwrap(),
                    );
                }
            }
        }
        let mut result = BTreeMap::new();
        visit(&self.0, &self.0, &mut result);
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn call(session: &mut ImageSession, method: &str, mut params: Value) -> Value {
    if method.starts_with("candidate/")
        || method.starts_with("hole/")
        || matches!(
            method,
            "change/catalog"
                | "validation/catalog"
                | "expression/catalog"
                | "protocol/constructor-schemas"
        )
    {
        params["image_revision"] = json!(session.image_revision());
    }
    let bytes = json!({"jsonrpc":"2.0","id":7,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(bytes.as_bytes()).unwrap()).unwrap()
}
fn payload(response: Value) -> Value {
    assert!(response.get("error").is_none(), "{response}");
    assert_eq!(response["result"]["protocol"], CANDIDATE_PROTOCOL_SCHEMA);
    response["result"]["payload"].clone()
}
fn opened(session: &mut ImageSession) -> String {
    payload(call(session, "candidate/open", json!({})))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn renamed(session: &mut ImageSession, candidate: &str, name: &str) -> Value {
    call(
        session,
        "candidate/apply-intent",
        json!({"candidate_revision":candidate,"intent":{"kind":"rename_declaration","target":"calculator.add","name":name}}),
    )
}
fn report(session: &mut ImageSession, candidate: &str) -> String {
    let mut result = String::new();
    let mut offset = 0usize;
    loop {
        let chunk = payload(call(
            session,
            "candidate/query",
            json!({"candidate_revision":candidate,"offset":offset,"chunk_bytes":1024}),
        ));
        assert_eq!(chunk["offset"], offset);
        result.push_str(chunk["chunk"].as_str().unwrap());
        match chunk["next_offset"].as_u64() {
            Some(next) => {
                assert!(next > offset as u64);
                offset = next as usize;
            }
            None => {
                assert_eq!(chunk["total_bytes"], result.len());
                break;
            }
        }
    }
    result
}

#[test]
fn candidate_profile_is_explicit_and_v1_discovery_stays_read_only() {
    let fixture = Fixture::new();
    let mut readonly = fixture.session(ImageHostCapability::ReadOnly);
    let old = call(&mut readonly, "protocol/capabilities", json!({}));
    assert_eq!(old["result"]["protocol"], PROTOCOL_SCHEMA);
    assert_eq!(
        old["result"]["payload"]["capabilities"],
        json!(["semantic_read"])
    );
    let old_methods = old["result"]["payload"]["methods"].as_array().unwrap();
    assert!(!old_methods
        .iter()
        .any(|name| name.as_str().unwrap().starts_with("candidate/")));
    assert_eq!(
        call(&mut readonly, "candidate/open", json!({}))["error"]["code"],
        -32601
    );
    let mut candidates = fixture.session(ImageHostCapability::CandidateOnly);
    let current = payload(call(&mut candidates, "protocol/capabilities", json!({})));
    assert_eq!(
        current["capabilities"],
        json!(["semantic_read", "candidate_prepare"])
    );
    assert_eq!(current["source_authority"], false);
    assert_eq!(current["target_execution"], false);
    let methods = current["methods"].as_array().unwrap();
    for method in old_methods {
        assert!(methods.contains(method));
    }
    let schemas = payload(call(&mut candidates, "protocol/schemas", json!({})));
    assert_eq!(schemas["methods"].as_array().unwrap().len(), methods.len());
    for descriptor in schemas["methods"].as_array().unwrap() {
        assert_eq!(
            descriptor["success_response_schema"]["properties"]["result"]["properties"]["protocol"]
                ["const"],
            CANDIDATE_PROTOCOL_SCHEMA
        );
    }
    for language in ["typescript", "python", "rust"] {
        let client = payload(call(
            &mut candidates,
            "protocol/client",
            json!({"language":language}),
        ));
        let source = client["source"].as_str().unwrap();
        assert!(source.contains(CANDIDATE_PROTOCOL_SCHEMA));
        for method in methods {
            assert!(source.contains(method.as_str().unwrap()));
        }
    }
    for method in [
        "build",
        "test",
        "change/apply",
        "authority/elevate",
        "workspace/refresh",
    ] {
        assert_eq!(
            call(&mut candidates, method, json!({}))["error"]["code"],
            -32601
        );
    }
}

#[test]
fn candidates_are_immutable_queryable_validated_and_discardable_without_disk_changes() {
    let fixture = Fixture::new();
    let before = fixture.inventory();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let original = report(&mut session, &root);
    let child = payload(renamed(&mut session, &root, "sum"))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(root, child);
    assert_eq!(report(&mut session, &root), original);
    let changed: Value = serde_json::from_str(&report(&mut session, &child)).unwrap();
    assert!(!changed["source_changes"].as_array().unwrap().is_empty());
    let compared = payload(call(
        &mut session,
        "candidate/compare",
        json!({"candidate_revision":root,"other_candidate_revision":child}),
    ));
    assert_eq!(compared["commit_authority"], false);
    let validated = payload(call(
        &mut session,
        "candidate/validate",
        json!({"candidate_revision":child}),
    ));
    assert_eq!(validated["independently_replayed"], true);
    assert_eq!(validated["tests"], "not_run");
    let impact = payload(call(
        &mut session,
        "candidate/impact",
        json!({"candidate_revision":child,"target":"calculator.add","depth":1,"max_bytes":65536,"max_nodes":32}),
    ));
    assert_eq!(impact["schema"], "semaprax.image-candidate-impact.v1");
    let catalogue = payload(call(
        &mut session,
        "change/catalog",
        json!({"candidate_revision":child,"target":"calculator.add"}),
    ));
    assert_eq!(catalogue["schema"], "semaprax.project-change-catalog.v1");
    assert_eq!(catalogue["requires_full_candidate_validation"], true);
    let aggregates = catalogue["aggregate_constructors"].as_array().unwrap();
    assert_eq!(
        aggregates
            .iter()
            .map(|item| item["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "core.option.none",
            "core.option.some",
            "core.result.err",
            "core.result.ok"
        ]
    );
    for descriptor in aggregates {
        assert_eq!(descriptor["kind"], "variant");
        assert_eq!(descriptor["generic"], true);
        assert_eq!(descriptor["identity_origin"], "compiler_owned");
        assert_eq!(descriptor["evidence_owner"], "compiler_checked_prelude");
        assert!(descriptor["path"].is_null());
        assert!(descriptor["module"].is_null());
        assert_eq!(
            descriptor["compiler_prelude"]["schema"],
            "semaprax.prelude.v1"
        );
        assert!(descriptor["compiler_prelude"]["digest"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(descriptor["requires_full_candidate_validation"], true);
        for parameter in descriptor["type_parameters"].as_array().unwrap() {
            assert_eq!(parameter["allowed_types"], json!(["i64", "bool"]));
        }
    }
    let matches = catalogue["aggregate_matches"].as_array().unwrap();
    assert_eq!(
        matches
            .iter()
            .map(|item| item["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["core.option", "core.result"]
    );
    for (descriptor, expected_cases) in matches.iter().zip([
        ["core.option.none", "core.option.some"],
        ["core.result.ok", "core.result.err"],
    ]) {
        assert_eq!(descriptor["kind"], "match");
        assert_eq!(descriptor["generic"], true);
        assert_eq!(descriptor["identity_origin"], "compiler_owned");
        assert!(descriptor["path"].is_null());
        assert!(descriptor["module"].is_null());
        assert_eq!(
            descriptor["compiler_prelude"],
            aggregates[0]["compiler_prelude"]
        );
        assert_eq!(descriptor["evidence_owner"], "compiler_checked_prelude");
        assert_eq!(descriptor["requires_full_candidate_validation"], true);
        assert_eq!(
            descriptor["base_evaluation"],
            "once_into_typed_value_binding"
        );
        assert_eq!(
            descriptor["cases"]
                .as_array()
                .unwrap()
                .iter()
                .map(|case| case["target"].as_str().unwrap())
                .collect::<Vec<_>>(),
            expected_cases
        );
    }
    let body = catalogue["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "replace_function_body")
        .unwrap();
    assert!(body["constructors"]
        .as_array()
        .unwrap()
        .contains(&json!("match")));
    payload(call(
        &mut session,
        "candidate/discard",
        json!({"candidate_revision":child}),
    ));
    assert_eq!(
        call(
            &mut session,
            "candidate/query",
            json!({"candidate_revision":child})
        )["error"]["code"],
        -32000
    );
    assert_eq!(report(&mut session, &root), original);
    assert_eq!(fixture.inventory(), before);
}

#[test]
fn failed_intents_and_stale_handles_preserve_existing_candidates() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let original = report(&mut session, &root);
    let invalid = call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"bool","value":true}}}),
    );
    assert_eq!(invalid["error"]["code"], -32000);
    let illegal = call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":{"kind":"rename_declaration","target":"calculator.add","name":"sum"},"path":"other.spx"}),
    );
    assert_eq!(illegal["error"]["code"], -32602);
    let stale = format!("sha256:{}", "0".repeat(64));
    let bytes =
        json!({"jsonrpc":"2.0","id":8,"method":"candidate/open","params":{"image_revision":stale}})
            .to_string();
    let response: Value =
        serde_json::from_slice(&session.handle_frame(bytes.as_bytes()).unwrap()).unwrap();
    assert!(response["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-G221"));
    assert_eq!(report(&mut session, &root), original);
}

#[test]
fn hole_context_fill_and_complete_enforce_unresolved_draft_boundary() {
    let fixture = Fixture::new();
    let before = fixture.inventory();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let draft = payload(call(
        &mut session,
        "hole/open",
        json!({"candidate_revision":root,"target":"calculator.add","hole_id":"sum.body"}),
    ))["draft_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let context = payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":draft,"hole_id":"sum.body"}),
    ));
    assert_eq!(
        context["schema"],
        "semaprax.project-candidate-hole-context.v1"
    );
    assert_eq!(
        call(
            &mut session,
            "hole/complete",
            json!({"draft_revision":draft})
        )["error"]["code"],
        -32000
    );
    let invalid = call(
        &mut session,
        "hole/fill",
        json!({"draft_revision":draft,"hole_id":"sum.body","expression":{"kind":"bool","value":true}}),
    );
    assert_eq!(invalid["error"]["code"], -32000);
    assert_eq!(
        payload(call(
            &mut session,
            "hole/query",
            json!({"draft_revision":draft,"hole_id":"sum.body"})
        )),
        context
    );
    let filled = payload(call(
        &mut session,
        "hole/fill",
        json!({"draft_revision":draft,"hole_id":"sum.body","expression":{"kind":"i64","value":7}}),
    ))["draft_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let completed = payload(call(
        &mut session,
        "hole/complete",
        json!({"draft_revision":filled}),
    ));
    assert!(completed["candidate_revision"].as_str().is_some());
    // The original draft remains unresolved and cannot escape via completion.
    assert_eq!(
        call(
            &mut session,
            "hole/complete",
            json!({"draft_revision":draft})
        )["error"]["code"],
        -32000
    );
    payload(call(
        &mut session,
        "hole/discard",
        json!({"draft_revision":draft}),
    ));
    assert_eq!(
        call(
            &mut session,
            "hole/query",
            json!({"draft_revision":draft,"hole_id":"sum.body"})
        )["error"]["code"],
        -32000
    );
    assert_eq!(fixture.inventory(), before);
}

#[test]
fn bounded_candidate_registry_reuses_identical_handles_and_recovers_after_discard() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    assert_eq!(opened(&mut session), root);
    let mut children = Vec::new();
    for index in 0..15 {
        children.push(
            payload(renamed(&mut session, &root, &format!("sum{index}")))["candidate_revision"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let failure = renamed(&mut session, &root, "sumoverflow");
    assert!(failure["error"]["message"]
        .as_str()
        .unwrap()
        .contains("registry is full"));
    payload(call(
        &mut session,
        "candidate/discard",
        json!({"candidate_revision":children[0]}),
    ));
    payload(renamed(&mut session, &root, "sumoverflow"));
    assert!(!report(&mut session, &root).is_empty());
}

#[test]
fn bounded_draft_registry_and_candidate_discard_do_not_break_retained_drafts() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let mut drafts = Vec::new();
    for index in 0..16 {
        drafts.push(payload(call(&mut session,"hole/open",json!({"candidate_revision":root,"target":"calculator.add","hole_id":format!("body{index}")})))["draft_revision"].as_str().unwrap().to_owned());
    }
    let failure = call(
        &mut session,
        "hole/open",
        json!({"candidate_revision":root,"target":"calculator.add","hole_id":"overflow"}),
    );
    assert!(failure["error"]["message"]
        .as_str()
        .unwrap()
        .contains("registry is full"));
    payload(call(
        &mut session,
        "candidate/discard",
        json!({"candidate_revision":root}),
    ));
    payload(call(
        &mut session,
        "hole/query",
        json!({"draft_revision":drafts[0],"hole_id":"body0"}),
    ));
    payload(call(
        &mut session,
        "hole/discard",
        json!({"draft_revision":drafts[1]}),
    ));
    let filled = payload(call(
        &mut session,
        "hole/fill",
        json!({"draft_revision":drafts[0],"hole_id":"body0","expression":{"kind":"i64","value":7}}),
    ));
    payload(call(
        &mut session,
        "hole/complete",
        json!({"draft_revision":filled["draft_revision"]}),
    ));
}

#[cfg(unix)]
#[test]
fn source_drift_absorbs_all_candidate_and_draft_registry_access() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let path = fixture.0.join("src/core.spx");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"drift\n").unwrap();
    assert_eq!(
        call(
            &mut session,
            "candidate/query",
            json!({"candidate_revision":root})
        )["error"]["code"],
        -32000
    );
    std::fs::write(&path, original).unwrap();
    assert_eq!(
        call(
            &mut session,
            "candidate/discard",
            json!({"candidate_revision":root})
        )["error"]["code"],
        -32000
    );
    assert_eq!(
        call(&mut session, "workspace/open", json!({}))["error"]["code"],
        -32000
    );
}

#[test]
fn semantic_merge_and_rebase_select_retained_candidates_without_writing_source() {
    let fixture = Fixture::new();
    let before = fixture.inventory();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let left = payload(renamed(&mut session, &root, "sum"))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let right_payload = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":{"kind":"replace_function_body","target":"calculator.subtract","body":{"kind":"i64","value":7}}}),
    ));
    let right = right_payload["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let merged = payload(call(
        &mut session,
        "candidate/merge",
        json!({"candidate_revision":left,"other_candidate_revision":right}),
    ));
    assert_eq!(merged["kind"], "merge");
    assert_eq!(
        merged["report"]["schema"],
        "semaprax.project-candidate-rebase.v1"
    );
    let merged_report: Value = serde_json::from_str(&report(
        &mut session,
        merged["candidate"]["candidate_revision"].as_str().unwrap(),
    ))
    .unwrap();
    let root_report: Value = serde_json::from_str(&report(&mut session, &root)).unwrap();
    assert_eq!(merged_report["base_revision"], root_report["base_revision"]);
    assert_eq!(merged_report["operations"].as_array().unwrap().len(), 2);
    let rebased = payload(call(
        &mut session,
        "candidate/rebase",
        json!({"candidate_revision":left,"new_base_candidate_revision":right}),
    ));
    assert_eq!(rebased["kind"], "rebase");
    let rebased_report: Value = serde_json::from_str(&report(
        &mut session,
        rebased["candidate"]["candidate_revision"].as_str().unwrap(),
    ))
    .unwrap();
    assert_eq!(
        rebased_report["base_revision"],
        right_payload["project_revision"]
    );
    let original = report(&mut session, &left);
    let conflict = payload(renamed(&mut session, &root, "addition"))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let failure = call(
        &mut session,
        "candidate/merge",
        json!({"candidate_revision":left,"other_candidate_revision":conflict}),
    );
    assert!(failure.get("error").is_some());
    assert_eq!(report(&mut session, &left), original);
    assert_eq!(fixture.inventory(), before);
}

#[test]
fn constructor_schemas_are_closed_and_resolve_recursion_locally() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let schemas = payload(call(
        &mut session,
        "protocol/constructor-schemas",
        json!({}),
    ));
    assert_eq!(
        schemas["schema"],
        "semaprax.candidate-constructor-schemas.v1"
    );
    assert_eq!(schemas["requires_compiler_admission"], true);
    fn inspect(node: &Value, root: &Value) {
        match node {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref") {
                    let reference = reference.as_str().unwrap();
                    assert!(reference.starts_with("#/"), "{reference}");
                    assert!(root.pointer(&reference[1..]).is_some(), "{reference}");
                }
                if object.contains_key("properties") {
                    assert_eq!(object.get("additionalProperties"), Some(&json!(false)));
                }
                for value in object.values() {
                    inspect(value, root);
                }
            }
            Value::Array(values) => {
                for value in values {
                    inspect(value, root);
                }
            }
            _ => (),
        }
    }
    let documents = schemas["documents"].as_array().unwrap();
    assert_eq!(documents.len(), 4);
    for document in documents {
        inspect(document, document);
    }
    let intent = documents
        .iter()
        .find(|document| document["$id"] == "urn:semaprax.semantic-change-intent.v1")
        .unwrap();
    let kinds = intent["$defs"]["intent"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .map(|schema| schema["properties"]["kind"]["const"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        vec![
            "rename_declaration",
            "change_function_signature",
            "change_function_signature",
            "replace_function_body",
            "repair_diagnostic",
            "replace_expression",
            "replace_contract_expression",
            "add_contract",
            "implement_interface",
            "add_declaration",
            "extract_function",
            "move_declaration",
            "add_record_field"
        ]
    );
    let record = intent["$defs"]["intent"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|schema| schema["properties"]["kind"]["const"] == "add_record_field")
        .unwrap();
    let fields = record["properties"]["field"]["oneOf"].as_array().unwrap();
    let expected_fields = [
        (
            "i64",
            json!({"type":"integer","minimum":-i64::MAX,"maximum":i64::MAX}),
        ),
        ("bool", json!({"type":"boolean"})),
        (
            "i32",
            json!({"type":"integer","minimum":-i32::MAX,"maximum":i32::MAX}),
        ),
        (
            "u8",
            json!({"type":"integer","minimum":0,"maximum":u8::MAX}),
        ),
        (
            "usize",
            json!({"type":"integer","minimum":0,"maximum":u64::MAX}),
        ),
    ];
    assert_eq!(fields.len(), expected_fields.len() + 2);
    for (field, (kind, value_schema)) in fields.iter().zip(expected_fields) {
        assert_eq!(field["required"], json!(["id", "name", "type", "default"]));
        assert_eq!(field["additionalProperties"], false);
        assert_eq!(field["properties"]["type"]["const"], kind);
        assert_eq!(
            field["properties"]["default"]["properties"]["kind"]["const"],
            kind
        );
        assert_eq!(
            field["properties"]["default"]["required"],
            json!(["kind", "value"])
        );
        assert_eq!(
            field["properties"]["default"]["properties"]["value"],
            value_schema
        );
    }
    let string = &fields[5];
    assert_eq!(string["properties"]["type"]["const"], "string");
    assert_eq!(
        string["properties"]["default"]["required"],
        json!(["kind", "value"])
    );
    assert!(string["properties"]["default"]["properties"]["value"]
        .get("minLength")
        .is_none());
    assert_eq!(
        string["properties"]["default"]["properties"]["value"]["maxLength"],
        4096
    );
    assert_eq!(
        string["properties"]["default"]["properties"]["value"]["x-max-utf8-bytes"],
        16_384
    );
    let bytes = &fields[6];
    assert_eq!(bytes["properties"]["type"]["const"], "Bytes");
    assert_eq!(
        bytes["properties"]["default"]["required"],
        json!(["kind", "values"])
    );
    assert_eq!(
        bytes["properties"]["default"]["properties"]["values"]["maxItems"],
        4093
    );
}

#[test]
fn constructor_schemas_expose_closed_variant_case_addition_with_explicit_string_refusal() {
    let schemas: Value =
        serde_json::from_str(&SemanticChange::constructor_schemas().unwrap()).unwrap();
    let intent = schemas["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|document| document["$id"] == "urn:semaprax.semantic-change-intent.v1")
        .unwrap();
    let addition = intent["$defs"]["intent"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["properties"]["kind"]["const"] == "add_variant_case")
        .unwrap();
    assert_eq!(addition["additionalProperties"], false);
    let case = &addition["properties"]["case"];
    assert_eq!(case["additionalProperties"], false);
    assert_eq!(case["required"], json!(["id", "name", "field"]));
    let field = &case["properties"]["field"];
    assert_eq!(field["additionalProperties"], false);
    assert_eq!(field["required"], json!(["id", "name", "type"]));
    assert_eq!(
        field["properties"]["type"]["enum"],
        json!(["Bytes", "string"])
    );
    assert_eq!(
        field["properties"]["type"]["x-semantic-admission"],
        "Bytes_only_string_is_explicitly_unsupported"
    );
}

#[test]
fn expression_discovery_selects_replacement_and_added_contract_uses_typed_predicate() {
    let fixture = Fixture::new();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let catalogue = payload(call(
        &mut session,
        "expression/catalog",
        json!({"candidate_revision":root,"target":"calculator.add"}),
    ));
    assert_eq!(
        catalogue["schema"],
        "semaprax.project-expression-catalog.v1"
    );
    let expression = catalogue["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|expression| {
            expression["replaceable"] == true && expression["expected_type"] == "i64"
        })
        .unwrap();
    let replacement = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":root,"intent":{"kind":"replace_expression","target":"calculator.add","expression_id":expression["expression_id"],"replacement":{"kind":"i64","value":7}}}),
    ));
    let contracted = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({"candidate_revision":replacement["candidate_revision"],"intent":{"kind":"add_contract","target":"calculator.add","phase":"ensures","predicate":{"kind":"bool","value":true}}}),
    ));
    let report: Value = serde_json::from_str(&report(
        &mut session,
        contracted["candidate_revision"].as_str().unwrap(),
    ))
    .unwrap();
    assert_eq!(report["operations"].as_array().unwrap().len(), 2);
    assert_eq!(report["validation"]["tests"], "not_run");
}

#[test]
fn recovery_export_restores_in_fresh_session_and_failure_preserves_handles() {
    let fixture = Fixture::new();
    let before = fixture.inventory();
    let mut session = fixture.session(ImageHostCapability::CandidateOnly);
    let root = opened(&mut session);
    let changed = payload(renamed(&mut session, &root, "sum"))["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let original_report = report(&mut session, &changed);
    let mut bytes = String::new();
    let mut offset = 0;
    loop {
        let part = payload(call(
            &mut session,
            "candidate/recovery-export",
            json!({"candidate_revision":changed,"offset":offset,"chunk_bytes":1024}),
        ));
        bytes.push_str(part["chunk"].as_str().unwrap());
        match part["next_offset"].as_u64() {
            Some(next) => offset = next as usize,
            None => break,
        }
    }
    let capsule: Value = serde_json::from_str(&bytes).unwrap();
    drop(session);
    let mut fresh = fixture.session(ImageHostCapability::CandidateOnly);
    let restored = payload(call(
        &mut fresh,
        "candidate/recovery-restore",
        json!({"capsule":capsule}),
    ));
    assert_eq!(restored["candidate_revision"], changed);
    assert_eq!(report(&mut fresh, &changed), original_report);
    let mut invalid = capsule.clone();
    invalid["compiler"]["compatibility"] = json!("unknown");
    assert!(call(
        &mut fresh,
        "candidate/recovery-restore",
        json!({"capsule":invalid})
    )
    .get("error")
    .is_some());
    assert_eq!(report(&mut fresh, &changed), original_report);
    let mut read_only = fixture.session(ImageHostCapability::ReadOnly);
    assert!(call(
        &mut read_only,
        "candidate/recovery-restore",
        json!({"capsule":capsule})
    )
    .get("error")
    .is_some());
    assert_eq!(fixture.inventory(), before);
}
