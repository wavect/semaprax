//! Additive Universal Semantic Transaction v2.
//!
//! V2 admits exactly one body-expression replacement selected by a
//! revision-scoped HIR expression identity. It deliberately has distinct wire
//! schemas and digest domains from the frozen v1 transaction kernel.

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{
    ExactProgramContext, ExactProgramContextV2, ProgramRoot, ProgramRootV2, ProgramRootV3,
    ProjectCandidate, ProjectRevision, SemanticChange, SEMANTIC_CHANGE_REQUIREMENTS,
};

pub const SEMANTIC_TRANSACTION_V2_SCHEMA: &str = "semaprax.semantic-transaction.v2";
pub const SEMANTIC_TRANSACTION_V2_IMPACT_SCHEMA: &str = "semaprax.semantic-transaction-impact.v2";
pub const SEMANTIC_TRANSACTION_V2_REVIEW_SCHEMA: &str = "semaprax.semantic-transaction-review.v2";
pub const SEMANTIC_TRANSACTION_V2_RESULT_SCHEMA: &str = "semaprax.semantic-transaction-result.v2";
pub const SEMANTIC_TRANSACTION_V2_EVIDENCE_SCHEMA: &str =
    "semaprax.semantic-transaction-evidence.v2";
pub const MAX_SEMANTIC_TRANSACTION_V2_BYTES: usize = 1024 * 1024;
pub const MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES: usize = 96 * 1024 * 1024;

