//! Closed generated-client types for supported workflow metadata.

pub(super) fn typescript() -> &'static str {
    r#"export type WorkflowEffect = 'read_only' | 'candidate_overlay_mutation' | 'bounded_test_execution' | 'source_publication' | 'receipt_read';
export interface WorkflowResponseAuthority { request_capability_changes: false; evidence_or_handoff_grants_authority: false; candidate_overlay_mutation: boolean; test_execution: boolean; source_publication: boolean; }
export interface WorkflowRuntimeUpdate { area: 'runtime_environment'; from: 'not_inspected'; to: 'partial'; requires: 'bound_successful_reference_interpreter_report'; }
export interface WorkflowResponseBlindSpots { ledger_reference: 'workflow.blind_spots'; permitted_runtime_update: WorkflowRuntimeUpdate | null; }
export interface WorkflowResponseContract { schema: 'semaprax.supported-product-workflow-response-contract.v1'; payload_schema: string; required_grants: readonly string[]; effect: WorkflowEffect; authority: WorkflowResponseAuthority; blind_spots: WorkflowResponseBlindSpots; }
export interface WorkflowStep { index: number; id?: string; method: string; response_contract: WorkflowResponseContract; required_state?: string; maximum_calls?: number; requires_state?: string; read_to_terminal_chunk?: true; }
export interface WorkflowPhase { id: 'review' | 'publish'; session: 'review_session' | 'separate_publish_session'; required_grants: readonly string[]; ordered_steps: readonly WorkflowStep[]; outcome: string; publication_authority?: false; raw_working_tree_write?: false; }
export interface WorkflowTestPolicy { max_steps: number; max_execution_bytes: number; max_report_bytes: number; engine: 'project_interpreter'; request_overrides: false; }
export interface WorkflowSelectedProfileBinding { basis: 'exact_selected_capabilities_document'; protocol: 'semaprax.image-agent-protocol.v5'; complete_method_set: readonly string[]; complete_grant_set: readonly string[]; host_test_policy: WorkflowTestPolicy | null; }
export interface WorkflowQualification { contract_and_composition_available: true; executed_support_status: 'not_qualified_by_discovery'; successful_support_requires: 'external_clean_exact_subject_evidence'; evidence_embedded: false; evidence_inferred: false; selected_profile_binding: WorkflowSelectedProfileBinding; selected_profile_revision: string; }
export interface WorkflowHandoff { export_method: 'candidate/recovery-export'; restore_method: 'candidate/recovery-restore'; carrier: 'exact_source_backed_recovery_capsule'; host_storage_and_transfer: 'out_of_band'; authority_transfer: false; same_original_source_required: true; bound_review_artifacts: readonly string[]; }
export interface WorkflowPublicationApproval { mode: 'out_of_band_host_approval_before_first_publish_session_frame'; request_can_approve: false; candidate_revision_exact: true; approval_single_use: true; }
export interface WorkflowTransitionContract { schema: 'semaprax.function-signature-review-publish-transition.v1'; events: readonly WorkflowEvent[]; outcomes: readonly WorkflowOutcome[]; repair_actions: readonly WorkflowRepairAction[]; rpc_error_shape: 'code_message_with_optional_closed_application_diagnostic_data'; compiler_diagnostic_interior: 'typed_application_diagnostics_with_unstructured_transport_and_grammar_fallback'; diagnostic_repair_catalog: 'not_selected_or_authorized_by_this_workflow'; }
export interface WorkflowTransition { event: WorkflowEvent; workflow_outcome: WorkflowOutcome; candidate_state: string; session_state: string; next: string; repair_action: WorkflowRepairAction; blind_retry: false; rollback_claim?: false; }
export interface WorkflowBlindSpot { area: 'analysis_completeness' | 'generated_file_provenance' | 'generated_artifacts' | 'deployment_configuration' | 'external_api_behavior' | 'runtime_environment' | 'external_consumers'; status: 'partial' | 'not_inspected'; basis: string; conditional_promotion?: 'partial_only_after_bound_successful_reference_interpreter_report'; }
export interface WorkflowAuthority { request_capability_changes: false; review_source_authority: false; recovery_capsule_authority: false; publication_only_in_selected_publish_phase: true; deployment_network_process_or_secret_authority: false; }
export interface SupportedWorkflow { schema: 'semaprax.supported-product-workflow.v1'; id: 'function_signature_review_publish_v1'; change_kind: 'change_function_signature'; qualification: WorkflowQualification; phases: readonly WorkflowPhase[]; separate_session_handoff: WorkflowHandoff; publication_approval: WorkflowPublicationApproval; transition_contract: WorkflowTransitionContract; transition_policy: readonly WorkflowTransition[]; blind_spots: readonly WorkflowBlindSpot[]; authority: WorkflowAuthority; }
/** Validate only the immutable producer-embedded catalogue; callers do not supply workflow metadata. */
export function validateWorkflowCatalogue(value: unknown, expectedRevision: string | null): readonly SupportedWorkflow[] { if (JSON.stringify(value) !== WORKFLOWS_JSON || !Array.isArray(value)) throw new Error('supported workflow catalogue mismatch'); if (expectedRevision === null) { if (value.length !== 0) throw new Error('supported workflow revision mismatch'); } else { if (value.length !== 1 || (value[0] as any)?.qualification?.selected_profile_revision !== expectedRevision) throw new Error('supported workflow revision mismatch'); } return value as readonly SupportedWorkflow[]; }
export function workflowResponseContract(workflow: SupportedWorkflow, phase: 'review' | 'publish', index: number, method: string): WorkflowResponseContract { const selected = workflow.phases.find(value => value.id === phase)?.ordered_steps.find(value => value.index === index); if (!selected || selected.method !== method) throw new Error('workflow response contract selection mismatch'); return selected.response_contract; }
"#
}

