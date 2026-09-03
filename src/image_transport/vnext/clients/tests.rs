use super::*;

fn workflow_capabilities(value: &mut Value) -> &mut Value {
    &mut value["documents"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|document| document["$id"] == "urn:semaprax.image-agent-capabilities.v5")
        .unwrap()["const"]
}

#[test]
fn supported_workflow_types_are_closed_and_resolve_exact_steps() {
    let typescript = workflow_client_types::typescript();
    assert!(typescript.contains("export interface WorkflowResponseContract"));
    assert!(typescript.contains("export function workflowResponseContract"));
    assert!(typescript.contains("export function validateWorkflowCatalogue"));
    assert!(typescript.contains("JSON.stringify(value) !== WORKFLOWS_JSON"));
    assert!(!typescript.contains("[key: string]: unknown"));

    let python = workflow_client_types::python();
    assert!(python.contains("class WorkflowResponseContract(TypedDict)"));
    assert!(python.contains("def workflow_response_contract("));
    assert!(python.contains("def validate_workflow_catalogue("));
    assert!(python.contains("!= WORKFLOWS_JSON"));

    let rust = workflow_client_types::rust();
    assert!(rust.contains("pub enum WorkflowPhaseId"));
    assert!(rust.contains("pub enum WorkflowSession"));
    assert!(rust.contains("pub enum WorkflowEffect"));
    assert!(rust.contains("pub fn workflow_response_contract"));
    assert!(rust.contains("pub fn validate_workflows"));
    assert!(rust.contains("fn validate_workflow_catalogue_json"));
}

#[test]
fn workflow_generation_rejects_changed_profile_revision_and_response_contract() {
    use super::super::super::VNextPolicy;
    let policy = VNextPolicy {
        candidate_prepare: true,
        test_policy: Some(crate::project::CandidateTestPolicy::new(100, 4096, 16384).unwrap()),
        ..VNextPolicy::default()
    };
    let methods = super::super::super::session_methods(&policy, false, false, false);
    let capabilities = super::super::capabilities(&methods, &policy, false);
    let bundle = super::super::bundle(
        &methods
            .iter()
            .map(|method| super::super::descriptor(method, &policy))
            .collect::<Vec<_>>(),
        &capabilities,
    )
    .unwrap();
    for language in ["typescript", "python", "rust"] {
        let source = generate(language, &bundle).unwrap();
        assert!(source.contains("EXPECTED_WORKFLOW_PROFILE_REVISION"));
    }

    let mut changed_revision = bundle.clone();
    workflow_capabilities(&mut changed_revision)["workflows"][0]["qualification"]
        ["selected_profile_revision"] = json!(format!("sha256:{}", "0".repeat(64)));
    for language in ["typescript", "python", "rust"] {
        assert!(generate(language, &changed_revision).is_err());
    }

    let mut changed_contract = bundle;
    workflow_capabilities(&mut changed_contract)["workflows"][0]["phases"][0]["ordered_steps"][0]
        ["response_contract"]["effect"] = json!("source_publication");
    for language in ["typescript", "python", "rust"] {
        assert!(generate(language, &changed_contract).is_err());
    }
}

