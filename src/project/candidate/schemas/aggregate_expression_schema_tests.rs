use super::*;

#[test]
fn storage_literals_and_record_defaults_have_closed_bounds() {
    let expression = expression_schema();
    let forms = expression["oneOf"].as_array().unwrap();
    let string = forms
        .iter()
        .find(|form| form["properties"]["kind"]["const"] == "string")
        .unwrap();
    assert_eq!(string["additionalProperties"], false);
    assert_eq!(string["required"], json!(["kind", "value"]));
    assert_eq!(
        string["properties"]["value"]["maxLength"],
        MAX_STRING_LITERAL_BYTES
    );
    assert_eq!(
        string["properties"]["value"]["x-max-utf8-bytes"],
        MAX_STRING_LITERAL_BYTES
    );
    assert!(string["properties"]["value"].get("minLength").is_none());
    let array = forms
        .iter()
        .find(|form| form["properties"]["kind"]["const"] == "array_u8")
        .unwrap();
    assert_eq!(array["additionalProperties"], false);
    assert_eq!(array["required"], json!(["kind", "values"]));
    let values = &array["properties"]["values"];
    assert_eq!(values["maxItems"], MAX_EXPRESSION_NODES - 1);
    assert_eq!(values["x-counts-toward-expression-node-budget"], true);
    assert_eq!(
        values["items"],
        json!({"type":"integer","minimum":0,"maximum":255})
    );
    assert!(values.get("minItems").is_none());
    assert_eq!(
        COMPUTED_SCALAR_KINDS,
        &["i64", "i32", "u8", "usize", "bool"]
    );
    assert_eq!(
        SIGNATURE_LITERAL_KINDS,
        &["i64", "i32", "u8", "usize", "bool", "char", "f32", "f64"]
    );
    assert!(!forms
        .iter()
        .any(|form| form["properties"]["kind"]["const"] == "repeat_array_u8"));
    for (kind, field, pattern, length) in [
        ("char", "scalar", HEX32_PATTERN, 8),
        ("f32", "bits", HEX32_PATTERN, 8),
        ("f64", "bits", HEX64_PATTERN, 16),
    ] {
        let form = forms
            .iter()
            .find(|form| form["properties"]["kind"]["const"] == kind)
            .unwrap();
        assert_eq!(form["additionalProperties"], false);
        assert_eq!(form["required"], json!(["kind", field]));
        assert_eq!(form["properties"].as_object().unwrap().len(), 2);
        assert_eq!(form["properties"][field]["type"], "string");
        assert_eq!(form["properties"][field]["minLength"], length);
        assert_eq!(form["properties"][field]["maxLength"], length);
        assert_eq!(form["properties"][field]["pattern"], pattern);
        assert!(form["properties"].get("value").is_none());
    }
}

#[test]
fn contract_replacement_is_a_distinct_closed_typed_intention() {
    let schema = intent_schema();
    for kind in ["replace_expression", "replace_contract_expression"] {
        let form = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|form| form["properties"]["kind"]["const"] == kind)
            .unwrap();
        assert_eq!(
            form["required"],
            json!(["kind", "target", "expression_id", "replacement"])
        );
        assert_eq!(form["additionalProperties"], false);
        assert_eq!(form["properties"]["replacement"], reference("expression"));
        assert_eq!(form["properties"].as_object().unwrap().len(), 4);
        assert!(form["properties"].get("phase").is_none());
        assert!(form["properties"].get("path").is_none());
    }
}

