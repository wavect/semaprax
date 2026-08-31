//! Typed body/expression/contract holes. Pending intentions never enter source.
use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{json, Value};

use super::{expression, parse_revision, wire, ProjectCandidate, SemanticChange};
use crate::diagnostic::Diagnostic;
use crate::hir::{IdentityOrigin, OwnershipMode, ResolvedFunction};
use crate::workspace_graph::WorkspaceGraphProjectionModule;

#[path = "draft_archive.rs"]
mod archive;
#[path = "draft_expression_catalog.rs"]
mod expression_catalog;
pub use archive::{
    ProjectCandidateDraftArchive, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES,
    PROJECT_CANDIDATE_DRAFT_ARCHIVE_COMPATIBILITY, PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,
};
pub use expression_catalog::{
    MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES, PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
};

#[path = "draft_recovery.rs"]
mod recovery;
pub use recovery::{
    MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES, PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY,
    PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
};

#[path = "draft_rebase.rs"]
mod rebase;
pub use rebase::{
    ProjectCandidateDraftRebase, MAX_PROJECT_CANDIDATE_DRAFT_REBASE_BYTES,
    PROJECT_CANDIDATE_DRAFT_REBASE_SCHEMA,
};

#[path = "draft_merge.rs"]
mod merge;
pub use merge::{
    ProjectCandidateDraftMerge, MAX_PROJECT_CANDIDATE_DRAFT_MERGE_BYTES,
    PROJECT_CANDIDATE_DRAFT_MERGE_SCHEMA,
};

pub const PROJECT_CANDIDATE_DRAFT_SCHEMA: &str = "semaprax.project-candidate-draft.v1";
pub const PROJECT_CANDIDATE_HOLE_CONTEXT_SCHEMA: &str =
    "semaprax.project-candidate-hole-context.v1";
pub const MAX_PROJECT_CANDIDATE_HOLES: usize = 16;
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_RENDER_BYTES: usize = 16 * 1024 * 1024;

/// Immutable pending body/expression holes over the last completely valid candidate.
/// There is deliberately no candidate/revision/source accessor. Only complete
/// may release the valid candidate after every pending hole has been filled.
pub struct ProjectCandidateDraft {
    last_valid: Arc<ProjectCandidate>,
    holes: BTreeMap<String, String>,
    expression_holes: BTreeMap<String, (String, String)>,
    contract_expression_holes: BTreeMap<String, (String, String)>,
    json: String,
    digest: String,
}

