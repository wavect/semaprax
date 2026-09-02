use super::*;
use serde_json::json;

fn method(name: &str, field: Value) -> Value {
    json!({"method":name,"request_schema":{"properties":{"params":{
        "type":"object","additionalProperties":false,"required":["value"],"properties":{"value":field}
    }}}})
}

#[test]
fn actual_recursive_constructor_and_recovery_documents_have_concrete_types() {
    let bundle: Value =
        serde_json::from_str(&crate::project::SemanticChange::constructor_schemas().unwrap())
            .unwrap();
    let documents = bundle["documents"]
        .as_array()
        .unwrap()
        .iter()
        .map(|document| {
            (
                document["$id"].as_str().unwrap().to_owned(),
                document.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let methods = [
        method(
            "candidate/apply-intent",
            json!({"type":"object","$ref":"urn:semaprax.semantic-change-intent.v1"}),
        ),
        method(
            "hole/fill",
            json!({"type":"object","$ref":"urn:semaprax.typed-expression.v1"}),
        ),
        method(
            "candidate/recovery-restore",
            json!({"type":"object","$ref":"urn:semaprax.project-candidate-recovery.v1"}),
        ),
    ];
    let model = build(&methods, &documents).unwrap();
    assert_eq!(model.params.len(), 3);
    assert!(model
        .definitions
        .iter()
        .any(|definition| matches!(&definition.shape, Shape::Alias(_))));
    assert!(model
        .definitions
        .iter()
        .any(|definition| matches!(&definition.shape, Shape::Union(_))));
    assert!(!model
        .definitions
        .iter()
        .any(|definition| matches!(&definition.shape, Shape::Any)));
    for language in ["rust", "typescript", "python"] {
        let generated = generate(language, &methods, &documents).unwrap();
        assert!(generated.source.len() <= MAX_SOURCE_BYTES);
        assert_eq!(generated.params.len(), 3);
    }
}

#[test]
fn alias_and_union_only_recursion_rejects_even_with_a_terminating_branch() {
    for definition in [
        json!({"$ref":"#/$defs/node"}),
        json!({"anyOf":[{"$ref":"#/$defs/node"},{"type":"string"}]}),
    ] {
        let documents = BTreeMap::from([(
            "urn:cycle".into(),
            json!({"$id":"urn:cycle","$ref":"#/$defs/node","$defs":{"node":definition}}),
        )]);
        let errors = build(&[method("probe", json!({"$ref":"urn:cycle"}))], &documents)
            .err()
            .unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
        assert!(errors[0].message.contains("unproductive"));
    }
    let documents = BTreeMap::from([(
        "urn:guarded".into(),
        json!({"$id":"urn:guarded","type":"object","additionalProperties":false,"properties":{"next":{"$ref":"urn:guarded"}}}),
    )]);
    assert!(build(
        &[method("probe", json!({"$ref":"urn:guarded"}))],
        &documents
    )
    .is_ok());
}

#[test]
fn local_references_keep_document_scope_and_literal_refs_are_plain_data() {
    let documents = BTreeMap::from([
        (
            "urn:left".into(),
            json!({"$id":"urn:left","$ref":"#/$defs/item","$defs":{"item":{"const":"left"}}}),
        ),
        (
            "urn:right".into(),
            json!({"$id":"urn:right","$ref":"#/$defs/item","$defs":{"item":{"const":"right"}}}),
        ),
    ]);
    let methods = [
        method("left", json!({"$ref":"urn:left"})),
        method("right", json!({"$ref":"urn:right"})),
        method("literal", json!({"const":{"$ref":"urn:missing"}})),
    ];
    let model = build(&methods, &documents).unwrap();
    for expected in ["left", "right", "urn:missing"] {
        assert!(model.definitions.iter().any(
            |definition| matches!(&definition.shape,Shape::Literal(value) if value==expected)
        ));
    }
    let mut missing = documents.clone();
    missing.get_mut("urn:right").unwrap()["$defs"] = json!({});
    let errors = build(&methods, &missing).err().unwrap();
    assert_eq!(errors[0].code, "SPX-G288");
    assert!(errors[0].message.contains("pointer is missing"));
    let errors = build(
        &[method("unscoped", json!({"$ref":"#/$defs/item"}))],
        &documents,
    )
    .err()
    .unwrap();
    assert_eq!(errors[0].code, "SPX-G288");
}

#[test]
fn reference_object_assertions_and_unknown_shapes_fail_closed() {
    let documents = BTreeMap::from([(
        "urn:scalar".into(),
        json!({"$id":"urn:scalar","type":"string"}),
    )]);
    let errors = build(
        &[method(
            "probe",
            json!({"type":"object","$ref":"urn:scalar"}),
        )],
        &documents,
    )
    .err()
    .unwrap();
    assert_eq!(errors[0].code, "SPX-G288");
    assert!(errors[0].message.contains("object assertion"));
    for schema in [
        json!({"allOf":[]}),
        json!({"type":"number"}),
        json!({"type":"integer","const":"not an integer"}),
        json!({"type":"string","enum":["accepted",false]}),
        json!({"$ref":"urn:absent"}),
        json!({"type":"string","not":{"type":"string"}}),
    ] {
        assert_eq!(
            build(&[method("probe", schema)], &documents).err().unwrap()[0].code,
            "SPX-G288"
        );
    }
}
