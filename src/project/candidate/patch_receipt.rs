//! Compact, replay-verifiable summaries for immutable semantic candidates.
//!
//! A receipt is a compiler projection over a retained [`ProjectCandidate`].
//! It carries references to the larger compiler reports but never embeds them,
//! accepts a caller assertion that a check passed, executes a target, or grants
//! apply/publication authority. Verification replays the candidate from its
//! retained base before recomputing the complete canonical receipt bytes.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;

use super::{wire, ProjectCandidate};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub const PROJECT_PATCH_RECEIPT_SCHEMA: &str = "semaprax.patch-receipt.v1";
pub const PROJECT_PATCH_RECEIPT_VERIFICATION_SCHEMA: &str =
    "semaprax.patch-receipt-verification.v1";
pub const PROJECT_PATCH_RECEIPT_COMPARISON_SCHEMA: &str = "semaprax.patch-receipt-comparison.v1";
pub const PROJECT_PATCH_RECEIPT_EVIDENCE_SUMMARY_SCHEMA: &str =
    "semaprax.patch-receipt-evidence-summary.v1";
pub const PROJECT_PATCH_RECEIPT_EVIDENCE_PAGE_SCHEMA: &str =
    "semaprax.patch-receipt-evidence-page.v1";
/// The default summary budget. Larger reports remain independently derivable
/// from the selected retained candidate through their own candidate APIs.
pub const MAX_PROJECT_PATCH_RECEIPT_BYTES: usize = 8 * 1024;
pub const MAX_PROJECT_PATCH_RECEIPT_EVIDENCE_SUMMARY_BYTES: usize = 64 * 1024;
pub const MAX_PROJECT_PATCH_RECEIPT_EVIDENCE_PAGE_BYTES: usize = 1024 * 1024;

const RECEIPT_DOMAIN: &[u8] = b"semaprax.patch-receipt.v1\\0";
const CANDIDATE_EVIDENCE_DOMAIN: &[u8] = b"semaprax.patch-receipt.candidate.v1\\0";
const CATALOG_EVIDENCE_DOMAIN: &[u8] = b"semaprax.patch-receipt.catalog.v1\\0";
const CONTRACT_EVIDENCE_DOMAIN: &[u8] = b"semaprax.patch-receipt.contract.v1\\0";
const OWNERSHIP_EVIDENCE_DOMAIN: &[u8] = b"semaprax.patch-receipt.ownership.v1\\0";
const REQUEST_EVIDENCE_DOMAIN: &[u8] = b"semaprax.patch-receipt.request.v1\\0";
const MAX_DECLARATION_PREVIEW: usize = 16;
const MAX_EVIDENCE_CURSOR_BYTES: usize = 128;
const MAX_EVIDENCE_CURSOR_OFFSET: usize = 65_536;

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G982", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G983", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G984", message)]
}

/// Closed retained-candidate evidence families referenced by a patch receipt.
/// This selector intentionally does not accept paths, receipt JSON, URLs, or
/// caller-supplied evidence bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectPatchReceiptEvidence {
    Candidate,
    DeclarationCatalog,
    ContractDelta,
    OwnershipDelta,
}