impl ProjectCandidateDraft {
    pub fn open(candidate: Arc<ProjectCandidate>) -> Result<Self, Vec<Diagnostic>> {
        Self::finish(candidate, BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
    }

    pub fn with_body_hole(
        &self,
        expected: &str,
        target: &str,
        hole_id: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        validate_id(hole_id)?;
        if self.pending_count() >= MAX_PROJECT_CANDIDATE_HOLES {
            return Err(capacity("candidate draft has too many pending holes"));
        }
        if self.holes.contains_key(hole_id)
            || self.expression_holes.contains_key(hole_id)
            || self.contract_expression_holes.contains_key(hole_id)
            || self.holes.values().any(|existing| existing == target)
            || self
                .expression_holes
                .values()
                .any(|(existing, _)| existing == target)
        {
            return Err(grammar("candidate draft hole ID and target must be unique"));
        }
        self.function(target)?;
        let mut holes = self.holes.clone();
        holes.insert(hole_id.to_owned(), target.to_owned());
        Self::finish(
            Arc::clone(&self.last_valid),
            holes,
            self.expression_holes.clone(),
            self.contract_expression_holes.clone(),
        )
    }

    /// Select an authored expression through its actual revision-scoped HIR ID.
    /// Overlapping holes reject; no caller-provided AST path or source is trusted.
    pub fn with_expression_hole(
        &self,
        expected: &str,
        target: &str,
        expression_id: &str,
        hole_id: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        validate_id(hole_id)?;
        if self.pending_count() >= MAX_PROJECT_CANDIDATE_HOLES {
            return Err(capacity("candidate draft has too many pending holes"));
        }
        if self.holes.contains_key(hole_id)
            || self.expression_holes.contains_key(hole_id)
            || self.contract_expression_holes.contains_key(hole_id)
            || self.holes.values().any(|existing| existing == target)
        {
            return Err(grammar(
                "candidate expression hole duplicates or overlaps a pending hole",
            ));
        }
        self.expression_fact(target, expression_id)?;
        let programs = parse_revision(self.last_valid.revision())?;
        let selection = expression::authored_selection(
            self.last_valid.revision(),
            &programs,
            target,
            expression_id,
        )?;
        for (other_target, other_id) in self.expression_holes.values() {
            if other_target == target {
                let other = expression::authored_selection(
                    self.last_valid.revision(),
                    &programs,
                    target,
                    other_id,
                )?;
                if selection.path.starts_with(&other.path)
                    || other.path.starts_with(&selection.path)
                {
                    return Err(grammar(
                        "candidate expression holes must select disjoint authored subtrees",
                    ));
                }
            }
        }
        let mut holes = self.expression_holes.clone();
        holes.insert(
            hole_id.to_owned(),
            (target.to_owned(), expression_id.to_owned()),
        );
        Self::finish(
            Arc::clone(&self.last_valid),
            self.holes.clone(),
            holes,
            self.contract_expression_holes.clone(),
        )
    }

    /// Select a unique authored pre/postcondition subtree. Contract and body
    /// regions are distinct; every selection still shares one bounded draft.
    pub fn with_contract_expression_hole(
        &self,
        expected: &str,
        target: &str,
        expression_id: &str,
        hole_id: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        validate_id(hole_id)?;
        if self.pending_count() >= MAX_PROJECT_CANDIDATE_HOLES {
            return Err(capacity("candidate draft has too many pending holes"));
        }
        if self.holes.contains_key(hole_id)
            || self.expression_holes.contains_key(hole_id)
            || self.contract_expression_holes.contains_key(hole_id)
        {
            return Err(grammar("candidate draft hole ID must be unique"));
        }
        self.contract_expression_fact(target, expression_id)?;
        let programs = parse_revision(self.last_valid.revision())?;
        let selection = expression::authored_contract_selection(
            self.last_valid.revision(),
            &programs,
            target,
            expression_id,
        )?;
        for (other_target, other_id) in self.contract_expression_holes.values() {
            if other_target == target {
                let other = expression::authored_contract_selection(
                    self.last_valid.revision(),
                    &programs,
                    target,
                    other_id,
                )?;
                if selection.phase == other.phase
                    && (selection.path.starts_with(&other.path)
                        || other.path.starts_with(&selection.path))
                {
                    return Err(grammar(
                        "candidate contract holes must select disjoint authored subtrees",
                    ));
                }
            }
        }
        let mut holes = self.contract_expression_holes.clone();
        holes.insert(
            hole_id.to_owned(),
            (target.to_owned(), expression_id.to_owned()),
        );
        Self::finish(
            Arc::clone(&self.last_valid),
            self.holes.clone(),
            self.expression_holes.clone(),
            holes,
        )
    }

    fn pending_count(&self) -> usize {
        self.holes.len() + self.expression_holes.len() + self.contract_expression_holes.len()
    }

    /// Context describes declarations and the last valid body's proof facts,
    /// not a fabricated HIR body or proof that an unfilled hole is valid.
    pub fn hole_context(&self, expected: &str, hole_id: &str) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if let Some((target, expression_id)) = self.contract_expression_holes.get(hole_id) {
            return self.expression_context_for(target, expression_id, hole_id, true);
        }
        if let Some((target, expression_id)) = self.expression_holes.get(hole_id) {
            return self.expression_context_for(target, expression_id, hole_id, false);
        }
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
        render(self.with_aggregate_context(module, json!({
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
            "constructor_kinds":["i64","i32","u8","usize","bool","place","call","unary","binary","if","let"],
            "validation":"pending_fill_full_source_replay", "materializable":false, "source_authority":false,
            "evidence_class":"descriptive_context_not_candidate_validation",
        }))?)
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
        wire::validate_value(expression)
            .map_err(|_| capacity("candidate hole expression exceeds its input bound"))?;
        let intent = if let Some((target, expression_id)) =
            self.contract_expression_holes.get(hole_id)
        {
            json!({"kind":"replace_contract_expression", "target":target, "expression_id":expression_id, "replacement":expression})
        } else if let Some((target, expression_id)) = self.expression_holes.get(hole_id) {
            json!({"kind":"replace_expression", "target":target, "expression_id":expression_id, "replacement":expression})
        } else {
            let target = self.target(hole_id)?;
            json!({"kind":"replace_function_body", "target":target, "body":expression})
        };
        let change = SemanticChange::new(self.last_valid.revision().project_revision(), &intent)?;
        let candidate = self
            .last_valid
            .apply(self.last_valid.candidate_digest(), &change)?;
        let mut holes = self.holes.clone();
        holes.remove(hole_id);
        let mut expression_holes = self.expression_holes.clone();
        expression_holes.remove(hole_id);
        for (target, expression_id) in expression_holes.values_mut() {
            *expression_id = expression::remap_selection(
                self.last_valid.revision(),
                candidate.revision(),
                target,
                expression_id,
            )?;
        }
        let mut contract_expression_holes = self.contract_expression_holes.clone();
        contract_expression_holes.remove(hole_id);
        for (target, expression_id) in contract_expression_holes.values_mut() {
            *expression_id = expression::remap_contract_selection(
                self.last_valid.revision(),
                candidate.revision(),
                target,
                expression_id,
            )?;
        }
        Self::finish(
            Arc::new(candidate),
            holes,
            expression_holes,
            contract_expression_holes,
        )
    }