const INTENT_DOMAIN: &[u8] = b"semaprax.semantic-transaction.intent.digest.v2\0";
const IMPACT_DOMAIN: &[u8] = b"semaprax.semantic-transaction.impact.digest.v2\0";
const REVIEW_DOMAIN: &[u8] = b"semaprax.semantic-transaction.review.digest.v2\0";
const RESULT_DOMAIN: &[u8] = b"semaprax.semantic-transaction.result.digest.v2\0";
const VALIDATION: &[&str] = &[
    "canonical_source_round_trip",
    "complete_project_admission",
    "ownership_and_cleanup",
    "native_and_wasm_emission",
    "canonical_workspace_revision",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransactionReplaceExpression {
    target: String,
    expression_id: String,
    expected_old_expression: String,
    replacement: Value,
}

impl SemanticTransactionReplaceExpression {
    pub fn new(
        target: impl Into<String>,
        expression_id: impl Into<String>,
        expected_old_expression: impl Into<String>,
        replacement: Value,
    ) -> Self {
        Self {
            target: target.into(),
            expression_id: expression_id.into(),
            expected_old_expression: expected_old_expression.into(),
            replacement,
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn expression_id(&self) -> &str {
        &self.expression_id
    }
    pub fn expected_old_expression(&self) -> &str {
        &self.expected_old_expression
    }
    pub fn replacement(&self) -> &Value {
        &self.replacement
    }

    fn value(&self) -> Value {
        json!({
            "expected_old_expression": self.expected_old_expression,
            "expression_id": self.expression_id,
            "kind": "replace_expression",
            "replacement": self.replacement,
            "target": self.target,
        })
    }
}

/// One authority-free ReplaceExpression intention over one exact workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticTransactionV2 {
    expected_workspace_revision: String,
    operation: SemanticTransactionReplaceExpression,
    json: String,
    digest: String,
}

impl SemanticTransactionV2 {
    pub fn replace_expression(
        expected_workspace_revision: &str,
        operation: SemanticTransactionReplaceExpression,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_workspace_revision)?;
        validate_text(&operation.target, 4096, "transaction target is not bounded")?;
        validate_text(
            &operation.expression_id,
            16_384,
            "ReplaceExpression expression identity is not bounded",
        )?;
        validate_text(
            &operation.expected_old_expression,
            MAX_SEMANTIC_TRANSACTION_V2_BYTES / 2,
            "ReplaceExpression expected old expression is not bounded",
        )?;
        super::candidate::validate_transaction_value(&operation.replacement).map_err(|_| {
            capacity("semantic transaction v2 replacement exceeds its node, depth, or string limit")
        })?;
        if operation
            .replacement
            .get("kind")
            .and_then(Value::as_str)
            .is_none()
        {
            return Err(invalid(
                "ReplaceExpression v2 requires a typed expression replacement",
            ));
        }
        let json = render(
            json!({
                "expected_workspace_revision": expected_workspace_revision,
                "invariants": SEMANTIC_CHANGE_REQUIREMENTS,
                "operations": [operation.value()],
                "requested_authority": "none",
                "requested_validation": VALIDATION,
                "schema": SEMANTIC_TRANSACTION_V2_SCHEMA,
            }),
            MAX_SEMANTIC_TRANSACTION_V2_BYTES,
        )?;
        let digest = digest(INTENT_DOMAIN, json.as_bytes());
        Ok(Self {
            expected_workspace_revision: expected_workspace_revision.to_owned(),
            operation,
            json,
            digest,
        })
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_SEMANTIC_TRANSACTION_V2_BYTES {
            return Err(capacity("semantic transaction v2 exceeds its byte limit"));
        }
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("semantic transaction v2 is not valid JSON"))?;
        super::candidate::validate_transaction_value(&value).map_err(|_| {
            capacity("semantic transaction v2 input exceeds its node, depth, or string limit")
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| invalid("semantic transaction v2 is not an object"))?;
        let keys = [
            "expected_workspace_revision",
            "invariants",
            "operations",
            "requested_authority",
            "requested_validation",
            "schema",
        ];
        if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
            return Err(invalid("semantic transaction v2 has an invalid field set"));
        }
        let operations = value["operations"]
            .as_array()
            .filter(|items| items.len() == 1)
            .ok_or_else(|| invalid("semantic transaction v2 requires exactly one operation"))?;
        let operation = operations[0]
            .as_object()
            .ok_or_else(|| invalid("semantic transaction v2 operation is not an object"))?;
        let operation_keys = [
            "expected_old_expression",
            "expression_id",
            "kind",
            "replacement",
            "target",
        ];
        if operation.len() != operation_keys.len()
            || operation_keys
                .iter()
                .any(|key| !operation.contains_key(*key))
            || operations[0]["kind"] != "replace_expression"
        {
            return Err(invalid(
                "semantic transaction v2 ReplaceExpression field set is invalid",
            ));
        }
        let text = |key: &str| {
            operations[0][key]
                .as_str()
                .ok_or_else(|| invalid("semantic transaction v2 operation text is invalid"))
        };
        let revision = value["expected_workspace_revision"]
            .as_str()
            .ok_or_else(|| invalid("semantic transaction v2 expected revision is invalid"))?;
        let transaction = Self::replace_expression(
            revision,
            SemanticTransactionReplaceExpression::new(
                text("target")?,
                text("expression_id")?,
                text("expected_old_expression")?,
                operations[0]["replacement"].clone(),
            ),
        )?;
        if transaction.json.as_bytes() != bytes {
            return Err(invalid(
                "semantic transaction is not the exact canonical v2 envelope",
            ));
        }
        Ok(transaction)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, Vec<Diagnostic>> {
        Self::from_json(bytes)
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn intent(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
    pub fn expected_workspace_revision(&self) -> &str {
        &self.expected_workspace_revision
    }
    pub fn operation(&self) -> &SemanticTransactionReplaceExpression {
        &self.operation
    }

    pub fn validate(
        &self,
        base: Arc<ProjectRevision>,
    ) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
        let base_workspace = base.canonical_workspace_revision()?;
        let base_program_root = base_workspace.program_root()?;
        if base_program_root.workspace_revision() != self.expected_workspace_revision {
            return Err(stale(
                "semantic transaction v2 expected workspace revision is stale",
            ));
        }
        require_canonical_comment_free_sources(&base)?;
        require_function_target(&base, &self.operation.target)?;

        let initial = ProjectCandidate::open(Arc::clone(&base), base.project_revision())?;
        let old = select_expression(
            &base,
            &initial,
            &self.operation.target,
            &self.operation.expression_id,
        )?;
        if old.phase != "body" {
            return Err(invalid(
                "ReplaceExpression v2 accepts body expressions only",
            ));
        }
        if !old.replaceable {
            return Err(invalid(
                "ReplaceExpression v2 requires one replaceable authored expression",
            ));
        }
        if old.source != self.operation.expected_old_expression {
            return Err(stale(
                "ReplaceExpression expected old expression does not match the exact base",
            ));
        }

        let change = SemanticChange::new(
            base.project_revision(),
            &json!({
                "expression_id": self.operation.expression_id,
                "kind": "replace_expression",
                "replacement": self.operation.replacement,
                "target": self.operation.target,
            }),
        )?;
        let candidate = initial.apply(initial.candidate_digest(), &change)?;
        require_canonical_comment_free_sources(candidate.revision())?;
        let replacement = require_source_preserving_expression_replacement(
            &base,
            candidate.revision(),
            &candidate,
            &self.operation,
            &old,
        )?;
        let candidate_workspace = candidate.revision().canonical_workspace_revision()?;
        let candidate_program_root = candidate_workspace.program_root()?;
        if base_workspace.manifest_digest() != candidate_workspace.manifest_digest()
            || base_workspace.dependency_lock_digest()
                != candidate_workspace.dependency_lock_digest()
            || base_workspace.authority_policies() != candidate_workspace.authority_policies()
            || base_workspace.target_profiles() != candidate_workspace.target_profiles()
        {
            return Err(stale(
                "semantic transaction v2 changed manifest, dependency, authority, or target facts",
            ));
        }
        let source_review_text = candidate.source_review(candidate.candidate_digest())?;
        let source_review: Value = serde_json::from_str(&source_review_text)
            .map_err(|_| invalid("candidate source review is not valid JSON"))?;

        let impact = render(
            json!({
                "base_workspace_revision": base_program_root.workspace_revision(),
                "candidate_workspace_revision": candidate_program_root.workspace_revision(),
                "classification": "descriptive_compiler_projection",
                "expression": {
                    "after": replacement.new_expression,
                    "before": self.operation.expected_old_expression,
                    "new_expression_id": replacement.new_expression_id,
                    "old_expression_id": self.operation.expression_id,
                    "source_outside_expression_preserved": true,
                    "source_path": replacement.path,
                },
                "identity": {"preserved": true, "target": self.operation.target},
                "nonclaims": ["not_behavioral_equivalence", "not_runtime_execution", "no_authority"],
                "schema": SEMANTIC_TRANSACTION_V2_IMPACT_SCHEMA,
                "transaction_digest": self.digest,
            }),
            MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES,
        )?;
        let impact_digest = digest(IMPACT_DOMAIN, impact.as_bytes());
        let review = render(
            json!({
                "authority": {"granted": false, "requested": "none"},
                "impact_digest": impact_digest,
                "review": {
                    "body_expression_only": true,
                    "candidate_rebuilt_from_canonical_source": true,
                    "exact_old_expression_precondition": true,
                    "revision_scoped_expression_identity": true,
                    "source_outside_selected_expression_preserved": true,
                    "stable_function_identity_preserved": true,
                    "trivia_preservation": "exact_outside_authenticated_expression_span",
                },
                "schema": SEMANTIC_TRANSACTION_V2_REVIEW_SCHEMA,
                "transaction_digest": self.digest,
            }),
            MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES,
        )?;
        let review_digest = digest(REVIEW_DOMAIN, review.as_bytes());
        let candidate_value: Value = serde_json::from_str(candidate.to_json())
            .map_err(|_| invalid("candidate evidence is not valid JSON"))?;
        let result = render(
            json!({
                "authority": {"commit_performed": false, "granted": false},
                "base": {
                    "project_revision": base.project_revision(),
                    "workspace_revision": base_program_root.workspace_revision(),
                },
                "candidate": {
                    "evidence": candidate_value,
                    "project_revision": candidate.revision().project_revision(),
                    "revision": candidate.candidate_digest(),
                    "workspace_revision": candidate_program_root.workspace_revision(),
                },
                "operation_results": [{
                    "expression_id": self.operation.expression_id,
                    "kind": "replace_expression",
                    "new_expression": replacement.new_expression,
                    "new_expression_id": replacement.new_expression_id,
                    "old_expression": self.operation.expected_old_expression,
                    "outcome": "validated",
                    "target": self.operation.target,
                }],
                "schema": SEMANTIC_TRANSACTION_V2_RESULT_SCHEMA,
                "source_review": source_review,
                "transaction_digest": self.digest,
                "validation": VALIDATION.iter().map(|name| ((*name).to_owned(), Value::Bool(true))).collect::<serde_json::Map<String, Value>>(),
            }),
            MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES,
        )?;
        let result_digest = digest(RESULT_DOMAIN, result.as_bytes());
        let evidence = render(
            json!({
                "artifacts": {
                    "impact": {"digest": impact_digest, "value": parse_value(&impact)?},
                    "intent": {"digest": self.digest, "value": parse_value(&self.json)?},
                    "result": {"digest": result_digest, "value": parse_value(&result)?},
                    "review": {"digest": review_digest, "value": parse_value(&review)?},
                },
                "authority": false,
                "schema": SEMANTIC_TRANSACTION_V2_EVIDENCE_SCHEMA,
            }),
            MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES,
        )?;

        Ok(SemanticTransactionArtifactsV2 {
            candidate,
            base_program_root,
            candidate_program_root,
            impact,
            impact_digest,
            review,
            review_digest,
            result,
            result_digest,
            evidence,
            base_program_root_v2: None,
            base_program_root_v3: None,
        })
    }

    pub fn replay(
        base: Arc<ProjectRevision>,
        transaction_bytes: &[u8],
        evidence_bytes: &[u8],
    ) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
        if evidence_bytes.len() > MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES {
            return Err(capacity(
                "semantic transaction v2 evidence exceeds its byte limit",
            ));
        }
        let transaction = Self::from_json(transaction_bytes)?;
        let artifacts = transaction.validate(base)?;
        if artifacts.evidence.as_bytes() != evidence_bytes {
            return Err(stale(
                "semantic transaction v2 evidence failed exact replay",
            ));
        }
        Ok(artifacts)
    }

    pub fn validate_exact(
        &self,
        context: Arc<ExactProgramContext>,
        expected_workspace_revision: &str,
        expected_program_root_v2_digest: &str,
    ) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
        let program_root_v2 = context
            .select(expected_workspace_revision, expected_program_root_v2_digest)?
            .clone();
        let mut artifacts = self.validate(Arc::clone(context.revision()))?;
        artifacts.base_program_root_v2 = Some(program_root_v2);
        Ok(artifacts)
    }

    pub fn replay_exact(
        context: Arc<ExactProgramContext>,
        expected_workspace_revision: &str,
        expected_program_root_v2_digest: &str,
        transaction_bytes: &[u8],
        evidence_bytes: &[u8],
    ) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
        context.select(expected_workspace_revision, expected_program_root_v2_digest)?;
        if evidence_bytes.len() > MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES {
            return Err(capacity(
                "semantic transaction v2 evidence exceeds its byte limit",
            ));
        }
        let transaction = Self::from_json(transaction_bytes)?;
        let artifacts = transaction.validate_exact(
            context,
            expected_workspace_revision,
            expected_program_root_v2_digest,
        )?;
        if artifacts.evidence.as_bytes() != evidence_bytes {
            return Err(stale(
                "semantic transaction v2 evidence failed exact replay",
            ));
        }
        Ok(artifacts)
    }

    pub fn validate_exact_v2(
        &self,
        context: Arc<ExactProgramContextV2>,
        expected_workspace_revision: &str,
        expected_program_root_v3_digest: &str,
    ) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
        let program_root_v3 = context
            .select(expected_workspace_revision, expected_program_root_v3_digest)?
            .clone();
        let mut artifacts =
            self.validate(Arc::clone(context.exact_program_context_v1().revision()))?;
        artifacts.base_program_root_v2 = Some(context.program_root_v2().clone());
        artifacts.base_program_root_v3 = Some(program_root_v3);
        Ok(artifacts)
    }

    pub fn replay_exact_v2(
        context: Arc<ExactProgramContextV2>,
        expected_workspace_revision: &str,
        expected_program_root_v3_digest: &str,
        transaction_bytes: &[u8],
        evidence_bytes: &[u8],
    ) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
        context.select(expected_workspace_revision, expected_program_root_v3_digest)?;
        if evidence_bytes.len() > MAX_SEMANTIC_TRANSACTION_V2_ARTIFACT_BYTES {
            return Err(capacity(
                "semantic transaction v2 evidence exceeds its byte limit",
            ));
        }
        let transaction = Self::from_json(transaction_bytes)?;
        let artifacts = transaction.validate_exact_v2(
            context,
            expected_workspace_revision,
            expected_program_root_v3_digest,
        )?;
        if artifacts.evidence.as_bytes() != evidence_bytes {
            return Err(stale(
                "semantic transaction v2 evidence failed exact replay",
            ));
        }
        Ok(artifacts)
    }
}

