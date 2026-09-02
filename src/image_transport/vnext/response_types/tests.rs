use super::*;
use serde_json::json;

fn methods(payload: Value) -> Vec<Value> {
    vec![
        json!({"method":"probe/read","success_response_schema":{"properties":{
            "result":{"properties":{"payload":payload}}
        }}}),
    ]
}

#[test]
fn response_references_reject_unguarded_cycles_and_require_explicit_opaque_inventory() {
    let request = methods(json!({"$ref":"urn:outer"}));
    let cycle = BTreeMap::from([
        ("urn:outer".into(), json!({"$ref":"urn:inner"})),
        ("urn:inner".into(), json!({"$ref":"urn:outer"})),
    ]);
    for language in ["rust", "python", "typescript"] {
        let errors = generate(language, &request, &cycle, &[]).err().unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
        assert!(errors[0].message.contains("recursion"));
        let errors = generate(language, &request, &BTreeMap::new(), &[])
            .err()
            .unwrap();
        assert_eq!(errors[0].code, "SPX-G288");
        assert!(errors[0].message.contains("not bundled"));
        let result = generate(language, &request, &BTreeMap::new(), &[json!("urn:outer")]).unwrap();
        assert_eq!(result.payloads.len(), 1);
    }
    let errors = generate("rust", &methods(json!({"allOf":[]})), &BTreeMap::new(), &[])
        .err()
        .unwrap();
    assert_eq!(errors[0].code, "SPX-G288");
}

#[test]
fn literal_reference_keys_remain_typed_data_without_schema_lookup() {
    let request =
        methods(json!({"const":{"$ref":"urn:not-a-schema","required":null,"items":[true,"x"]}}));
    let generated = generate("rust", &request, &BTreeMap::new(), &[]).unwrap();
    assert!(generated.source.contains("#[serde(rename = \"$ref\")]"));
    assert!(generated.source.contains("urn:not-a-schema"));
    assert!(generated.source.contains("response literal mismatch"));
    assert!(generated.source.contains("#[serde(deny_unknown_fields)]"));
}

#[test]
fn guarded_recursive_objects_and_arrays_have_finite_named_client_representations() {
    let request = methods(json!({"$ref":"urn:node"}));
    let documents = BTreeMap::from([(
        "urn:node".into(),
        json!({
            "type":"object","additionalProperties":false,"required":["value"],
            "properties":{"value":{"type":"integer"},
                "parent":{"$ref":"urn:node"},
                "next":{"anyOf":[{"$ref":"urn:node"},{"type":"null"}]},
                "children":{"type":"array","items":{"$ref":"urn:node"}}}
        }),
    )]);
    for language in ["rust", "python", "typescript"] {
        let generated = generate(language, &request, &documents, &[]).unwrap();
        assert!(!generated.source.contains("ResponsePending"));
        assert_eq!(
            generated.source,
            generate(language, &request, &documents, &[])
                .unwrap()
                .source
        );
        assert_eq!(generated.payloads.len(), 1);
        match language {
            "rust" => {
                assert!(generated.source.contains("#[serde(transparent)]"));
                assert!(generated.source.contains("Presence<Box<ResponseType"));
                assert!(generated.source.contains("Vec<ResponseType"));
                assert!(generated.source.contains("Signed(i64), Unsigned(u64)"));
            }
            "python" => {
                assert!(generated.source.contains("NotRequired[\"ResponseType"));
                assert!(generated.source.contains("list[\"ResponseType"));
            }
            "typescript" => assert!(generated.source.contains("Array<ResponseType")),
            _ => unreachable!(),
        }
    }
    let array = BTreeMap::from([(
        "urn:node".into(),
        json!({"type":"array","items":{"$ref":"urn:node"}}),
    )]);
    let generated = generate("rust", &request, &array, &[]).unwrap();
    assert!(generated.source.contains("(pub Vec<ResponseType"));
    assert!(generated.source.contains("(pub Box<ResponseType"));
}