impl ProjectPatchReceiptEvidence {
    const ALL: [Self; 4] = [
        Self::Candidate,
        Self::DeclarationCatalog,
        Self::ContractDelta,
        Self::OwnershipDelta,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::DeclarationCatalog => "declaration_catalog",
            Self::ContractDelta => "contract_delta",
            Self::OwnershipDelta => "ownership_delta",
        }
    }

    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "declaration_catalog" => Ok(Self::DeclarationCatalog),
            "contract_delta" => Ok(Self::ContractDelta),
            "ownership_delta" => Ok(Self::OwnershipDelta),
            _ => Err(invalid("patch receipt evidence selector is unsupported")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectPatchReceiptEvidencePageOptions {
    page_size: usize,
    max_bytes: usize,
}

impl ProjectPatchReceiptEvidencePageOptions {
    pub fn new(page_size: usize, max_bytes: usize) -> Result<Self> {
        if !(1..=128).contains(&page_size)
            || !(1024..=MAX_PROJECT_PATCH_RECEIPT_EVIDENCE_PAGE_BYTES).contains(&max_bytes)
        {
            return Err(invalid(
                "patch receipt evidence page options require 1..128 items and 1024..1048576 bytes",
            ));
        }
        Ok(Self {
            page_size,
            max_bytes,
        })
    }

    pub const fn page_size(self) -> usize {
        self.page_size
    }

    pub const fn max_bytes(self) -> usize {
        self.max_bytes
    }
}

impl Default for ProjectPatchReceiptEvidencePageOptions {
    fn default() -> Self {
        Self {
            page_size: 32,
            max_bytes: 65_536,
        }
    }
}

impl ProjectCandidate {
    /// Produce the bounded `semaprax.patch-receipt.v1` summary for this exact
    /// immutable candidate. This derives source replay/admission facts and
    /// descriptive contract and ownership projections from compiler-held data;
    /// it never runs tests, effects, or an assurance engine.
    pub fn patch_receipt(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let catalog_text = self.semantic_delta_catalog(expected_candidate)?;
        let catalog: Value = serde_json::from_str(&catalog_text)
            .map_err(|_| invalid("semantic delta catalog is not valid compiler JSON"))?;
        let roots = catalog["roots"]
            .as_array()
            .ok_or_else(|| invalid("semantic delta catalog roots are absent"))?;
        let contract_text = self.contract_delta(expected_candidate)?;
        let contract: Value = serde_json::from_str(&contract_text)
            .map_err(|_| invalid("contract delta is not valid compiler JSON"))?;
        let ownership_text = self.ownership_delta(expected_candidate)?;
        let ownership: Value = serde_json::from_str(&ownership_text)
            .map_err(|_| invalid("ownership delta is not valid compiler JSON"))?;

        for preview_len in (0..=roots.len().min(MAX_DECLARATION_PREVIEW)).rev() {
            let content = self.patch_receipt_content(
                &catalog,
                &contract,
                &ownership,
                &catalog_text,
                &contract_text,
                &ownership_text,
                preview_len,
            )?;
            // The digest commits to exactly these canonical UTF-8 bytes,
            // including the terminal LF produced by `wire::render`.
            let content_bytes = wire::render(content.clone(), MAX_PROJECT_PATCH_RECEIPT_BYTES)?;
            let receipt = wire::render(
                json!({
                    "schema": PROJECT_PATCH_RECEIPT_SCHEMA,
                    "canonical_bytes_hashed": "content canonical UTF-8 JSON with one terminal LF",
                    "content": content,
                    "receipt_digest": wire::digest(RECEIPT_DOMAIN, content_bytes.as_bytes()),
                }),
                MAX_PROJECT_PATCH_RECEIPT_BYTES,
            );
            if let Ok(receipt) = receipt {
                return Ok(receipt);
            }
        }
        Err(capacity("patch receipt exceeds its 8 KiB summary budget"))
    }

    /// Independently reconstruct the retained candidate, then recompute and
    /// compare every submitted receipt byte. Rehashing caller JSON alone can
    /// therefore never satisfy this verifier.
    pub fn verify_patch_receipt(&self, expected_candidate: &str, bytes: &[u8]) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        if bytes.len() > MAX_PROJECT_PATCH_RECEIPT_BYTES {
            return Err(capacity("patch receipt verification input exceeds 8 KiB"));
        }
        let replay = Self::replay(
            Arc::clone(&self.base),
            self.base.project_revision(),
            &self.changes,
            self.to_json().as_bytes(),
        )?;
        let expected = replay.patch_receipt(expected_candidate)?;
        if expected.as_bytes() != bytes {
            return Err(stale(
                "patch receipt failed exact independent recomputation",
            ));
        }
        wire::render(
            json!({
                "schema": PROJECT_PATCH_RECEIPT_VERIFICATION_SCHEMA,
                "result": "exact_recomputation",
                "candidate_digest": expected_candidate,
                "base_project_revision": self.base.project_revision(),
                "project_revision": self.revision.project_revision(),
                "receipt_digest": receipt_digest(bytes)?,
                "execution": false,
                "source_authority": false,
                "publication_authority": false,
            }),
            65_536,
        )
    }

    /// Lists the closed, compiler-derived evidence families referenced by an
    /// admitted receipt. The handles bind candidate identity, evidence family,
    /// and complete canonical evidence bytes; cursors additionally bind page
    /// options.
    pub fn patch_receipt_evidence_summary(&self, expected_candidate: &str) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let binding = receipt_binding(self);
        let evidence = ProjectPatchReceiptEvidence::ALL
            .into_iter()
            .map(|kind| {
                let evidence = retained_evidence(self, expected_candidate, kind)?;
                Ok(json!({
                    "id": kind.id(),
                    "schema": evidence.schema,
                    "digest": evidence.digest,
                    "total_items": evidence.items.len(),
                    "handle": evidence_handle(expected_candidate, kind, &evidence.digest),
                    "availability": "recomputed_from_retained_candidate",
                }))
            })
            .collect::<Result<Vec<_>>>()?;
        wire::render(
            json!({
                "schema": PROJECT_PATCH_RECEIPT_EVIDENCE_SUMMARY_SCHEMA,
                "binding": binding,
                "evidence": evidence,
                "execution": false,
                "source_authority": false,
                "publication_authority": false,
                "nonclaims": [
                    "not_arbitrary_receipt_path_or_caller_supplied_document_retrieval",
                    "not_test_execution_or_runtime_effect_observation",
                    "no_effect_or_publication_authority",
                ],
            }),
            MAX_PROJECT_PATCH_RECEIPT_EVIDENCE_SUMMARY_BYTES,
        )
    }

    /// Page one closed evidence family in compiler order. Every page repeats
    /// candidate selection and recomputes the complete evidence object before
    /// accepting its handle or cursor, so a page cannot cross candidates,
    /// evidence families, or pagination policies.
    pub fn patch_receipt_evidence_page(
        &self,
        expected_candidate: &str,
        evidence_id: &str,
        expected_handle: &str,
        cursor: Option<&str>,
        options: ProjectPatchReceiptEvidencePageOptions,
    ) -> Result<String> {
        self.require_candidate(expected_candidate)?;
        let kind = ProjectPatchReceiptEvidence::parse(evidence_id)?;
        let evidence = retained_evidence(self, expected_candidate, kind)?;
        let handle = evidence_handle(expected_candidate, kind, &evidence.digest);
        if expected_handle.len() != 71 || expected_handle != handle {
            return Err(stale(
                "patch receipt evidence handle does not match the retained candidate evidence",
            ));
        }
        let offset = cursor
            .map(|cursor| evidence_cursor_offset(cursor, &handle, options))
            .transpose()?
            .unwrap_or(0);
        if cursor.is_some() && offset >= evidence.items.len() {
            return Err(stale(
                "patch receipt evidence cursor is outside its selected inventory",
            ));
        }
        let end = offset
            .saturating_add(options.page_size)
            .min(evidence.items.len());
        let next_cursor =
            (end < evidence.items.len()).then(|| evidence_cursor(end, &handle, options));
        wire::render(
            json!({
                "schema": PROJECT_PATCH_RECEIPT_EVIDENCE_PAGE_SCHEMA,
                "binding": receipt_binding(self),
                "evidence": {
                    "id": kind.id(),
                    "schema": evidence.schema,
                    "digest": evidence.digest,
                    "handle": handle,
                },
                "cursor": cursor,
                "offset": offset,
                "total_items": evidence.items.len(),
                "page_size": options.page_size,
                "max_bytes": options.max_bytes,
                "next_cursor": next_cursor,
                "items": evidence.items[offset..end].to_vec(),
                "execution": false,
                "source_authority": false,
                "publication_authority": false,
                "nonclaims": [
                    "not_arbitrary_receipt_path_or_caller_supplied_document_retrieval",
                    "not_test_execution_or_runtime_effect_observation",
                    "no_effect_or_publication_authority",
                ],
            }),
            options.max_bytes,
        )
    }

    /// Render a bounded receipt for a request whose candidate selector is
    /// stale. The receipt deliberately has no resulting candidate identity
    /// and records every check after selector validation as `not_run`.
    pub fn patch_receipt_refusal(&self, requested_candidate: &str) -> Result<String> {
        wire::validate_digest(requested_candidate)?;
        if requested_candidate == self.candidate_digest() {
            return Err(invalid(
                "patch refusal receipt requires a stale candidate selector",
            ));
        }
        let binding = json!({
            "requested_candidate_digest": requested_candidate,
            "base_project_revision": self.base.project_revision(),
            "project_revision": Value::Null,
            "workspace": {
                "manifest_digest": wire::digest(
                    b"semaprax.patch-receipt.manifest.v1\\0",
                    self.base.manifest().to_canonical_toml().as_bytes(),
                ),
                "identity_method": "retained_canonical_project_manifest",
            },
        });
        let content = json!({
            "schema": PROJECT_PATCH_RECEIPT_SCHEMA,
            "binding": binding,
            "policy": receipt_policy("candidate_selector_refusal", "request_identity", "not_applicable"),
            "attempt": {
                "status": "refused_stale_candidate_selector",
                "identity": requested_candidate,
                "reason": "candidate_selector_does_not_match_the_retained_candidate",
            },
            "declarations": {
                "directly_changed_count": Value::Null,
                "directly_changed_preview": [],
                "omitted_directly_changed_count": Value::Null,
                "details_complete": false,
                "affected_through_dependencies": {"status":"not_run","count":Value::Null,"reason":"candidate selection was refused before semantic derivation"},
                "full_details_evidence": Value::Null,
            },
            "checks": [
                check("candidate_selector_verification", "failed", "exact_candidate_selector", "request_identity", "request"),
                check("candidate_admission_and_source_replay", "not_run", "selector_refused_before_candidate_replay", "not_observed", "request"),
                check("contract_inventory_and_delta", "not_run", "selector_refused_before_contract_analysis", "not_observed", "request"),
                check("ownership_and_cleanup_validation", "not_run", "selector_refused_before_ownership_analysis", "not_observed", "request"),
                check("candidate_test_execution", "not_run", "selector_refused_before_test_execution", "not_observed", "request"),
                check("additional_assurance", "not_run", "selector_refused_before_assurance_selection", "not_observed", "request"),
            ],
            "effect_usage": {"status":"not_applicable","reason":"candidate selector refusal has no agent runtime accounting input"},
            "evidence": [evidence_ref("request", "semaprax.patch-receipt-request.v1", wire::digest(REQUEST_EVIDENCE_DOMAIN, requested_candidate.as_bytes()), &binding, "bound_request_identity")],
            "nonclaims": [
                "no_candidate_was_admitted_or_invented",
                "not_source_replay_or_contract_or_ownership_validation",
                "not_test_execution_or_formal_proof",
                "no_effect_or_publication_authority",
            ],
        });
        render_receipt(content)
    }

    /// Independently recompute a stale-selector refusal receipt. This does
    /// not turn a rejected selector into a candidate admission.
    pub fn verify_patch_receipt_refusal(
        &self,
        requested_candidate: &str,
        bytes: &[u8],
    ) -> Result<String> {
        let expected = self.patch_receipt_refusal(requested_candidate)?;
        verify_exact_receipt(
            self,
            requested_candidate,
            bytes,
            &expected,
            "exact_refusal_recomputation",
        )
    }

    /// Compare two independently recomputed receipts. The comparison never
    /// selects a winner and keeps refusal, missing accounting and differing
    /// policy/context bindings explicit.
    pub fn compare_patch_receipts(
        &self,
        expected_candidate: &str,
        bytes: &[u8],
        other: &Self,
        other_expected_candidate: &str,
        other_bytes: &[u8],
    ) -> Result<String> {
        let left = verified_receipt_content(self, expected_candidate, bytes)?;
        let right = verified_receipt_content(other, other_expected_candidate, other_bytes)?;
        let mut reasons = Vec::new();
        if left["attempt"]["status"] != "admitted_candidate" {
            reasons.push("left_receipt_did_not_admit_a_candidate");
        }
        if right["attempt"]["status"] != "admitted_candidate" {
            reasons.push("right_receipt_did_not_admit_a_candidate");
        }
        for (field, reason) in [
            ("base_project_revision", "base_project_revisions_differ"),
            ("workspace", "workspace_contexts_differ"),
        ] {
            if left["binding"][field] != right["binding"][field] {
                reasons.push(reason);
            }
        }
        if left["policy"] != right["policy"] {
            reasons.push("receipt_policies_or_accounting_scopes_differ");
        }
        let result = if reasons.is_empty() {
            "comparable"
        } else {
            "not_comparable"
        };
        wire::render(
            json!({
                "schema": PROJECT_PATCH_RECEIPT_COMPARISON_SCHEMA,
                "result": result,
                "reasons": reasons,
                "left": receipt_comparison_side(bytes, &left)?,
                "right": receipt_comparison_side(other_bytes, &right)?,
                "comparison": if result == "comparable" { json!({
                    "declarations": {"left":left["declarations"].clone(),"right":right["declarations"].clone()},
                    "checks": {"left":left["checks"].clone(),"right":right["checks"].clone()},
                    "effect_usage": {"left":left["effect_usage"].clone(),"right":right["effect_usage"].clone()},
                }) } else { Value::Null },
                "execution": false,
                "source_authority": false,
                "publication_authority": false,
                "nonclaims": ["not_a_universal_best_patch_score","not_merge_or_publication_authority"],
            }),
            65_536,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn patch_receipt_content(
        &self,
        catalog: &Value,
        contract: &Value,
        ownership: &Value,
        catalog_text: &str,
        contract_text: &str,
        ownership_text: &str,
        preview_len: usize,
    ) -> Result<Value> {
        let roots = catalog["roots"]
            .as_array()
            .ok_or_else(|| invalid("semantic delta catalog roots are absent"))?;
        let preview = roots
            .iter()
            .take(preview_len)
            .map(declaration_preview)
            .collect::<Result<Vec<_>>>()?;
        let omitted = roots.len().saturating_sub(preview.len());
        let binding = json!({
            "candidate_digest": self.candidate_digest(),
            "base_project_revision": self.base.project_revision(),
            "project_revision": self.revision.project_revision(),
            "workspace": {
                "manifest_digest": wire::digest(
                    b"semaprax.patch-receipt.manifest.v1\\0",
                    self.revision.manifest().to_canonical_toml().as_bytes(),
                ),
                "identity_method": "retained_canonical_project_manifest",
            },
        });
        let evidence = vec![
            evidence_ref(
                "candidate",
                "semaprax.project-candidate.v1",
                wire::digest(CANDIDATE_EVIDENCE_DOMAIN, self.to_json().as_bytes()),
                &binding,
                "derivable_from_retained_candidate",
            ),
            evidence_ref(
                "declaration_catalog",
                catalog["schema"]
                    .as_str()
                    .ok_or_else(|| invalid("catalog schema is absent"))?,
                wire::digest(CATALOG_EVIDENCE_DOMAIN, catalog_text.as_bytes()),
                &binding,
                "derivable_from_retained_candidate",
            ),
            evidence_ref(
                "contract_delta",
                contract["schema"]
                    .as_str()
                    .ok_or_else(|| invalid("contract schema is absent"))?,
                wire::digest(CONTRACT_EVIDENCE_DOMAIN, contract_text.as_bytes()),
                &binding,
                "derivable_from_retained_candidate",
            ),
            evidence_ref(
                "ownership_delta",
                ownership["schema"]
                    .as_str()
                    .ok_or_else(|| invalid("ownership schema is absent"))?,
                wire::digest(OWNERSHIP_EVIDENCE_DOMAIN, ownership_text.as_bytes()),
                &binding,
                "derivable_from_retained_candidate",
            ),
        ];
        Ok(json!({
            "schema": PROJECT_PATCH_RECEIPT_SCHEMA,
            "binding": binding,
            "policy": receipt_policy("candidate_projection", "retained_candidate", "not_applicable"),
            "attempt": {
                "status": "admitted_candidate",
                "semantic_change_count": self.changes.len(),
                "identity": self.candidate_digest(),
            },
            "declarations": {
                "directly_changed_count": roots.len(),
                "directly_changed_preview": preview,
                "omitted_directly_changed_count": omitted,
                "details_complete": omitted == 0,
                "affected_through_dependencies": {
                    "status": "not_derived",
                    "count": Value::Null,
                    "reason": "semantic_delta_catalog records authored roots; dependency impact requires its separate bounded query",
                },
                "full_details_evidence": "declaration_catalog",
            },
            "checks": [
                check("candidate_admission_and_source_replay", "passed", "candidate_apply_and_source_replay", "complete_candidate", "candidate"),
                check("contract_inventory_and_delta", "passed", "compiler_descriptive_contract_delta", "complete_candidate_contract_inventory", "contract_delta"),
                check("ownership_and_cleanup_validation", "passed", "compiler_descriptive_ownership_delta", "complete_candidate_ownership_inventory", "ownership_delta"),
                check("candidate_test_execution", "not_run", "no_test_execution_in_receipt_generation", "not_observed", "candidate"),
                check("additional_assurance", "not_run", "no_assurance_evidence_selected", "not_observed", "candidate"),
            ],
            "effect_usage": {
                "status": "not_applicable",
                "reason": "candidate receipt generation has no agent runtime accounting input",
            },
            "evidence": evidence,
            "nonclaims": [
                "not_behavioral_equivalence",
                "not_test_execution",
                "not_formal_proof",
                "not_live_checkout_freshness",
                "no_effect_or_publication_authority",
            ],
        }))
    }
}

struct RetainedEvidence {
    schema: String,
    digest: String,
    items: Vec<Value>,
}

fn receipt_binding(candidate: &ProjectCandidate) -> Value {
    json!({
        "candidate_digest": candidate.candidate_digest(),
        "base_project_revision": candidate.base.project_revision(),
        "project_revision": candidate.revision.project_revision(),
        "workspace": {
            "manifest_digest": wire::digest(
                b"semaprax.patch-receipt.manifest.v1\\0",
                candidate.revision.manifest().to_canonical_toml().as_bytes(),
            ),
            "identity_method": "retained_canonical_project_manifest",
        },
    })
}

fn retained_evidence(
    candidate: &ProjectCandidate,
    expected_candidate: &str,
    kind: ProjectPatchReceiptEvidence,
) -> Result<RetainedEvidence> {
    let (bytes, domain, array_key, schema) = match kind {
        ProjectPatchReceiptEvidence::Candidate => (
            candidate.to_json().to_owned(),
            CANDIDATE_EVIDENCE_DOMAIN,
            "changes",
            "semaprax.project-candidate.v1",
        ),
        ProjectPatchReceiptEvidence::DeclarationCatalog => (
            candidate.semantic_delta_catalog(expected_candidate)?,
            CATALOG_EVIDENCE_DOMAIN,
            "roots",
            "semaprax.project-candidate-semantic-delta-catalog.v1",
        ),
        ProjectPatchReceiptEvidence::ContractDelta => (
            candidate.contract_delta(expected_candidate)?,
            CONTRACT_EVIDENCE_DOMAIN,
            "functions",
            "semaprax.project-candidate-contract-delta.v1",
        ),
        ProjectPatchReceiptEvidence::OwnershipDelta => (
            candidate.ownership_delta(expected_candidate)?,
            OWNERSHIP_EVIDENCE_DOMAIN,
            "functions",
            "semaprax.project-candidate-ownership-delta.v1",
        ),
    };
    let value: Value = serde_json::from_str(&bytes)
        .map_err(|_| invalid("retained patch receipt evidence is not compiler JSON"))?;
    if value.get("schema").and_then(Value::as_str) != Some(schema) {
        return Err(invalid(
            "retained patch receipt evidence has an unexpected compiler schema",
        ));
    }
    let mut items = value
        .get(array_key)
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| invalid("retained patch receipt evidence inventory is absent"))?;
    if kind == ProjectPatchReceiptEvidence::OwnershipDelta {
        let types = value
            .get("types")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("retained ownership evidence type inventory is absent"))?;
        items = items
            .into_iter()
            .map(|value| json!({"kind":"function","value":value}))
            .chain(
                types
                    .iter()
                    .cloned()
                    .map(|value| json!({"kind":"type","value":value})),
            )
            .collect();
    }
    if items.len() > MAX_EVIDENCE_CURSOR_OFFSET {
        return Err(capacity(
            "retained patch receipt evidence inventory exceeds its cursor bound",
        ));
    }
    Ok(RetainedEvidence {
        schema: schema.to_owned(),
        digest: wire::digest(domain, bytes.as_bytes()),
        items,
    })
}