pub fn validate_semantic_transaction_v2(
    base: &Arc<ProjectRevision>,
    transaction: &SemanticTransactionV2,
) -> Result<SemanticTransactionArtifactsV2, Vec<Diagnostic>> {
    transaction.validate(Arc::clone(base))
}

pub struct SemanticTransactionArtifactsV2 {
    candidate: ProjectCandidate,
    base_program_root: ProgramRoot,
    candidate_program_root: ProgramRoot,
    impact: String,
    impact_digest: String,
    review: String,
    review_digest: String,
    result: String,
    result_digest: String,
    evidence: String,
    base_program_root_v2: Option<ProgramRootV2>,
    base_program_root_v3: Option<ProgramRootV3>,
}

impl SemanticTransactionArtifactsV2 {
    pub fn candidate(&self) -> &ProjectCandidate {
        &self.candidate
    }
    pub fn base_program_root(&self) -> &ProgramRoot {
        &self.base_program_root
    }
    pub fn candidate_program_root(&self) -> &ProgramRoot {
        &self.candidate_program_root
    }
    pub fn base_program_root_v2(&self) -> Option<&ProgramRootV2> {
        self.base_program_root_v2.as_ref()
    }
    pub fn base_program_root_v3(&self) -> Option<&ProgramRootV3> {
        self.base_program_root_v3.as_ref()
    }
    pub fn impact(&self) -> &str {
        &self.impact
    }
    pub fn impact_digest(&self) -> &str {
        &self.impact_digest
    }
    pub fn review(&self) -> &str {
        &self.review
    }
    pub fn review_digest(&self) -> &str {
        &self.review_digest
    }
    pub fn result(&self) -> &str {
        &self.result
    }
    pub fn result_digest(&self) -> &str {
        &self.result_digest
    }
    pub fn evidence(&self) -> &str {
        &self.evidence
    }
}

