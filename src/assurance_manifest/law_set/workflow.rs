//! Bounded read-only law diagnostics over an independently replayed report.
//! A failed law is an implementation/proof repair target; changing its intent
//! requires the separate protected-law review route.
use super::{invalid, verify_report, wire, LawPolicy, LawSet, Result};
use crate::assurance_manifest::VerifiedProjectProof;
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use serde_json::{json, Value};

pub const SUMMARY_SCHEMA: &str = "semaprax.law-workflow-summary.v1";
pub const DETAIL_SCHEMA: &str = "semaprax.law-workflow-detail.v1";
pub const STRICT_SUMMARY_SCHEMA: &str = "semaprax.strict-law-workflow-summary.v1";
pub const STRICT_DETAIL_SCHEMA: &str = "semaprax.strict-law-workflow-detail.v1";
const MAX_PAGE: usize = 64;
const MAX_VIEW_BYTES: usize = 64 * 1024;

fn bounded(value: &Value, max_bytes: usize) -> Result<String> {
    if !(256..=MAX_VIEW_BYTES).contains(&max_bytes) {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "law workflow byte budget must be between 256 and 65536",
        )]);
    }
    let text = wire::canonical(value)?;
    if text.len() > max_bytes {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "law workflow view exceeds the selected byte budget",
        )]);
    }
    Ok(text)
}

fn checked<'a>(
    report: &'a str,
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
) -> Result<Value> {
    verify_report(report, revision, candidate, policy)?;
    wire::report_value(report)
}

/// Page stable law identities and verdicts. Counts and the whole-report
/// acceptance verdict are repeated on every page and never truncated.
pub fn summary(
    report: &str,
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
    offset: usize,
    limit: usize,
    max_bytes: usize,
) -> Result<String> {
    if limit == 0 || limit > MAX_PAGE {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "law workflow page limit must be between 1 and 64",
        )]);
    }
    let payload = checked(report, revision, candidate, policy)?;
    let laws = payload["laws"]
        .as_array()
        .ok_or_else(|| invalid("verified law report lacks law rows"))?;
    if offset > laws.len() {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "law workflow page offset exceeds required inventory",
        )]);
    }
    let page = laws
        .iter()
        .skip(offset)
        .take(limit)
        .map(|law| {
            json!({
                "law_id": law["law_id"], "status": law["status"],
                "reason": law["reason"], "obligation_id": law["obligation_id"],
            })
        })
        .collect::<Vec<_>>();
    bounded(
        &json!({
            "schema": SUMMARY_SCHEMA,
            "candidate_revision": payload["project_revision"],
            "candidate_inventory_digest": payload["candidate_inventory_digest"],
            "protected_baseline_digest": payload["protected_baseline_digest"],
            "accepted": payload["accepted"], "counts": payload["counts"],
            "offset": offset, "returned": page.len(), "total": laws.len(),
            "next_offset": if offset + page.len() < laws.len() { Some(offset + page.len()) } else { None },
            "laws": page,
            "detail_method": "law_set::workflow::detail",
            "source_authority": false,
        }),
        max_bytes,
    )
}

/// Retrieve one exact law row after full report replay. The typed target never
/// proposes weakening a specification or counts fewer laws as improvement.
pub fn detail(
    report: &str,
    revision: &ProjectRevision,
    candidate: &LawSet,
    policy: &LawPolicy,
    law_id: &str,
    max_bytes: usize,
) -> Result<String> {
    let payload = checked(report, revision, candidate, policy)?;
    let row = payload["laws"]
        .as_array()
        .and_then(|laws| laws.iter().find(|row| row["law_id"] == law_id))
        .ok_or_else(|| invalid("law is absent from the protected required inventory"))?;
    bounded(
        &json!({
            "schema": DETAIL_SCHEMA,
            "candidate_revision": payload["project_revision"],
            "candidate_inventory_digest": payload["candidate_inventory_digest"],
            "protected_baseline_digest": payload["protected_baseline_digest"],
            "accepted": payload["accepted"], "counts": payload["counts"],
            "law": row,
            "repair_target": "implementation_or_proof",
            "specification_change_path": "protected_law_review",
            "source_authority": false,
        }),
        max_bytes,
    )
}

fn checked_strict(
    report: &str,
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &super::strict::StrictLawPolicy,
    proofs: &[VerifiedProjectProof],
    native_proofs: &[super::native_proof::VerifiedLawProof],
) -> Result<Value> {
    if report.len() > super::MAX_BYTES {
        return Err(super::capacity());
    }
    let expected =
        super::strict::derive_with_native_proofs(revision, laws, policy, proofs, native_proofs)?;
    if report != expected {
        return Err(super::drift(
            "strict law workflow report differs from exact checked evidence replay",
        ));
    }
    serde_json::from_str(&expected).map_err(|_| invalid("derived strict law report is malformed"))
}

/// Exact proof-bearing strict report projection. Independently held opaque
/// proof tokens are required to rederive the verdict, including failures.
pub fn strict_summary(
    report: &str,
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &super::strict::StrictLawPolicy,
    proofs: &[VerifiedProjectProof],
    native_proofs: &[super::native_proof::VerifiedLawProof],
    offset: usize,
    limit: usize,
    max_bytes: usize,
) -> Result<String> {
    if limit == 0 || limit > MAX_PAGE {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "law workflow page limit must be between 1 and 64",
        )]);
    }
    let value = checked_strict(report, revision, laws, policy, proofs, native_proofs)?;
    let rows = value["laws"]
        .as_array()
        .ok_or_else(|| invalid("derived strict law report lacks law rows"))?;
    if offset > rows.len() {
        return Err(vec![Diagnostic::io(
            "SPX-LW130",
            "law workflow page offset exceeds required inventory",
        )]);
    }
    let page = rows
        .iter()
        .skip(offset)
        .take(limit)
        .map(|row| {
            json!({
                "law_id": row["law_id"], "satisfied": row["satisfied"],
                "failure": row["failure"], "obligation_id": row["obligation_id"],
            })
        })
        .collect::<Vec<_>>();
    bounded(
        &json!({
            "schema": STRICT_SUMMARY_SCHEMA,
            "candidate_revision": value["project_revision"],
            "policy_digest": value["policy_digest"],
            "law_digest": value["law_digest"],
            "accepted": value["accepted"], "counts": value["counts"],
            "offset": offset, "returned": page.len(), "total": rows.len(),
            "next_offset": if offset + page.len() < rows.len() { Some(offset + page.len()) } else { None },
            "laws": page,
            "detail_method": "law_set::workflow::strict_detail",
            "source_authority": false,
        }),
        max_bytes,
    )
}

pub fn strict_detail(
    report: &str,
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &super::strict::StrictLawPolicy,
    proofs: &[VerifiedProjectProof],
    native_proofs: &[super::native_proof::VerifiedLawProof],
    law_id: &str,
    max_bytes: usize,
) -> Result<String> {
    let value = checked_strict(report, revision, laws, policy, proofs, native_proofs)?;
    let row = value["laws"]
        .as_array()
        .and_then(|rows| rows.iter().find(|row| row["law_id"] == law_id))
        .ok_or_else(|| invalid("law is absent from the strict required inventory"))?;
    bounded(
        &json!({
            "schema": STRICT_DETAIL_SCHEMA,
            "candidate_revision": value["project_revision"],
            "policy_digest": value["policy_digest"],
            "law_digest": value["law_digest"],
            "accepted": value["accepted"], "counts": value["counts"],
            "law": row,
            "repair_target": "implementation_or_proof",
            "specification_change_path": "protected_law_review",
            "source_authority": false,
        }),
        max_bytes,
    )
}
