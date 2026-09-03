use super::*;

#[test]
fn computed_argument_discovery_is_optional_on_mapping_and_absent_on_append() {
    let schema = signature_form();
    let forms = schema["oneOf"].as_array().unwrap();
    assert_eq!(forms.len(), 2);
    for form in forms {
        assert_eq!(
            form["properties"]["new_parameter_types"]["const"],
            json!(["i64", "i32", "u8", "usize", "bool", "char", "f32", "f64"])
        );
    }
    assert_eq!(
        forms[0]["properties"]["selector"]["const"],
        "append_parameters"
    );
    assert!(forms[0]["properties"].get("computed_argument").is_none());
    assert!(forms[0]["properties"]
        .get("computed_parameter_fields")
        .is_none());
    let mapping = &forms[1];
    assert_eq!(mapping["additionalProperties"], false);
    assert_eq!(
        mapping["properties"]["new_parameter_fields"]["const"],
        json!(["name", "type", "argument"])
    );
    assert_eq!(
        mapping["properties"]["borrowed_parameter_fields"]["const"],
        json!(["name", "borrow_from"])
    );
    assert_eq!(
        mapping["properties"]["borrowed_parameter"]["const"],
        json!({
            "source":"authenticated_original_borrowed_view",
            "admitted_views":["borrow str","borrow Slice<u8>"],
            "caller_lowering":"reuse_exact_left_to_right_staged_view",
            "root_provenance":"ordinary_full_project_loan_and_provenance_replay",
            "source_must_be_retained_exactly_once":true,
            "new_root_or_lifetime":false,
        })
    );
    assert_eq!(
        mapping["properties"]["computed_parameter_fields"]["const"],
        json!(["name", "type", "argument_expression"])
    );
    assert!(!mapping["required"]
        .as_array()
        .unwrap()
        .contains(&json!("computed_argument")));
    assert!(!mapping["required"]
        .as_array()
        .unwrap()
        .contains(&json!("computed_parameter_fields")));
    assert!(!mapping["required"]
        .as_array()
        .unwrap()
        .contains(&json!("borrowed_parameter_fields")));
    assert!(!mapping["required"]
        .as_array()
        .unwrap()
        .contains(&json!("borrowed_parameter")));
    assert_eq!(
        mapping["properties"]["computed_argument"]["const"],
        json!({
            "constructor_schema":"semaprax.typed-expression.v1",
            "place_scope":"original_target_parameter_names",
            "evaluation_order":"after_all_original_arguments_in_computed_mapping_order",
            "caller_bindings":"every_affected_caller_existing_bindings",
            "nominal_type_selector":"nominal_types",
            "type_bindings":"provider_and_every_affected_caller_existing_bindings",
            "nominal_admission":"rebuilt_copy_sized_resource_free_no_drop_signature",
            "admission":"full_candidate_revalidation",
        })
    );
}

#[test]
fn extraction_and_data_evolution_catalog_shapes_close_owning_metadata() {
    let schema = operation();
    let operations = schema["oneOf"].as_array().unwrap();
    let extraction = operations
        .iter()
        .find(|entry| entry["properties"]["kind"]["const"] == "extract_function")
        .unwrap();
    assert_eq!(
        extraction["properties"]["capture_lanes"]["const"],
        json!([
            {"kind":"copy", "admission":"immutable_checked_sized_copy_values"},
            {"kind":"single_local_owner", "types":["own Bytes","string"], "admission":"one_exact_whole_unprojected_body_local_consuming_occurrence", "helper_parameter":"owning_hir_boundary", "caller_action":"transfer_at_original_expression_position"}
        ])
    );
    let record = operations
        .iter()
        .find(|entry| entry["properties"]["kind"]["const"] == "add_record_field")
        .unwrap();
    assert_eq!(
        record["properties"]["field_types"]["const"],
        json!(["i64", "bool", "i32", "u8", "usize", "string", "Bytes"])
    );
    assert_eq!(
        record["properties"]["owning_field_lane"]["const"],
        json!({
            "types":["string","Bytes"],
            "requires":"original_copy_sized_drop_free_resource_free_record_with_authenticated_constructor_and_no_target_record_patterns",
            "transition":"copy_to_needs_drop",
            "bytes_default":"fresh_bytes_copy_of_bounded_array"
        })
    );
    for shape in [extraction, record] {
        assert_eq!(shape["additionalProperties"], false);
    }
    let variant = operations
        .iter()
        .find(|entry| entry["properties"]["kind"]["const"] == "add_variant_case")
        .unwrap();
    assert_eq!(
        variant["properties"]["case_fields"]["const"],
        json!(["id", "name", "field"])
    );
    assert_eq!(
        variant["properties"]["field_fields"]["const"],
        json!(["id", "name", "type"])
    );
    assert_eq!(
        variant["properties"]["field_types"]["const"],
        json!(["Bytes"])
    );
    assert_eq!(
        variant["properties"]["unsupported_field_types"]["const"],
        json!(["string"])
    );
    assert_eq!(variant["additionalProperties"], false);
}