struct ExpressionSelection {
    path: String,
    phase: String,
    start: usize,
    end: usize,
    source: String,
    replaceable: bool,
}

struct ExpressionReplacement {
    path: String,
    new_expression_id: String,
    new_expression: String,
}

fn select_expression(
    revision: &ProjectRevision,
    candidate: &ProjectCandidate,
    target: &str,
    expression_id: &str,
) -> Result<ExpressionSelection, Vec<Diagnostic>> {
    let catalog: Value = serde_json::from_str(&candidate.expression_catalog(target)?)
        .map_err(|_| invalid("expression catalog is not valid JSON"))?;
    let path = catalog["source"]["path"]
        .as_str()
        .ok_or_else(|| invalid("expression catalog source path is unavailable"))?;
    let rows = catalog["expressions"]
        .as_array()
        .ok_or_else(|| invalid("expression catalog inventory is unavailable"))?;
    let mut matches = rows
        .iter()
        .filter(|row| row["expression_id"] == expression_id);
    let row = matches
        .next()
        .ok_or_else(|| stale("ReplaceExpression expression identity is stale"))?;
    if matches.next().is_some() {
        return Err(invalid(
            "ReplaceExpression expression identity is ambiguous",
        ));
    }
    let phase = row["phase"]
        .as_str()
        .ok_or_else(|| invalid("expression catalog phase is unavailable"))?;
    let start = usize_field(&row["source_span"], "start")?;
    let end = usize_field(&row["source_span"], "end")?;
    if start >= end {
        return Err(invalid("expression catalog span is empty or reversed"));
    }
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .ok_or_else(|| stale("expression source owner is unavailable"))?
        .source()
        .get(start..end)
        .ok_or_else(|| stale("expression source span is unavailable"))?
        .to_owned();
    Ok(ExpressionSelection {
        path: path.to_owned(),
        phase: phase.to_owned(),
        start,
        end,
        source,
        replaceable: row["replaceable"] == true,
    })
}

