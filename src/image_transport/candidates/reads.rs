//! Detached immutable subjects and value-only candidate reads. Registry lookup
//! belongs to the authenticated serial coordinator, never a worker closure.
use super::*;
use crate::project::ProjectCandidateAttempt;

#[derive(Default)]
pub(in crate::image_transport) struct ReadSubjects {
    pub(in crate::image_transport) candidate: Option<Arc<ProjectCandidate>>,
    pub(in crate::image_transport) other: Option<Arc<ProjectCandidate>>,
    pub(in crate::image_transport) draft: Option<Arc<ProjectCandidateDraft>>,
    pub(in crate::image_transport) attempt: Option<Arc<ProjectCandidateAttempt>>,
    pub(in crate::image_transport) retained_attempts: Vec<Arc<ProjectCandidateAttempt>>,
}

impl Registry {
    pub(in crate::image_transport) fn detach_read(
        &self,
        operation: Operation,
        params: &Map<String, Value>,
    ) -> Result<ReadSubjects, Vec<Diagnostic>> {
        if matches!(
            operation,
            Operation::VNext(super::super::vnext::Action::SymbolDiagnostics)
        ) {
            super::super::vnext::symbol_diagnostics::validate_parameters_before_selection(params)?;
        }
        let mut selected = ReadSubjects::default();
        if let Some(id) = params.get("candidate_revision").and_then(Value::as_str) {
            selected.candidate = Some(Arc::clone(self.candidate(id)?));
        }
        if let Some(id) = params
            .get("other_candidate_revision")
            .and_then(Value::as_str)
        {
            selected.other = Some(Arc::clone(self.candidate(id)?));
        }
        if let Some(id) = params.get("draft_revision").and_then(Value::as_str) {
            selected.draft = Some(Arc::clone(&self.draft(id)?.draft));
        }
        if let Some(id) = params.get("attempt_revision").and_then(Value::as_str) {
            selected.attempt = Some(Arc::clone(self.attempts.get(id).ok_or_else(|| {
                failure("SPX-G243", "attempt handle is stale, discarded, or unknown")
            })?));
        }
        if matches!(
            operation,
            Operation::VNext(super::super::vnext::Action::SymbolDiagnostics)
        ) {
            // Preserve registry order and defer provenance/matching failures to
            // the shared renderer, where their established precedence belongs.
            selected.retained_attempts = self.attempts.values().cloned().collect();
        }
        Ok(selected)
    }
}

pub(in crate::image_transport) fn supports(action: Action) -> bool {
    matches!(
        action,
        Action::Query
            | Action::RecoveryExport
            | Action::Validate
            | Action::Impact
            | Action::Compare
            | Action::ChangeCatalog
            | Action::ExpressionCatalog
            | Action::ConstructorSchemas
            | Action::ValidationCatalog
            | Action::HoleQuery
            | Action::TestPlan
    )
}