fn evidence_handle(candidate: &str, kind: ProjectPatchReceiptEvidence, digest: &str) -> String {
    wire::digest(
        b"semaprax.patch-receipt-evidence-handle.v1\\0",
        format!("{candidate}\\n{}\\n{digest}", kind.id()).as_bytes(),
    )
}

fn evidence_cursor(
    offset: usize,
    handle: &str,
    options: ProjectPatchReceiptEvidencePageOptions,
) -> String {
    let text = format!(
        "{handle}\\n{offset}\\n{}\\n{}",
        options.page_size, options.max_bytes
    );
    format!(
        "{offset}:{}",
        wire::digest(
            b"semaprax.patch-receipt-evidence-cursor.v1\\0",
            text.as_bytes()
        )
    )
}

fn evidence_cursor_offset(
    cursor: &str,
    handle: &str,
    options: ProjectPatchReceiptEvidencePageOptions,
) -> Result<usize> {
    if cursor.len() > MAX_EVIDENCE_CURSOR_BYTES {
        return Err(stale("patch receipt evidence cursor exceeds its bound"));
    }
    let (number, _) = cursor
        .split_once(':')
        .ok_or_else(|| stale("patch receipt evidence cursor is malformed"))?;
    let offset = number
        .parse::<usize>()
        .map_err(|_| stale("patch receipt evidence cursor offset is invalid"))?;
    if offset == 0
        || offset > MAX_EVIDENCE_CURSOR_OFFSET
        || offset % options.page_size != 0
        || offset.to_string() != number
        || evidence_cursor(offset, handle, options) != cursor
    {
        return Err(stale(
            "patch receipt evidence cursor does not match its retained evidence handle and options",
        ));
    }
    Ok(offset)
}

