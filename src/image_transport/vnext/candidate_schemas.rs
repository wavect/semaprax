//! Closed response shapes read from candidate catalogue, rebase, delta and
//! frontend-work emitters. Heterogeneous HIR report interiors stay unbundled.
use super::{array, digest, document, nullable, object, text, uint};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn strings() -> Value {
    array(text())
}
fn boolean() -> Value {
    json!({"type":"boolean"})
}
fn reference(id: &str) -> Value {
    json!({"$ref":format!("urn:{id}")})
}

fn change_parameter() -> Value {
    let fields = vec![
        ("name", text()),
        ("type", text()),
        ("mode", json!({"enum":["value","own","borrow","shared"]})),
    ];
    let plain = object(fields.clone());
    let mut named = fields;
    named.extend([
        ("type_identity", text()),
        (
            "type_provenance",
            object(vec![
                ("declaration", text()),
                ("arguments", strings()),
                ("ownership", json!({"const":"copy"})),
                ("evidence_owner", json!({"const":"retained_checked_hir"})),
                ("copy", json!({"const":true})),
                ("sized", json!({"const":true})),
                ("contains_resource", json!({"const":false})),
                ("needs_drop", json!({"const":false})),
            ]),
        ),
    ]);
    json!({"oneOf":[plain,object(named)]})
}

fn aggregate_constructor() -> Value {
    let mut fields = array(object(vec![
        ("target", text()),
        ("name", text()),
        ("index", uint()),
        ("type_identity", text()),
    ]));
    fields["maxItems"] = json!(4095);
    let fields = vec![
        ("kind", json!({"enum":["record","variant"]})),
        ("target", text()),
        ("owner", text()),
        ("name", text()),
        ("binding", text()),
        ("path", text()),
        ("module", text()),
        ("generic", json!({"const":false})),
        ("fields", fields),
        ("evidence_owner", json!({"const":"retained_checked_hir"})),
        ("requires_full_candidate_validation", json!({"const":true})),
    ];
    let monomorphic = object(fields.clone());
    let mut generic_fields = fields;
    generic_fields.retain(|(name, _)| *name != "generic");
    generic_fields.extend([
        ("generic", json!({"const":true})),
        (
            "type_parameters",
            json!({"type":"array","minItems":1,"maxItems":4095,
            "items":object(vec![
                ("name",text()),("index",uint()),
                ("allowed_types",json!({"const":["i64","bool"]})),
            ])}),
        ),
    ]);
    let generic = object(generic_fields.clone());
    generic_fields.retain(|(name, _)| !matches!(*name, "kind" | "path" | "module"));
    generic_fields.extend([
        ("kind", json!({"const":"variant"})),
        ("path", json!({"type":"null"})),
        ("module", json!({"type":"null"})),
        ("identity_origin", json!({"const":"compiler_owned"})),
        (
            "compiler_prelude",
            object(vec![
                ("schema", json!({"const":"semaprax.prelude.v1"})),
                ("digest", digest()),
            ]),
        ),
    ]);
    json!({"oneOf":[monomorphic,generic,object(generic_fields)]})
}

fn aggregate_projection() -> Value {
    let fields = vec![
        ("kind", json!({"const":"project"})),
        ("target", text()),
        ("owner", text()),
        ("name", text()),
        ("index", uint()),
        ("type_identity", text()),
        ("binding", text()),
        ("path", text()),
        ("module", text()),
        ("generic", json!({"const":false})),
        ("evidence_owner", json!({"const":"retained_checked_hir"})),
        ("requires_full_candidate_validation", json!({"const":true})),
        (
            "base_evaluation",
            json!({"const":"once_into_typed_value_binding"}),
        ),
    ];
    let monomorphic = object(fields.clone());
    let mut generic = fields;
    generic.retain(|(name, _)| *name != "generic");
    generic.extend([
        ("generic", json!({"const":true})),
        (
            "type_parameters",
            json!({"type":"array","minItems":1,"maxItems":4095,
            "items":object(vec![
                ("name",text()),("index",uint()),
                ("allowed_types",json!({"const":["i64","bool"]})),
            ])}),
        ),
    ]);
    json!({"oneOf":[monomorphic,object(generic)]})
}

