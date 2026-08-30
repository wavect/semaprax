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
            (
                "parameters",
                array(object(vec![
                    ("name", text()),
                    ("type", text()),
                    ("mode", json!({"enum":["value","own","borrow","shared"]})),
                ])),
            ),
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
