//! Versioned, read-only summary proof transcript with live solver replay.
//!
//! The JSON envelope is evidence, never authority. Independent acceptance
//! rederives the current linked Project plan and reruns each checked query.
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

use crate::assurance_manifest::smt_discharge::{self as smt, Provisioning, RunLimits};
use crate::project::ProjectRevision;
use crate::proof_export::installed::InstalledProofTool;

use super::{
    plan,
    summary::{prove_straight_line, ModularFailure, ModularProof},
    Refusal,
};

pub const SCHEMA: &str = "semaprax.modular-scalar-summary-proof.v1";
const MAX_CERTIFICATE_BYTES: usize = 262_144;
const DOMAIN: &[u8] = b"semaprax.modular-scalar-summary-proof.payload.v1\0";

#[derive(Clone, Debug)]
pub enum ReplayFailure {
    Refused(Refusal),
    Proof(ModularFailure),
    Malformed,
    Capacity,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Drift {
    Exact,
    UnrelatedRevision,
    DependencyChanged,
}

fn digest(payload: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update(payload.as_bytes());
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn summary_rows(proof: &ModularProof) -> Value {
    Value::Array(
        proof
            .plan
            .summaries
            .iter()
            .map(|row| {
                json!({
                    "declaration_id":row.declaration_id,
                    "local_digest":row.local_digest,
                    "digest":row.digest,
                    "calls":row.calls.iter().map(|call| json!({
                        "expression_id":call.expression_id,"callee":call.callee
                    })).collect::<Vec<_>>(),
                    "dependencies":row.dependencies.iter().map(|dependency| json!({
                        "callee":dependency.callee,"digest":dependency.digest
                    })).collect::<Vec<_>>()
                })
            })
            .collect(),
    )
}

fn payload(proof: &ModularProof, identity: &str, version: &str, boundary: &str) -> Value {
    json!({
        "profile":proof.plan.profile,
        "project_revision":proof.plan.project_revision,
        "target":proof.plan.target,
        "summaries":summary_rows(proof),
        "callee_clauses":proof.checked_callee_clauses.iter().map(|clause| json!({
            "declaration_id":clause.declaration_id,
            "summary_digest":clause.summary_digest,
            "ensures_index":clause.ensures_index,
            "script_digest":clause.script_digest,
            "solver_identity":clause.solver_identity,
            "solver_version":clause.solver_version
        })).collect::<Vec<_>>(),
        "caller_precondition_scripts":proof.caller_precondition_scripts,
        "caller_postcondition_scripts":proof.caller_postcondition_scripts,
        "toolchain":{"identity":identity,"version":version},
        "trusted_boundary":boundary,
        "publication_authority":false,
        "runtime_guard_removal":false
    })
}

fn envelope(
    proof: &ModularProof,
    identity: &str,
    version: &str,
    boundary: &str,
) -> Result<String, ReplayFailure> {
    let payload = payload(proof, identity, version, boundary);
    let canonical_payload =
        serde_json::to_string(&payload).map_err(|_| ReplayFailure::Malformed)?;
    let mut document = serde_json::to_string(&json!({
        "schema":SCHEMA,"payload":payload,"payload_digest":digest(&canonical_payload)
    }))
    .map_err(|_| ReplayFailure::Malformed)?;
    document.push('\n');
    if document.len() > MAX_CERTIFICATE_BYTES {
        return Err(ReplayFailure::Capacity);
    }
    Ok(document)
}

fn decode(document: &str) -> Result<Value, ReplayFailure> {
    if document.len() > MAX_CERTIFICATE_BYTES {
        return Err(ReplayFailure::Capacity);
    }
    let value: Value = serde_json::from_str(document).map_err(|_| ReplayFailure::Malformed)?;
    if value["schema"] != SCHEMA {
        return Err(ReplayFailure::Malformed);
    }
    let payload = value.get("payload").ok_or(ReplayFailure::Malformed)?;
    let encoded = serde_json::to_string(payload).map_err(|_| ReplayFailure::Malformed)?;
    let expected_digest = digest(&encoded);
    if value["payload_digest"].as_str() != Some(expected_digest.as_str()) {
        return Err(ReplayFailure::Malformed);
    }
    let mut canonical = serde_json::to_string(&value).map_err(|_| ReplayFailure::Malformed)?;
    canonical.push('\n');
    if canonical != document {
        return Err(ReplayFailure::Malformed);
    }
    Ok(value)
}

/// Advisory drift classification. It does not validate proof claims and must
/// not attach evidence; use `replay` for a result eligible for consideration.
pub fn classify_drift(
    document: &str,
    revision: &ProjectRevision,
    target: &str,
) -> Result<Drift, ReplayFailure> {
    let value = decode(document)?;
    let payload = &value["payload"];
    if payload["target"] != target {
        return Ok(Drift::DependencyChanged);
    }
    let current = plan(revision, target).map_err(ReplayFailure::Refused)?;
    let old = payload["summaries"]
        .as_array()
        .ok_or(ReplayFailure::Malformed)?;
    if old.len() != current.summaries.len()
        || old.iter().zip(&current.summaries).any(|(record, row)| {
            record["declaration_id"] != row.declaration_id || record["digest"] != row.digest
        })
    {
        return Ok(Drift::DependencyChanged);
    }
    if payload["project_revision"] == revision.project_revision() {
        Ok(Drift::Exact)
    } else {
        Ok(Drift::UnrelatedRevision)
    }
}

/// Export after real solver discharge under exact authenticated Project data.
pub fn export(
    revision: &ProjectRevision,
    target: &str,
    provisioning: &Provisioning,
    limits: &RunLimits,
) -> Result<String, ReplayFailure> {
    let proof = prove_straight_line(revision, target, Some(provisioning), limits)
        .map_err(ReplayFailure::Proof)?;
    let version = smt::solver_version(provisioning).ok_or(ReplayFailure::Malformed)?;
    envelope(
        &proof,
        provisioning.identity,
        &version,
        "trusted_local_explicit_z3",
    )
}

/// Independently rerun all callee, precondition and caller queries, then
/// compare every versioned byte. Unknown, drift, missing solver or SAT refuse.
pub fn replay(
    document: &str,
    revision: &ProjectRevision,
    target: &str,
    provisioning: &Provisioning,
    limits: &RunLimits,
) -> Result<ModularProof, ReplayFailure> {
    decode(document)?;
    let proof = prove_straight_line(revision, target, Some(provisioning), limits)
        .map_err(ReplayFailure::Proof)?;
    let version = smt::solver_version(provisioning).ok_or(ReplayFailure::Malformed)?;
    if envelope(
        &proof,
        provisioning.identity,
        &version,
        "trusted_local_explicit_z3",
    )? != document
    {
        return Err(ReplayFailure::Stale);
    }
    Ok(proof)
}

/// Export only after all queries pass through the held installed tool.
pub fn export_installed(
    revision: &ProjectRevision,
    target: &str,
    tool: &InstalledProofTool,
) -> Result<String, ReplayFailure> {
    let proof = super::installed::prove_straight_line_installed(revision, target, tool)
        .map_err(ReplayFailure::Proof)?;
    encode_installed(&proof, tool)
}

pub(crate) fn encode_installed(
    proof: &ModularProof,
    tool: &InstalledProofTool,
) -> Result<String, ReplayFailure> {
    envelope(
        proof,
        "z3",
        tool.expected_version(),
        "trusted_local_registered_z3",
    )
}

/// Independent registered-tool replay before any Project attachment.
pub fn replay_installed(
    document: &str,
    revision: &ProjectRevision,
    target: &str,
    tool: &InstalledProofTool,
) -> Result<ModularProof, ReplayFailure> {
    decode(document)?;
    let proof = super::installed::prove_straight_line_installed(revision, target, tool)
        .map_err(ReplayFailure::Proof)?;
    if envelope(
        &proof,
        "z3",
        tool.expected_version(),
        "trusted_local_registered_z3",
    )? != document
    {
        return Err(ReplayFailure::Stale);
    }
    Ok(proof)
}
