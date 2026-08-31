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

fn builtin_constructor() -> Value {
    let mut parameters = array(object(vec![
        ("index", json!({"type":"integer","minimum":0,"maximum":2})),
        ("name", text()),
        ("type_id", nullable(text())),
        (
            "type_family",
            nullable(json!({"const":"array_u8_any_length"})),
        ),
        (
            "ownership",
            json!({"enum":["value","own","borrow","shared"]}),
        ),
    ]));
    parameters["minItems"] = json!(1);
    parameters["maxItems"] = json!(3);
    object(vec![
        ("kind", json!({"const":"builtin_call"})),
        (
            "target",
            json!({"enum":crate::byte_ops::ByteOp::ALL.iter().map(|operation|operation.id()).collect::<Vec<_>>()}),
        ),
        (
            "name",
            json!({"enum":crate::byte_ops::ByteOp::ALL.iter().map(|operation|operation.name()).collect::<Vec<_>>()}),
        ),
        ("arity", json!({"type":"integer","minimum":1,"maximum":3})),
        ("parameters", parameters),
        ("return_type_id", text()),
        ("effects", json!({"const":[]})),
        (
            "evidence_owner",
            json!({"const":"compiler_byte_operations"}),
        ),
        ("requires_full_candidate_validation", json!({"const":true})),
    ])
}

