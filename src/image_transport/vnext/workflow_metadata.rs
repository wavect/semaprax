//! Closed supported-workflow metadata derived from the selected method registry.
use super::super::super::Method;
use super::super::{VNextPolicy, VNEXT_PROTOCOL_SCHEMA};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(in crate::image_transport::vnext) const EVENTS: &[&str] = &[
    "transport_or_response_uncertain_before_publication",
    "stale_image_reference_or_source_drift",
    "semantic_review_rejection",
    "publish_precondition_rejection",
    "definite_pre_pivot_commit_failure",
    "publication_uncertain",
    "published_and_independently_inspected",
];
pub(in crate::image_transport::vnext) const OUTCOMES: &[&str] = &[
    "transport_uncertain_no_publish_claim",
    "stale_subject",
    "review_rejected",
    "publish_precondition_rejected",
    "publish_failed_pre_pivot",
    "publication_uncertain",
    "published",
];
pub(in crate::image_transport::vnext) const REPAIR_ACTIONS: &[&str] =
    &["none", "start_new_review_with_different_intention"];

const PROFILE_DOMAIN: &[u8] = b"semaprax.supported-product-workflow.selected-profile-binding.v1\0";
const CONTRACT_SCHEMA: &str = "semaprax.supported-product-workflow-response-contract.v1";

pub(super) fn profile_revision(profile: &Value) -> String {
    let bytes = serde_json::to_vec(profile).expect("selected profile binding serializes");
    let mut hash = Sha256::new();
    hash.update(PROFILE_DOMAIN);
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn effect(method: &str) -> &'static str {
    match method {
        "candidate/open" | "candidate/apply-intent" | "candidate/recovery-restore" => {
            "candidate_overlay_mutation"
        }
        "candidate/test" => "bounded_test_execution",
        "candidate/commit" => "source_publication",
        "candidate/commit-report" => "receipt_read",
        _ => "read_only",
    }
}

fn response_contract(method: &Method) -> Value {
    let effect = effect(method.name);
    json!({
        "schema":CONTRACT_SCHEMA,
        "payload_schema":method.payload_schema,
        "required_grants":required_grants(method),
        "effect":effect,
        "authority":{
            "request_capability_changes":false,
            "evidence_or_handoff_grants_authority":false,
            "candidate_overlay_mutation":effect == "candidate_overlay_mutation",
            "test_execution":effect == "bounded_test_execution",
            "source_publication":effect == "source_publication"
        },
        "blind_spots":{
            "ledger_reference":"workflow.blind_spots",
            "permitted_runtime_update":if method.name == "candidate/test" {
                json!({
                    "area":"runtime_environment",
                    "from":"not_inspected",
                    "to":"partial",
                    "requires":"bound_successful_reference_interpreter_report"
                })
            } else {
                Value::Null
            }
        }
    })
}

fn required_grants(method: &Method) -> Vec<&'static str> {
    let descriptor = super::method_capability(method);
    match method.name {
        "candidate/test" | "candidate/test-plan" => {
            vec!["candidate_prepare", "candidate_test"]
        }
        "candidate/commit" => {
            vec!["candidate_prepare", "source_commit"]
        }
        name if name.starts_with("candidate/") && name != "candidate/commit-report" => {
            vec!["candidate_prepare"]
        }
        _ => vec![descriptor],
    }
}

fn step(methods: &[&Method], index: usize, id: Option<&str>, method: &str) -> Value {
    let selected = methods
        .iter()
        .copied()
        .find(|candidate| candidate.name == method)
        .expect("workflow methods were checked before step construction");
    let mut value = json!({
        "index":index,
        "method":method,
        "response_contract":response_contract(selected)
    });
    if let Some(id) = id {
        value["id"] = json!(id);
    }
    value
}