fn render_receipt(content: Value) -> Result<String> {
    let content_bytes = wire::render(content.clone(), MAX_PROJECT_PATCH_RECEIPT_BYTES)?;
    wire::render(
        json!({
            "schema": PROJECT_PATCH_RECEIPT_SCHEMA,
            "canonical_bytes_hashed": "content canonical UTF-8 JSON with one terminal LF",
            "content": content,
            "receipt_digest": wire::digest(RECEIPT_DOMAIN, content_bytes.as_bytes()),
        }),
        MAX_PROJECT_PATCH_RECEIPT_BYTES,
    )
}

fn receipt_policy(
    check_profile: &str,
    evidence_selection_scope: &str,
    effect_accounting_scope: &str,
) -> Value {
    json!({"schema":"semaprax.patch-receipt-policy.v1","check_profile":check_profile,"evidence_selection_scope":evidence_selection_scope,"effect_accounting_scope":effect_accounting_scope})
}

fn verify_exact_receipt(
    candidate: &ProjectCandidate,
    requested_candidate: &str,
    bytes: &[u8],
    expected: &str,
    result: &str,
) -> Result<String> {
    if bytes.len() > MAX_PROJECT_PATCH_RECEIPT_BYTES {
        return Err(capacity("patch receipt verification input exceeds 8 KiB"));
    }
    if expected.as_bytes() != bytes {
        return Err(stale(
            "patch receipt failed exact independent recomputation",
        ));
    }
    let content = receipt_content(bytes)?;
    wire::render(
        json!({
            "schema": PROJECT_PATCH_RECEIPT_VERIFICATION_SCHEMA,
            "result": result,
            "requested_candidate_digest": requested_candidate,
            "base_project_revision": candidate.base_revision().project_revision(),
            "project_revision": content["binding"]["project_revision"].clone(),
            "receipt_digest": receipt_digest(bytes)?,
            "execution": false,
            "source_authority": false,
            "publication_authority": false,
        }),
        65_536,
    )
}

