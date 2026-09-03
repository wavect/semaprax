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
    json!({"oneOf":[byte_builtin_constructor(), string_builtin_constructor()]})
}

fn byte_builtin_constructor() -> Value {
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

fn string_builtin_constructor() -> Value {
    use crate::string_ops::{resolved_params, StringOp};
    use std::collections::BTreeSet;
    let parameters = StringOp::ALL
        .into_iter()
        .flat_map(resolved_params)
        .collect::<Vec<_>>();
    let types = parameters
        .iter()
        .map(|parameter| parameter.ty.identity_key())
        .collect::<BTreeSet<_>>();
    let names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<BTreeSet<_>>();
    let ownership = parameters
        .iter()
        .map(|parameter| match parameter.ownership {
            crate::hir::OwnershipMode::Value => "value",
            crate::hir::OwnershipMode::Own => "own",
            crate::hir::OwnershipMode::Borrow => "borrow",
            crate::hir::OwnershipMode::Shared => "shared",
        })
        .collect::<BTreeSet<_>>();
    let returns = StringOp::ALL
        .into_iter()
        .map(|operation| operation.return_type().identity_key())
        .collect::<BTreeSet<_>>();
    let parameters = json!({"type":"array","minItems":1,"maxItems":2,"items":object(vec![
        ("index",json!({"type":"integer","minimum":0,"maximum":1})),
        ("name",json!({"enum":names})),
        ("type_id",json!({"enum":types})),
        ("type_family",json!({"const":null})),
        ("ownership",json!({"enum":ownership})),
    ])});
    object(vec![
        ("kind", json!({"const":"builtin_call"})),
        (
            "target",
            json!({"enum":StringOp::ALL.iter().map(|operation|operation.id()).collect::<Vec<_>>()}),
        ),
        (
            "name",
            json!({"enum":StringOp::ALL.iter().map(|operation|operation.name()).collect::<Vec<_>>()}),
        ),
        ("arity", json!({"type":"integer","minimum":1,"maximum":2})),
        ("parameters", parameters),
        ("return_type_id", json!({"enum":returns})),
        ("effects", json!({"const":[]})),
        (
            "evidence_owner",
            json!({"const":"compiler_string_operations"}),
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
            ("right_holes", parent_holes.clone()),
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
    let filled_lineage = json!({"type":"array","maxItems":crate::project::MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE,
    "x-sorted-by":"event_id","items":object(vec![
        ("event_id",digest()),("hole_id",text()),
        ("kind",json!({"enum":["replace_function_body","replace_expression","replace_contract_expression"]})),
        ("target",text()),("expression_id",nullable(text())),("intent_digest",digest()),
        ("history_ordinal",uint()),("origin_draft_digest",digest()),
    ])});
    let branch_ancestry = json!({"type":"array","maxItems":crate::project::MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE,
    "items":object(vec![
        ("operation",json!({"enum":["rebase","merge"]})),
        ("parents",json!({"type":"array","minItems":1,"maxItems":2,"items":digest()})),
        ("onto_revision",nullable(digest())),
    ])});
    let rebase_holes = json!({"type":"array","maxItems":16,"items":object(vec![
        ("hole_id", text()),
        ("kind",json!({"enum":["function_body","expression","contract_expression"]})),
        ("target", text()),("old_expression_id", nullable(text())),
        ("new_expression_id", nullable(text())),("concurrent_contract_change", boolean()),
        ("concurrent_body_change", boolean()),("context_refreshed", json!({"const":true})),
    ])});
    put(
        "semaprax.project-candidate-draft-rebase.v2",
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
            ("holes", rebase_holes),
            ("filled_hole_lineage", filled_lineage.clone()),
            ("branch_ancestry", branch_ancestry.clone()),
            ("materializable", json!({"const":false})),
            ("source_authority", json!({"const":false})),
            (
                "validation",
                json!({"const":"checked_history_replay_and_pending_selector_readmission"}),
            ),
            ("nonclaims", strings()),
        ],
    );
    let merge_holes = json!({"type":"array","maxItems":16,"items":object(vec![
        ("hole_id",text()),("kind",json!({"enum":["function_body","expression","contract_expression"]})),
        ("target",text()),("expression_id",nullable(text())),
        ("parents",json!({"enum":[["left"],["right"],["left","right"]]})),
        ("context_refreshed",json!({"const":true})),
    ])});
    put(
        "semaprax.project-candidate-draft-merge.v2",
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
            ("holes", merge_holes),
            ("filled_hole_lineage", filled_lineage),
            ("branch_ancestry", branch_ancestry),
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
                    ("monomorphic_functions_resolved", uint()),
                    ("monomorphic_function_HIR_reused", json!({"const":0})),
                    ("selective_function_HIR_resolution", json!({"const":false})),
                    ("full_source_verification", json!({"const":true})),
                    ("full_HIR_validation", json!({"const":true})),
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
                    ("checked_monomorphic_functions", uint()),
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
    semantic["properties"]["work"]["properties"]["monomorphic_function_HIR_reused"] = json!({"type":"integer","minimum":0,"maximum":crate::project::incremental::MAX_PROJECT_CHECKED_FUNCTIONS});
    semantic["properties"]["work"]["properties"]["selective_function_HIR_resolution"] =
        json!({"const":true});
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
    builtins["maxItems"] =
        json!(crate::byte_ops::ByteOp::ALL.len() + crate::string_ops::StringOp::ALL.len());
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
                    "nominal_owning_admission",
                    json!({"const":"checked_candidate_owning_signature"}),
                ),
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
                json!({"const":["i64","i32","u8","usize","bool","char","f32","f64"]}),
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
            shape["properties"]["borrowed_parameter_fields"] =
                json!({"const":["name","borrow_from"]});
            shape["properties"]["computed_parameter_fields"] =
                json!({"const":["name","type","argument_expression"]});
            shape["properties"]["borrowed_parameter"] = json!({"const":{
                "source":"authenticated_original_borrowed_view",
                "admitted_views":["borrow str","borrow Slice<u8>"],
                "caller_lowering":"reuse_exact_left_to_right_staged_view",
                "root_provenance":"ordinary_full_project_loan_and_provenance_replay",
                "source_must_be_retained_exactly_once":true,
                "new_root_or_lifetime":false,
            }});
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
#[path = "candidate_schemas/signature_parameter_schema_tests.rs"]
mod signature_parameter_schema_tests;