#[test]
fn declaration_discovery_adds_exact_type_forms_without_changing_function_placement() {
    let schema = operation();
    let operation = schema["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["properties"]["kind"]["const"] == "add_declaration")
        .unwrap();
    assert_eq!(operation["additionalProperties"], false);
    assert_eq!(
        operation["properties"]["placement"]["const"],
        "append_function_in_anchor_module"
    );
    assert_eq!(
        operation["properties"]["nominal_owning_admission"]["const"],
        "checked_candidate_owning_signature"
    );
    assert!(!operation["required"]
        .as_array()
        .unwrap()
        .contains(&json!("type_declaration_forms")));
    assert_eq!(
        operation["properties"]["type_declaration_forms"]["const"],
        json!([
            {"kind":"record","placement":"append_record_in_anchor_module","max_fields":64,"max_combined_identities":4096,"field_types":["i64","bool","i32","u8","usize","string","Bytes"],"nominal_type_selector":"nominal_types","field_type_admission":"checked_resource_free_field_type","requires_full_candidate_validation":true},
            {"kind":"variant","placement":"append_variant_in_anchor_module","min_cases":1,"max_cases":64,"max_fields_per_case":64,"max_combined_identities":4096,"field_types":["i64","bool","i32","u8","usize","string","Bytes"],"nominal_type_selector":"nominal_types","field_type_admission":"checked_resource_free_field_type","requires_full_candidate_validation":true},
        ])
    );
}

#[test]
fn nominal_type_templates_are_closed_and_do_not_claim_copy_admission() {
    let schema = nominal_type();
    let choices = schema["oneOf"].as_array().unwrap();
    assert_eq!(choices.len(), 3);
    for choice in choices {
        assert_eq!(choice["additionalProperties"], false);
        let properties = &choice["properties"];
        assert_eq!(properties["kind"]["const"], "nominal");
        assert_eq!(
            properties["copy_admission"]["const"],
            "checked_candidate_signature"
        );
        assert_eq!(
            properties["requires_full_candidate_validation"]["const"],
            true
        );
        assert!(properties.get("copy").is_none());
        assert!(properties.get("fields").is_none());
        assert!(properties.get("name").is_none());
    }
    assert_eq!(choices[0]["properties"]["generic"]["const"], false);
    assert!(choices[0]["properties"].get("type_parameters").is_none());
    for source in &choices[..2] {
        assert_eq!(
            source["properties"]["evidence_owner"]["const"],
            "retained_checked_hir"
        );
        assert_eq!(source["properties"]["path"]["type"], "string");
    }
    for generic in &choices[1..] {
        assert_eq!(generic["properties"]["generic"]["const"], true);
        assert_eq!(
            generic["properties"]["type_parameters"]["items"]["properties"]["allowed_types"]
                ["const"],
            json!(["i64", "bool"])
        );
    }
    let prelude = &choices[2]["properties"];
    assert_eq!(
        prelude["evidence_owner"]["const"],
        "compiler_checked_prelude"
    );
    assert_eq!(prelude["declaration_kind"]["const"], "variant");
    assert_eq!(prelude["path"]["type"], "null");
    assert_eq!(prelude["module"]["type"], "null");
    assert_eq!(prelude["compiler_prelude"]["additionalProperties"], false);
    let docs = documents();
    let catalog = &docs["urn:semaprax.project-change-catalog.v1"];
    assert_eq!(catalog["properties"]["nominal_types"]["items"], schema);
    assert!(!catalog["required"]
        .as_array()
        .unwrap()
        .contains(&json!("nominal_types")));
    let operation = catalog["properties"]["operations"]["items"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["properties"]["kind"]["const"] == "add_declaration")
        .unwrap();
    assert_eq!(
        operation["properties"]["nominal_type_selector"]["const"],
        "nominal_types"
    );
}

#[test]
fn named_parameter_facts_are_a_closed_paired_extension() {
    let schema = change_parameter();
    let choices = schema["oneOf"].as_array().unwrap();
    assert_eq!(choices.len(), 3);
    assert_eq!(choices[0]["additionalProperties"], false);
    assert_eq!(choices[0]["properties"].as_object().unwrap().len(), 3);
    assert_eq!(choices[1]["additionalProperties"], false);
    assert_eq!(choices[1]["properties"].as_object().unwrap().len(), 5);
    let required = choices[1]["required"].as_array().unwrap();
    assert!(required.contains(&json!("type_identity")));
    assert!(required.contains(&json!("type_provenance")));
    let provenance = &choices[1]["properties"]["type_provenance"];
    assert_eq!(provenance["additionalProperties"], false);
    assert_eq!(provenance["properties"].as_object().unwrap().len(), 8);
    assert_eq!(provenance["properties"]["ownership"]["const"], "copy");
    assert_eq!(provenance["properties"]["needs_drop"]["const"], false);
    let owning = choices[2]["oneOf"].as_array().unwrap();
    assert_eq!(owning.len(), 2);
    for shape in owning {
        assert_eq!(shape["additionalProperties"], false);
        assert_eq!(shape["properties"].as_object().unwrap().len(), 5);
        let facts = &shape["properties"]["type_provenance"];
        assert_eq!(facts["additionalProperties"], false);
        assert_eq!(facts["properties"].as_object().unwrap().len(), 8);
        assert_eq!(facts["properties"]["ownership"]["const"], "own");
        assert_eq!(facts["properties"]["copy"]["const"], false);
        assert_eq!(facts["properties"]["needs_drop"]["const"], true);
        assert_eq!(facts["properties"]["sized"]["const"], true);
        assert_eq!(facts["properties"]["contains_resource"]["const"], false);
    }
    assert_eq!(owning[0]["properties"]["type"]["const"], "string");
    assert_eq!(owning[0]["properties"]["mode"]["const"], "value");
    assert_eq!(owning[0]["properties"]["type_identity"]["const"], "string");
    assert_eq!(
        owning[0]["properties"]["type_provenance"]["properties"]["declaration"]["type"],
        "null"
    );
    assert_eq!(
        owning[0]["properties"]["type_provenance"]["properties"]["arguments"]["const"],
        json!([])
    );
    assert_eq!(owning[1]["properties"]["mode"]["const"], "own");
    assert_eq!(
        owning[1]["properties"]["type_provenance"]["properties"]["declaration"]["type"],
        "string"
    );
}

#[test]
fn aggregate_template_and_prelude_provenance_are_separate_closed_shapes() {
    let schema = aggregate_constructor();
    let choices = schema["oneOf"].as_array().unwrap();
    assert_eq!(choices.len(), 3);
    for choice in choices {
        assert_eq!(choice["additionalProperties"], false);
    }
    let mono = &choices[0];
    assert_eq!(mono["properties"]["generic"]["const"], false);
    assert!(mono["properties"].get("type_parameters").is_none());
    assert_eq!(mono["properties"].as_object().unwrap().len(), 11);
    for generic in &choices[1..] {
        assert_eq!(generic["properties"]["generic"]["const"], true);
        assert!(generic["required"]
            .as_array()
            .unwrap()
            .contains(&json!("type_parameters")));
        let params = &generic["properties"]["type_parameters"];
        assert_eq!(params["maxItems"], 4095);
        assert_eq!(params["items"]["additionalProperties"], false);
        assert_eq!(
            params["items"]["properties"]["allowed_types"]["const"],
            json!(["i64", "bool"])
        );
    }
    assert_eq!(choices[1]["properties"]["path"]["type"], "string");
    assert!(choices[1]["properties"].get("compiler_prelude").is_none());
    let prelude = &choices[2]["properties"];
    assert_eq!(prelude["kind"]["const"], "variant");
    assert_eq!(prelude["path"]["type"], "null");
    assert_eq!(prelude["module"]["type"], "null");
    assert_eq!(prelude["identity_origin"]["const"], "compiler_owned");
    assert_eq!(
        prelude["evidence_owner"]["const"],
        "compiler_checked_prelude"
    );
    assert_eq!(prelude["compiler_prelude"]["additionalProperties"], false);
}

#[test]
fn record_projection_descriptors_close_owner_and_base_evaluation_facts() {
    let schema = aggregate_projection();
    let choices = schema["oneOf"].as_array().unwrap();
    assert_eq!(choices.len(), 2);
    for choice in choices {
        assert_eq!(choice["additionalProperties"], false);
        assert_eq!(choice["properties"]["kind"]["const"], "project");
        assert_eq!(choice["properties"]["path"]["type"], "string");
        assert_eq!(
            choice["properties"]["base_evaluation"]["const"],
            "once_into_typed_value_binding"
        );
        for field in [
            "target",
            "owner",
            "type_identity",
            "binding",
            "base_evaluation",
        ] {
            assert!(choice["required"]
                .as_array()
                .unwrap()
                .contains(&json!(field)));
        }
        assert!(choice["properties"].get("compiler_prelude").is_none());
    }
    assert_eq!(choices[0]["properties"]["generic"]["const"], false);
    assert!(choices[0]["properties"].get("type_parameters").is_none());
    assert_eq!(choices[1]["properties"]["generic"]["const"], true);
    assert!(choices[1]["required"]
        .as_array()
        .unwrap()
        .contains(&json!("type_parameters")));
}

#[test]
fn match_catalogue_exposes_exact_cases_and_keeps_prelude_provenance_distinct() {
    let schema = aggregate_match();
    let choices = schema["oneOf"].as_array().unwrap();
    assert_eq!(choices.len(), 3);
    for choice in choices {
        assert_eq!(choice["additionalProperties"], false);
        assert_eq!(choice["properties"]["kind"]["const"], "match");
        let case = &choice["properties"]["cases"]["items"];
        assert_eq!(case["additionalProperties"], false);
        assert_eq!(case["properties"].as_object().unwrap().len(), 4);
        assert_eq!(
            case["properties"]["fields"]["items"]["additionalProperties"],
            false
        );
        assert!(case["properties"]["fields"]["items"]["properties"]
            .get("type_identity")
            .is_some());
    }
    assert_eq!(choices[0]["properties"]["generic"]["const"], false);
    assert!(choices[0]["properties"].get("type_parameters").is_none());
    assert_eq!(choices[1]["properties"]["generic"]["const"], true);
    assert_eq!(choices[1]["properties"]["path"]["type"], "string");
    assert_eq!(choices[2]["properties"]["path"]["type"], "null");
    assert_eq!(choices[2]["properties"]["module"]["type"], "null");
    assert_eq!(
        choices[2]["properties"]["identity_origin"]["const"],
        "compiler_owned"
    );
    assert_eq!(
        choices[2]["properties"]["evidence_owner"]["const"],
        "compiler_checked_prelude"
    );
    assert_eq!(
        choices[2]["properties"]["compiler_prelude"]["additionalProperties"],
        false
    );
}

#[test]
fn update_descriptor_has_only_closed_source_record_forms_and_subset_policy() {
    let schema = aggregate_update();
    let choices = schema["oneOf"].as_array().unwrap();
    assert_eq!(choices.len(), 2);
    for choice in choices {
        assert_eq!(choice["additionalProperties"], false);
        assert_eq!(choice["properties"]["kind"]["const"], "update");
        assert_eq!(choice["properties"]["field_coverage"]["const"], "subset");
        assert_eq!(
            choice["properties"]["base_evaluation"]["const"],
            "once_into_typed_value_binding"
        );
        assert_eq!(choice["properties"]["path"]["type"], "string");
        assert!(choice["properties"].get("compiler_prelude").is_none());
        for required in [
            "target",
            "owner",
            "fields",
            "field_coverage",
            "base_evaluation",
        ] {
            assert!(choice["required"]
                .as_array()
                .unwrap()
                .contains(&json!(required)));
        }
    }
    assert_eq!(choices[0]["properties"]["generic"]["const"], false);
    assert!(choices[0]["properties"].get("type_parameters").is_none());
    assert_eq!(choices[1]["properties"]["generic"]["const"], true);
    assert!(choices[1]["required"]
        .as_array()
        .unwrap()
        .contains(&json!("type_parameters")));
}