fn verified_receipt_content(
    candidate: &ProjectCandidate,
    requested: &str,
    bytes: &[u8],
) -> Result<Value> {
    let expected = if requested == candidate.candidate_digest() {
        candidate.patch_receipt(requested)?
    } else {
        candidate.patch_receipt_refusal(requested)?
    };
    verify_exact_receipt(
        candidate,
        requested,
        bytes,
        &expected,
        "exact_recomputation",
    )?;
    receipt_content(bytes)
}

fn receipt_content(bytes: &[u8]) -> Result<Value> {
    let receipt: Value =
        serde_json::from_slice(bytes).map_err(|_| invalid("patch receipt is not valid JSON"))?;
    if receipt["schema"] != PROJECT_PATCH_RECEIPT_SCHEMA {
        return Err(invalid("patch receipt schema is not supported"));
    }
    receipt
        .get("content")
        .cloned()
        .ok_or_else(|| invalid("patch receipt content is absent"))
}

fn receipt_comparison_side(bytes: &[u8], content: &Value) -> Result<Value> {
    Ok(json!({
        "receipt_digest": receipt_digest(bytes)?,
        "attempt": content["attempt"].clone(),
        "binding": content["binding"].clone(),
        "policy": content["policy"].clone(),
    }))
}

fn declaration_preview(root: &Value) -> Result<Value> {
    let target = root["target"]
        .as_str()
        .ok_or_else(|| invalid("semantic delta root target is absent"))?;
    let change = root["change"]
        .as_str()
        .ok_or_else(|| invalid("semantic delta root change is absent"))?;
    let subject = root["candidate"]
        .as_object()
        .or_else(|| root["base"].as_object());
    let subject = subject.ok_or_else(|| invalid("semantic delta root subject is absent"))?;
    Ok(json!({
        "id": target,
        "kind": subject.get("kind").cloned().unwrap_or(Value::Null),
        "change": change,
    }))
}

fn evidence_ref(
    id: &str,
    schema: &str,
    digest: String,
    binding: &Value,
    availability: &str,
) -> Value {
    json!({
        "id": id,
        "schema": schema,
        "digest": digest,
        "subject_binding": binding,
        "availability": availability,
        "resolver": "retained_candidate_compiler_projection",
    })
}

fn check(category: &str, result: &str, method: &str, coverage: &str, evidence: &str) -> Value {
    json!({"category":category,"result":result,"method":method,"coverage":coverage,"evidence":evidence})
}

fn receipt_digest(bytes: &[u8]) -> Result<String> {
    let receipt: Value =
        serde_json::from_slice(bytes).map_err(|_| invalid("patch receipt is not valid JSON"))?;
    let content = receipt
        .get("content")
        .cloned()
        .ok_or_else(|| invalid("patch receipt content is absent"))?;
    let canonical = wire::render(content, MAX_PROJECT_PATCH_RECEIPT_BYTES)?;
    let digest = wire::digest(RECEIPT_DOMAIN, canonical.as_bytes());
    if receipt.get("receipt_digest").and_then(Value::as_str) != Some(&digest) {
        return Err(stale(
            "patch receipt digest does not bind its canonical content",
        ));
    }
    Ok(digest)
}