#[test]
fn client_contract_revision_is_deterministic_closed_and_surface_sensitive() {
    use super::super::super::VNextPolicy;
    let policy = VNextPolicy {
        candidate_prepare: true,
        test_policy: Some(crate::project::CandidateTestPolicy::new(100, 4096, 16384).unwrap()),
        ..VNextPolicy::default()
    };
    let selected = super::super::super::session_methods(&policy, false, false, false);
    let capabilities = super::super::capabilities(&selected, &policy, false);
    let bundle = super::super::bundle(
        &selected
            .iter()
            .map(|method| super::super::descriptor(method, &policy))
            .collect::<Vec<_>>(),
        &capabilities,
    )
    .unwrap();
    let methods = bundle["methods"].as_array().unwrap();
    let requests = request_documents(&bundle).unwrap();
    let responses = response_documents(&bundle).unwrap();
    let workflows = responses["urn:semaprax.image-agent-capabilities.v5"]["const"]["workflows"]
        .as_array()
        .unwrap();
    let contract = client_contract(methods, &requests, &responses, workflows, &bundle).unwrap();
    let revision = client_contract_revision(&contract).unwrap();
    assert_eq!(revision.len(), "sha256:".len() + 64);
    assert!(revision
        .strip_prefix("sha256:")
        .unwrap()
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    assert_eq!(client_contract_revision(&contract).unwrap(), revision);

    let reversed = Value::Object(
        contract
            .as_object()
            .unwrap()
            .iter()
            .rev()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
    );
    assert_eq!(client_contract_revision(&reversed).unwrap(), revision);

    let mut revisions = BTreeSet::from([revision.clone()]);
    for (field, mutation) in [
        ("methods", json!({"tampered":true})),
        ("request_documents", json!({"tampered":true})),
        ("response_documents", json!({"tampered":true})),
        ("workflows", json!({"tampered":true})),
        ("capabilities", json!({"tampered":true})),
        ("unbundled", json!("urn:tampered")),
    ] {
        let mut changed = contract.clone();
        match field {
            "methods" | "workflows" | "unbundled" => {
                changed[field].as_array_mut().unwrap().push(mutation)
            }
            _ => {
                changed[field]["urn:tampered"] = mutation;
            }
        }
        revisions.insert(client_contract_revision(&changed).unwrap());
    }
    assert_eq!(revisions.len(), 7);

    for malformed in [
        Value::Null,
        json!({}),
        json!({
            "protocol":VNEXT_PROTOCOL_SCHEMA,
            "result_schema":VNEXT_RESULT_SCHEMA,
            "methods":[],
            "request_documents":{},
            "response_documents":{},
            "workflows":[],
            "capabilities":{},
            "unbundled":[]
        }),
    ] {
        assert!(client_contract_revision(&malformed).is_err());
    }

    for language in ["typescript", "python", "rust"] {
        let source = generate(language, &bundle).unwrap();
        assert!(source.contains("CLIENT_CONTRACT_REVISION"));
        assert!(source.contains(&revision));
    }
}

#[test]
fn unsupported_response_assertions_fail_before_client_generation() {
    for unsupported in [
        json!({"allOf":[{"type":"string"}]}),
        json!({"type":"object","additionalProperties":{"type":"string"}}),
        json!({"$ref":"#/$defs/x"}),
        json!({"$ref":"urn:example","type":"string"}),
        json!({"pattern":"not_a_supported_pattern"}),
        json!({"type":"array","uniqueItems":true}),
        json!({"minimum":1}),
    ] {
        assert!(
            audit_schema(&unsupported, &mut BTreeSet::new(), 0).is_err(),
            "{unsupported}"
        );
    }
    // Constant JSON is data, including keys which resemble schema keywords.
    audit_schema(
        &json!({"const":{"allOf":[],"$ref":"#not_a_schema_reference"}}),
        &mut BTreeSet::new(),
        0,
    )
    .unwrap();
    for (pattern, length) in [("^[0-9a-f]{8}$", 8), ("^[0-9a-f]{16}$", 16)] {
        audit_schema(
            &json!({"type":"string","minLength":length,"maxLength":length,"pattern":pattern}),
            &mut BTreeSet::new(),
            0,
        )
        .unwrap();
    }
}

#[test]
fn metadata_contains_only_transitive_selected_response_documents() {
    let bundle = json!({"methods":[{"request_schema":{"properties":{"params":{"type":"object","properties":{}}}},"success_response_schema":{"properties":{"result":{"properties":{"payload":{"$ref":"urn:outer"}}}}}}],
            "documents":[{"$id":"urn:outer","type":"object","properties":{"child":{"$ref":"urn:inner"}}},{"$id":"urn:inner","type":"string"},{"$id":"urn:unselected","allOf":[]}],"unbundled_payload_schemas":[]});
    let docs = response_documents(&bundle).unwrap();
    assert_eq!(
        docs.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["urn:inner", "urn:outer"]
    );
    let mut changed = bundle;
    changed["documents"][1]["allOf"] = json!([]);
    assert!(response_documents(&changed).is_err());
}

#[test]
fn rust_client_fits_the_actual_serialized_discovery_payload() {
    use super::super::super::VNextPolicy;
    for policy in [
        VNextPolicy::default(),
        VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        },
        VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            ..VNextPolicy::default()
        },
        VNextPolicy {
            candidate_prepare: true,
            diagnostics: true,
            build_enabled: true,
            ..VNextPolicy::default()
        },
    ] {
        for batch_selected in [false, true] {
            let methods =
                super::super::super::session_methods(&policy, false, false, batch_selected);
            let method = methods
                .iter()
                .find(|method| method.name == "protocol/client")
                .unwrap();
            let params = serde_json::Map::from_iter([("language".to_owned(), json!("rust"))]);
            // Exercise the production payload builder and its unchanged cap:
            // the JSON source string escapes quotes/newlines and is larger
            // than the generated source's own byte count.
            let report = super::super::payload(method, &params, &methods, &policy, false).unwrap();
            assert!(
                serde_json::to_vec(&report).unwrap().len() <= super::super::MAX_DISCOVERY_BYTES
            );
            let source = report["source"].as_str().unwrap();
            assert!(source.contains("response_literal!"));
            assert_eq!(source.matches("macro_rules! response_literal").count(), 1);
            assert!(source.contains("response literal mismatch"));
            assert_eq!(
                source.contains("pub fn request_workspace_read_batch("),
                batch_selected
            );
        }
    }
}