pub(super) fn python() -> &'static str {
    r#"WorkflowEffect: TypeAlias = Literal['read_only', 'candidate_overlay_mutation', 'bounded_test_execution', 'source_publication', 'receipt_read']
class WorkflowResponseAuthority(TypedDict):
    request_capability_changes: Literal[False]
    evidence_or_handoff_grants_authority: Literal[False]
    candidate_overlay_mutation: bool
    test_execution: bool
    source_publication: bool
WorkflowRuntimeUpdate = TypedDict('WorkflowRuntimeUpdate', {
    'area': Literal['runtime_environment'],
    'from': Literal['not_inspected'],
    'to': Literal['partial'],
    'requires': Literal['bound_successful_reference_interpreter_report'],
})
class WorkflowResponseBlindSpots(TypedDict):
    ledger_reference: Literal['workflow.blind_spots']
    permitted_runtime_update: WorkflowRuntimeUpdate | None
class WorkflowResponseContract(TypedDict):
    schema: Literal['semaprax.supported-product-workflow-response-contract.v1']
    payload_schema: str
    required_grants: list[str]
    effect: WorkflowEffect
    authority: WorkflowResponseAuthority
    blind_spots: WorkflowResponseBlindSpots
class WorkflowStep(TypedDict):
    index: int
    id: NotRequired[str]
    method: str
    response_contract: WorkflowResponseContract
    required_state: NotRequired[str]
    maximum_calls: NotRequired[int]
    requires_state: NotRequired[str]
    read_to_terminal_chunk: NotRequired[Literal[True]]
class WorkflowPhase(TypedDict):
    id: Literal['review', 'publish']
    session: Literal['review_session', 'separate_publish_session']
    required_grants: list[str]
    ordered_steps: list[WorkflowStep]
    outcome: str
    publication_authority: NotRequired[Literal[False]]
    raw_working_tree_write: NotRequired[Literal[False]]
class WorkflowTestPolicy(TypedDict):
    max_steps: int
    max_execution_bytes: int
    max_report_bytes: int
    engine: Literal['project_interpreter']
    request_overrides: Literal[False]