    /// The sole escape to a materializable candidate is fail-closed while any
    /// unresolved hole remains. This still grants no filesystem authority.
    pub fn complete(&self, expected: &str) -> Result<Arc<ProjectCandidate>, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if self.pending_count() != 0 {
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
        expression_holes: BTreeMap<String, (String, String)>,
        contract_expression_holes: BTreeMap<String, (String, String)>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut draft = Self {
            last_valid: candidate,
            holes,
            expression_holes,
            contract_expression_holes,
            json: String::new(),
            digest: String::new(),
        };
        let mut pending = Vec::new();
        for (id, target) in &draft.holes {
            let (_, function) = draft.function(target)?;
            pending.push(json!({"hole_id":id, "target":target, "expected_type_id":function.return_type.identity_key(), "kind":"function_body", "state":"unresolved"}));
        }
        for (id, (target, expression_id)) in &draft.expression_holes {
            let (_, fact) = draft.expression_fact(target, expression_id)?;
            pending.push(
                json!({"hole_id":id, "target":target, "expression_id":expression_id,
                "expected_type_id":fact["expected_type"], "expected_ownership":fact["ownership"],
                "kind":"expression", "state":"unresolved"}),
            );
        }
        for (id, (target, expression_id)) in &draft.contract_expression_holes {
            let (_, fact) = draft.contract_expression_fact(target, expression_id)?;
            pending.push(
                json!({"hole_id":id, "target":target, "expression_id":expression_id,
                "expected_type_id":fact["expected_type"], "expected_ownership":fact["ownership"],
                "phase":fact["phase"], "kind":"contract_expression", "state":"unresolved"}),
            );
        }
        pending.sort_by(|left, right| left["hole_id"].as_str().cmp(&right["hole_id"].as_str()));
        draft.json = render(json!({
            "schema":PROJECT_CANDIDATE_DRAFT_SCHEMA,
            "last_valid_revision":draft.last_valid.revision().project_revision(),
            "last_valid_candidate_digest":draft.last_valid.candidate_digest(),
            "unresolved_holes":pending,
            "state":if draft.pending_count() == 0 {"ready_to_complete"} else {"incomplete"},
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
    fn expression_fact(
        &self,
        target: &str,
        expression_id: &str,
    ) -> Result<(Value, Value), Vec<Diagnostic>> {
        self.expression_fact_for(target, expression_id, false)
    }
    fn contract_expression_fact(
        &self,
        target: &str,
        expression_id: &str,
    ) -> Result<(Value, Value), Vec<Diagnostic>> {
        self.expression_fact_for(target, expression_id, true)
    }
    fn expression_fact_for(
        &self,
        target: &str,
        expression_id: &str,
        contract: bool,
    ) -> Result<(Value, Value), Vec<Diagnostic>> {
        if expression_id.is_empty() || expression_id.len() > 4096 || expression_id.contains('\0') {
            return Err(grammar(
                "candidate expression hole requires a bounded HIR identity",
            ));
        }
        let catalog = trusted_json(&if contract {
            self.last_valid.contract_expression_catalog(target)?
        } else {
            self.last_valid.expression_catalog(target)?
        })?;
        let mut facts = catalog["expressions"]
            .as_array()
            .ok_or_else(|| grammar("candidate expression catalogue is unavailable"))?
            .iter()
            .filter(|fact| fact["expression_id"] == expression_id);
        let fact = facts
            .next()
            .ok_or_else(|| grammar("candidate expression hole selector is unavailable"))?
            .clone();
        let phase_valid = if contract {
            fact["phase"] == "requires" || fact["phase"] == "ensures"
        } else {
            fact["phase"] == "body"
        };
        if facts.next().is_some() || fact["replaceable"] != true || !phase_valid {
            return Err(grammar(if contract {
                "candidate contract hole requires a uniquely authored contract selection"
            } else {
                "candidate expression hole requires a uniquely authored body selection"
            }));
        }
        Ok((catalog, fact))
    }
    fn expression_context_for(
        &self,
        target: &str,
        expression_id: &str,
        hole_id: &str,
        contract: bool,
    ) -> Result<String, Vec<Diagnostic>> {
        let (catalog, fact) = self.expression_fact_for(target, expression_id, contract)?;
        let (module, function) = self.resolved_function(target)?;
        let (prior, overflow) = crate::bounded_output::with_limit(MAX_RENDER_BYTES, || {
            let mut contracts = Vec::new();
            for (phase, expressions) in [
                ("requires", &function.requires),
                ("ensures", &function.ensures),
            ] {
                for expression in expressions {
                    let rendered = crate::graph::agent_contract_expr_json(expression)
                        .map_err(|error| vec![error])?;
                    contracts.push(json!({"phase":phase,"expression_id":expression.id.as_str(),"expression":trusted_json(&rendered)?}));
                }
            }
            Ok::<_, Vec<Diagnostic>>(json!({
                "basis":"last_valid_body_not_the_unfilled_hole",
                "contracts":contracts,
                "loan_plan":trusted_json(&crate::graph_loan::loan_plan_json(&function.loan_plan))?,
                "cleanup_plan":trusted_json(&crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan))?
            }))
        });
        if overflow {
            return Err(capacity(
                "candidate expression hole proof context exceeds its render bound",
            ));
        }
        let domain: &[u8] = if contract {
            b"semaprax.project-candidate-contract-expression-hole-handle.v1\0"
        } else {
            b"semaprax.project-candidate-expression-hole-handle.v1\0"
        };
        let schema = if contract {
            "semaprax.project-candidate-contract-expression-hole-context.v1"
        } else {
            "semaprax.project-candidate-expression-hole-context.v1"
        };
        let intent_kind = if contract {
            "replace_contract_expression"
        } else {
            "replace_expression"
        };
        let effect_policy = if contract {
            json!({"allowed":[],"forbidden":"all_effects_in_contract_predicates",
                "enclosing_declared_effects":function.effects,"module_permits":module.permits()})
        } else {
            json!({"allowed":function.effects,"forbidden":"all_undeclared_effects","module_permits":module.permits()})
        };
        let contracts_obligation = if contract {
            "preserve_unselected_contracts_and_predicate_order"
        } else {
            "preserve_declared_contracts"
        };
        let nonclaims = if contract {
            json!([
                "lexical_scope_is_not_owned_value_liveness",
                "prior_body_proofs_are_not_hole_validity",
                "no_placeholder_ast_or_source",
                "no_execution_or_publication_authority",
                "contract_replacement_may_change_valid_inputs_or_runtime_failures",
                "no_logical_implication_satisfaction_or_compatibility_proof"
            ])
        } else {
            json!([
                "lexical_scope_is_not_owned_value_liveness",
                "prior_body_proofs_are_not_hole_validity",
                "no_placeholder_ast_or_source",
                "no_execution_or_publication_authority"
            ])
        };
        let mut calls = self.accessible_calls(module, function)?;
        if contract {
            for call in &mut calls {
                // Lexical accessibility is unchanged; predicates have a pure
                // budget even when their enclosing function declares effects.
                let pure = call["effects"].as_array().is_some_and(Vec::is_empty);
                call["within_effect_budget"] = json!(pure);
            }
        }
        let handle = wire::digest(domain,
            render(json!({"draft":self.draft_digest(),"hole":hole_id,"target":target,"expression_id":expression_id}))?.as_bytes());
        render(
            self.with_aggregate_context(module, json!({"schema":schema,
            "draft_digest":self.draft_digest(),"hole_id":hole_id,"hole_handle":handle,
            "target":target,"expression_id":expression_id,"source":catalog["source"],
            "last_valid_revision":self.last_valid.revision().project_revision(),
            "expected_type_id":fact["expected_type"],"expected_ownership":fact["ownership"],
            "scope":fact["scope"],"selected_expression":fact,
            "effect_policy":effect_policy,
            "accessible_calls":calls,"prior_body_proof":prior?,
            "obligations":["preserve_selected_type_and_ownership",contracts_obligation,"revalidate_whole_function_ownership_loans_cleanup","no_new_effects_or_capabilities","preserve_project_profile_and_previously_admitted_core_targets"],
            "intent_kind":intent_kind,"constructor_owner":"semaprax.semantic-change.v1",
            "constructor_kinds":["i64","i32","u8","usize","bool","place","call","unary","binary","if","let"],
            "validation":"pending_fill_full_source_replay","materializable":false,"source_authority":false,
            "nonclaims":nonclaims}))?,
        )
    }

    fn with_aggregate_context(
        &self,
        module: &WorkspaceGraphProjectionModule,
        mut report: Value,
    ) -> Result<Value, Vec<Diagnostic>> {
        let source = self
            .last_valid
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == module.path())
            .ok_or_else(|| grammar("aggregate hole context source is unavailable"))?;
        let program = crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
        let builtins = super::intent::builtin_constructors(self.last_valid.revision(), &program)?;
        if !builtins.is_empty() {
            report["constructor_kinds"]
                .as_array_mut()
                .ok_or_else(|| grammar("builtin hole constructor inventory is unavailable"))?
                .push(json!("builtin_call"));
            report["builtin_calls"] = json!(builtins);
        }
        let aggregates =
            super::intent::aggregate_constructors(self.last_valid.revision(), &program)?;
        let projections =
            super::intent::aggregate_projections(self.last_valid.revision(), &program)?;
        let matches = super::intent::aggregate_matches(self.last_valid.revision(), &program)?;
        let updates = super::intent::aggregate_updates(self.last_valid.revision(), &program)?;
        if !aggregates.is_empty() {
            let kinds = report["constructor_kinds"]
                .as_array_mut()
                .ok_or_else(|| grammar("aggregate hole constructor inventory is unavailable"))?;
            for kind in ["record", "variant"] {
                if aggregates.iter().any(|item| item["kind"] == kind) {
                    kinds.push(json!(kind));
                }
            }
            report["aggregate_constructors"] = json!(aggregates);
        }
        if !projections.is_empty() {
            report["constructor_kinds"]
                .as_array_mut()
                .ok_or_else(|| grammar("projection hole constructor inventory is unavailable"))?
                .push(json!("project"));
            report["aggregate_projections"] = json!(projections);
        }
        if !matches.is_empty() {
            report["constructor_kinds"]
                .as_array_mut()
                .ok_or_else(|| grammar("match hole constructor inventory is unavailable"))?
                .push(json!("match"));
            report["aggregate_matches"] = json!(matches);
        }
        if !updates.is_empty() {
            report["constructor_kinds"]
                .as_array_mut()
                .ok_or_else(|| grammar("update hole constructor inventory is unavailable"))?
                .push(json!("update"));
            report["aggregate_updates"] = json!(updates);
        }
        Ok(report)
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
        self.resolved_function(target)
    }
    fn resolved_function(
        &self,
        target: &str,
    ) -> Result<(&WorkspaceGraphProjectionModule, &ResolvedFunction), Vec<Diagnostic>> {
        self.last_valid
            .revision()
            .semantic
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