pub(super) fn supported(methods: &[&Method], grants: &[&str], policy: &VNextPolicy) -> Vec<Value> {
    let selected = methods
        .iter()
        .map(|method| method.name)
        .collect::<BTreeSet<_>>();
    let review = [
        ("open_original_subject", "workspace/open"),
        (
            "export_function_reference",
            "image/function-reference-export",
        ),
        (
            "resolve_function_reference",
            "image/function-reference-resolve",
        ),
        ("inspect_base_analysis_coverage", "image/analysis-coverage"),
        ("open_candidate", "candidate/open"),
        ("apply_signature_intention", "candidate/apply-intent"),
        ("validate_candidate", "candidate/validate"),
        ("read_semantic_delta", "candidate/semantic-delta"),
        ("read_test_plan", "candidate/test-plan"),
        ("execute_selected_tests", "candidate/test"),
        ("read_source_review", "candidate/source-review"),
        (
            "inspect_candidate_analysis_coverage",
            "candidate/analysis-coverage",
        ),
        ("export_recovery_capsule", "candidate/recovery-export"),
    ];
    if review.iter().any(|(_, method)| !selected.contains(method)) {
        return Vec::new();
    }
    let mut phases = vec![json!({
        "id":"review",
        "session":"review_session",
        "required_grants":["candidate_prepare","candidate_test"],
        "ordered_steps":review.iter().enumerate().map(|(index, (_, method))|
            step(methods, index + 1, None, method)).collect::<Vec<_>>(),
        "outcome":"reviewed_candidate_and_source_backed_recovery_capsule",
        "publication_authority":false
    })];
    let publish = [
        ("open_original_subject", "workspace/open"),
        (
            "resolve_reviewed_function",
            "image/function-reference-resolve",
        ),
        ("restore_candidate", "candidate/recovery-restore"),
        ("repeat_validation", "candidate/validate"),
        ("repeat_source_review", "candidate/source-review"),
        ("precommit_status", "source-commit/status"),
        ("commit_once", "candidate/commit"),
        ("postcommit_status", "source-commit/status"),
        (
            "read_receipt_after_published_status",
            "candidate/commit-report",
        ),
    ];
    if publish.iter().all(|(_, method)| selected.contains(method)) {
        let mut steps = publish
            .iter()
            .enumerate()
            .map(|(index, (id, method))| step(methods, index + 1, Some(id), method))
            .collect::<Vec<_>>();
        steps[5]["required_state"] = json!("available");
        steps[6]["maximum_calls"] = json!(1);
        steps[7]["required_state"] = json!("published_or_publication_uncertain");
        steps[8]["requires_state"] = json!("published");
        steps[8]["read_to_terminal_chunk"] = json!(true);
        phases.push(json!({
            "id":"publish",
            "session":"separate_publish_session",
            "required_grants":["candidate_prepare","source_commit"],
            "ordered_steps":steps,
            "outcome":"published_or_publication_uncertain",
            "raw_working_tree_write":false
        }));
    }
    let profile = json!({
        "basis":"exact_selected_capabilities_document",
        "protocol":VNEXT_PROTOCOL_SCHEMA,
        "complete_method_set":methods.iter().map(|method|method.name).collect::<Vec<_>>(),
        "complete_grant_set":grants,
        "host_test_policy":policy.test_policy.as_ref().map(|policy|json!({
            "max_steps":policy.max_steps(),
            "max_execution_bytes":policy.max_execution_bytes(),
            "max_report_bytes":policy.max_report_bytes(),
            "engine":"project_interpreter",
            "request_overrides":false
        }))
    });
    let profile_revision = profile_revision(&profile);
    vec![json!({
        "id":"function_signature_review_publish_v1",
        "schema":"semaprax.supported-product-workflow.v1",
        "change_kind":"change_function_signature",
        "qualification":{
            "contract_and_composition_available":true,
            "executed_support_status":"not_qualified_by_discovery",
            "successful_support_requires":"external_clean_exact_subject_evidence",
            "evidence_embedded":false,
            "evidence_inferred":false,
            "selected_profile_binding":profile,
            "selected_profile_revision":profile_revision
        },
        "phases":phases,
        "separate_session_handoff":{
            "export_method":"candidate/recovery-export",
            "restore_method":"candidate/recovery-restore",
            "carrier":"exact_source_backed_recovery_capsule",
            "host_storage_and_transfer":"out_of_band",
            "authority_transfer":false,
            "same_original_source_required":true,
            "bound_review_artifacts":[
                "candidate_recovery_capsule","candidate_revision",
                "compact_function_reference","typed_intention_bytes",
                "validation_and_semantic_delta","test_plan_and_report",
                "source_review_digest","base_and_candidate_analysis_coverage_digests"
            ]
        },
        "publication_approval":{
            "mode":"out_of_band_host_approval_before_first_publish_session_frame",
            "request_can_approve":false,
            "candidate_revision_exact":true,
            "approval_single_use":true
        },
        "transition_contract":{
            "schema":"semaprax.function-signature-review-publish-transition.v1",
            "events":EVENTS,
            "outcomes":OUTCOMES,
            "repair_actions":REPAIR_ACTIONS,
            "rpc_error_shape":"code_message_with_optional_closed_application_diagnostic_data",
            "compiler_diagnostic_interior":"typed_application_diagnostics_with_unstructured_transport_and_grammar_fallback",
            "diagnostic_repair_catalog":"not_selected_or_authorized_by_this_workflow"
        },
        "transition_policy":[
            {"event":"transport_or_response_uncertain_before_publication","workflow_outcome":"transport_uncertain_no_publish_claim","candidate_state":"not_a_publication_receipt","session_state":"retired","next":"inspect_out_of_band_state_before_any_new_workflow","repair_action":"none","blind_retry":false},
            {"event":"stale_image_reference_or_source_drift","workflow_outcome":"stale_subject","candidate_state":"not_authoritative_for_live_source","session_state":"invalidated","next":"open_new_session_and_rederive","repair_action":"none","blind_retry":false},
            {"event":"semantic_review_rejection","workflow_outcome":"review_rejected","candidate_state":"preserved","session_state":"open_or_finished_without_approval","next":"start_new_review_with_different_intention","repair_action":"start_new_review_with_different_intention","blind_retry":false},
            {"event":"publish_precondition_rejection","workflow_outcome":"publish_precondition_rejected","candidate_state":"preserved_without_commit","session_state":"retired","next":"inspect_handoff_approval_and_subject_mismatch","repair_action":"none","blind_retry":false},
            {"event":"definite_pre_pivot_commit_failure","workflow_outcome":"publish_failed_pre_pivot","candidate_state":"not_published","session_state":"terminal_approval_consumed","next":"new_host_configured_and_approved_session_required","repair_action":"none","blind_retry":false},
            {"event":"publication_uncertain","workflow_outcome":"publication_uncertain","candidate_state":"publication_outcome_unknown","session_state":"terminal","next":"inspect_source_commit_status_fixed_git_ref_and_prepared_commit","repair_action":"none","blind_retry":false,"rollback_claim":false},
            {"event":"published_and_independently_inspected","workflow_outcome":"published","candidate_state":"published_exact_candidate","session_state":"terminal","next":"no_further_publication_action","repair_action":"none","blind_retry":false}
        ],
        "blind_spots":[
            {"area":"analysis_completeness","status":"partial","basis":"bounded_retained_compiler_facts_are_not_complete_impact_or_behavior_proof"},
            {"area":"generated_file_provenance","status":"not_inspected","basis":"listed_source_or_output_names_do_not_authenticate_generators_inputs_or_freshness"},
            {"area":"generated_artifacts","status":"not_inspected","basis":"this_workflow_does_not_project_materialize_install_or_bind_artifacts"},
            {"area":"deployment_configuration","status":"not_inspected","basis":"no_environment_secret_route_or_infrastructure_input_is_read"},
            {"area":"external_api_behavior","status":"not_inspected","basis":"declared_contracts_do_not_verify_provider_versions_availability_authentication_or_side_effects"},
            {"area":"runtime_environment","status":"not_inspected","basis":"static_discovery_precedes_execution_and_binds_no_successful_runtime_report","conditional_promotion":"partial_only_after_bound_successful_reference_interpreter_report"},
            {"area":"external_consumers","status":"not_inspected","basis":"exports_and_graph_edges_do_not_enumerate_installed_or_dynamic_consumers"}
        ],
        "authority":{
            "request_capability_changes":false,
            "review_source_authority":false,
            "recovery_capsule_authority":false,
            "publication_only_in_selected_publish_phase":true,
            "deployment_network_process_or_secret_authority":false
        }
    })]
}
