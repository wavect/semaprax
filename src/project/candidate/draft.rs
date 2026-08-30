//! Ephemeral typed body holes. Pending intentions never enter canonical source.
use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{wire, ProjectCandidate, SemanticChange};
use crate::diagnostic::Diagnostic;
use crate::hir::{IdentityOrigin, OwnershipMode, ResolvedFunction};
use crate::workspace_graph::WorkspaceGraphProjectionModule;

pub const PROJECT_CANDIDATE_DRAFT_SCHEMA: &str = "semaprax.project-candidate-draft.v1";
pub const PROJECT_CANDIDATE_HOLE_CONTEXT_SCHEMA: &str =
    "semaprax.project-candidate-hole-context.v1";
pub const MAX_PROJECT_CANDIDATE_HOLES: usize = 16;
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_RENDER_BYTES: usize = 16 * 1024 * 1024;

/// Immutable pending body holes over the last completely valid candidate.
/// There is deliberately no candidate/revision/source accessor. Only complete
/// may release the valid candidate after every pending hole has been filled.
pub struct ProjectCandidateDraft {
    last_valid: Arc<ProjectCandidate>,
    holes: BTreeMap<String, String>,
    json: String,
    digest: String,
}

impl ProjectCandidateDraft {
    pub fn open(candidate: Arc<ProjectCandidate>) -> Result<Self, Vec<Diagnostic>> {
        Self::finish(candidate, BTreeMap::new())
    }

    pub fn with_body_hole(
        &self,
        expected: &str,
        target: &str,
        hole_id: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        validate_id(hole_id)?;
        if self.holes.len() >= MAX_PROJECT_CANDIDATE_HOLES {
            return Err(capacity("candidate draft has too many pending holes"));
        }
        if self.holes.contains_key(hole_id)
            || self.holes.values().any(|existing| existing == target)
        {
            return Err(grammar("candidate draft hole ID and target must be unique"));
        }
        self.function(target)?;
        let mut holes = self.holes.clone();
        holes.insert(hole_id.to_owned(), target.to_owned());
        Self::finish(Arc::clone(&self.last_valid), holes)
    }