#[test]
fn selected_recursive_definitions_normalize_without_erasing_assertions() {
    let root = json!({"$id":"urn:recursive","$ref":"#/$defs/expression","$defs":{
        "expression":{"oneOf":[
            {"type":"string","minLength":1,"x-max-utf8-bytes":128,"pattern":"^[A-Za-z_][A-Za-z0-9_]*$","not":{"enum":["let"]}},
            {"type":"array","items":{"$ref":"#/$defs/expression"},"maxItems":2,"x-max-expression-nodes":4096},
            {"const":{"$ref":"#literal-data","$id":"literal-data"}}
        ]},
        "unused":{"type":"string"}
    }});
    let bundle = json!({"methods":[{"request_schema":{"properties":{"params":{"type":"object","properties":{}}}},"success_response_schema":{"properties":{"result":{"properties":{"payload":{"$ref":"urn:recursive"}}}}}}],
            "documents":[root],"unbundled_payload_schemas":[]});
    let docs = response_documents(&bundle).unwrap();
    assert_eq!(docs.len(), 2);
    assert_eq!(
        docs["urn:recursive"]["$ref"],
        "urn:recursive:response-def:expression"
    );
    let cases = &docs["urn:recursive:response-def:expression"]["oneOf"];
    assert_eq!(cases[0]["not"], json!({"enum":["let"]}));
    assert_eq!(cases[0]["x-max-utf8-bytes"], 128);
    assert_eq!(
        cases[1]["items"]["$ref"],
        "urn:recursive:response-def:expression"
    );
    assert!(cases[1].get("x-max-expression-nodes").is_none());
    assert_eq!(cases[2]["const"]["$ref"], "#literal-data");
    for hostile in [
        json!({"$ref":"#/$defs/missing"}),
        json!({"$ref":"https://example.invalid/schema"}),
        json!({"$ref":"urn:other#/$defs/x"}),
        json!({"$id":"urn:nested","type":"string"}),
        json!({"type":"string","x-unknown-validation":true}),
        json!({"type":"string","not":{"type":"string"}}),
        json!({"type":"string","not":{"enum":["let","let"]}}),
        json!({"type":"string","allOf":[{"minLength":1}]}),
    ] {
        let mut changed = bundle.clone();
        changed["documents"][0]["$defs"]["expression"]["oneOf"][0] = hostile;
        assert!(response_documents(&changed).is_err());
    }
    let mut collision = bundle;
    collision["documents"]
        .as_array_mut()
        .unwrap()
        .push(json!({"$id":"urn:recursive:response-def:expression","type":"string"}));
    assert!(response_documents(&collision).is_err());
}

#[test]
fn typed_public_names_reject_colliding_method_spellings() {
    let methods = ["sample/value", "sample-value"].map(|method| {
        json!({
            "method":method,
            "request_schema":{"properties":{"params":{
                "type":"object","properties":{},"required":[],"additionalProperties":false
            }}},
            "success_response_schema":{"properties":{"result":{"properties":{
                "payload":{"$ref":"urn:semaprax.image-agent-capabilities.v5"}
            }}}}
        })
    });
    let bundle = json!({"methods":methods,"documents":[{
        "$id":"urn:semaprax.image-agent-capabilities.v5","const":{"workflows":[]}
    }],"unbundled_payload_schemas":[]});
    for language in ["typescript", "python", "rust"] {
        let errors = generate(language, &bundle).unwrap_err();
        assert_eq!(errors[0].code, "SPX-G288");
        assert!(errors[0].message.contains("names collide"));
    }
}
