//! Ordered composition of several already-validated Universal Semantic
//! Transaction v2 `ReplaceExpression` operations into one multi-file feature
//! change.
//!
//! This module invents no editing route, transaction kernel, or patch
//! application path of its own. Each step is validated by the exact frozen
//! [`SemanticTransactionV2::validate`] core, unmodified; this module's only
//! job is to feed step N's resulting [`ProjectRevision`] as step N+1's base,
//! so every step reselects its expression identity and old-source precondition
//! fresh against the exact revision the prior step actually produced. The
//! final structural summary reuses the existing
//! [`super::SemanticWorkspaceStructuralDiff`] composition core rather than
//! defining a second diff representation.
//!
//! Composition is authority-free and touches no filesystem: a failing step
//! simply returns an error before any [`SemanticTransactionV2Workflow`] value
//! exists, so a failed or partial workflow can neither publish a prefix of its
//! steps nor leave behind an executable draft. See
//! `docs/UNIVERSAL-SEMANTIC-TRANSACTION-V2-WORKFLOW.md`.

use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{
    ProgramRoot, ProjectCandidate, ProjectRevision, SemanticTransactionArtifactsV2,
    SemanticTransactionV2, SemanticWorkspaceStructuralDiff,
};

pub const SEMANTIC_TRANSACTION_V2_WORKFLOW_SCHEMA: &str =
    "semaprax.semantic-transaction-workflow.v2";
pub const MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS: usize = 8;
pub const MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_BYTES: usize = 96 * 1024 * 1024;

const WORKFLOW_DOMAIN: &[u8] = b"semaprax.semantic-transaction-workflow.digest.v2\0";

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// One fully admitted ordered sequence of Universal Semantic Transaction v2
/// `ReplaceExpression` steps, each validated against the exact revision its
/// predecessor produced, ending in one immutable [`ProjectCandidate`].
///
/// This is not itself a v2 transaction: the frozen v2 envelope retains its
/// one-operation cardinality unchanged. A workflow is Rust/service-side
/// composition evidence over already-closed v2 transactions, the same
/// relationship [`super::SemanticTransactionMerge`] has to Universal Semantic
/// Transaction v1.
pub struct SemanticTransactionV2Workflow {
    base_program_root: ProgramRoot,
    final_step: SemanticTransactionArtifactsV2,
    structural_diff: SemanticWorkspaceStructuralDiff,
    json: String,
    digest: String,
}

impl SemanticTransactionV2Workflow {
    /// Validate `transactions` strictly in order against `base`. Step 0 is
    /// validated against `base`; step N (N > 0) is validated against the
    /// exact [`ProjectRevision`] step N-1 produced, so a stale expression
    /// identity or old-source slice carried over from an earlier base is
    /// rejected by the reused v2 core rather than silently reselected onto an
    /// unrelated expression. The first failing step aborts the whole
    /// composition: no candidate for any earlier step is retained or exposed,
    /// and nothing is written anywhere.
    pub fn derive(
        base: Arc<ProjectRevision>,
        transactions: &[SemanticTransactionV2],
    ) -> Result<Self> {
        if transactions.is_empty() {
            return Err(invalid(
                "semantic transaction v2 workflow requires at least one operation",
            ));
        }
        if transactions.len() > MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS {
            return Err(capacity(
                "semantic transaction v2 workflow exceeds its step limit",
            ));
        }
        let base_workspace = base.canonical_workspace_revision()?;
        let base_program_root = base_workspace.program_root()?;

        let mut current = Arc::clone(&base);
        let mut steps = Vec::with_capacity(transactions.len());
        let mut final_step: Option<SemanticTransactionArtifactsV2> = None;
        for (index, transaction) in transactions.iter().enumerate() {
            let artifacts = transaction
                .validate(Arc::clone(&current))
                .map_err(|diagnostics| {
                    annotate_step(diagnostics, index, transaction.operation().target())
                })?;
            let impact_value =
                parse_embedded(artifacts.impact(), "workflow step impact is invalid JSON")?;
            let result_value =
                parse_embedded(artifacts.result(), "workflow step result is invalid JSON")?;
            let source_path = impact_value["expression"]["source_path"]
                .as_str()
                .ok_or_else(|| invalid("workflow step impact is missing its source path"))?
                .to_owned();
            steps.push(json!({
                "impact": {"digest": artifacts.impact_digest(), "value": impact_value},
                "index": index,
                "result": {"digest": artifacts.result_digest(), "value": result_value},
                "source_path": source_path,
                "target": transaction.operation().target(),
                "transaction_digest": transaction.digest(),
            }));
            current = Arc::clone(artifacts.candidate().revision());
            final_step = Some(artifacts);
        }
        let final_step = final_step.expect("transactions is nonempty");
        let candidate = final_step.candidate();
        let structural_diff =
            SemanticWorkspaceStructuralDiff::derive(candidate, candidate.candidate_digest())?;

        let json = render(
            json!({
                "authority": false,
                "base": {
                    "project_revision": base.project_revision(),
                    "workspace_revision": base_program_root.workspace_revision(),
                },
                "nonclaims": [
                    "replace_expression_operations_only",
                    "no_new_editing_route_or_transaction_kernel",
                    "no_runtime_or_project_test_execution",
                    "no_source_commit_or_publication_authority",
                    "not_behavioral_equivalence",
                ],
                "result": {
                    "candidate_digest": candidate.candidate_digest(),
                    "project_revision": candidate.revision().project_revision(),
                    "workspace_revision": final_step.candidate_program_root().workspace_revision(),
                },
                "schema": SEMANTIC_TRANSACTION_V2_WORKFLOW_SCHEMA,
                "source_review": {
                    "digest": structural_diff.source_review_digest(),
                    "value": parse_embedded(structural_diff.source_review(), "workflow source review is invalid JSON")?,
                },
                "step_count": steps.len(),
                "steps": steps,
                "structural_diff": {
                    "digest": structural_diff.digest(),
                    "value": parse_embedded(structural_diff.to_json(), "workflow structural diff is invalid JSON")?,
                },
                "validation": {
                    "canonical_workspace_revision": true,
                    "complete_project_admission": true,
                    "each_step_revalidated_against_its_exact_predecessor_revision": true,
                },
            }),
            MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_BYTES,
        )?;
        let digest = digest(WORKFLOW_DOMAIN, json.as_bytes());
        Ok(Self {
            base_program_root,
            final_step,
            structural_diff,
            json,
            digest,
        })
    }