fn require_source_preserving_expression_replacement(
    before: &ProjectRevision,
    after: &ProjectRevision,
    candidate: &ProjectCandidate,
    operation: &SemanticTransactionReplaceExpression,
    old: &ExpressionSelection,
) -> Result<ExpressionReplacement, Vec<Diagnostic>> {
    if before.sources().len() != after.sources().len() {
        return Err(stale("ReplaceExpression changed the source inventory"));
    }
    for base_source in before.sources() {
        let candidate_source = after
            .sources()
            .iter()
            .find(|source| source.path() == base_source.path())
            .ok_or_else(|| stale("ReplaceExpression changed the source inventory"))?;
        if base_source.path() != old.path {
            if base_source.source() != candidate_source.source() {
                return Err(stale("ReplaceExpression changed an unrelated source"));
            }
            continue;
        }
        let prefix = base_source
            .source()
            .get(..old.start)
            .ok_or_else(|| stale("ReplaceExpression base prefix is unavailable"))?;
        let suffix = base_source
            .source()
            .get(old.end..)
            .ok_or_else(|| stale("ReplaceExpression base suffix is unavailable"))?;
        let unchanged_bytes = prefix
            .len()
            .checked_add(suffix.len())
            .ok_or_else(|| stale("ReplaceExpression source span size overflow"))?;
        if old.start >= old.end
            || !candidate_source.source().is_char_boundary(old.start)
            || !candidate_source.source().starts_with(prefix)
            || !candidate_source.source().ends_with(suffix)
            || candidate_source.source().len() < unchanged_bytes
        {
            return Err(stale(
                "ReplaceExpression changed source outside the authenticated expression span",
            ));
        }
        let new_end = candidate_source.source().len() - suffix.len();
        let new_expression = candidate_source
            .source()
            .get(old.start..new_end)
            .ok_or_else(|| stale("ReplaceExpression candidate span is unavailable"))?
            .to_owned();
        let catalog: Value =
            serde_json::from_str(&candidate.expression_catalog(&operation.target)?)
                .map_err(|_| invalid("candidate expression catalog is not valid JSON"))?;
        if catalog["source"]["path"] != old.path {
            return Err(stale(
                "ReplaceExpression changed the expression source owner",
            ));
        }
        let rows = catalog["expressions"]
            .as_array()
            .ok_or_else(|| invalid("candidate expression inventory is unavailable"))?;
        let mut matches = rows.iter().filter(|row| {
            row["phase"] == "body"
                && row["replaceable"] == true
                && row["source_span"]["start"] == old.start as u64
                && row["source_span"]["end"] == new_end as u64
        });
        let row = matches
            .next()
            .ok_or_else(|| stale("ReplaceExpression candidate expression was not reselected"))?;
        if matches.next().is_some() {
            return Err(stale(
                "ReplaceExpression candidate expression selection is ambiguous",
            ));
        }
        let new_expression_id = row["expression_id"]
            .as_str()
            .ok_or_else(|| invalid("candidate expression identity is unavailable"))?
            .to_owned();
        return Ok(ExpressionReplacement {
            path: old.path.clone(),
            new_expression_id,
            new_expression,
        });
    }
    Err(stale(
        "ReplaceExpression source owner disappeared from the candidate",
    ))
}