class WorkflowSelectedProfileBinding(TypedDict):
    basis: Literal['exact_selected_capabilities_document']
    protocol: Literal['semaprax.image-agent-protocol.v5']
    complete_method_set: list[str]
    complete_grant_set: list[str]
    host_test_policy: WorkflowTestPolicy | None
class WorkflowQualification(TypedDict):
    contract_and_composition_available: Literal[True]
    executed_support_status: Literal['not_qualified_by_discovery']
    successful_support_requires: Literal['external_clean_exact_subject_evidence']
    evidence_embedded: Literal[False]
    evidence_inferred: Literal[False]
    selected_profile_binding: WorkflowSelectedProfileBinding
    selected_profile_revision: str
class WorkflowHandoff(TypedDict):
    export_method: Literal['candidate/recovery-export']
    restore_method: Literal['candidate/recovery-restore']
    carrier: Literal['exact_source_backed_recovery_capsule']
    host_storage_and_transfer: Literal['out_of_band']
    authority_transfer: Literal[False]
    same_original_source_required: Literal[True]
    bound_review_artifacts: list[str]
class WorkflowPublicationApproval(TypedDict):
    mode: Literal['out_of_band_host_approval_before_first_publish_session_frame']
    request_can_approve: Literal[False]
    candidate_revision_exact: Literal[True]
    approval_single_use: Literal[True]
class WorkflowTransitionContract(TypedDict):
    schema: Literal['semaprax.function-signature-review-publish-transition.v1']
    events: list[WorkflowEvent]
    outcomes: list[WorkflowOutcome]
    repair_actions: list[WorkflowRepairAction]
    rpc_error_shape: Literal['code_message_with_optional_closed_application_diagnostic_data']
    compiler_diagnostic_interior: Literal['typed_application_diagnostics_with_unstructured_transport_and_grammar_fallback']
    diagnostic_repair_catalog: Literal['not_selected_or_authorized_by_this_workflow']
class WorkflowTransition(TypedDict):
    event: WorkflowEvent
    workflow_outcome: WorkflowOutcome
    candidate_state: str
    session_state: str
    next: str
    repair_action: WorkflowRepairAction
    blind_retry: Literal[False]
    rollback_claim: NotRequired[Literal[False]]
class WorkflowBlindSpot(TypedDict):
    area: Literal['analysis_completeness', 'generated_file_provenance', 'generated_artifacts', 'deployment_configuration', 'external_api_behavior', 'runtime_environment', 'external_consumers']
    status: Literal['partial', 'not_inspected']
    basis: str
    conditional_promotion: NotRequired[Literal['partial_only_after_bound_successful_reference_interpreter_report']]
class WorkflowAuthority(TypedDict):
    request_capability_changes: Literal[False]
    review_source_authority: Literal[False]
    recovery_capsule_authority: Literal[False]
    publication_only_in_selected_publish_phase: Literal[True]
    deployment_network_process_or_secret_authority: Literal[False]
class SupportedWorkflow(TypedDict):
    schema: Literal['semaprax.supported-product-workflow.v1']
    id: Literal['function_signature_review_publish_v1']
    change_kind: Literal['change_function_signature']
    qualification: WorkflowQualification
    phases: list[WorkflowPhase]
    separate_session_handoff: WorkflowHandoff
    publication_approval: WorkflowPublicationApproval
    transition_contract: WorkflowTransitionContract
    transition_policy: list[WorkflowTransition]
    blind_spots: list[WorkflowBlindSpot]
    authority: WorkflowAuthority
def validate_workflow_catalogue(value: Any, expected_revision: str | None) -> list[SupportedWorkflow]:
    """Validate only immutable producer-embedded metadata; callers do not supply it."""
    if not isinstance(value, list) or json.dumps(value, separators=(',', ':'), ensure_ascii=False) != WORKFLOWS_JSON:
        raise ValueError('supported workflow catalogue mismatch')
    if expected_revision is None:
        if value:
            raise ValueError('supported workflow revision mismatch')
    elif len(value) != 1 or value[0].get('qualification', {}).get('selected_profile_revision') != expected_revision:
        raise ValueError('supported workflow revision mismatch')
    return value