pub(in crate::image_transport) fn payload(
    action: Action,
    params: &Map<String, Value>,
    _image: &ProjectSemanticImage,
    subjects: &ReadSubjects,
    test_enabled: bool,
) -> Result<Value, Vec<Diagnostic>> {
    let candidate = || {
        subjects.candidate.as_ref().ok_or_else(|| {
            failure(
                "SPX-G224",
                "candidate handle is stale, discarded, or unknown",
            )
        })
    };
    match action {
        Action::TestPlan => {
            if !test_enabled {
                return Err(failure(
                    "SPX-G239",
                    "candidate test profile is not selected by the host",
                ));
            }
            let candidate = candidate()?;
            parse_payload(candidate.test_plan(candidate.candidate_digest())?)
        }
        Action::Query => {
            let candidate = candidate()?;
            let report = candidate.to_json();
            let offset = number(params, "offset", 0);
            if offset > report.len() || !report.is_char_boundary(offset) {
                return Err(failure(
                    "SPX-G222",
                    "candidate query offset is outside canonical UTF-8 report",
                ));
            }
            let mut end = offset
                .saturating_add(number(params, "chunk_bytes", 16_384))
                .min(report.len());
            while !report.is_char_boundary(end) {
                end -= 1;
            }
            Ok(
                json!({"schema":"semaprax.image-candidate-report-chunk.v1","candidate_revision":candidate.candidate_digest(),"report_schema":crate::project::PROJECT_CANDIDATE_SCHEMA,"offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),"source_authority":false}),
            )
        }
        Action::RecoveryExport => {
            let candidate = candidate()?;
            let capsule = candidate.recovery_capsule()?;
            let offset = number(params, "offset", 0);
            if offset > capsule.len() || !capsule.is_char_boundary(offset) {
                return Err(failure(
                    "SPX-G236",
                    "recovery offset is outside canonical UTF-8 capsule",
                ));
            }
            let mut end = offset
                .saturating_add(number(params, "chunk_bytes", 16_384))
                .min(capsule.len());
            while !capsule.is_char_boundary(end) {
                end -= 1;
            }
            Ok(
                json!({"schema":"semaprax.image-candidate-recovery-chunk.v1","candidate_revision":candidate.candidate_digest(),"capsule_schema":crate::project::PROJECT_CANDIDATE_RECOVERY_SCHEMA,"offset":offset,"total_bytes":capsule.len(),"chunk":&capsule[offset..end],"next_offset":(end<capsule.len()).then_some(end),"source_authority":false}),
            )
        }
        Action::Validate => {
            let candidate = candidate()?;
            let report = parse_payload(candidate.to_json().to_owned())?;
            let changes = report["changes"]
                .as_array()
                .ok_or_else(|| failure("SPX-G222", "retained candidate lacks change inventory"))?
                .iter()
                .map(|change| {
                    SemanticChange::new(
                        change["base_revision"]
                            .as_str()
                            .ok_or_else(|| failure("SPX-G222", "retained change lacks revision"))?,
                        &change["intent"],
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let replay = ProjectCandidate::replay(
                Arc::clone(candidate.base_revision()),
                candidate.base_revision().project_revision(),
                &changes,
                candidate.to_json().as_bytes(),
            )?;
            Ok(
                json!({"schema":"semaprax.image-candidate-validation.v1","candidate_revision":replay.candidate_digest(),"independently_replayed":true,"source_reparsed":true,"project_profile_admitted":true,"tests":"not_run","target_execution":false,"commit_authority":false}),
            )
        }
        Action::Impact => {
            let candidate = candidate()?;
            let options = WorkspaceImpactOptions::new(
                number(params, "depth", 16),
                number(params, "max_bytes", MAX_QUERY_BYTES),
                number(params, "max_nodes", 1024),
            )
            .map_err(|error| vec![error])?;
            let impact = parse_payload(candidate.revision().semantic_impact(
                WorkspaceAnalysisTargetKind::Declaration,
                text(params, "target"),
                options,
            )?)?;
            Ok(
                json!({"schema":"semaprax.image-candidate-impact.v1","candidate_revision":candidate.candidate_digest(),"impact":impact}),
            )
        }
        Action::Compare => {
            let left = candidate()?;
            let right = subjects.other.as_ref().ok_or_else(|| {
                failure(
                    "SPX-G224",
                    "candidate handle is stale, discarded, or unknown",
                )
            })?;
            parse_payload(left.compare(right)?)
        }
        Action::ChangeCatalog => {
            parse_payload(candidate()?.change_catalog(text(params, "target"))?)
        }
        Action::ExpressionCatalog => {
            parse_payload(candidate()?.expression_catalog(text(params, "target"))?)
        }
        Action::ConstructorSchemas => parse_payload(SemanticChange::constructor_schemas()?),
        Action::ValidationCatalog => {
            let mut payload = json!({"schema":"semaprax.image-validation-catalog.v1","available":[{"method":"candidate/validate","kind":"independent_source_and_target_projection_replay","runtime_execution":false}],"required_external_gates":["affected_project_tests","native_and_wasm_runtime_conformance","full_quality_profile"],"tests":"not_run","source_authority":false});
            if test_enabled {
                payload["schema"] = json!("semaprax.image-validation-catalog.v2");
                payload["available"].as_array_mut().expect("array").push(json!({"method":"candidate/test","kind":"bounded_project_interpreter_test_closure","runtime_execution":true}));
                payload["tests"] = json!("available_only_on_explicit_request");
                payload["required_external_gates"] = json!([
                    "native_and_wasm_runtime_conformance",
                    "full_quality_profile"
                ]);
            }
            Ok(payload)
        }
        Action::HoleQuery => {
            let draft = subjects.draft.as_ref().ok_or_else(|| {
                failure("SPX-G232", "draft handle is stale, discarded, or unknown")
            })?;
            parse_payload(
                draft.hole_context(text(params, "draft_revision"), text(params, "hole_id"))?,
            )
        }
        _ => Err(failure(
            "SPX-G294",
            "operation is not a detached immutable candidate read",
        )),
    }
}
