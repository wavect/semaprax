//! Read-only, deterministic current-law work inventory for LAW-11.
//!
//! Verdicts come only from independently rederived LAW-04 and strict policy
//! reports. Process-local cache events describe work, not proof authority.
use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{
    dependency_index, evaluate, invalid, wire, ContractKind, LawRow, LawSelector, LawSet, Result,
};
use crate::assurance_manifest::law_set::native_proof::VerifiedLawProof;
use crate::assurance_manifest::law_set::strict::{self, StrictLawPolicy};
use crate::assurance_manifest::modular_law::cache::{ProofTaskCache, WorkMetrics};
use crate::assurance_manifest::smt_discharge as smt;
use crate::assurance_manifest::VerifiedProjectProof;
use crate::project::ProjectRevision;

pub const SCHEMA: &str = "semaprax.law-proof-work-inventory.v1";

/// Render every selected law in stable identity order, with its independently
/// derived status and strict verdict. Fresh/reused/stale describe only
/// compiler-owned checked-success events recorded for this exact revision.
/// Missing, unsupported, and inconclusive remain explicit distinct outcomes.
#[allow(clippy::too_many_arguments)]
pub fn derive(
    revision: &ProjectRevision,
    laws: &LawSet,
    policy: &StrictLawPolicy,
    proofs: &[VerifiedProjectProof],
    native_proofs: &[VerifiedLawProof],
    cache: &ProofTaskCache,
) -> Result<String> {
    let laws = LawSet::replay(revision, &laws.payload.proof_profile, laws.to_json())?;
    let strict = strict::derive_with_native_proofs(revision, &laws, policy, proofs, native_proofs)?;
    let law_policy = evaluate::LawPolicy::strict(policy.baseline().clone())?;
    let inventory =
        evaluate::derive_report_with_evidence(revision, &laws, &law_policy, proofs, native_proofs)?;
    let strict_value: Value = serde_json::from_str(&strict)
        .map_err(|_| invalid("strict law work inventory is incomplete"))?;
    let inventory_value = wire::report_value(&inventory)?;
    let index = dependency_index::derive(&laws)?;
    let events = cache.events_for(revision)?;
    let current = laws
        .payload
        .laws
        .iter()
        .map(|row| (row.definition.law_id.as_str(), row))
        .collect::<BTreeMap<_, _>>();
    let strict_rows = strict_value["laws"]
        .as_array()
        .ok_or_else(|| invalid("strict law work inventory is incomplete"))?
        .iter()
        .map(|row| {
            let id = row["law_id"]
                .as_str()
                .ok_or_else(|| invalid("strict law row has no identity"))?;
            let satisfied = row["satisfied"]
                .as_bool()
                .ok_or_else(|| invalid("strict law row has no verdict"))?;
            Ok((id.to_owned(), satisfied))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let law_rows = inventory_value["laws"]
        .as_array()
        .ok_or_else(|| invalid("law work inventory is incomplete"))?;
    if law_rows.len() != strict_rows.len() {
        return Err(invalid("strict and law inventory sizes differ"));
    }

    let mut rows = Vec::with_capacity(law_rows.len());
    let mut totals = WorkMetrics::default();
    let mut selected_events = BTreeMap::<(String, String), Vec<String>>::new();
    let mut unsupported = 0usize;
    let mut inconclusive = 0usize;
    let mut missing = 0usize;
    for row in law_rows {
        let id = row["law_id"]
            .as_str()
            .ok_or_else(|| invalid("law work row has no identity"))?;
        let satisfied = *strict_rows
            .get(id)
            .ok_or_else(|| invalid("strict work verdict is missing"))?;
        let status = row["status"]
            .as_str()
            .ok_or_else(|| invalid("law work row has no outcome"))?;
        let outcome = match status {
            "present" if satisfied => "proved",
            "present" | "awaiting_evidence" => {
                inconclusive += 1;
                "inconclusive"
            }
            "unsupported" => {
                unsupported += 1;
                "unsupported"
            }
            "missing" => {
                missing += 1;
                "missing"
            }
            _ => return Err(invalid("unknown law work outcome")),
        };
        let mut law_work = WorkMetrics::default();
        if let Some(current_row) = current.get(id) {
            for ((role, owner), event) in &events {
                if matches_law(current_row, row["obligation_id"].as_str(), role, owner) {
                    law_work.fresh += event.fresh;
                    law_work.reused += event.reused;
                    law_work.stale += event.stale;
                    selected_events
                        .entry((role.clone(), owner.clone()))
                        .or_default()
                        .push(id.to_owned());
                }
            }
        }
        rows.push(json!({
            "law_id":id,
            "semantic_digest":row["semantic_digest"],
            "logical_digest":index.logical_digest(id),
            "obligation_id":row["obligation_id"],
            "outcome":outcome,
            "reason":row["reason"],
            "strict_satisfied":satisfied,
            "work":{"fresh":law_work.fresh,"validated_reuse":law_work.reused,"stale":law_work.stale},
        }));
    }
    let tasks = events
        .iter()
        .filter_map(|((role, owner), work)| {
            let associated = selected_events.get(&(role.clone(), owner.clone()))?;
            totals.fresh += work.fresh;
            totals.reused += work.reused;
            totals.stale += work.stale;
            Some(
                json!({"role":role,"owner":owner,"associated_laws":associated,
                "fresh":work.fresh,"validated_reuse":work.reused,"stale":work.stale}),
            )
        })
        .collect::<Vec<Value>>();
    wire::canonical(&json!({
        "schema":SCHEMA,
        "project_revision":revision.project_revision(),
        "program_root":laws.payload.program_root,
        "law_digest":laws.digest(),
        "strict_report_digest":wire::digest(b"semaprax.law-proof-work.strict.v1\0",&strict),
        "law_report_digest":wire::digest(b"semaprax.law-proof-work.inventory.v1\0",&inventory),
        "counts":{"laws":rows.len(),"unsupported":unsupported,"inconclusive":inconclusive,
            "missing":missing,"fresh":totals.fresh,"validated_reuse":totals.reused,"stale":totals.stale},
        "laws":rows,"tasks":tasks,
        "nonclaims":["work_metrics_are_not_proof","cache_report_grants_no_execution_or_publication_authority"]
    }))
}

fn matches_law(row: &LawRow, obligation: Option<&str>, role: &str, owner: &str) -> bool {
    match &row.definition.selector {
        LawSelector::ScalarRelational { .. } => {
            owner == row.definition.law_id
                && matches!(role, "native-relational-z3" | "native-relational-lean")
        }
        LawSelector::Contract {
            declaration_id,
            clause: ContractKind::Postcondition,
            ..
        } => {
            if role == "modular-scalar" && owner == declaration_id {
                return true;
            }
            for index in 0..super::MAX_REFERENCES {
                if obligation
                    == Some(smt::postcondition_obligation_id(declaration_id, index).as_str())
                    && owner == format!("{declaration_id}:{index}")
                    && matches!(
                        role,
                        "direct-project-z3"
                            | "direct-project-lean"
                            | "structured-z3"
                            | "structured-lean"
                    )
                {
                    return true;
                }
            }
            false
        }
        _ => false,
    }
}