def workflow_response_contract(workflow: SupportedWorkflow, phase: Literal['review', 'publish'], index: int, method: str) -> WorkflowResponseContract:
    selected = next((step for item in workflow['phases'] if item['id'] == phase for step in item['ordered_steps'] if step['index'] == index), None)
    if selected is None or selected['method'] != method:
        raise ValueError('workflow response contract selection mismatch')
    return selected['response_contract']
"#
}

pub(super) fn rust() -> &'static str {
    r#"#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum WorkflowEffect { ReadOnly, CandidateOverlayMutation, BoundedTestExecution, SourcePublication, ReceiptRead }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowResponseAuthority { pub request_capability_changes: bool, pub evidence_or_handoff_grants_authority: bool, pub candidate_overlay_mutation: bool, pub test_execution: bool, pub source_publication: bool }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum WorkflowBlindSpotArea { AnalysisCompleteness, GeneratedFileProvenance, GeneratedArtifacts, DeploymentConfiguration, ExternalApiBehavior, RuntimeEnvironment, ExternalConsumers }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum WorkflowBlindSpotStatus { Partial, NotInspected }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowRuntimeUpdate { pub area: WorkflowBlindSpotArea, #[serde(rename="from")] pub from_: WorkflowBlindSpotStatus, pub to: WorkflowBlindSpotStatus, pub requires: String }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowResponseBlindSpots { pub ledger_reference: String, pub permitted_runtime_update: Option<WorkflowRuntimeUpdate> }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowResponseContract { pub schema: String, pub payload_schema: String, pub required_grants: Vec<String>, pub effect: WorkflowEffect, pub authority: WorkflowResponseAuthority, pub blind_spots: WorkflowResponseBlindSpots }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowStep { pub index: u64, #[serde(default)] pub id: Option<String>, pub method: String, pub response_contract: WorkflowResponseContract, #[serde(default)] pub required_state: Option<String>, #[serde(default)] pub maximum_calls: Option<u64>, #[serde(default)] pub requires_state: Option<String>, #[serde(default)] pub read_to_terminal_chunk: Option<bool> }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum WorkflowPhaseId { Review, Publish }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum WorkflowSession { ReviewSession, SeparatePublishSession }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowPhase { pub id: WorkflowPhaseId, pub session: WorkflowSession, pub required_grants: Vec<String>, pub ordered_steps: Vec<WorkflowStep>, pub outcome: String, #[serde(default)] pub publication_authority: Option<bool>, #[serde(default)] pub raw_working_tree_write: Option<bool> }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowTestPolicy { pub max_steps: u64, pub max_execution_bytes: u64, pub max_report_bytes: u64, pub engine: String, pub request_overrides: bool }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowSelectedProfileBinding { pub basis: String, pub protocol: String, pub complete_method_set: Vec<String>, pub complete_grant_set: Vec<String>, pub host_test_policy: Option<WorkflowTestPolicy> }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowQualification { pub contract_and_composition_available: bool, pub executed_support_status: String, pub successful_support_requires: String, pub evidence_embedded: bool, pub evidence_inferred: bool, pub selected_profile_binding: WorkflowSelectedProfileBinding, pub selected_profile_revision: String }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowHandoff { pub export_method: String, pub restore_method: String, pub carrier: String, pub host_storage_and_transfer: String, pub authority_transfer: bool, pub same_original_source_required: bool, pub bound_review_artifacts: Vec<String> }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowPublicationApproval { pub mode: String, pub request_can_approve: bool, pub candidate_revision_exact: bool, pub approval_single_use: bool }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowTransitionContract { pub schema: String, pub events: Vec<WorkflowEvent>, pub outcomes: Vec<WorkflowOutcome>, pub repair_actions: Vec<WorkflowRepairAction>, pub rpc_error_shape: String, pub compiler_diagnostic_interior: String, pub diagnostic_repair_catalog: String }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowTransition { pub event: WorkflowEvent, pub workflow_outcome: WorkflowOutcome, pub candidate_state: String, pub session_state: String, pub next: String, pub repair_action: WorkflowRepairAction, pub blind_retry: bool, #[serde(default)] pub rollback_claim: Option<bool> }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowBlindSpot { pub area: WorkflowBlindSpotArea, pub status: WorkflowBlindSpotStatus, pub basis: String, #[serde(default)] pub conditional_promotion: Option<String> }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct WorkflowAuthority { pub request_capability_changes: bool, pub review_source_authority: bool, pub recovery_capsule_authority: bool, pub publication_only_in_selected_publish_phase: bool, pub deployment_network_process_or_secret_authority: bool }