fn require_function_target(
    revision: &ProjectRevision,
    target: &str,
) -> Result<(), Vec<Diagnostic>> {
    let mut matches = 0usize;
    let mut admitted = false;
    for source in revision.sources() {
        let program =
            crate::parse(source.source(), Path::new(source.path())).map_err(|error| vec![error])?;
        for function in &program.functions {
            if function.stable_id == target {
                matches += 1;
                admitted = function.explicit_id && function.type_parameters.is_empty();
            }
        }
    }
    if matches != 1 || !admitted {
        return Err(invalid(
            "ReplaceExpression v2 requires one explicit monomorphic function",
        ));
    }
    Ok(())
}

fn require_canonical_comment_free_sources(
    revision: &ProjectRevision,
) -> Result<(), Vec<Diagnostic>> {
    for source in revision.sources() {
        let (program, comments) =
            crate::parse_with_comments(source.source(), Path::new(source.path()))
                .map_err(|error| vec![error])?;
        if !comments.items.is_empty() || crate::format::canonical(&program) != source.source() {
            return Err(invalid(
                "semantic transaction v2 requires comment-free canonical source",
            ));
        }
    }
    Ok(())
}

fn usize_field(value: &Value, key: &str) -> Result<usize, Vec<Diagnostic>> {
    value[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("expression catalog span is invalid"))
}

fn parse_value(source: &str) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(source)
        .map_err(|_| invalid("semantic transaction v2 artifact is invalid JSON"))
}

fn validate_text(value: &str, max: usize, message: &'static str) -> Result<(), Vec<Diagnostic>> {
    if value.is_empty() || value.len() > max || value.contains('\0') {
        Err(invalid(message))
    } else {
        Ok(())
    }
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "semantic transaction v2 revision is not a canonical SHA-256 digest",
        ));
    }
    Ok(())
}

fn render(mut value: Value, limit: usize) -> Result<String, Vec<Diagnostic>> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("semantic transaction v2 artifact could not be rendered"))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(capacity(
            "semantic transaction v2 artifact exceeds its byte limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid("semantic transaction v2 artifact is not UTF-8"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G525", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G526", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G527", message)]
}