    /// Reparse every step's transaction bytes, rederive the workflow, and
    /// require exact submitted envelope bytes and digest.
    pub fn replay(
        base: Arc<ProjectRevision>,
        transaction_bytes: &[Vec<u8>],
        expected_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        validate_submitted(
            bytes,
            MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_BYTES,
            SEMANTIC_TRANSACTION_V2_WORKFLOW_SCHEMA,
        )?;
        require_digest(expected_digest)?;
        if digest(WORKFLOW_DOMAIN, bytes) != expected_digest {
            return Err(stale("semantic transaction v2 workflow digest is stale"));
        }
        let transactions = transaction_bytes
            .iter()
            .map(|bytes| SemanticTransactionV2::from_json(bytes))
            .collect::<Result<Vec<_>>>()?;
        let derived = Self::derive(base, &transactions)?;
        if derived.to_json().as_bytes() != bytes || derived.digest() != expected_digest {
            return Err(stale(
                "semantic transaction v2 workflow failed exact replay",
            ));
        }
        Ok(derived)
    }

    pub fn candidate(&self) -> &ProjectCandidate {
        self.final_step.candidate()
    }
    pub fn base_program_root(&self) -> &ProgramRoot {
        &self.base_program_root
    }
    pub fn candidate_program_root(&self) -> &ProgramRoot {
        self.final_step.candidate_program_root()
    }
    pub fn structural_diff(&self) -> &SemanticWorkspaceStructuralDiff {
        &self.structural_diff
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn annotate_step(diagnostics: Vec<Diagnostic>, index: usize, target: &str) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| Diagnostic {
            message: format!("workflow step {index} ({target}): {}", diagnostic.message),
            ..diagnostic
        })
        .collect()
}

fn parse_embedded(source: &str, message: &'static str) -> Result<Value> {
    serde_json::from_str(source).map_err(|_| invalid(message))
}

fn validate_submitted(bytes: &[u8], limit: usize, schema: &'static str) -> Result<Value> {
    if bytes.len() > limit {
        return Err(capacity(
            "semantic transaction v2 workflow exceeds its byte limit",
        ));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| invalid("semantic transaction v2 workflow is not valid JSON"))?;
    if value.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(
            "semantic transaction v2 workflow has an invalid schema",
        ));
    }
    if render(value.clone(), limit)?.as_bytes() != bytes {
        return Err(invalid(
            "semantic transaction v2 workflow is not canonical JSON",
        ));
    }
    Ok(value)
}

fn require_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "semantic transaction v2 workflow selector is not a canonical SHA-256 digest",
        ));
    }
    Ok(())
}

fn render(mut value: Value, limit: usize) -> Result<String> {
    value.sort_all_objects();
    let mut bytes = serde_json::to_vec(&value)
        .map_err(|_| invalid("semantic transaction v2 workflow could not be rendered"))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(capacity(
            "semantic transaction v2 workflow exceeds its byte limit",
        ));
    }
    String::from_utf8(bytes).map_err(|_| invalid("semantic transaction v2 workflow is not UTF-8"))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G600", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G601", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G602", message)]
}