#[derive(Clone, Debug, Serialize, Deserialize)] #[serde(deny_unknown_fields)]
pub struct SupportedWorkflow { pub schema: String, pub id: String, pub change_kind: String, pub qualification: WorkflowQualification, pub phases: Vec<WorkflowPhase>, pub separate_session_handoff: WorkflowHandoff, pub publication_approval: WorkflowPublicationApproval, pub transition_contract: WorkflowTransitionContract, pub transition_policy: Vec<WorkflowTransition>, pub blind_spots: Vec<WorkflowBlindSpot>, pub authority: WorkflowAuthority }
/// Validate only the immutable producer-embedded catalogue; callers do not supply it.
fn validate_workflow_catalogue_json(value:&Value)->Result<(),String> { if serde_json::to_string(value).map_err(|error|error.to_string())? != WORKFLOWS_JSON { return Err("supported workflow catalogue mismatch".into()); } Ok(()) }
pub fn workflow_response_contract<'a>(workflow:&'a SupportedWorkflow, phase:WorkflowPhaseId, index:u64, method:&str)->Result<&'a WorkflowResponseContract,String> { let selected=workflow.phases.iter().find(|item|item.id==phase).and_then(|item|item.ordered_steps.iter().find(|step|step.index==index)).ok_or("workflow response contract selection mismatch")?; if selected.method!=method { return Err("workflow response contract selection mismatch".into()); } Ok(&selected.response_contract) }
fn selected_profile_revision_is_sha256(value:&str)->bool { value.strip_prefix("sha256:").is_some_and(|digest|digest.len()==64 && digest.bytes().all(|byte|byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))) }
fn expected_grants(method:&str)->&'static [&'static str] { match method { "candidate/test" | "candidate/test-plan" => &["candidate_prepare","candidate_test"], "candidate/commit" => &["candidate_prepare","source_commit"], "candidate/commit-report" | "source-commit/status" => &["source_commit"], value if value.starts_with("candidate/") => &["candidate_prepare"], _ => &["semantic_read"] } }
fn expected_effect(method:&str)->WorkflowEffect { match method { "candidate/open" | "candidate/apply-intent" | "candidate/recovery-restore" => WorkflowEffect::CandidateOverlayMutation, "candidate/test" => WorkflowEffect::BoundedTestExecution, "candidate/commit" => WorkflowEffect::SourcePublication, "candidate/commit-report" => WorkflowEffect::ReceiptRead, _ => WorkflowEffect::ReadOnly } }
pub fn validate_workflows(values:&[SupportedWorkflow],expected_revision:Option<&str>)->Result<(),String> {
    if values.is_empty() && expected_revision.is_none() { return Ok(()); }
    if values.len()!=1 { return Err("supported workflow inventory mismatch".into()); }
    let value=&values[0];
    if expected_revision!=Some(value.qualification.selected_profile_revision.as_str()) || value.schema!="semaprax.supported-product-workflow.v1" || value.id!="function_signature_review_publish_v1" || value.change_kind!="change_function_signature" || !value.qualification.contract_and_composition_available || value.qualification.executed_support_status!="not_qualified_by_discovery" || value.qualification.successful_support_requires!="external_clean_exact_subject_evidence" || value.qualification.evidence_embedded || value.qualification.evidence_inferred || value.qualification.selected_profile_binding.basis!="exact_selected_capabilities_document" || value.qualification.selected_profile_binding.protocol!="semaprax.image-agent-protocol.v5" || value.qualification.selected_profile_binding.host_test_policy.as_ref().is_some_and(|policy|policy.engine!="project_interpreter" || policy.request_overrides) || !selected_profile_revision_is_sha256(&value.qualification.selected_profile_revision) || value.separate_session_handoff.export_method!="candidate/recovery-export" || value.separate_session_handoff.restore_method!="candidate/recovery-restore" || value.separate_session_handoff.authority_transfer || !value.separate_session_handoff.same_original_source_required || value.publication_approval.request_can_approve || !value.publication_approval.candidate_revision_exact || !value.publication_approval.approval_single_use || value.transition_contract.schema!="semaprax.function-signature-review-publish-transition.v1" || value.transition_contract.rpc_error_shape!="code_message_with_optional_closed_application_diagnostic_data" || value.transition_contract.compiler_diagnostic_interior!="typed_application_diagnostics_with_unstructured_transport_and_grammar_fallback" || value.transition_contract.diagnostic_repair_catalog!="not_selected_or_authorized_by_this_workflow" || value.authority.request_capability_changes || value.authority.review_source_authority || value.authority.recovery_capsule_authority || !value.authority.publication_only_in_selected_publish_phase || value.authority.deployment_network_process_or_secret_authority { return Err("supported workflow literal contract mismatch".into()); }
    if value.phases.is_empty() || value.phases.len()>2 || value.phases[0].id!=WorkflowPhaseId::Review || value.phases[0].session!=WorkflowSession::ReviewSession || value.phases[0].ordered_steps.len()!=13 || value.phases.get(1).is_some_and(|phase|phase.id!=WorkflowPhaseId::Publish || phase.session!=WorkflowSession::SeparatePublishSession || phase.ordered_steps.len()!=9) { return Err("supported workflow phase contract mismatch".into()); }
    for phase in &value.phases { for (offset,step) in phase.ordered_steps.iter().enumerate() {
        if step.index != (offset+1) as u64 { return Err("workflow response contract index mismatch".into()); }
        let contract=&step.response_contract;
        let effect=expected_effect(&step.method);
        if contract.schema!="semaprax.supported-product-workflow-response-contract.v1" || contract.payload_schema.is_empty() || contract.required_grants.iter().map(String::as_str).collect::<Vec<_>>()!=expected_grants(&step.method) || contract.effect!=effect || contract.authority.request_capability_changes || contract.authority.evidence_or_handoff_grants_authority || contract.authority.candidate_overlay_mutation!=(effect==WorkflowEffect::CandidateOverlayMutation) || contract.authority.test_execution!=(effect==WorkflowEffect::BoundedTestExecution) || contract.authority.source_publication!=(effect==WorkflowEffect::SourcePublication) || contract.blind_spots.ledger_reference!="workflow.blind_spots" { return Err("workflow response contract literal mismatch".into()); }
        match (&contract.blind_spots.permitted_runtime_update,&effect) { (Some(update),WorkflowEffect::BoundedTestExecution) if update.area==WorkflowBlindSpotArea::RuntimeEnvironment && update.from_==WorkflowBlindSpotStatus::NotInspected && update.to==WorkflowBlindSpotStatus::Partial && update.requires=="bound_successful_reference_interpreter_report" => {}, (None,WorkflowEffect::BoundedTestExecution) | (Some(_),_) => return Err("workflow runtime blind-spot update mismatch".into()), _ => {} }
    } }
    Ok(())
}
"#
}