fn aggregate_match() -> Value {
    let mut payloads = array(object(vec![
        ("target", text()),
        ("name", text()),
        ("index", uint()),
        ("type_identity", text()),
    ]));
    payloads["maxItems"] = json!(4095);
    let mut cases = array(object(vec![
        ("target", text()),
        ("name", text()),
        ("index", uint()),
        ("fields", payloads),
    ]));
    cases["maxItems"] = json!(4095);
    let fields = vec![
        ("kind", json!({"const":"match"})),
        ("target", text()),
        ("name", text()),
        ("path", text()),
        ("module", text()),
        ("generic", json!({"const":false})),
        ("binding", text()),
        ("evidence_owner", json!({"const":"retained_checked_hir"})),
        ("requires_full_candidate_validation", json!({"const":true})),
        (
            "base_evaluation",
            json!({"const":"once_into_typed_value_binding"}),
        ),
        ("cases", cases),
    ];
    let monomorphic = object(fields.clone());
    let mut generic = fields;
    generic.retain(|(name, _)| *name != "generic");
    generic.extend([
        ("generic", json!({"const":true})),
        (
            "type_parameters",
            json!({"type":"array","minItems":1,"maxItems":4095,
            "items":object(vec![
                ("name",text()),("index",uint()),
                ("allowed_types",json!({"const":["i64","bool"]})),
            ])}),
        ),
    ]);
    let source_generic = object(generic.clone());
    generic.retain(|(name, _)| !matches!(*name, "path" | "module"));
    generic.extend([
        ("path", json!({"type":"null"})),
        ("module", json!({"type":"null"})),
        ("identity_origin", json!({"const":"compiler_owned"})),
        (
            "compiler_prelude",
            object(vec![
                ("schema", json!({"const":"semaprax.prelude.v1"})),
                ("digest", digest()),
            ]),
        ),
    ]);
    json!({"oneOf":[monomorphic,source_generic,object(generic)]})
}

