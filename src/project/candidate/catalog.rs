//! Target-specific constructor discovery, never a validation receipt.

use serde_json::{json, Value};

use crate::ast::{ParamMode, Type};
use crate::diagnostic::Diagnostic;

use super::{capacity, invalid, parse_revision, wire, ProjectCandidate, MAX_CHANGES};

impl ProjectCandidate {
    /// Describes available intention constructors for this exact candidate.
    /// Payload-dependent legality is decided only by `apply`; this read-only
    /// catalogue does not claim that arbitrary payloads satisfy contracts or
    /// target profiles and supplies no source or publication authority.
    pub fn change_catalog(&self, target: &str) -> Result<String, Vec<Diagnostic>> {
        if target.len() > 4096 {
            return Err(capacity("change catalogue target exceeds its byte bound"));
        }
        if target.is_empty() || target.contains('\0') {
            return Err(invalid("change catalogue requires a stable ID"));
        }
        let programs = parse_revision(&self.revision)?;
        let selected = programs
            .iter()
            .flat_map(|program| &program.functions)
            .find(|function| function.stable_id == target);
        let (aggregates, projections, matches, updates, nominal_types, builtins, field_places) =
            match selected {
                Some(function) if function.explicit_id && function.type_parameters.is_empty() => {
                    let program = programs
                        .iter()
                        .find(|program| {
                            program
                                .functions
                                .iter()
                                .any(|item| item.stable_id == target)
                        })
                        .ok_or_else(|| invalid("aggregate discovery source is unavailable"))?;
                    (
                        super::intent::aggregate_constructors(&self.revision, program)?,
                        super::intent::aggregate_projections(&self.revision, program)?,
                        super::intent::aggregate_matches(&self.revision, program)?,
                        super::intent::aggregate_updates(&self.revision, program)?,
                        super::intent::nominal_types(&self.revision, program)?,
                        super::intent::builtin_constructors(&self.revision, program)?,
                        super::intent::field_places(&self.revision, program)?,
                    )
                }
                _ => (
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
            };
        let mut operations = Vec::<Value>::new();
        let mut parameters = Vec::<Value>::new();
        let mut reason = "target_is_not_a_supported_top_level_function";
        if let Some(function) = selected {
            reason = "target_requires_explicit_identity_monomorphic_non_main_function";
            if function.explicit_id
                && function.type_parameters.is_empty()
                && function.name != "main"
            {
                reason = "candidate_intention_limit_reached";
                if self.changes.len() < MAX_CHANGES {
                    reason = "constructor_available_payload_requires_full_candidate_admission";
                    let ordered_parameters =
                        super::intent::ordered_signature_parameters(&self.revision, function)?;
                    parameters = function
                        .params
                        .iter()
                        .enumerate()
                        .map(|(index, param)| {
                            let mut descriptor = json!({
                                "name":param.name,
                                "type":param.ty.to_string(),
                                "mode":match param.mode {
                                    ParamMode::Value=>"value", ParamMode::Own=>"own",
                                    ParamMode::Borrow=>"borrow", ParamMode::Shared=>"shared",
                                },
                            });
                            if let Some(Some(facts)) = ordered_parameters
                                .as_ref()
                                .and_then(|parameters| parameters.get(index))
                            {
                                // The admission seam owns Copy classification
                                // and exact source-to-HIR signature matching.
                                if let Some(facts) = facts.as_object() {
                                    descriptor.as_object_mut().unwrap().extend(facts.clone());
                                }
                            }
                            descriptor
                        })
                        .collect();
                    operations.push(json!({
                        "kind":"rename_declaration", "required_fields":["kind","target","name"],
                        "constraints":["new_identifier_max_128_bytes", "different_display_name", "no_call_binding_collision", "preserve_stable_identity"],
                    }));
                    let mut forms = vec![json!({
                        "selector":"append_parameters", "minimum":1, "maximum":16,
                        "item_fields":["name","type","argument"],
                        "new_parameter_types":["i64","i32","u8","usize","bool"],
                        "argument":"matching_typed_scalar_literal",
                        "evaluation_order":"original_arguments_unchanged_then_pure_literals",
                    })];
                    if ordered_parameters.is_some() {
                        forms.push(json!({
                            "selector":"parameters", "minimum":0, "maximum":4096,
                            "existing_parameter_fields":["from"],
                            "existing_parameter_rename_fields":["from","name"],
                            "new_parameter_fields":["name","type","argument"],
                            "computed_parameter_fields":["name","type","argument_expression"],
                            "computed_argument":{
                                "constructor_schema":"semaprax.typed-expression.v1",
                                "place_scope":"original_target_parameter_names",
                                "evaluation_order":"after_all_original_arguments_in_computed_mapping_order",
                                "caller_bindings":"every_affected_caller_existing_bindings",
                                "nominal_type_selector":"nominal_types",
                                "type_bindings":"provider_and_every_affected_caller_existing_bindings",
                                "nominal_admission":"rebuilt_copy_sized_resource_free_no_drop_signature",
                                "admission":"full_candidate_revalidation",
                            },
                            "new_parameter_types":["i64","i32","u8","usize","bool"],
                            "argument":"matching_typed_scalar_literal",
                            "constraints":["existing_parameter_selected_at_most_once", "existing_type_mode_preserved", "scope_preserving_display_rename", "own_bytes_retained_exactly_once", "new_name_distinct_from_all_old_names", "removed_parameters_must_not_remain_referenced", "ordinary_ownership_cleanup_admission"],
                            "evaluation_order":"stage_every_original_argument_once_left_to_right_including_removed_arguments",
                        }));
                    }
                    operations.push(json!({
                        "kind":"change_function_signature", "required_fields":["kind","target"],
                        "exactly_one_form":forms,
                        "constraints":["all_authenticated_callers_migrated", "preserve_return_type", "full_project_profile_and_target_admission"],
                    }));
                    operations.push(json!({
                        "kind":"replace_function_body", "required_fields":["kind","target","body"],
                        "constructors":["i64","i32","u8","usize","bool","place","binary","unary","if","call","let"],
                        "expression_nodes_maximum":4096, "expression_depth_maximum":64,
                        "constraints":["place_selects_existing_parameter_or_active_lexical_binding", "call_selects_accessible_stable_id", "expected_return_type", "declared_effect_budget", "contracts_ownership_cleanup_and_target_revalidation"],
                    }));
                    if matches!(
                        function.return_type,
                        Type::I64 | Type::I32 | Type::U8 | Type::Usize
                    ) {
                        operations.push(json!({
                            "kind":"repair_diagnostic", "required_fields":["kind","target","rejected_intent","repair_id"],
                            "repair_class":"retag_integer_literal_to_retained_return_type",
                            "selector_source":"candidate-attempt/repair-catalog",
                            "rejected_kind":"replace_function_body",
                            "constraints":["exact_rejected_target", "integer_literal_only", "fresh_rejected_attempt_and_repair_derivation", "exact_predecessor_bound_repair_id", "full_candidate_admission", "rebase_requires_rediscovery"],
                        }));
                    }
                    operations.push(json!({
                        "kind":"repair_diagnostic", "required_fields":["kind","target","rejected_intent","repair_id"],
                        "repair_class":"borrow_owned_byte_field_without_staging",
                        "selector_source":"attempt/repair-catalog",
                        "rejected_kind":"replace_function_body",
                        "constraints":["exact_rejected_target", "recorded_SPX-T266_diagnostic", "closed_builtin_byte_view_of_direct_lexical_field_projection", "fresh_rejected_attempt_and_repair_derivation", "exact_predecessor_bound_repair_id", "full_candidate_admission", "rebase_requires_rediscovery"],
                    }));
                    operations.push(json!({
                        "kind":"replace_expression", "required_fields":["kind","target","expression_id","replacement"],
                        "selector_source":"expression/catalog",
                        "constraints":["exact_revision_scoped_hir_expression", "unambiguous_source_expression", "body_region_only", "preserve_expected_type", "authenticated_lexical_scope", "full_candidate_revalidation"],
                    }));
                    if function
                        .requires
                        .len()
                        .saturating_add(function.ensures.len())
                        < 1024
                    {
                        operations.push(json!({
                            "kind":"add_contract", "required_fields":["kind","target","phase","predicate"],
                            "phases":["requires","ensures"],
                            "constraints":["append_one_predicate", "preserve_existing_predicate_order_and_content", "pure_boolean_predicate", "parameters_in_scope", "result_only_in_ensures", "full_candidate_revalidation"],
                        }));
                    }
                }
            }
            if function.explicit_id
                && function.type_parameters.is_empty()
                && function.name == "main"
                && self.changes.len() < MAX_CHANGES
            {
                reason = "constructor_available_payload_requires_full_candidate_admission";
                operations.push(json!({
                    "kind":"replace_expression", "required_fields":["kind","target","expression_id","replacement"],
                    "selector_source":"expression/catalog",
                    "constraints":["exact_revision_scoped_hir_expression", "unambiguous_source_expression", "body_region_only", "preserve_expected_type", "authenticated_lexical_scope", "full_candidate_revalidation"],
                }));
            }
            if function.explicit_id
                && function.type_parameters.is_empty()
                && self.changes.len() < MAX_CHANGES
            {
                operations.push(json!({
                    "kind":"add_declaration", "required_fields":["kind","target","declaration"],
                    "anchor":target, "placement":"append_function_in_anchor_module",
                    "nominal_type_selector":"nominal_types",
                    "type_declaration_forms":[
                        {"kind":"record","placement":"append_record_in_anchor_module","max_fields":64,"max_combined_identities":4096,"requires_full_candidate_validation":true},
                        {"kind":"variant","placement":"append_variant_in_anchor_module","min_cases":1,"max_cases":64,"max_fields_per_case":64,"max_combined_identities":4096,"requires_full_candidate_validation":true},
                    ],
                    "constraints":["globally_new_explicit_identity", "non_main_monomorphic_function", "unambiguous_module_namespace", "effects_within_anchor_budget_and_module_permits", "preserve_all_existing_declarations_and_exports", "full_candidate_revalidation"],
                }));
                operations.push(json!({
                    "kind":"extract_function", "required_fields":["kind","target","expression_id","new_id","new_name"],
                    "selector_source":"expression/catalog",
                    "constraints":["unique_authored_body_expression", "globally_new_explicit_identity", "compiler_derived_copy_captures", "checked_sized_copy_scalar_or_nominal_values", "field_reads_capture_immutable_copy_root", "preserve_original_lazy_position_and_evaluation_order", "no_mutable_or_escaping_owned_captures", "no_borrowed_or_resource_values", "full_candidate_revalidation"],
                }));
                let destinations = super::movement::destinations(&self.revision, target)?;
                if !destinations.is_empty() {
                    operations.push(json!({
                        "kind":"move_declaration", "required_fields":["kind","target","destination"],
                        "destination_anchors":destinations,
                        "constraints":["distinct_existing_module", "preserve_exact_stable_identity", "migrate_authenticated_call_bindings_and_import_origins", "preserve_checked_nominal_type_identities", "migrate_authenticated_type_bindings", "copy_values_only", "preserve_manifest_exports_and_effect_budgets", "full_candidate_revalidation"],
                    }));
                }
            }
        }
        if self.changes.len() < MAX_CHANGES
            && selected.is_some_and(|function| {
                function.explicit_id
                    && function.type_parameters.is_empty()
                    && (!function.requires.is_empty() || !function.ensures.is_empty())
            })
        {
            operations.push(json!({
                "kind":"replace_contract_expression", "required_fields":["kind","target","expression_id","replacement"],
                "selector_source":"candidate/contract-expression-catalog", "phases":["requires","ensures"],
                "constraints":["exact_revision_scoped_hir_expression", "unambiguous_source_expression", "existing_contract_region_only", "preserve_expected_type_and_ownership", "authenticated_lexical_scope", "preserve_all_other_source", "full_candidate_revalidation"],
            }));
        }
        if self.changes.len() < MAX_CHANGES
            && selected.is_none()
            && super::type_rename::eligible(&self.revision, target)?
        {
            reason = "constructor_available_payload_requires_full_candidate_admission";
            if let Some(kind) = super::type_rename::member_kind(&self.revision, target)? {
                operations.push(json!({
                    "kind":"rename_declaration", "required_fields":["kind","target","name"],
                    "member_kind":kind,
                    "constraints":["explicit_source_owner_and_member_chain", "new_identifier_max_128_bytes", "different_display_name", "unambiguous_owner_member_namespace", "preserve_stable_identity_and_member_order", "preserve_import_aliases", "migrate_authenticated_member_occurrences", "full_candidate_revalidation"],
                }));
            } else {
                operations.push(json!({
                    "kind":"rename_declaration", "required_fields":["kind","target","name"],
                    "constraints":["source_record_or_variant_owner", "new_identifier_max_128_bytes", "different_display_name", "unambiguous_type_namespace", "preserve_stable_identity_and_member_identities", "preserve_import_aliases", "migrate_authenticated_type_occurrences", "full_candidate_revalidation"],
                }));
            }
        }
        if self.changes.len() < MAX_CHANGES
            && super::record_field::eligible(&self.revision, target)?
        {
            reason = "constructor_available_payload_requires_full_candidate_admission";
            operations.push(json!({
                "kind":"add_record_field", "required_fields":["kind","target","field"],
                "field_fields":["id","name","type","default"], "field_types":["i64","bool","i32","u8","usize"],
                "constraints":["globally_new_explicit_field_identity", "unique_field_name", "monomorphic_checked_copy_or_flat_owned_bytes_record", "matching_pure_literal_default", "append_default_after_existing_field_evaluations", "migrate_all_authenticated_constructors_and_exact_patterns", "preserve_existing_field_identities_and_projection_meaning", "no_new_owned_field_or_ownership_transfer", "revalidate_layout_ownership_cleanup_and_targets"],
            }));
        }
        if self.changes.len() < MAX_CHANGES {
            let protocols = super::interface::discover(&self.revision, target)?;
            if protocols
                .iter()
                .any(|protocol| protocol["complete_mapping_available"] == true)
            {
                operations.push(json!({
                    "kind":"implement_interface","required_fields":["kind","target","protocol","id","members"],
                    "member_fields":["method","implementation"],"discovery":"ProjectCandidate::interface_catalog",
                    "constraints":["explicit_local_monomorphic_record","explicit_local_protocol_and_members","exact_complete_member_coverage","existing_local_matching_functions","fresh_explicit_implementation_identity","source_static_conformance_validation","full_candidate_revalidation"]
                }));
                reason = "constructor_available_payload_requires_full_candidate_admission";
            }
        }
        if !aggregates.is_empty()
            || !projections.is_empty()
            || !matches.is_empty()
            || !updates.is_empty()
            || !builtins.is_empty()
            || !field_places.is_empty()
        {
            for operation in &mut operations {
                if operation["kind"] == "replace_function_body" {
                    let constructors = operation["constructors"].as_array_mut().unwrap();
                    if !builtins.is_empty() {
                        constructors.push(json!("builtin_call"));
                    }
                    if !field_places.is_empty() {
                        constructors.push(json!("field_place"));
                    }
                    for kind in ["record", "variant"] {
                        if aggregates.iter().any(|item| item["kind"] == kind) {
                            constructors.push(json!(kind));
                        }
                    }
                    if !projections.is_empty() {
                        constructors.push(json!("project"));
                    }
                    if !matches.is_empty() {
                        constructors.push(json!("match"));
                    }
                    if !updates.is_empty() {
                        constructors.push(json!("update"));
                    }
                }
            }
        }
        let mut report = json!({
            "schema":"semaprax.project-change-catalog.v1",
            "candidate_digest":self.candidate_digest(),
            "project_revision":self.revision.project_revision(),
            "target":target, "parameters":parameters, "operations":operations,
            "reason":reason,
            "admission":"constructor_discovery_only",
            "requires_full_candidate_validation":true,
            "source_authority":false,
        });
        if !aggregates.is_empty() {
            report["aggregate_constructors"] = json!(aggregates);
        }
        if !projections.is_empty() {
            report["aggregate_projections"] = json!(projections);
        }
        if !matches.is_empty() {
            report["aggregate_matches"] = json!(matches);
        }
        if !updates.is_empty() {
            report["aggregate_updates"] = json!(updates);
        }
        if !nominal_types.is_empty() {
            report["nominal_types"] = json!(nominal_types);
        }
        if !builtins.is_empty() {
            report["builtin_calls"] = json!(builtins);
        }
        if !field_places.is_empty() {
            report["field_places"] = json!(field_places);
        }
        wire::render(report, 256 * 1024)
    }
}
