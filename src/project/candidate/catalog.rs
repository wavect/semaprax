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
        let (aggregates, projections, matches, updates) = match selected {
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
                )
            }
            _ => (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
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
                        "constructors":["i64","i32","u8","usize","bool","place","binary","unary","if","call"],
                        "expression_nodes_maximum":4096, "expression_depth_maximum":64,
                        "constraints":["place_selects_existing_parameter", "call_selects_accessible_stable_id", "expected_return_type", "declared_effect_budget", "contracts_ownership_cleanup_and_target_revalidation"],
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
                    "constraints":["globally_new_explicit_identity", "non_main_monomorphic_function", "unambiguous_module_namespace", "effects_within_anchor_budget_and_module_permits", "preserve_all_existing_declarations_and_exports", "full_candidate_revalidation"],
                }));
                operations.push(json!({
                    "kind":"extract_function", "required_fields":["kind","target","expression_id","new_id","new_name"],
                    "selector_source":"expression/catalog",
                    "constraints":["unique_authored_body_expression", "globally_new_explicit_identity", "compiler_derived_copy_captures", "preserve_original_lazy_position_and_evaluation_order", "no_mutable_or_escaping_owned_captures", "full_candidate_revalidation"],
                }));
                let destinations = super::movement::destinations(&self.revision, target)?;
                if !destinations.is_empty() {
                    operations.push(json!({
                        "kind":"move_declaration", "required_fields":["kind","target","destination"],
                        "destination_anchors":destinations,
                        "constraints":["distinct_existing_module", "preserve_exact_stable_identity", "migrate_authenticated_call_bindings_and_import_origins", "preserve_manifest_exports_and_effect_budgets", "full_candidate_revalidation"],
                    }));
                }
            }
        }
        if self.changes.len() < MAX_CHANGES
            && super::record_field::eligible(&self.revision, target)?
        {
            reason = "constructor_available_payload_requires_full_candidate_admission";
            operations.push(json!({
                "kind":"add_record_field", "required_fields":["kind","target","field"],
                "field_fields":["id","name","type","default"], "field_types":["i64","bool"],
                "constraints":["globally_new_explicit_field_identity", "unique_field_name", "monomorphic_copy_record", "matching_pure_literal_default", "migrate_all_authenticated_constructors_and_exact_patterns", "preserve_existing_field_identities_and_projection_meaning", "revalidate_layout_ownership_and_targets"],
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
        {
            for operation in &mut operations {
                if operation["kind"] == "replace_function_body" {
                    let constructors = operation["constructors"].as_array_mut().unwrap();
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
        wire::render(report, 256 * 1024)
    }
}