pub(super) fn documents() -> BTreeMap<String, Value> {
    let mut docs = BTreeMap::new();
    let mut put = |id: &str, fields| {
        docs.insert(format!("urn:{id}"), document(id, fields));
    };
    for (version, tests) in [("v1", false), ("v2", true)] {
        let mut available = vec![
            json!({"method":"candidate/validate","kind":"independent_source_and_target_projection_replay","runtime_execution":false}),
        ];
        if tests {
            available.push(json!({"method":"candidate/test","kind":"bounded_project_interpreter_test_closure","runtime_execution":true}));
        }
        put(
            &format!("semaprax.image-validation-catalog.{version}"),
            vec![
                ("available", json!({"const":available})),
                (
                    "required_external_gates",
                    json!({"const":if tests {vec!["native_and_wasm_runtime_conformance","full_quality_profile"]} else {vec!["affected_project_tests","native_and_wasm_runtime_conformance","full_quality_profile"]}}),
                ),
                (
                    "tests",
                    json!({"const":if tests {"available_only_on_explicit_request"} else {"not_run"}}),
                ),
                ("source_authority", json!({"const":false})),
            ],
        );
    }
    put(
        "semaprax.project-candidate-comparison.v1",
        vec![
            ("base_revision", digest()),
            ("left", digest()),
            ("right", digest()),
            ("same_source_revision", boolean()),
            ("overlapping_targets", strings()),
            (
                "classification",
                json!({"const":"descriptive_requires_revalidation_before_merge"}),
            ),
            ("commit_authority", json!({"const":false})),
        ],
    );
    let classification = object(vec![
        ("target", text()),
        ("intent", text()),
        ("concurrent_display_change", boolean()),
        ("concurrent_signature_change", boolean()),
        ("concurrent_body_change", boolean()),
        ("concurrent_contract_change", boolean()),
        ("concurrent_effect_change", boolean()),
        ("decision", json!({"const":"replay_required"})),
    ]);
    put(
        "semaprax.project-candidate-rebase.v1",
        vec![
            ("operation", json!({"enum":["rebase","merge"]})),
            ("left_parent_candidate", digest()),
            ("right_parent_candidate", nullable(digest())),
            ("original_base_revision", digest()),
            ("onto_revision", digest()),
            ("result_base_revision", digest()),
            ("result_revision", digest()),
            ("result_candidate_digest", digest()),
            ("shared_history_prefix", uint()),
            ("classifications", array(classification)),
            (
                "validation",
                json!({"const":"complete_candidate_source_replay"}),
            ),
            ("source_authority", json!({"const":false})),
            ("nonclaims", strings()),
        ],
    );
    put(
        "semaprax.image-candidate-reconciliation.v1",
        vec![
            ("kind", json!({"enum":["merge","rebase"]})),
            ("selected_candidate_revision", digest()),
            ("candidate", reference("semaprax.image-candidate-handle.v1")),
            ("report", reference("semaprax.project-candidate-rebase.v1")),
        ],
    );
    put(
        "semaprax.project-change-catalog.v1",
        vec![
            ("candidate_digest", digest()),
            ("project_revision", digest()),
            ("target", text()),
            ("parameters", array(change_parameter())),
            ("operations", array(operation())),
            ("reason", text()),
            ("admission", json!({"const":"constructor_discovery_only"})),
            ("requires_full_candidate_validation", json!({"const":true})),
            ("source_authority", json!({"const":false})),
        ],
    );
    let authored = object(vec![
        ("id", text()),
        ("name", text()),
        ("kind", text()),
        ("path", text()),
        ("module", text()),
        ("fragment_digest", digest()),
    ]);
    let mut roots = array(object(vec![
        ("target", text()),
        (
            "change",
            json!({"enum":["added","removed","modified","absent","moved"]}),
        ),
        ("base", nullable(authored.clone())),
        ("candidate", nullable(authored)),
    ]));
    roots["maxItems"] = json!(65_536);
    put(
        "semaprax.project-candidate-semantic-delta-catalog.v1",
        vec![
            ("candidate_digest", digest()),
            ("base_project_revision", digest()),
            ("project_revision", digest()),
            ("roots", roots),
            (
                "selection_basis",
                json!({"const":"authored_declaration_identity_origin_and_canonical_fragment_changes"}),
            ),
            (
                "source_changes",
                array(object(vec![
                    ("path", text()),
                    ("base_source_digest", digest()),
                    ("source_digest", digest()),
                ])),
            ),
            ("nonclaims", strings()),
        ],
    );
    let origin = object(vec![
        ("module", text()),
        ("stable_id", text()),
        ("path", text()),
        ("source_revision", digest()),
        ("source_digest", digest()),
    ]);
    put(
        "semaprax.project-candidate-test-plan.v1",
        vec![
            ("candidate_digest", digest()),
            ("base_project_revision", digest()),
            ("project_revision", digest()),
            ("declared_test_count", json!({"const":1})),
            (
                "selection_basis",
                json!({"const":"static_transitive_HIR_calls_not_runtime_coverage"}),
            ),
            ("changed_targets", strings()),
            ("base_reachable_changed_targets", strings()),
            ("candidate_reachable_changed_targets", strings()),
            ("conservative_reasons", strings()),
            ("base_reachable_callable_count", uint()),
            ("candidate_reachable_callable_count", uint()),
            ("selected", boolean()),
            ("test_origin", origin.clone()),
            ("selected_tests", array(origin)),
            ("execution", json!({"const":"not_run"})),
            (
                "explicit_execution_scope",
                json!({"const":"complete_manifest_declared_test_closure"}),
            ),
            ("nonclaims", strings()),
        ],
    );
    put(
        "semaprax.project-frontend-cache-work.v1",
        vec![
            (
                "compiler",
                object(vec![
                    ("package", text()),
                    ("version", text()),
                    ("compatibility", text()),
                    ("binary_identity_claimed", json!({"const":false})),
                ]),
            ),
            ("context_digest", digest()),
            ("project_revision", digest()),
            ("manifest_context_reset", boolean()),
            ("invalidated_sources", strings()),
            (
                "work",
                object(vec![
                    ("modules_parsed", uint()),
                    ("modules_reused", uint()),
                    ("canonicalizer_calls", uint()),
                    ("parsed_source_bytes", uint()),
                    ("reused_source_bytes", uint()),
                    ("cached_AST_clones", uint()),
                    ("modules_resolved", uint()),
                    ("checked_HIR_reused", json!({"const":0})),
                    ("full_cross_file_checks", json!({"const":true})),
                    ("full_link_and_profile_admission", json!({"const":true})),
                ]),
            ),
            (
                "retained",
                object(vec![
                    ("modules", uint()),
                    ("source_bytes", uint()),
                    ("AST_construction_prebound", uint()),
                ]),
            ),
            (
                "limits",
                object(vec![
                    ("modules", uint()),
                    ("source_bytes", uint()),
                    ("AST_construction_prebound", uint()),
                ]),
            ),
            ("nonclaims", strings()),
        ],
    );
    // Keep the AST-only report's zero-HIR contract intact. The independently
    // selected semantic cache reports actual hits under its own schema.
    let mut semantic = docs["urn:semaprax.project-frontend-cache-work.v1"].clone();
    semantic["$id"] = json!("urn:semaprax.project-semantic-cache-work.v1");
    semantic["properties"]["schema"]["const"] = json!("semaprax.project-semantic-cache-work.v1");
    semantic["properties"]["work"]["properties"]["checked_HIR_reused"] =
        json!({"type":"integer","minimum":0,"maximum":16});
    docs.insert(
        "urn:semaprax.project-semantic-cache-work.v1".into(),
        semantic,
    );
    // Empty aggregate inventories are omitted to preserve existing scalar
    // catalogue bytes; when present the complete descriptor remains closed.
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["aggregate_constructors"] = array(aggregate_constructor());
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["aggregate_projections"] = array(aggregate_projection());
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["aggregate_matches"] = array(aggregate_match());
    docs
}