    /// Context describes declarations and the last valid body's proof facts,
    /// not a fabricated HIR body or proof that an unfilled hole is valid.
    pub fn hole_context(&self, expected: &str, hole_id: &str) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        let target = self.target(hole_id)?;
        let (module, function) = self.function(target)?;
        let mut contracts = Vec::new();
        for (phase, expressions) in [
            ("requires", &function.requires),
            ("ensures", &function.ensures),
        ] {
            for expression in expressions {
                let (rendered, overflow) =
                    crate::bounded_output::with_limit(MAX_RENDER_BYTES, || {
                        crate::graph::agent_contract_expr_json(expression)
                    });
                if overflow {
                    return Err(capacity(
                        "candidate hole contract context exceeds its render bound",
                    ));
                }
                contracts.push(json!({"phase":phase, "expression_id":expression.id.as_str(), "expression":trusted_json(&rendered.map_err(|diagnostic| vec![diagnostic])?)?}));
            }
        }
        let (cleanup, overflow) = crate::bounded_output::with_limit(MAX_RENDER_BYTES, || {
            crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan)
        });
        if overflow {
            return Err(capacity(
                "candidate hole cleanup context exceeds its render bound",
            ));
        }
        let (loans, overflow) = crate::bounded_output::with_limit(MAX_RENDER_BYTES, || {
            crate::graph_loan::loan_plan_json(&function.loan_plan)
        });
        if overflow {
            return Err(capacity(
                "candidate hole loan context exceeds its render bound",
            ));
        }
        let handle = wire::digest(
            b"semaprax.project-candidate-hole-handle.v1\0",
            render(json!({"draft":self.draft_digest(), "hole":hole_id, "target":target}))?
                .as_bytes(),
        );
        render(json!({
            "schema":PROJECT_CANDIDATE_HOLE_CONTEXT_SCHEMA, "draft_digest":self.draft_digest(),
            "hole_id":hole_id, "hole_handle":handle, "target":target,
            "last_valid_revision":self.last_valid.revision().project_revision(),
            "path":module.path(), "module":module.module(), "source_revision":module.source_revision(),
            "expected_type_id":function.return_type.identity_key(),
            "scope":function.params.iter().map(|param| json!({"id":param.id.as_str(),"name":param.name,"type_id":param.ty.identity_key(),"ownership":ownership(param.ownership)})).collect::<Vec<_>>(),
            "effect_policy":{"allowed":function.effects, "forbidden":"all_undeclared_effects", "module_permits":module.permits()},
            "contracts":contracts,
            "accessible_calls":self.accessible_calls(module, function)?,
            "prior_body_proof":{"basis":"last_valid_body_not_the_unfilled_hole", "loan_plan":trusted_json(&loans)?, "cleanup_plan":trusted_json(&cleanup)?},
            "obligations":["return_expected_type", "preserve_declared_contracts", "satisfy_parameter_ownership", "revalidate_loans_and_cleanup", "no_new_effects_or_capabilities", "preserve_project_profile_and_previously_admitted_core_targets"],
            "constructor_owner":"semaprax.semantic-change.v1", "intent_kind":"replace_function_body",
            "constructor_kinds":["i64","i32","u8","usize","bool","place","call","unary","binary","if"],
            "validation":"pending_fill_full_source_replay", "materializable":false, "source_authority":false,
            "evidence_class":"descriptive_context_not_candidate_validation",
        }))
    }

    /// Failed construction/admission leaves this draft and all sibling drafts
    /// unchanged. Successful fills are rebuilt through the existing candidate
    /// source/ownership/cleanup/profile/target validation path.
    pub fn fill_hole(
        &self,
        expected: &str,
        hole_id: &str,
        expression: &Value,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        let target = self.target(hole_id)?;
        wire::validate_value(expression)
            .map_err(|_| capacity("candidate hole expression exceeds its input bound"))?;
        let change = SemanticChange::new(
            self.last_valid.revision().project_revision(),
            &json!({"kind":"replace_function_body", "target":target, "body":expression}),
        )?;
        let candidate = self
            .last_valid
            .apply(self.last_valid.candidate_digest(), &change)?;
        let mut holes = self.holes.clone();
        holes.remove(hole_id);
        Self::finish(Arc::new(candidate), holes)
    }

    /// The sole escape to a materializable candidate is fail-closed while any
    /// unresolved hole remains. This still grants no filesystem authority.
    pub fn complete(&self, expected: &str) -> Result<Arc<ProjectCandidate>, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if !self.holes.is_empty() {
            return Err(stale("candidate draft contains unresolved holes"));
        }
        Ok(Arc::clone(&self.last_valid))
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn draft_digest(&self) -> &str {
        &self.digest
    }
    /// Conservative registry accounting includes the valid candidate retained
    /// behind an incomplete draft, even when another Arc also references it.
    pub(crate) fn retained_report_bytes(&self) -> usize {
        self.json
            .len()
            .saturating_add(self.last_valid.to_json().len())
    }
    pub fn summary(&self, expected: &str) -> Result<&str, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        Ok(self.to_json())
    }

    fn finish(
        candidate: Arc<ProjectCandidate>,
        holes: BTreeMap<String, String>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut draft = Self {
            last_valid: candidate,
            holes,
            json: String::new(),
            digest: String::new(),
        };
        let mut pending = Vec::new();
        for (id, target) in &draft.holes {
            let (_, function) = draft.function(target)?;
            pending.push(json!({"hole_id":id, "target":target, "expected_type_id":function.return_type.identity_key(), "kind":"function_body", "state":"unresolved"}));
        }
        draft.json = render(json!({
            "schema":PROJECT_CANDIDATE_DRAFT_SCHEMA,
            "last_valid_revision":draft.last_valid.revision().project_revision(),
            "last_valid_candidate_digest":draft.last_valid.candidate_digest(),
            "unresolved_holes":pending,
            "state":if draft.holes.is_empty() {"ready_to_complete"} else {"incomplete"},
            "materializable":false, "source_authority":false,
            "nonclaims":["last_valid_revision_is_not_the_incomplete_candidate", "no_placeholder_ast_or_source", "no_candidate_source_or_evidence_until_complete", "no_execution_or_commit_authority"],
        }))?;
        draft.digest = wire::digest(
            b"semaprax.project-candidate-draft.v1\0",
            draft.json.as_bytes(),
        );
        Ok(draft)
    }
    fn target(&self, id: &str) -> Result<&str, Vec<Diagnostic>> {
        validate_id(id)?;
        self.holes
            .get(id)
            .map(String::as_str)
            .ok_or_else(|| grammar("candidate draft hole is unavailable"))
    }
    fn require_digest(&self, expected: &str) -> Result<(), Vec<Diagnostic>> {
        if expected.len() > 71 {
            return Err(capacity("candidate draft digest exceeds its byte bound"));
        }
        if expected != self.digest {
            return Err(stale("candidate draft digest is stale or invalid"));
        }
        Ok(())
    }
    fn function(
        &self,
        target: &str,
    ) -> Result<(&WorkspaceGraphProjectionModule, &ResolvedFunction), Vec<Diagnostic>> {
        if target.len() > 4096 {
            return Err(capacity("candidate hole target exceeds its byte bound"));
        }
        if target.is_empty() || target.contains('\0') {
            return Err(grammar("candidate hole target is invalid"));
        }
        let semantic = &self.last_valid.revision().semantic;
        if semantic.rename_function(target).is_none_or(|function| {
            function.origin != IdentityOrigin::Explicit || function.name == "main"
        }) {
            return Err(grammar(
                "candidate body hole requires an explicit non-main function",
            ));
        }
        semantic
            .image_modules()
            .iter()
            .find_map(|module| {
                module
                    .functions()
                    .iter()
                    .find(|function| function.id.as_str() == target)
                    .map(|function| (module, function))
            })
            .ok_or_else(|| grammar("candidate body hole requires a monomorphic resolved function"))
    }
    fn accessible_calls(
        &self,
        module: &WorkspaceGraphProjectionModule,
        target: &ResolvedFunction,
    ) -> Result<Vec<Value>, Vec<Diagnostic>> {
        let semantic = &self.last_valid.revision().semantic;
        let mut bindings = BTreeMap::<String, Vec<String>>::new();
        for function in module.functions() {
            bindings
                .entry(function.id.as_str().to_owned())
                .or_default()
                .push(function.name.clone());
        }
        for edge in semantic.image_edges() {
            if edge.kind() == "function_import" && edge.caller_path() == module.path() {
                bindings
                    .entry(edge.target().to_owned())
                    .or_default()
                    .push(edge.alias().to_owned());
            }
        }
        if bindings.len() > 1024 {
            return Err(capacity(
                "candidate hole accessible-call inventory exceeds its bound",
            ));
        }
        let mut calls = Vec::new();
        for (id, names) in bindings {
            // The typed constructor requires exactly one source binding.
            if names.len() != 1 {
                continue;
            }
            let Some(callee) = semantic
                .image_modules()
                .iter()
                .flat_map(|module| module.functions())
                .find(|function| function.id.as_str() == id)
            else {
                continue;
            };
            calls.push(json!({"id":id, "binding":names[0], "return_type_id":callee.return_type.identity_key(), "parameters":callee.params.iter().map(|param| json!({"name":param.name,"type_id":param.ty.identity_key(),"ownership":ownership(param.ownership)})).collect::<Vec<_>>(), "effects":callee.effects, "within_effect_budget":callee.effects.iter().all(|effect| target.effects.contains(effect)), "basis":"existing_local_or_authenticated_import_binding", "admission":"requires_fill_revalidation"}));
        }
        Ok(calls)
    }
}

fn validate_id(id: &str) -> Result<(), Vec<Diagnostic>> {
    if id.len() > 128 {
        return Err(capacity("candidate hole ID exceeds its byte bound"));
    }
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(grammar(
            "candidate hole ID must contain bounded ASCII identifier characters",
        ));
    }
    Ok(())
}
fn render(value: Value) -> Result<String, Vec<Diagnostic>> {
    wire::render(value, MAX_REPORT_BYTES)
        .map_err(|_| capacity("candidate draft report exceeds its byte bound"))
}
fn trusted_json(text: &str) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(text).map_err(|_| grammar("candidate hole compiler projection is invalid"))
}
fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}
fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G230", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G231", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G232", message)]
}
