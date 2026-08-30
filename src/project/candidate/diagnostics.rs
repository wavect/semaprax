//! Failed intentions are diagnostic records, never checked semantic images.
use super::{wire, ProjectCandidate, SemanticChange};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedType;
use serde_json::{json, Value};
use std::sync::Arc;

pub const PROJECT_CANDIDATE_ATTEMPT_SCHEMA: &str = "semaprax.project-candidate-attempt.v1";
pub const PROJECT_CANDIDATE_REPAIR_CATALOG_SCHEMA: &str =
    "semaprax.project-candidate-repair-catalog.v1";
const MAX_REPORT_BYTES: usize = 2 * 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 256;
const MAX_DIAGNOSTIC_BYTES: usize = 1024 * 1024;
const ATTEMPT_DOMAIN: &[u8] = b"semaprax.project-candidate-attempt.v1\0";
const REPAIR_DOMAIN: &[u8] = b"semaprax.project-candidate-typed-repair.v1\0";

pub enum ProjectCandidateAttemptOutcome {
    Accepted(Arc<ProjectCandidate>),
    Rejected(Arc<ProjectCandidateAttempt>),
}

/// Retains only the verified predecessor, exact bounded change and diagnostics.
/// There is no revision/source/image/materialization accessor for a rejection.
pub struct ProjectCandidateAttempt {
    base: Arc<ProjectCandidate>,
    change: SemanticChange,
    diagnostics: Vec<Diagnostic>,
    json: String,
    digest: String,
}
impl ProjectCandidateAttempt {
    /// Stale bindings and structurally oversized requests remain outer errors.
    /// A bounded, canonical intention rejected by ordinary apply is retained.
    pub fn apply(
        base: Arc<ProjectCandidate>,
        expected_candidate: &str,
        intent: &Value,
    ) -> Result<ProjectCandidateAttemptOutcome, Vec<Diagnostic>> {
        base.require_candidate(expected_candidate)?;
        let change = SemanticChange::new(base.revision().project_revision(), intent)?;
        match base.apply(expected_candidate, &change) {
            Ok(candidate) => Ok(ProjectCandidateAttemptOutcome::Accepted(Arc::new(
                candidate,
            ))),
            Err(diagnostics) => Ok(ProjectCandidateAttemptOutcome::Rejected(Arc::new(
                Self::rejected(base, change, diagnostics)?,
            ))),
        }
    }
    pub fn attempt_digest(&self) -> &str {
        &self.digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn summary(&self, expected: &str) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        render(
            json!({"schema":"semaprax.project-candidate-attempt-summary.v1","attempt_revision":self.digest,"base_candidate_revision":self.base.candidate_digest(),"base_project_revision":self.base.revision().project_revision(),"state":"rejected","diagnostic_count":self.diagnostics.len(),"report_bytes":self.json.len(),"materializable":false,"checked_image":false,"source_authority":false}),
        )
    }
    /// Conservative serialized report accounting includes the private base.
    /// Shared Arc reports may be counted twice; this is not a HIR memory bound.
    pub fn retained_report_bytes(&self) -> usize {
        self.json.len().saturating_add(self.base.to_json().len())
    }
    pub fn repair_catalog(&self, expected: &str) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        let (proposal, reason) = self.repair()?;
        render(
            json!({"schema":PROJECT_CANDIDATE_REPAIR_CATALOG_SCHEMA,"attempt_revision":self.digest,"base_candidate_revision":self.base.candidate_digest(),"base_project_revision":self.base.revision().project_revision(),"repairs":proposal.as_ref().map(|proposal|vec![proposal.description.clone()]).unwrap_or_default(),"availability_reason":reason,"legacy_identity_repair":"assign_function_id_is_a_breaking_identity_rebase_and_not_a_stable_identity_preserving_candidate_change","tests":"not_run","source_authority":false,"nonclaims":["not_general_diagnostic_repair","no_invalid_source_or_hir_admission","no_automatic_repair_selection","no_repair_diagnostic_semantic_change_wire_kind"]}),
        )
    }
    /// A selector requests one exact compiler-derived proposal. Replay ordinary
    /// full candidate admission afresh; the rejected record is never modified.
    pub fn repair_diagnostic(
        &self,
        expected: &str,
        repair_id: &str,
    ) -> Result<Arc<ProjectCandidate>, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if repair_id.len() != 71 {
            return Err(grammar("typed repair selector must be a SHA-256 digest"));
        }
        wire::validate_digest(repair_id)?;
        let (proposal, _) = self.repair()?;
        let proposal =
            proposal.ok_or_else(|| stale("no compiler-admitted typed repair is available"))?;
        if proposal.id != repair_id {
            return Err(stale("typed repair selector is stale or unknown"));
        }
        Ok(proposal.candidate)
    }
    fn rejected(
        base: Arc<ProjectCandidate>,
        change: SemanticChange,
        diagnostics: Vec<Diagnostic>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if diagnostics.is_empty() || diagnostics.len() > MAX_DIAGNOSTICS {
            return Err(capacity(
                "rejected attempt diagnostic count exceeds its bounds",
            ));
        }
        let mut diagnostic_bytes = 0usize;
        let mut records = Vec::with_capacity(diagnostics.len());
        for (index, diagnostic) in diagnostics.iter().enumerate() {
            diagnostic_bytes = diagnostic_bytes
                .saturating_add(diagnostic.code.len())
                .saturating_add(diagnostic.message.len())
                .saturating_add(diagnostic.path.as_ref().map_or(0, String::len))
                .saturating_add(diagnostic.help.as_ref().map_or(0, String::len));
            if diagnostic_bytes > MAX_DIAGNOSTIC_BYTES {
                return Err(capacity(
                    "rejected attempt diagnostic text exceeds its bound",
                ));
            }
            records.push(json!({"index":index,"code":diagnostic.code,"severity":diagnostic.severity.as_str(),"message":diagnostic.message,"path":diagnostic.path,"span":diagnostic.span.map(location),"help":diagnostic.help,"location_basis":"uncommitted_attempt_or_constructor_input_not_authenticated_base_span"}));
        }
        let target = change
            .intent
            .get("target")
            .and_then(Value::as_str)
            .filter(|target| target.len() <= 4096);
        let provenance = target.and_then(|target| {
            let symbol=base.revision().semantic.image_symbol(target)?;
            let source=base.revision().sources().iter().find(|source|Some(source.path())==symbol["path"].as_str())?;
            Some(json!({"id":target,"kind":symbol["kind"],"identity_origin":symbol["identity_origin"],"owner":symbol["owner"],"path":source.path(),"module":symbol["module"],"source_revision":source.source_revision(),"source_digest":source.source_digest(),"evidence_owner":"retained_verified_predecessor_semantic_index"}))
        });
        let change_value: Value = serde_json::from_str(change.to_json())
            .map_err(|_| grammar("retained change serialization is invalid"))?;
        let json = render(
            json!({"schema":PROJECT_CANDIDATE_ATTEMPT_SCHEMA,"base_candidate_revision":base.candidate_digest(),"base_project_revision":base.revision().project_revision(),"state":"rejected","change":change_value,"target_provenance":provenance,"diagnostics":records,"materializable":false,"checked_image":false,"source_authority":false,"tests":"not_run","nonclaims":["no_invalid_source_or_hir_retained","diagnostic_spans_do_not_identify_verified_base_expressions","no_automatic_repair_or_authority"]}),
        )?;
        let digest = wire::digest(ATTEMPT_DOMAIN, json.as_bytes());
        Ok(Self {
            base,
            change,
            diagnostics,
            json,
            digest,
        })
    }
    fn require_digest(&self, expected: &str) -> Result<(), Vec<Diagnostic>> {
        wire::validate_digest(expected)?;
        if expected != self.digest {
            return Err(stale("candidate attempt revision is stale"));
        }
        Ok(())
    }
    fn repair(&self) -> Result<(Option<Proposal>, &'static str), Vec<Diagnostic>> {
        let intent = &self.change.intent;
        let Some(object) = intent.as_object() else {
            return Ok((None, "no_supported_repair_class"));
        };
        if object.len() != 3 || intent["kind"] != "replace_function_body" {
            return Ok((None, "no_supported_repair_class"));
        }
        let Some(target) = intent["target"].as_str() else {
            return Ok((None, "no_supported_repair_class"));
        };
        let Some(body) = intent["body"].as_object() else {
            return Ok((None, "no_supported_repair_class"));
        };
        if body.len() != 2 {
            return Ok((None, "no_supported_repair_class"));
        }
        let Some(from) = body.get("kind").and_then(Value::as_str) else {
            return Ok((None, "no_supported_repair_class"));
        };
        let Some(value) = body.get("value") else {
            return Ok((None, "no_supported_repair_class"));
        };
        if !integer_fits(from, value) {
            return Ok((None, "requires_an_exact_supported_integer_literal"));
        }
        let Some(function) = self
            .base
            .revision()
            .semantic
            .image_modules()
            .iter()
            .flat_map(|module| module.functions())
            .find(|function| function.id.as_str() == target)
        else {
            return Ok((None, "target_has_no_retained_function_signature"));
        };
        let expected = match function.return_type {
            ResolvedType::I64 => "i64",
            ResolvedType::I32 => "i32",
            ResolvedType::U8 => "u8",
            ResolvedType::Usize => "usize",
            _ => return Ok((None, "target_return_type_has_no_supported_integer_repair")),
        };
        if expected == from {
            return Ok((None, "literal_already_matches_retained_return_type"));
        }
        if !integer_fits(expected, value) {
            return Ok((None, "integer_value_does_not_fit_retained_return_type"));
        }
        let change = SemanticChange::new(
            self.base.revision().project_revision(),
            &json!({"kind":"replace_function_body","target":target,"body":{"kind":expected,"value":value}}),
        )?;
        let candidate = match self.base.apply(self.base.candidate_digest(), &change) {
            Ok(candidate) => Arc::new(candidate),
            Err(_) => return Ok((None, "derived_change_failed_full_candidate_admission")),
        };
        let identity = render(
            json!({"attempt_revision":self.digest,"class":"retag_integer_literal_to_retained_return_type","change":serde_json::from_str::<Value>(change.to_json()).map_err(|_|grammar("derived repair change is invalid"))?}),
        )?;
        let id = wire::digest(REPAIR_DOMAIN, identity.as_bytes());
        let description = json!({"repair_id":id,"class":"retag_integer_literal_to_retained_return_type","target":target,"from_type":from,"expected_type":expected,"preserved_integer_value":value,"change":serde_json::from_str::<Value>(change.to_json()).map_err(|_|grammar("derived repair change is invalid"))?,"validated_candidate_revision":candidate.candidate_digest(),"validation":"normal_full_candidate_apply","evidence_owner":"retained_target_return_type_and_full_candidate_admission","tests":"not_run","source_authority":false});
        Ok((
            Some(Proposal {
                id,
                description,
                candidate,
            }),
            "one_compiler_admitted_typed_repair",
        ))
    }
}
struct Proposal {
    id: String,
    description: Value,
    candidate: Arc<ProjectCandidate>,
}
fn integer_fits(kind: &str, value: &Value) -> bool {
    match kind {
        "i64" => value.as_i64().is_some(),
        "i32" => value
            .as_i64()
            .is_some_and(|value| i32::try_from(value).is_ok()),
        "u8" => value
            .as_u64()
            .is_some_and(|value| u8::try_from(value).is_ok()),
        "usize" => value.as_u64().is_some(),
        _ => false,
    }
}
fn location(span: crate::ast::Span) -> Value {
    json!({"start":span.start,"end":span.end,"line":span.line,"column":span.column})
}
fn render(value: Value) -> Result<String, Vec<Diagnostic>> {
    wire::render(value, MAX_REPORT_BYTES)
        .map_err(|_| capacity("candidate attempt report exceeds its byte bound"))
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G241", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G242", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G243", message)]
}