fn operation() -> Value {
    let mut choices = Vec::new();
    for kind in [
        "rename_declaration",
        "change_function_signature",
        "replace_function_body",
        "repair_diagnostic",
        "replace_expression",
        "add_contract",
        "add_declaration",
        "extract_function",
        "move_declaration",
        "add_record_field",
        "implement_interface",
    ] {
        let mut fields = vec![
            ("kind", json!({"const":kind})),
            ("required_fields", strings()),
            ("constraints", strings()),
        ];
        match kind {
            "change_function_signature" => {
                fields.push(("exactly_one_form", array(signature_form())))
            }
            "replace_function_body" => fields.extend([
                ("constructors", strings()),
                ("expression_nodes_maximum", json!({"const":4096})),
                ("expression_depth_maximum", json!({"const":64})),
            ]),
            "repair_diagnostic" => fields.extend([
                (
                    "repair_class",
                    json!({"const":"retag_integer_literal_to_retained_return_type"}),
                ),
                (
                    "selector_source",
                    json!({"const":"candidate-attempt/repair-catalog"}),
                ),
                ("rejected_kind", json!({"const":"replace_function_body"})),
            ]),
            "replace_expression" | "extract_function" => {
                fields.push(("selector_source", json!({"const":"expression/catalog"})))
            }
            "add_contract" => fields.push(("phases", json!({"const":["requires","ensures"]}))),
            "add_declaration" => fields.extend([
                ("anchor", text()),
                (
                    "placement",
                    json!({"const":"append_function_in_anchor_module"}),
                ),
            ]),
            "move_declaration" => fields.push(("destination_anchors", strings())),
            "add_record_field" => fields.extend([
                (
                    "field_fields",
                    json!({"const":["id","name","type","default"]}),
                ),
                ("field_types", json!({"const":["i64","bool"]})),
            ]),
            "implement_interface" => fields.extend([
                (
                    "member_fields",
                    json!({"const":["method","implementation"]}),
                ),
                (
                    "discovery",
                    json!({"const":"ProjectCandidate::interface_catalog"}),
                ),
            ]),
            _ => {}
        }
        choices.push(object(fields));
    }
    json!({"oneOf":choices})
}

fn signature_form() -> Value {
    let mut choices = Vec::new();
    for append in [true, false] {
        let mut fields = vec![
            (
                "selector",
                json!({"const":if append {"append_parameters"}else{"parameters"}}),
            ),
            ("minimum", json!({"const":if append {1}else{0}})),
            ("maximum", json!({"const":if append {16}else{4096}})),
            (
                "new_parameter_types",
                json!({"const":["i64","i32","u8","usize","bool"]}),
            ),
            ("argument", json!({"const":"matching_typed_scalar_literal"})),
            ("evaluation_order", text()),
        ];
        if append {
            fields.push(("item_fields", json!({"const":["name","type","argument"]})));
        } else {
            fields.extend([
                ("existing_parameter_fields", json!({"const":["from"]})),
                (
                    "existing_parameter_rename_fields",
                    json!({"const":["from","name"]}),
                ),
                (
                    "new_parameter_fields",
                    json!({"const":["name","type","argument"]}),
                ),
                ("constraints", strings()),
            ]);
        }
        choices.push(object(fields));
    }
    json!({"oneOf":choices})
}

#[cfg(test)]
mod signature_parameter_schema_tests {
    use super::*;

    #[test]
    fn named_parameter_facts_are_a_closed_paired_extension() {
        let schema = change_parameter();
        let choices = schema["oneOf"].as_array().unwrap();
        assert_eq!(choices.len(), 2);
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
            choices[2]["properties"]["compiler_prelude"]["additionalProperties"],
            false
        );
    }
}