fn change_parameter() -> Value {
    let fields = vec![
        ("name", text()),
        ("type", text()),
        ("mode", json!({"enum":["value","own","borrow","shared"]})),
    ];
    let plain = object(fields.clone());
    let mut named = fields.clone();
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
    let mut owning = Vec::new();
    for string in [true, false] {
        let mut owned = fields.clone();
        owned[1].1 = if string {
            json!({"const":"string"})
        } else {
            text()
        };
        owned[2].1 = json!({"const":if string { "value" } else { "own" }});
        owned.extend([
            (
                "type_identity",
                if string {
                    json!({"const":"string"})
                } else {
                    text()
                },
            ),
            (
                "type_provenance",
                object(vec![
                    (
                        "declaration",
                        if string {
                            json!({"type":"null"})
                        } else {
                            text()
                        },
                    ),
                    (
                        "arguments",
                        if string {
                            json!({"const":[]})
                        } else {
                            strings()
                        },
                    ),
                    ("ownership", json!({"const":"own"})),
                    ("evidence_owner", json!({"const":"retained_checked_hir"})),
                    ("copy", json!({"const":false})),
                    ("sized", json!({"const":true})),
                    ("contains_resource", json!({"const":false})),
                    ("needs_drop", json!({"const":true})),
                ]),
            ),
        ]);
        owning.push(object(owned));
    }
    json!({"oneOf":[plain,object(named),{"oneOf":owning}]})
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
    generic_fields
        .retain(|(name, _)| !matches!(*name, "kind" | "path" | "module" | "evidence_owner"));
    generic_fields.extend([
        ("kind", json!({"const":"variant"})),
        ("path", json!({"type":"null"})),
        ("module", json!({"type":"null"})),
        ("identity_origin", json!({"const":"compiler_owned"})),
        (
            "evidence_owner",
            json!({"const":"compiler_checked_prelude"}),
        ),
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

fn field_place() -> Value {
    let mut schema = aggregate_projection();
    for branch in schema["oneOf"].as_array_mut().unwrap() {
        branch["properties"]["kind"] = json!({"const":"field_place"});
        branch["properties"]["base_evaluation"] = json!({"const":"direct_named_place_no_staging"});
        branch["properties"]["root_requirement"] =
            json!({"const":"authenticated_lexical_nominal_binding"});
        branch["required"]
            .as_array_mut()
            .unwrap()
            .push(json!("root_requirement"));
    }
    schema
}

fn aggregate_update() -> Value {
    // Update discovery carries the same complete source-record field inventory
    // as construction. Prelude and variant alternatives are not update owners.
    let constructors = aggregate_constructor();
    let mut alternatives = constructors["oneOf"].as_array().unwrap()[..2].to_vec();
    for alternative in &mut alternatives {
        alternative["properties"]["kind"] = json!({"const":"update"});
        alternative["properties"]["base_evaluation"] =
            json!({"const":"once_into_typed_value_binding"});
        alternative["properties"]["field_coverage"] = json!({"const":"subset"});
        alternative["required"]
            .as_array_mut()
            .unwrap()
            .extend([json!("base_evaluation"), json!("field_coverage")]);
    }
    json!({"oneOf":alternatives})
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
    generic.retain(|(name, _)| !matches!(*name, "path" | "module" | "evidence_owner"));
    generic.extend([
        ("path", json!({"type":"null"})),
        ("module", json!({"type":"null"})),
        ("identity_origin", json!({"const":"compiler_owned"})),
        (
            "evidence_owner",
            json!({"const":"compiler_checked_prelude"}),
        ),
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

fn nominal_type() -> Value {
    let fields = vec![
        ("kind", json!({"const":"nominal"})),
        ("target", text()),
        ("binding", text()),
        ("generic", json!({"const":false})),
        ("declaration_kind", json!({"enum":["record","variant"]})),
        ("path", text()),
        ("module", text()),
        ("evidence_owner", json!({"const":"retained_checked_hir"})),
        ("requires_full_candidate_validation", json!({"const":true})),
        (
            "copy_admission",
            json!({"const":"checked_candidate_signature"}),
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
    let source_generic = object(generic.clone());
    generic.retain(|(name, _)| {
        !matches!(
            *name,
            "declaration_kind" | "path" | "module" | "evidence_owner"
        )
    });
    generic.extend([
        ("declaration_kind", json!({"const":"variant"})),
        ("path", json!({"type":"null"})),
        ("module", json!({"type":"null"})),
        (
            "evidence_owner",
            json!({"const":"compiler_checked_prelude"}),
        ),
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
        "semaprax.project-candidate-draft-rebase.v1",
        vec![
            ("parent_draft_digest", digest()),
            ("original_base_revision", digest()),
            ("onto_revision", digest()),
            ("result_base_revision", digest()),
            ("result_draft_digest", digest()),
            (
                "last_valid_rebase",
                reference("semaprax.project-candidate-rebase.v1"),
            ),
            (
                "holes",
                json!({"type":"array","maxItems":16,"items":object(vec![
                    ("hole_id", text()),
                    (
                        "kind",
                        json!({"enum":["function_body","expression","contract_expression"]}),
                    ),
                    ("target", text()),
                    ("old_expression_id", nullable(text())),
                    ("new_expression_id", nullable(text())),
                    ("concurrent_contract_change", boolean()),
                    ("concurrent_body_change", boolean()),
                    ("context_refreshed", json!({"const":true})),
                ])}),
            ),
            ("materializable", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            (
                "validation",
                json!({"const":"checked_history_replay_and_pending_selector_readmission"}),
            ),
            ("nonclaims", strings()),
        ],
    );
    let parent_holes = json!({"type":"array","maxItems":16,"items":object(vec![
        ("hole_id", text()),
        ("kind", json!({"enum":["function_body","expression","contract_expression"]})),
        ("target", text()),
        ("old_expression_id", nullable(text())),
        ("new_expression_id", nullable(text())),
        ("concurrent_contract_change", boolean()),
        ("concurrent_body_change", boolean()),
        ("context_refreshed", json!({"const":true})),
    ])});
    put(
        "semaprax.project-candidate-draft-merge.v1",
        vec![
            ("left_parent_draft_digest", digest()),
            ("right_parent_draft_digest", digest()),
            ("original_base_revision", digest()),
            ("result_base_revision", digest()),
            ("result_draft_digest", digest()),
            (
                "last_valid_merge",
                reference("semaprax.project-candidate-rebase.v1"),
            ),
            ("left_holes", parent_holes.clone()),
            ("right_holes", parent_holes),
            (
                "holes",
                json!({"type":"array","maxItems":16,"items":object(vec![
                    ("hole_id", text()),
                    ("kind", json!({"enum":["function_body","expression","contract_expression"]})),
                    ("target", text()),
                    ("expression_id", nullable(text())),
                    ("parents", json!({"enum":[["left"],["right"],["left","right"]]})),
                    ("context_refreshed", json!({"const":true})),
                ])}),
            ),
            ("materializable", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            (
                "validation",
                json!({"const":"checked_history_merge_and_pending_selector_readmission"}),
            ),
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
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["aggregate_updates"] = array(aggregate_update());
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["nominal_types"] = array(nominal_type());
    let mut builtins = array(builtin_constructor());
    builtins["maxItems"] = json!(crate::byte_ops::ByteOp::ALL.len());
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["builtin_calls"] = builtins;
    let mut places = array(field_place());
    places["maxItems"] = json!(65536);
    docs.get_mut("urn:semaprax.project-change-catalog.v1")
        .unwrap()["properties"]["field_places"] = places;
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
        "replace_contract_expression",
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
            "replace_contract_expression" => fields.extend([
                (
                    "selector_source",
                    json!({"const":"candidate/contract-expression-catalog"}),
                ),
                ("phases", json!({"const":["requires","ensures"]})),
            ]),
            "add_contract" => fields.push(("phases", json!({"const":["requires","ensures"]}))),
            "add_declaration" => fields.extend([
                ("anchor", text()),
                ("nominal_type_selector", json!({"const":"nominal_types"})),
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
                (
                    "field_types",
                    json!({"const":["i64","bool","i32","u8","usize"]}),
                ),
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
        let mut shape = object(fields);
        if kind == "rename_declaration" {
            // Optional for legacy function and nominal-owner descriptors;
            // present only for the source-authenticated member route.
            shape["properties"]["member_kind"] =
                json!({"enum":["record_field","variant_case","variant_field"]});
        }
        if kind == "add_declaration" {
            shape["properties"]["type_declaration_forms"] = json!({"const":[
                {"kind":"record","placement":"append_record_in_anchor_module","max_fields":64,"max_combined_identities":4096,"field_types":["i64","bool","i32","u8","usize","string","Bytes"],"nominal_type_selector":"nominal_types","field_type_admission":"checked_resource_free_field_type","requires_full_candidate_validation":true},
                {"kind":"variant","placement":"append_variant_in_anchor_module","min_cases":1,"max_cases":64,"max_fields_per_case":64,"max_combined_identities":4096,"field_types":["i64","bool","i32","u8","usize","string","Bytes"],"nominal_type_selector":"nominal_types","field_type_admission":"checked_resource_free_field_type","requires_full_candidate_validation":true},
            ]});
        }
        if kind == "repair_diagnostic" {
            let mut borrowed_field = shape.clone();
            borrowed_field["properties"]["repair_class"] =
                json!({"const":"borrow_owned_byte_field_without_staging"});
            borrowed_field["properties"]["selector_source"] =
                json!({"const":"attempt/repair-catalog"});
            choices.push(borrowed_field);
        }
        choices.push(shape);
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
        let mut shape = object(fields);
        if !append {
            shape["properties"]["computed_parameter_fields"] =
                json!({"const":["name","type","argument_expression"]});
            shape["properties"]["computed_argument"] = json!({"const":{
                "constructor_schema":"semaprax.typed-expression.v1",
                "place_scope":"original_target_parameter_names",
                "evaluation_order":"after_all_original_arguments_in_computed_mapping_order",
                "caller_bindings":"every_affected_caller_existing_bindings",
                "nominal_type_selector":"nominal_types",
                "type_bindings":"provider_and_every_affected_caller_existing_bindings",
                "nominal_admission":"rebuilt_copy_sized_resource_free_no_drop_signature",
                "admission":"full_candidate_revalidation",
            }});
        }
        choices.push(shape);
    }
    json!({"oneOf":choices})
}

#[cfg(test)]
mod signature_parameter_schema_tests {
    use super::*;

    #[test]
    fn computed_argument_discovery_is_optional_on_mapping_and_absent_on_append() {
        let schema = signature_form();
        let forms = schema["oneOf"].as_array().unwrap();
        assert_eq!(forms.len(), 2);
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
}