#[test]
fn terminating_union_branch_does_not_authorize_unguarded_recursion() {
    let documents = BTreeMap::from([
        (
            "urn:a".into(),
            json!({"anyOf":[{"$ref":"urn:b"},{"type":"integer"}]}),
        ),
        ("urn:b".into(), json!({"$ref":"urn:a"})),
    ]);
    for language in ["rust", "python", "typescript"] {
        let error = generate(language, &methods(json!({"$ref":"urn:a"})), &documents, &[])
            .err()
            .unwrap();
        assert_eq!(error[0].code, "SPX-G288");
        assert!(error[0].message.contains("unproductive"));
    }
    let documents = BTreeMap::from([(
        "urn:a".into(),
        json!({"type":"array","items":{"type":"integer"}}),
    )]);
    assert!(generate(
        "rust",
        &methods(json!({"type":"object","$ref":"urn:a"})),
        &documents,
        &[]
    )
    .is_err());
    assert!(generate(
        "rust",
        &methods(json!({"type":"integer","const":"wrong"})),
        &BTreeMap::new(),
        &[]
    )
    .is_err());
}

#[test]
fn recursive_component_analysis_includes_cross_edges_to_finished_branches() {
    let model = Model {
        payloads: BTreeMap::new(),
        definitions: vec![
            Definition {
                name: "A".into(),
                shape: Shape::Object {
                    open: false,
                    fields: vec![
                        Field {
                            name: "b".into(),
                            ty: "B".into(),
                            required: true,
                        },
                        Field {
                            name: "d".into(),
                            ty: "D".into(),
                            required: false,
                        },
                    ],
                },
            },
            Definition {
                name: "B".into(),
                shape: Shape::Alias("C".into()),
            },
            Definition {
                name: "C".into(),
                shape: Shape::Alias("A".into()),
            },
            Definition {
                name: "D".into(),
                shape: Shape::Alias("C".into()),
            },
            Definition {
                name: "Leaf".into(),
                shape: Shape::Integer,
            },
        ],
    };
    assert_eq!(
        recursive_names(&model).unwrap(),
        ["A", "B", "C", "D"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}

#[test]
fn recursive_rust_union_checks_discriminants_before_children_and_bounds_retries() {
    let documents = BTreeMap::from([(
        "urn:expr".into(),
        json!({"oneOf":[
            {"type":"object","additionalProperties":false,"required":["kind","arguments"],
                "properties":{"arguments":{"type":"array","items":{"$ref":"urn:expr"}},
                    "kind":{"const":"call"}}},
            {"type":"object","additionalProperties":false,"required":["kind","target","arguments"],
                "properties":{"arguments":{"type":"array","items":{"$ref":"urn:expr"}},
                    "kind":{"const":"builtin_call"},"target":{"const":"core.bytes.len"}}},
            {"type":"integer"}
        ]}),
    )]);
    let generated = generate(
        "rust",
        &methods(json!({"$ref":"urn:expr"})),
        &documents,
        &[],
    )
    .unwrap();
    let dispatch_and_helpers = generated
        .source
        .split("        for branch in 0..3 {")
        .nth(1)
        .unwrap();
    let (dispatcher, helpers) = dispatch_and_helpers
        .split_once("\nimpl ResponseType")
        .unwrap();
    let charge = dispatcher
        .find("ResponseTypeDecodeGuard::charge()")
        .unwrap();
    let selection = dispatcher
        .find("let convert:Option<fn(&Value)->Result<Self,serde_json::Error>>=match branch")
        .unwrap();
    let conversion = dispatcher.find("convert(&value)").unwrap();
    assert!(charge < selection && selection < conversion);
    assert_eq!(dispatcher.matches("convert(&value)").count(), 1);
    assert!(!dispatcher.contains("serde_json::from_value"));
    assert!(!dispatcher.contains("value.clone()"));
    let sticky = dispatcher.find("ResponseTypeDecodeGuard::check()").unwrap();
    assert!(conversion < sticky && sticky < dispatcher.find("return Ok(parsed)").unwrap());
    assert_eq!(
        dispatcher
            .matches("ResponseTypeDecodeGuard::check()")
            .count(),
        2
    );
    // Both recursive object alternatives inspect their discriminants before
    // choosing an outlined conversion, including the builtin's target.
    for index in 0..2 {
        let arm = dispatcher
            .split(&format!("{index} if value.is_object()"))
            .nth(1)
            .unwrap()
            .split('\n')
            .next()
            .unwrap();
        let constant = arm.find("value.get(\"kind\")").unwrap();
        let selected = arm
            .find(&format!("Some(Self::__response_decode_choice_{index})"))
            .unwrap();
        assert!(constant < selected);
        if index == 1 {
            assert!(arm.find("value.get(\"target\")").unwrap() < selected);
        }
    }
    // Debug builds must not reserve all branch-specific serde temporaries
    // in the recursive dispatcher. Keep each conversion in its own frame.
    for index in 0..3 {
        let signature = format!("    #[inline(never)]\n    fn __response_decode_choice_{index}(value:&Value)->Result<Self,serde_json::Error> {{");
        let helper = helpers
            .split(&signature)
            .nth(1)
            .unwrap()
            .split("\n    }")
            .next()
            .unwrap();
        assert_eq!(
            helper
                .matches(" as Deserialize>::deserialize(value)")
                .count(),
            1
        );
        assert!(!helper.contains("serde_json::from_value"));
        assert!(!helper.contains("value.clone()"));
        assert!(helper.contains(&format!(".map(Self::Choice{index})")));
    }
    assert!(generated
        .source
        .contains("literal.as_str()==Some(\"core.bytes.len\")"));
    assert!(generated
        .source
        .contains("let _budget=ResponseTypeDecodeGuard::enter()?;"));
    assert!(generated.source.contains("remaining=65_536"));
    assert!(generated.source.contains("depth>=128"));
    assert!(generated
        .source
        .contains("state.set((remaining,depth,true))"));
    assert!(generated
        .source
        .contains("if depth==0 { remaining=65_536; failed=false; }"));
    assert!(generated
        .source
        .contains("ResponseTypeDecodeGuard::check()?;\n    let payload=payload.map_err"));
}

#[test]
fn optional_nullable_fields_keep_presence_separate_from_required_null() {
    let nullable = json!({"anyOf":[{"type":"string"},{"type":"null"}]});
    let schema = json!({"type":"object","additionalProperties":false,
            "required":["must"],"properties":{"must":nullable,"maybe":nullable}});
    let documents = BTreeMap::new();
    let mut builder = Builder {
        documents: &documents,
        unbundled: &[],
        definitions: Vec::new(),
        names: BTreeMap::new(),
        reservations: BTreeMap::new(),
        object_guards: Vec::new(),
        work: 0,
        key_bytes: 0,
    };
    let root = builder.schema(&schema, 0).unwrap();
    let object = builder
        .definitions
        .iter()
        .find(|definition| definition.name == root)
        .unwrap();
    let Shape::Object {
        fields,
        open: false,
    } = &object.shape
    else {
        panic!("closed response object expected")
    };
    let must = fields.iter().find(|field| field.name == "must").unwrap();
    let maybe = fields.iter().find(|field| field.name == "maybe").unwrap();
    assert!(must.required);
    assert!(!maybe.required);
    assert_eq!(must.ty, maybe.ty);
    let generated = generate("rust", &methods(schema), &documents, &[]).unwrap();
    assert!(generated
        .source
        .contains(&format!("pub r#must: {},", must.ty)));
    assert!(generated
        .source
        .contains(&format!("pub r#maybe: Presence<{}>,", maybe.ty)));
    assert!(generated.source.contains("impl<T> Default for Presence<T>"));
    assert!(generated
        .source
        .contains("T::deserialize(deserializer).map(Self::Present)"));
}

#[test]
fn large_literal_arrays_keep_each_position_and_full_integer_extremes() {
    let mut values = (0..32).map(|value| json!(value)).collect::<Vec<_>>();
    values.extend([json!(i64::MIN), json!(u64::MAX)]);
    let generated = generate(
        "rust",
        &methods(json!({"const":values})),
        &BTreeMap::new(),
        &[],
    )
    .unwrap();
    assert!(generated.source.contains("pub item_33:"));
    assert!(generated
        .source
        .contains("deserializer.deserialize_tuple(34,SequenceVisitor)"));
    assert!(generated
        .source
        .contains("sequence.next_element::<serde::de::IgnoredAny>()?"));
    assert!(generated
        .source
        .contains("serde::ser::SerializeTuple::serialize_element"));
    assert!(generated.source.contains("-9223372036854775808"));
    assert!(generated.source.contains("18446744073709551615"));
    assert!(generated.source.contains("Signed(i64), Unsigned(u64)"));
    let thirteen = (0..13).map(|value| json!(value)).collect::<Vec<_>>();
    let nested = generate(
        "rust",
        &methods(json!({"const":{"items":thirteen}})),
        &BTreeMap::new(),
        &[],
    )
    .unwrap();
    assert!(nested.source.contains("pub item_12:"));
    assert!(nested
        .source
        .contains("deserializer.deserialize_tuple(13,SequenceVisitor)"));
}