#[test]
fn record_field_defaults_close_scalar_string_and_bytes_shapes() {
    let schema = intent_schema();
    let record = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|form| form["properties"]["kind"]["const"] == "add_record_field")
        .unwrap();
    let fields = record["properties"]["field"]["oneOf"].as_array().unwrap();
    assert_eq!(fields.len(), RECORD_FIELD_LITERAL_KINDS.len() + 2);
    assert_eq!(
        fields
            .iter()
            .map(|field| field["properties"]["type"]["const"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["i64", "bool", "i32", "u8", "usize", "string", "Bytes"]
    );
    assert!(fields[..RECORD_FIELD_LITERAL_KINDS.len()]
        .iter()
        .all(|field| {
            field["properties"]["default"]["properties"]
                .get("value")
                .is_some()
        }));
    let string = &fields[RECORD_FIELD_LITERAL_KINDS.len()];
    assert_eq!(
        string["properties"]["default"]["properties"]["value"]["maxLength"],
        MAX_STRING_LITERAL_BYTES / 4
    );
    assert!(string["properties"]["default"]["properties"]["value"]
        .get("minLength")
        .is_none());
    assert_eq!(
        string["properties"]["default"]["properties"]["value"]["x-max-utf8-bytes"],
        MAX_STRING_LITERAL_BYTES
    );
    let bytes = &fields[RECORD_FIELD_LITERAL_KINDS.len() + 1];
    assert_eq!(
        bytes["properties"]["default"]["properties"]["values"]["maxItems"],
        MAX_EXPRESSION_NODES - 3
    );
    assert_eq!(
        bytes["properties"]["default"]["properties"]["values"]["items"],
        json!({"type":"integer","minimum":0,"maximum":255})
    );
}

#[test]
fn computed_signature_arguments_are_a_separate_recursive_mapping_only_form() {
    let schema = intent_schema();
    let forms = schema["oneOf"].as_array().unwrap();
    let append = forms
        .iter()
        .find(|form| form["properties"].get("append_parameters").is_some())
        .unwrap();
    let literal_items = append["properties"]["append_parameters"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(literal_items.len(), 8);
    for literal in literal_items {
        assert_eq!(literal["additionalProperties"], false);
        assert_eq!(literal["required"], json!(["name", "type", "argument"]));
        assert!(literal["properties"].get("argument_expression").is_none());
        assert!(SIGNATURE_LITERAL_KINDS
            .contains(&literal["properties"]["type"]["const"].as_str().unwrap()));
        assert!(literal["properties"]["type"].get("oneOf").is_none());
        assert_eq!(
            literal["properties"]["type"]["const"],
            literal["properties"]["argument"]["properties"]["kind"]["const"]
        );
    }
    let literal_kinds = literal_items
        .iter()
        .map(|item| item["properties"]["type"]["const"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(literal_kinds, SIGNATURE_LITERAL_KINDS);
    let mapped = forms
        .iter()
        .find(|form| form["properties"].get("parameters").is_some())
        .unwrap();
    let choices = mapped["properties"]["parameters"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(choices.len(), 5);
    assert_eq!(choices[0]["required"], json!(["from"]));
    assert_eq!(choices[1]["required"], json!(["from", "name"]));
    assert_eq!(choices[2]["additionalProperties"], false);
    assert_eq!(choices[2]["required"], json!(["name", "borrow_from"]));
    assert_eq!(choices[2]["properties"].as_object().unwrap().len(), 2);
    assert_eq!(choices[2]["properties"]["borrow_from"]["type"], "string");
    assert_eq!(choices[3]["oneOf"].as_array().unwrap(), literal_items);
    let computed = &choices[4];
    assert_eq!(computed["additionalProperties"], false);
    assert_eq!(
        computed["required"],
        json!(["name", "type", "argument_expression"])
    );
    assert_eq!(computed["properties"].as_object().unwrap().len(), 3);
    let types = computed["properties"]["type"]["oneOf"].as_array().unwrap();
    assert_eq!(types.len(), 2);
    assert_eq!(types[0]["enum"], json!(COMPUTED_SCALAR_KINDS));
    assert_eq!(types[1], nominal_type_schema());
    assert_eq!(types[1]["additionalProperties"], false);
    assert_eq!(
        types[1]["required"],
        json!(["kind", "target", "type_arguments"])
    );
    assert_eq!(
        types[1]["properties"]["type_arguments"]["items"]["enum"],
        json!(["i64", "bool"])
    );
    assert!(types[1]["properties"]["type_arguments"]
        .get("minItems")
        .is_none());
    assert_eq!(
        types[1]["properties"]["type_arguments"]["maxItems"],
        MAX_AGGREGATE_TYPE_ARGUMENTS
    );
    assert_eq!(
        computed["properties"]["argument_expression"],
        reference("expression")
    );
    for excluded in ["from", "argument", "source", "mode"] {
        assert!(computed["properties"].get(excluded).is_none());
    }
}

#[test]
fn lexical_binding_closes_scope_and_recursive_children_without_type_authority() {
    let schema = expression_schema();
    let binding = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|variant| variant["properties"]["kind"]["const"] == "let")
        .unwrap();
    assert_eq!(binding["additionalProperties"], false);
    assert_eq!(
        binding["required"],
        json!(["kind", "name", "value", "body"])
    );
    assert_eq!(binding["properties"].as_object().unwrap().len(), 4);
    assert_eq!(binding["properties"]["value"], reference("expression"));
    assert_eq!(binding["properties"]["body"], reference("expression"));
    let name = &binding["properties"]["name"];
    assert_eq!(name["maxLength"], MAX_NAME_BYTES);
    assert_eq!(name["pattern"], "^[A-Za-z_][A-Za-z0-9_]*$");
    let reserved = name["not"]["enum"].as_array().unwrap();
    for token in [
        "let", "mut", "_", "record", "variant", "class", "resource", "type", "protocol", "impl",
        "for", "extends", "Option", "Result",
    ] {
        assert!(reserved.contains(&json!(token)));
    }
    assert_eq!(binding["x-implicit-let-nodes"], 1);
    assert_eq!(binding["x-value-and-body-depth-increment"], 1);
    assert_eq!(binding["x-initializer-scope"], "outside_new_binding");
    assert_eq!(binding["x-body-scope"], "immutable_local_binding");
    assert_eq!(binding["x-evaluation-order"], "value_then_body");
    for field in ["type", "mutable", "source", "declared_type"] {
        assert!(binding["properties"].get(field).is_none());
    }
}

#[test]
fn declaration_nominal_types_preserve_copy_forms_and_add_owning_forms() {
    let declaration = function_declaration_schema();
    let parameters = declaration["properties"]["parameters"]["items"]["oneOf"]
        .as_array()
        .unwrap();
    assert_eq!(parameters.len(), 6);
    assert_eq!(
        parameters[0]["properties"]["type"]["enum"],
        json!(COMPUTED_SCALAR_KINDS)
    );
    assert_eq!(
        parameters[1]["properties"]["type"]["enum"],
        json!(["Bytes"])
    );
    assert_eq!(
        parameters[2]["properties"]["type"]["enum"],
        json!(["str", "Slice<u8>"])
    );
    assert_eq!(parameters[3]["properties"]["mode"]["const"], "value");
    assert_eq!(parameters[4]["properties"]["mode"]["const"], "own");
    assert_eq!(
        parameters[4]["properties"]["type"],
        parameters[3]["properties"]["type"]
    );
    assert_eq!(parameters[4]["additionalProperties"], false);
    assert_eq!(parameters[5]["properties"]["type"]["const"], "string");
    assert_eq!(parameters[5]["properties"]["mode"]["const"], "value");
    assert_eq!(parameters[5]["additionalProperties"], false);
    assert_eq!(
        declaration["properties"]["return_type"]["oneOf"][0]["enum"],
        json!(["i64", "i32", "u8", "usize", "bool", "Bytes", "string"])
    );
    let nominal = &parameters[3]["properties"]["type"];
    assert_eq!(nominal["additionalProperties"], false);
    assert_eq!(
        nominal["required"],
        json!(["kind", "target", "type_arguments"])
    );
    assert_eq!(
        nominal["properties"]["type_arguments"]["maxItems"],
        MAX_AGGREGATE_TYPE_ARGUMENTS
    );
    assert_eq!(
        nominal["properties"]["type_arguments"]["items"]["enum"],
        json!(["i64", "bool"])
    );
    assert!(nominal["properties"]["type_arguments"]
        .get("minItems")
        .is_none());
    assert_eq!(
        &declaration["properties"]["return_type"]["oneOf"][1],
        nominal
    );
}

#[test]
fn type_declarations_close_members_and_preserve_function_shape() {
    let schema = declaration_schema();
    let forms = schema["oneOf"].as_array().unwrap();
    assert_eq!(forms.len(), 3);
    assert_eq!(forms[0], function_declaration_schema());
    assert!(forms[0]["properties"].get("kind").is_none());
    assert_eq!(
        forms[1]["required"],
        json!(["kind", "id", "name", "fields"])
    );
    assert_eq!(forms[2]["required"], json!(["kind", "id", "name", "cases"]));
    for form in &forms[1..] {
        assert_eq!(form["additionalProperties"], false);
        assert_eq!(form["x-max-combined-identities"], 4096);
        assert!(form["properties"].get("type_parameters").is_none());
    }
    let cases = &forms[2]["properties"]["cases"];
    assert_eq!(cases["minItems"], 1);
    assert_eq!(cases["maxItems"], 64);
    assert_eq!(cases["items"]["additionalProperties"], false);
    assert_eq!(cases["items"]["required"], json!(["id", "name", "fields"]));
    for fields in [
        &forms[1]["properties"]["fields"],
        &cases["items"]["properties"]["fields"],
    ] {
        assert_eq!(fields["maxItems"], 64);
        assert!(fields.get("minItems").is_none());
        assert_eq!(fields["items"]["additionalProperties"], false);
        assert_eq!(fields["items"]["required"], json!(["id", "name", "type"]));
        let types = fields["items"]["properties"]["type"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(types.len(), 2);
        assert_eq!(
            types[0]["enum"],
            json!(["i64", "bool", "i32", "u8", "usize", "string", "Bytes"])
        );
        assert_eq!(types[1], nominal_type_schema());
        assert_eq!(types[1]["additionalProperties"], false);
        assert_eq!(
            types[1]["required"],
            json!(["kind", "target", "type_arguments"])
        );
        assert_eq!(
            types[1]["properties"]["type_arguments"]["items"]["enum"],
            json!(["i64", "bool"])
        );
    }
}

#[test]
fn aggregate_constructors_are_closed_recursive_identity_selected_shapes() {
    let schema = expression_schema();
    let variants = schema["oneOf"].as_array().unwrap();
    let kinds = variants
        .iter()
        .map(|variant| variant["properties"]["kind"]["const"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        kinds,
        [
            "i64",
            "i32",
            "u8",
            "usize",
            "bool",
            "char",
            "f32",
            "f64",
            "string",
            "array_u8",
            "place",
            "call",
            "binary",
            "unary",
            "if",
            "record",
            "variant",
            "project",
            "field_place",
            "match",
            "update",
            "let",
            "builtin_call"
        ]
        .into_iter()
        .collect()
    );
    let field_place = variants
        .iter()
        .find(|variant| variant["properties"]["kind"]["const"] == "field_place")
        .unwrap();
    assert_eq!(field_place["additionalProperties"], false);
    assert_eq!(field_place["required"], json!(["kind", "target", "root"]));
    assert_eq!(field_place["properties"].as_object().unwrap().len(), 3);
    assert_eq!(field_place["properties"]["root"]["type"], "string");
    assert_eq!(field_place["properties"]["root"], identifier());
    assert_eq!(field_place["x-implicit-field-place-nodes"], 1);
    assert_eq!(field_place["x-root-depth-increment"], 1);
    assert!(field_place["properties"].get("base").is_none());
    assert!(field_place["properties"].get("type_arguments").is_none());
    let builtins = variants
        .iter()
        .filter(|variant| variant["properties"]["kind"]["const"] == "builtin_call")
        .collect::<Vec<_>>();
    assert_eq!(
        builtins.len(),
        crate::byte_ops::ByteOp::ALL.len() + crate::string_ops::StringOp::ALL.len()
    );
    for (target, arity) in crate::byte_ops::ByteOp::ALL
        .into_iter()
        .map(|operation| (operation.id(), operation.arity()))
        .chain(
            crate::string_ops::StringOp::ALL
                .into_iter()
                .map(|operation| (operation.id(), operation.arity())),
        )
    {
        let shape = builtins
            .iter()
            .find(|shape| shape["properties"]["target"]["const"] == target)
            .unwrap();
        assert_eq!(shape["additionalProperties"], false);
        assert_eq!(shape["required"], json!(["kind", "target", "arguments"]));
        assert_eq!(shape["properties"]["arguments"]["minItems"], arity);
        assert_eq!(shape["properties"]["arguments"]["maxItems"], arity);
        assert_eq!(
            shape["properties"]["arguments"]["items"],
            reference("expression")
        );
    }
    for kind in ["record", "variant"] {
        let variant = variants
            .iter()
            .find(|variant| variant["properties"]["kind"]["const"] == kind)
            .unwrap();
        assert_eq!(variant["additionalProperties"], false);
        assert_eq!(variant["required"], json!(["kind", "target", "fields"]));
        assert_eq!(variant["properties"].as_object().unwrap().len(), 4);
        let fields = &variant["properties"]["fields"];
        assert_eq!(fields["maxItems"], MAX_EXPRESSION_NODES - 1);
        assert_eq!(fields["items"]["additionalProperties"], false);
        assert_eq!(fields["items"]["required"], json!(["target", "value"]));
        assert_eq!(
            fields["items"]["properties"]["value"],
            reference("expression")
        );
        let arguments = &variant["properties"]["type_arguments"];
        assert_eq!(arguments["type"], "array");
        assert_eq!(arguments["maxItems"], MAX_AGGREGATE_TYPE_ARGUMENTS);
        assert_eq!(arguments["items"]["enum"], json!(["i64", "bool"]));
        assert!(arguments.get("minItems").is_none());
        assert!(!variant["required"]
            .as_array()
            .unwrap()
            .contains(&json!("type_arguments")));
        assert!(variant["properties"].get("source").is_none());
    }
}

#[test]
fn projection_recurses_through_one_base_and_keeps_owner_selection_compiler_owned() {
    let schema = expression_schema();
    let projection = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["properties"]["kind"]["const"] == "project")
        .unwrap();
    assert_eq!(projection["additionalProperties"], false);
    assert_eq!(projection["required"], json!(["kind", "target", "base"]));
    assert_eq!(projection["properties"].as_object().unwrap().len(), 4);
    assert_eq!(projection["properties"]["base"], reference("expression"));
    assert_eq!(
        projection["properties"]["type_arguments"]["items"]["enum"],
        json!(["i64", "bool"])
    );
    assert_eq!(
        projection["properties"]["type_arguments"]["maxItems"],
        MAX_AGGREGATE_TYPE_ARGUMENTS
    );
    assert_eq!(projection["x-implicit-project-nodes"], 3);
    assert_eq!(projection["x-base-depth-increment"], 2);
    assert!(projection["properties"].get("owner").is_none());
    assert!(projection["properties"].get("name").is_none());
    assert!(projection["properties"].get("source").is_none());
}

#[test]
fn exhaustive_match_schema_closes_case_payload_bindings_and_recursive_bodies() {
    let schema = expression_schema();
    let matching = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["properties"]["kind"]["const"] == "match")
        .unwrap();
    assert_eq!(matching["additionalProperties"], false);
    assert_eq!(
        matching["required"],
        json!(["kind", "target", "value", "arms"])
    );
    assert_eq!(matching["properties"].as_object().unwrap().len(), 5);
    assert_eq!(matching["properties"]["value"], reference("expression"));
    let arm = &matching["properties"]["arms"]["items"];
    assert_eq!(arm["additionalProperties"], false);
    assert_eq!(arm["required"], json!(["target", "fields", "body"]));
    assert_eq!(arm["properties"]["body"], reference("expression"));
    let field = &arm["properties"]["fields"]["items"];
    assert_eq!(field["additionalProperties"], false);
    assert_eq!(field["required"], json!(["target", "name"]));
    assert_eq!(field["properties"]["name"], identifier());
    assert!(arm["properties"].get("guard").is_none());
    assert!(matching["properties"].get("mode").is_none());
    assert_eq!(matching["x-implicit-match-nodes"], 3);
    assert_eq!(matching["x-value-and-body-depth-increment"], 2);
    assert!(!matching["required"]
        .as_array()
        .unwrap()
        .contains(&json!("type_arguments")));
}

#[test]
fn record_update_schema_closes_ordered_field_subset_and_recursive_base() {
    let schema = expression_schema();
    let update = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["properties"]["kind"]["const"] == "update")
        .unwrap();
    assert_eq!(update["additionalProperties"], false);
    assert_eq!(
        update["required"],
        json!(["kind", "target", "base", "fields"])
    );
    assert_eq!(update["properties"].as_object().unwrap().len(), 5);
    assert_eq!(update["properties"]["base"], reference("expression"));
    let fields = &update["properties"]["fields"];
    assert_eq!(fields["maxItems"], MAX_EXPRESSION_NODES - 1);
    assert!(fields.get("minItems").is_none());
    assert_eq!(fields["items"]["additionalProperties"], false);
    assert_eq!(fields["items"]["required"], json!(["target", "value"]));
    assert_eq!(
        fields["items"]["properties"]["value"],
        reference("expression")
    );
    assert_eq!(update["x-implicit-update-nodes"], 3);
    assert_eq!(update["x-base-and-fields-depth-increment"], 2);
    assert!(update["properties"].get("owner").is_none());
    assert!(update["properties"].get("name").is_none());
}

#[test]
fn diagnostic_repair_rejected_body_uses_expression_grammar_without_nested_repairs() {
    let schema = intent_schema();
    let repair = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["properties"]["kind"]["const"] == "repair_diagnostic")
        .unwrap();
    assert_eq!(repair["additionalProperties"], false);
    assert_eq!(
        repair["required"],
        json!(["kind", "target", "rejected_intent", "repair_id"])
    );
    let rejected = &repair["properties"]["rejected_intent"];
    assert_eq!(rejected["additionalProperties"], false);
    assert_eq!(rejected["required"], json!(["kind", "target", "body"]));
    assert_eq!(
        rejected["properties"]["kind"]["const"],
        "replace_function_body"
    );
    assert_eq!(rejected["properties"]["body"], reference("expression"));
    assert_eq!(repair["properties"]["repair_id"], digest_schema());
    assert!(!expression_schema()["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| { item["properties"]["kind"]["const"] == "repair_diagnostic" }));
}
